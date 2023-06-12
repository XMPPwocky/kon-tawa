use std::sync::{atomic::AtomicUsize, atomic::Ordering, Mutex, MutexGuard, TryLockError};

use std::sync::Arc;

use audio_processor_analysis::fft_processor::FftProcessorOptions;
use audio_processor_traits::simple_processor::MonoAudioProcessor;

pub struct DoubleBuffer<T> {
    idx: AtomicUsize,
    bufs: [Mutex<T>; 2],
}
impl<T> DoubleBuffer<T> {
    pub fn new([front, back]: [T; 2]) -> DoubleBuffer<T> {
        let idx = AtomicUsize::new(0);
        DoubleBuffer {
            idx,
            bufs: [Mutex::new(front), Mutex::new(back)],
        }
    }
    pub fn front(&self) -> MutexGuard<'_, T> {
        self.get(self.idx.load(Ordering::SeqCst))
    }
    pub fn back(&self) -> MutexGuard<'_, T> {
        self.get(1 ^ self.idx.load(Ordering::SeqCst))
    }
    pub fn flip(&self) {
        self.idx.fetch_xor(1, Ordering::SeqCst);
    }
    fn get(&self, init_idx: usize) -> MutexGuard<'_, T> {
        let mut idx = init_idx;

        loop {
            let mew = &self.bufs[idx];
            match mew.try_lock() {
                Ok(x) => {
                    return x;
                }
                Err(TryLockError::WouldBlock) => {
                    // try the other buffer then
                    idx ^= 1;
                }
                Err(TryLockError::Poisoned(e)) => {
                    panic!("{e:?}")
                }
            }
        }
    }
}

pub fn do_audio(dbuf: Arc<DoubleBuffer<Vec<f32>>>) -> cpal::Stream {
    use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
    use cpal::{Data, FromSample, Sample, SampleFormat};
    let host = cpal::default_host();

    let device = host
        .default_input_device()
        .expect("no output device available");

    let mut supported_configs_range = device
        .supported_input_configs()
        .expect("error while querying configs");
    let supported_config = supported_configs_range
        .next()
        .expect("no supported config?!")
        .with_max_sample_rate();

    let err_fn = |err| eprintln!("an error occurred on the output audio stream: {}", err);
    let sample_format = supported_config.sample_format();
    let config = supported_config.into();
   
    use audio_processor_analysis::fft_processor::FftProcessor;
    use audio_processor_traits::*;

    let settings = AudioProcessorSettings::default();
    let mut context = AudioContext::from(settings);

    let mut fft_processor = FftProcessor::new(FftProcessorOptions {
        size: 512,
        overlap_ratio: 0.75,
        ..Default::default()
    });
    fft_processor.m_prepare(&mut context);

    let mut buffer: AudioBuffer<f32> = AudioBuffer::empty();
    buffer.resize(1, fft_processor.size());

    let chunk_size = fft_processor.size();

    // fixme: use a ring buffer here (lol)
    let mut accum_buffer = Vec::<f32>::with_capacity(fft_processor.size());
    let write_silence = move |data: &[f32], _: &cpal::InputCallbackInfo| {
        accum_buffer.extend(data.iter().cloned());
        
        let mut chunks = accum_buffer.chunks_exact(chunk_size);
        for chunk in chunks {
            buffer.copy_from_interleaved(chunk);
            simple_processor::process_buffer(&mut context, &mut fft_processor, &mut buffer);
        }

        let fft_buf = fft_processor.buffer();

        let mut out_buf = dbuf.back();
        out_buf.clear();
        out_buf.extend(fft_buf
            .iter()
            .map(|complex| complex.norm().ln()));
    };

    let stream = match sample_format {
        SampleFormat::F32 => device.build_input_stream(&config, write_silence, err_fn, None),
        //ampleFormat::I16 => device.build_input_stream(&config, write_silence::<i16>, err_fn, None),
        //SampleFormat::U16 => device.build_input_stream(&config, write_silence::<u16>, err_fn, None),
        sample_format => panic!("Unsupported sample format '{sample_format}'"),
    }
    .unwrap();

    stream.play().unwrap();

    stream
}

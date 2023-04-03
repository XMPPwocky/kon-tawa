#![deny(clippy::all)]
#![forbid(unsafe_code)]

use crate::gui::Framework;
use std::sync::{Arc, Mutex};
use glam::Vec2;
use log::error;
use pixels::{Error, Pixels, SurfaceTexture};
use egui_winit::winit::{
    dpi::LogicalSize, event::{Event, VirtualKeyCode},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder
};
use simulation::Array2D;
use winit_input_helper::WinitInputHelper;
use rayon::prelude::*;

mod gui;
mod simulation;

const WIDTH: u32 = 512;
const HEIGHT: u32 = 512;

#[derive(Clone, Debug, PartialEq)]
enum Material {
    Fluid,
    Solid,
    Emitter
}

struct SimParams {
    grad_alpha: f32,
    grad_damping: f32
}
impl Default for SimParams {
    fn default() -> Self {
        SimParams { grad_alpha: 0.1, grad_damping: 0.9999 }
    }
}

struct World {
    pressures: Array2D<f32>,
    pressures_back: Array2D<f32>,
    velocities: Array2D<glam::Vec2>,
    velocities_back: Array2D<glam::Vec2>,
    materials: Array2D<Material>,
    params: Arc<Mutex<SimParams>>,
    ticks: u32
}

fn main() -> Result<(), Error> {
    let event_loop = EventLoop::new();
    let mut input = WinitInputHelper::new();
    let window = {
        let size = LogicalSize::new(WIDTH as f64, HEIGHT as f64);
        WindowBuilder::new()
            .with_title("kon tawa")
            .with_inner_size(size)
            .with_min_inner_size(size)
            .build(&event_loop)
            .unwrap()
    };

    let params = Arc::new(Mutex::new(SimParams::default()));

    let (mut pixels, mut framework) = {
        let window_size = window.inner_size();
        let scale_factor = window.scale_factor() as f32;
        let surface_texture = SurfaceTexture::new(window_size.width, window_size.height, &window);
        let pixels = Pixels::new(WIDTH, HEIGHT, surface_texture)?;
        let framework = Framework::new(
            &event_loop,
            window_size.width,
            window_size.height,
            scale_factor,
            &pixels,
            params.clone()
        );

        (pixels, framework)
    };

    let mut world = World::new(params);

    event_loop.run(move |event, _, control_flow| {
        // Handle input events
        if input.update(&event) {
            // Close events
            if input.key_pressed(VirtualKeyCode::Escape) || input.quit() {
                *control_flow = ControlFlow::Exit;
                return;
            }

            if input.key_pressed(VirtualKeyCode::R) || input.quit() {
                world = World::new(world.params.clone());
                return;
            }

            // Update the scale factor
            if let Some(scale_factor) = input.scale_factor() {
                framework.scale_factor(scale_factor);
            }

            // Resize the window
            if let Some(size) = input.window_resized() {
                if let Err(err) = pixels.resize_surface(size.width, size.height) {
                    error!("pixels.resize_surface() failed: {err}");
                    *control_flow = ControlFlow::Exit;
                    return;
                }
                framework.resize(size.width, size.height);
            }

            // Update internal state and request a redraw
            world.update();
            world.update();
            world.update();
            window.request_redraw();
        }

        match event {
            Event::WindowEvent { event, .. } => {
                // Update egui inputs
                framework.handle_event(&event);
            }
            // Draw the current frame
            Event::RedrawRequested(_) => {
                // Draw the world
                world.draw(pixels.get_frame_mut());

                // Prepare egui
                framework.prepare(&window);

                // Render everything together
                let render_result = pixels.render_with(|encoder, render_target, context| {
                    // Render the world texture
                    context.scaling_renderer.render(encoder, render_target);

                    // Render egui
                    framework.render(encoder, render_target, context);

                    Ok(())
                });

                // Basic error handling
                if let Err(err) = render_result {
                    error!("pixels.render() failed: {err}");
                    *control_flow = ControlFlow::Exit;
                }
            }
            _ => (),
        }
    });
}

impl World {
    fn new(params: Arc<Mutex<SimParams>>) -> Self {
        let mut materials = Array2D::new(WIDTH as usize, HEIGHT as usize, Material::Fluid);
        for y in 224..288 {
            *materials.get_mut(32, y).unwrap() = Material::Emitter;
        }
        for x in 192..196 {
            for y in 0..HEIGHT as isize { if (y + 0x20) & 0x7F < 0x40 { *materials.get_mut(x, y).unwrap() = Material::Solid; } }
        }
        
        Self {
            pressures: Array2D::new(WIDTH as usize, HEIGHT as usize, 0.0),
            pressures_back: Array2D::new(WIDTH as usize, HEIGHT as usize, 0.0),
            velocities: Array2D::new(WIDTH as usize, HEIGHT as usize, Vec2::ZERO),
            velocities_back: Array2D::new(WIDTH as usize, HEIGHT as usize, Vec2::ZERO),
            materials,
            params,
            ticks: 0
        }
    }

    /// Update the `World` internal state; bounce the box around the screen.
    fn update(&mut self) {
        std::mem::swap(&mut self.pressures, &mut self.pressures_back);
        std::mem::swap(&mut self.velocities, &mut self.velocities_back);
        self.ticks += 1;

        let params = self.params.lock().unwrap();
        
        //let vec_down_right = Vec2::new(1.0, 1.0).normalize();
        //let vec_down_left = Vec2::new(-1.0, 1.0).normalize();

        let time = self.ticks as f32 / 16.0;

        self.pressures.par_iter_mut()
            .zip(self.pressures_back.par_iter().cloned())
            .zip(
                self.velocities.par_iter_mut()
                .zip(self.velocities_back.par_iter().cloned())
            )
            
            .enumerate().for_each(|(i, ((front, back), (front_v, _back_v)))| {
                assert!(!back.is_infinite());
                let x = i as isize % WIDTH as isize;
                let y = i as isize / WIDTH as isize;

                let left = self.pressures_back.get(x - 1, y).copied().unwrap_or(0.0);
                let right = self.pressures_back.get(x + 1, y).copied().unwrap_or(0.0);
                let up = self.pressures_back.get(x, y - 1 ).copied().unwrap_or(0.0);
                let down = self.pressures_back.get(x, y + 1).copied().unwrap_or(0.0);

                /*let upleft    = self.pressures_back.get(x - 1, y - 1).copied().unwrap_or(0.0);
                let upright   = self.pressures_back.get(x + 1, y - 1).copied().unwrap_or(0.0);
                let downleft  = self.pressures_back.get(x - 1, y + 1).copied().unwrap_or(0.0);
                let downright = self.pressures_back.get(x + 1, y + 1).copied().unwrap_or(0.0);
                */
                let hgrad = right - left;
                let vgrad = down - up;
                //let drgrad = downright - upleft;
                //let dlgrad = downleft - upright;

                let grad =  Vec2::new(hgrad, vgrad);// + (vec_down_left*dlgrad) + (vec_down_right*drgrad);
                *front_v += grad * params.grad_alpha;
                *front_v *= 1.0 - params.grad_damping;

                let mut accum = 0.0;

                accum += self.velocities_back.get(x - 1, y).copied().unwrap_or(Vec2::ZERO).x;
                accum -= self.velocities_back.get(x + 1, y).copied().unwrap_or(Vec2::ZERO).x;
                accum += self.velocities_back.get(x, y - 1).copied().unwrap_or(Vec2::ZERO).y;
                accum -= self.velocities_back.get(x, y + 1).copied().unwrap_or(Vec2::ZERO).y;

                match self.materials.get(x, y).unwrap() {
                    Material::Fluid => {
                        *front -= accum / 8.0;
                    },
                    Material::Emitter => {
                        *front = 2.5 * (time/3.0).sin();
                    },
                    Material::Solid => {
                        *front = 0.0;
                        *front_v = Vec2::ZERO;
                    }
                }
            });
    }

    fn draw(&self, frame: &mut [u8]) {
        for (i, pixel) in frame.chunks_exact_mut(4).enumerate() {
            let x = i % WIDTH as usize;
            let y = i / WIDTH as usize;

            let p = self.pressures[x + (y * WIDTH as usize)];
            let pos = p > 0.0;
            let is_solid = matches!(self.materials[x + (y * WIDTH as usize)], Material::Solid | Material::Emitter);
            let g = if is_solid { 0xff } else { 0x00 };
            let rgba = if pos {
                [(p * 255.0) as u8, g, 0x0, 0xff]
            } else {
                [0, g, (-p * 255.0) as u8, 0xff]
            };

            pixel.copy_from_slice(&rgba);
        }
    }
}

#[derive(Clone)]
pub struct Array2D<T> {
    width: usize,
    height: usize,
    storage: Vec<T>
}
impl<T> Array2D<T> {
    pub fn new(
        width: usize,
        height: usize,
        val: T) -> Array2D<T> where T: Clone {
            assert_ne!(width, 0);
            assert_ne!(height, 0);
            
            Array2D {
                width,
                height,
                storage: std::iter::repeat(val).take(width.checked_mul(height).unwrap()).collect()
            }
        }
    
    pub fn width(&self) -> usize { self.width }
    pub fn height(&self) -> usize { self.height }
    
    pub fn get(&self, x: isize, y: isize) -> Option<&T> {
        if x < 0 || y < 0 || x >= self.width as isize || y >= self.height as isize {
            None
        } else {
            self.storage.get((x + (self.width as isize * y)) as usize)
        }
    }
    pub fn get_mut(&mut self, x: isize, y: isize) -> Option<&mut T> {
        if x < 0 || y < 0 || x >= self.width as isize || y >= self.height as isize {
            None
        } else {
            self.storage.get_mut((x + (self.width as isize * y)) as usize)
        }
    }
}
impl<T> std::ops::Deref for Array2D<T> {
    type Target = [T];
    fn deref(&self) -> &Self::Target {
        &self.storage
    }
}
impl<T> std::ops::DerefMut for Array2D<T> {
    fn deref_mut(&mut self) -> &mut [T] {
        &mut self.storage
    }
}
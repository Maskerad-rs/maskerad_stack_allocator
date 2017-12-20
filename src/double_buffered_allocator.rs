use StackAllocator;

pub struct DoubleBufferedAllocator {
    buffers: [StackAllocator; 2],
    current: bool,
}

impl DoubleBufferedAllocator {
    pub fn with_capacity(capacity: usize) -> Self {
        DoubleBufferedAllocator {
            buffers: [StackAllocator::with_capacity(capacity), StackAllocator::with_capacity(capacity)],
            current: false,
        }
    }

    pub fn reset_current(&self) {
        self.buffers[self.current as usize].reset();
    }

    pub fn swap_buffers(&mut self) {
        self.current = !self.current;
    }

    pub fn alloc<T>(&self, value: T) -> &mut T {
        self.buffers[self.current as usize].alloc(value)
    }
}


#[cfg(test)]
mod double_buffer_allocator_test {
    use super::*;

    #[test]
    fn new() {
        let alloc = DoubleBufferedAllocator::with_capacity(100);
        assert_eq!(alloc.buffers[0].stack().cap(), 100);
        assert_eq!(alloc.buffers[1].stack().cap(), 100);
    }

    #[test]
    fn reset() {
        let alloc = DoubleBufferedAllocator::with_capacity(100);
        assert_eq!(alloc.buffers[0].stack().ptr(), alloc.buffers[0].current_offset().get());
        assert_eq!(alloc.buffers[1].stack().ptr(), alloc.buffers[1].current_offset().get());
        let my_i32 = alloc.alloc(25);
        assert_ne!(alloc.buffers[0].stack().ptr(), alloc.buffers[0].current_offset().get());
        assert_eq!(alloc.buffers[1].stack().ptr(), alloc.buffers[1].current_offset().get());
        alloc.reset_current();
        assert_eq!(alloc.buffers[0].stack().ptr(), alloc.buffers[0].current_offset().get());
        assert_eq!(alloc.buffers[1].stack().ptr(), alloc.buffers[1].current_offset().get());
    }

    #[test]
    fn swap() {
        let mut alloc = DoubleBufferedAllocator::with_capacity(100);
        assert_eq!(alloc.buffers[0].stack().ptr(), alloc.buffers[0].current_offset().get());
        assert_eq!(alloc.buffers[1].stack().ptr(), alloc.buffers[1].current_offset().get());
        alloc.swap_buffers();
        let my_i32 = alloc.alloc(25);
        assert_eq!(alloc.buffers[0].stack().ptr(), alloc.buffers[0].current_offset().get());
        assert_ne!(alloc.buffers[1].stack().ptr(), alloc.buffers[1].current_offset().get());
    }
}
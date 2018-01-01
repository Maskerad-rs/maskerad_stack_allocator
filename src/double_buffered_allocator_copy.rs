// Copyright 2017-2018 Maskerad Developers
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

use StackAllocatorCopy;

/// A double-buffered allocator for data implementing the Copy trait.
///
/// This allocator is a wrapper around two StackAllocatorCopy.
/// It works like a StackAllocatorCopy, and allows you to swap the buffers.
///
/// # Example
/// ```
/// use maskerad_memory_allocators::DoubleBufferedAllocatorCopy;
///
///
/// let mut allocator = DoubleBufferedAllocatorCopy::with_capacity(100); //100 bytes.
/// let mut closed = false;
///
/// while !closed {
///     //swap the active and inactive buffers of the allocator.
///     allocator.swap_buffers();
///
///     //clear the newly active buffer.
///     allocator.reset();
///
///     //allocate with the current buffer, leaving the data in the inactive buffer intact.
///     //You can use this data during this frame, or the next frame.
///     let my_i32 = allocator.alloc(|| {
///         47 as i32
///     });
///
///     closed = true;
/// }
/// ```
pub struct DoubleBufferedAllocatorCopy {
    buffers: [StackAllocatorCopy; 2],
    current: bool,
}

impl DoubleBufferedAllocatorCopy {

    /// Create a DoubleBufferedAllocatorCopy with the given capacity (in bytes).
    /// # Example
    /// ```
    /// #![feature(alloc)]
    /// use maskerad_memory_allocators::DoubleBufferedAllocatorCopy;
    ///
    /// let allocator = DoubleBufferedAllocatorCopy::with_capacity(100);
    ///
    /// assert_eq!(allocator.active_buffer().storage().borrow().capacity(), 100);
    /// assert_eq!(allocator.inactive_buffer().storage().borrow().capacity(), 100);
    /// ```
    pub fn with_capacity(capacity: usize) -> Self {
        DoubleBufferedAllocatorCopy {
            buffers: [StackAllocatorCopy::with_capacity(capacity), StackAllocatorCopy::with_capacity(capacity)],
            current: false,
        }
    }

    /// Allocates data in the active buffer.
    ///
    /// # Panic
    /// This function will panic if the allocation exceeds the maximum storage capacity of the active allocator.
    ///
    pub fn alloc<T: Copy, F>(&self, op: F) -> &mut T
        where F: FnOnce() -> T
    {
        self.active_buffer().alloc(op)
    }

    /// Resets completely the active buffer, setting the index of the first unused memory address to 0,
    /// the bottom of the memory chunk.
    pub fn reset(&self) {
        self.active_buffer().reset();
    }

    /// Resets partially the active buffer, setting the index of the first unused memory address to the marker.
    pub fn reset_to_marker(&self, marker: usize) {
        self.active_buffer().reset_to_marker(marker);
    }

    /// Returns the index of the first unused memory address in the active buffer.
    pub fn marker(&self) -> usize {
        self.active_buffer().marker()
    }

    /// Return an immutable reference to the active StackAllocatorCopy.
    ///
    /// Most of the time, you should not have to access the stack allocators directly.
    /// This structure mimics the StackAllocatorCopy's API and have the swap_buffers() function to swap
    /// the active allocator to the inactive one, and vice-versa.
    pub fn active_buffer(&self) -> &StackAllocatorCopy {
        &self.buffers[self.current as usize]
    }

    /// Return an immutable reference to the inactive StackAllocatorCopy.
    ///
    /// Most of the time, you should not have to access the stack allocators directly.
    /// This structure mimics the StackAllocatorCopy's API and have the swap_buffers() function to swap
    /// the active allocator to the inactive one, and vice-versa.
    pub fn inactive_buffer(&self) -> &StackAllocatorCopy {
        &self.buffers[!self.current as usize]
    }

    /// Swap the buffers. The inactive one becomes the active.
    pub fn swap_buffers(&mut self) {
        self.current = !self.current;
    }
}


#[cfg(test)]
mod double_buffer_allocator_test {
    use super::*;

    #[test]
    fn new() {
        let alloc = DoubleBufferedAllocatorCopy::with_capacity(100);
        assert_eq!(alloc.active_buffer().storage().borrow().capacity(), 100);
        assert_eq!(alloc.inactive_buffer().storage().borrow().capacity(), 100);
    }

    #[test]
    fn reset() {
        let alloc = DoubleBufferedAllocatorCopy::with_capacity(100);

        let start_chunk_active_buffer = alloc.active_buffer().storage().borrow().as_ptr();
        let start_chunk_inactive_buffer = alloc.inactive_buffer().storage().borrow().as_ptr();

        let index_active_buffer_top_stack = alloc.active_buffer().marker();
        let index_inactive_buffer_top_stack = alloc.inactive_buffer().marker();

        unsafe {
            let active_buffer_top_stack = start_chunk_active_buffer.offset(index_active_buffer_top_stack as isize);
            let inactive_buffer_top_stack = start_chunk_inactive_buffer.offset(index_inactive_buffer_top_stack as isize);

            assert_eq!(start_chunk_active_buffer, active_buffer_top_stack);
            assert_eq!(start_chunk_inactive_buffer, inactive_buffer_top_stack);
        }

        let _my_beef = alloc.alloc(|| {
            0xb33f as i32
        });

        let index_active_buffer_top_stack = alloc.active_buffer().marker();
        let index_inactive_buffer_top_stack = alloc.inactive_buffer().marker();

        unsafe {
            let active_buffer_top_stack = start_chunk_active_buffer.offset(index_active_buffer_top_stack as isize);
            let inactive_buffer_top_stack = start_chunk_inactive_buffer.offset(index_inactive_buffer_top_stack as isize);

            assert_ne!(start_chunk_active_buffer, active_buffer_top_stack);
            assert_eq!(start_chunk_inactive_buffer, inactive_buffer_top_stack);
        }

        alloc.reset();
        let index_active_buffer_top_stack = alloc.active_buffer().marker();
        let index_inactive_buffer_top_stack = alloc.inactive_buffer().marker();

        unsafe {
            let active_buffer_top_stack = start_chunk_active_buffer.offset(index_active_buffer_top_stack as isize);
            let inactive_buffer_top_stack = start_chunk_inactive_buffer.offset(index_inactive_buffer_top_stack as isize);

            assert_eq!(start_chunk_active_buffer, active_buffer_top_stack);
            assert_eq!(start_chunk_inactive_buffer, inactive_buffer_top_stack);
        }
    }

    #[test]
    fn swap() {
        let mut alloc = DoubleBufferedAllocatorCopy::with_capacity(100);
        let start_chunk_first_buffer = alloc.buffers[0].storage().borrow().as_ptr();
        let start_chunk_second_buffer = alloc.buffers[1].storage().borrow().as_ptr();

        let index_first_buffer_top_stack = alloc.buffers[0].marker();
        let index_second_buffer_top_stack = alloc.buffers[1].marker();

        unsafe {
            let first_buffer_top_stack = start_chunk_first_buffer.offset(index_first_buffer_top_stack as isize);
            let second_buffer_top_stack = start_chunk_second_buffer.offset(index_second_buffer_top_stack as isize);

            assert_eq!(start_chunk_first_buffer, first_buffer_top_stack);
            assert_eq!(start_chunk_second_buffer, second_buffer_top_stack);
        }



        alloc.swap_buffers();
        let _my_i32 = alloc.alloc(|| {
            25
        });
        let index_first_buffer_top_stack = alloc.buffers[0].marker();
        let index_second_buffer_top_stack = alloc.buffers[1].marker();

        unsafe {
            let first_buffer_top_stack = start_chunk_first_buffer.offset(index_first_buffer_top_stack as isize);
            let second_buffer_top_stack = start_chunk_second_buffer.offset(index_second_buffer_top_stack as isize);

            assert_eq!(start_chunk_first_buffer, first_buffer_top_stack);
            assert_ne!(start_chunk_second_buffer, second_buffer_top_stack);
        }
    }
}
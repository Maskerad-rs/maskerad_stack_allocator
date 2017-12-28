// Copyright 2017 Maskerad Developers
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

use StackAllocator;

/// A double-buffered allocator.
///
/// This allocator is a wrapper around two StackAllocator.
/// It works like a StackAllocator, and allows you to swap the buffers.
///
/// Useful if you want to use data created at frame N during frame N + 1.
/// # Example
/// ```
/// use maskerad_stack_allocator::DoubleBufferedAllocator;
///
/// struct Monster {
///     hp :u32,
///     level: u32,
/// }
///
/// impl Default for Monster {
///     fn default() -> Self {
///         Monster {
///         hp: 1,
///         level: 1,
///         }
///     }
/// }
///
/// let mut allocator = DoubleBufferedAllocator::with_capacity(100); //100 bytes.
/// let mut closed = false;
///
/// while !closed {
///     //swap the active and inactive buffers of the allocator.
///     allocator.swap_buffers();
///
///     //clear the newly active buffer.
///     allocator.reset_current();
///
///     //allocate with the current buffer, leaving the data in the inactive buffer intact.
///     //You can use this data during this frame, or the next frame.
///     let my_monster: &mut Monster = allocator.alloc(Monster::default());
///
///     closed = true;
/// }
/// ```
pub struct DoubleBufferedAllocator {
    buffers: [StackAllocator; 2],
    current: bool,
}

impl DoubleBufferedAllocator {

    /// Create a DoubleBufferedAllocator with the given capacity (in bytes).
    /// # Example
    /// ```
    /// #![feature(alloc)]
    /// use maskerad_stack_allocator::DoubleBufferedAllocator;
    ///
    /// let allocator = DoubleBufferedAllocator::with_capacity(100);
    ///
    /// assert_eq!(allocator.active_buffer().stack().cap(), 100);
    /// assert_eq!(allocator.inactive_buffer().stack().cap(), 100);
    /// ```
    pub fn with_capacity(capacity: usize) -> Self {
        DoubleBufferedAllocator {
            buffers: [StackAllocator::with_capacity(capacity), StackAllocator::with_capacity(capacity)],
            current: false,
        }
    }

    /// Return an immutable reference to the active StackAllocator.
    pub fn active_buffer(&self) -> &StackAllocator {
        &self.buffers[self.current as usize]
    }

    /// Return an immutable reference to the inactive StackAllocator.
    pub fn inactive_buffer(&self) -> &StackAllocator {
        &self.buffers[!self.current as usize]
    }

    /// Reset the pointer of the active StackAllocator, from the current top of its stack to the bottom of its stack.
    /// # Example
    /// ```
    /// #![feature(alloc)]
    /// use maskerad_stack_allocator::DoubleBufferedAllocator;
    ///
    /// let allocator = DoubleBufferedAllocator::with_capacity(100);
    ///
    /// let my_i32 = allocator.alloc(26);
    /// let active_buffer_top_stack = allocator.active_buffer().marker();
    /// let inactive_buffer_top_stack = allocator.inactive_buffer().marker();
    ///
    /// assert_eq!(allocator.inactive_buffer().stack().ptr(), inactive_buffer_top_stack);
    /// assert_ne!(allocator.active_buffer().stack().ptr(), active_buffer_top_stack);
    ///
    /// allocator.reset_current();
    /// let active_buffer_top_stack = allocator.active_buffer().marker();
    /// let inactive_buffer_top_stack = allocator.inactive_buffer().marker();
    ///
    /// assert_eq!(allocator.inactive_buffer().stack().ptr(), inactive_buffer_top_stack);
    /// assert_eq!(allocator.active_buffer().stack().ptr(), active_buffer_top_stack);
    /// ```
    pub fn reset_current(&self) {
        self.buffers[self.current as usize].reset();
    }

    /// Swap the buffers. The inactive one becomes the active.
    pub fn swap_buffers(&mut self) {
        self.current = !self.current;
    }

    /// Allocate data in the active allocator's memory, from the current top of its stack.
    ///
    /// # Panics
    /// This function will panic if the current length of the active allocator + the size of the allocated object
    /// exceed the allocator's capacity.
    ///
    /// # Example
    /// ```
    /// use maskerad_stack_allocator::DoubleBufferedAllocator;
    ///
    /// let allocator = DoubleBufferedAllocator::with_capacity(100);
    ///
    /// let my_i32 = allocator.alloc(26);
    /// assert_eq!(my_i32, &mut 26);
    /// ```
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
        assert_eq!(alloc.active_buffer().stack().cap(), 100);
        assert_eq!(alloc.inactive_buffer().stack().cap(), 100);
    }

    #[test]
    fn reset() {
        let alloc = DoubleBufferedAllocator::with_capacity(100);
        let active_buffer_top_stack = alloc.active_buffer().marker();
        let inactive_buffer_top_stack = alloc.inactive_buffer().marker();

        assert_eq!(alloc.active_buffer().stack().ptr(), active_buffer_top_stack);
        assert_eq!(alloc.inactive_buffer().stack().ptr(), inactive_buffer_top_stack);

        let my_i32 = alloc.alloc(25);
        let active_buffer_top_stack = alloc.active_buffer().marker();
        let inactive_buffer_top_stack = alloc.inactive_buffer().marker();

        assert_ne!(alloc.active_buffer().stack().ptr(), active_buffer_top_stack);
        assert_eq!(alloc.inactive_buffer().stack().ptr(), inactive_buffer_top_stack);

        alloc.reset_current();
        let active_buffer_top_stack = alloc.active_buffer().marker();
        let inactive_buffer_top_stack = alloc.inactive_buffer().marker();

        assert_eq!(alloc.active_buffer().stack().ptr(), active_buffer_top_stack);
        assert_eq!(alloc.inactive_buffer().stack().ptr(), inactive_buffer_top_stack);
    }

    #[test]
    fn swap() {
        let mut alloc = DoubleBufferedAllocator::with_capacity(100);
        let first_buffer_top_stack = alloc.buffers[0].marker();
        let second_buffer_top_stack = alloc.buffers[1].marker();

        assert_eq!(alloc.buffers[0].stack().ptr(), first_buffer_top_stack);
        assert_eq!(alloc.buffers[1].stack().ptr(), second_buffer_top_stack);
        alloc.swap_buffers();
        let my_i32 = alloc.alloc(25);
        let first_buffer_top_stack = alloc.buffers[0].marker();
        let second_buffer_top_stack = alloc.buffers[1].marker();

        assert_eq!(alloc.buffers[0].stack().ptr(), first_buffer_top_stack);
        assert_ne!(alloc.buffers[1].stack().ptr(), second_buffer_top_stack);
    }
}
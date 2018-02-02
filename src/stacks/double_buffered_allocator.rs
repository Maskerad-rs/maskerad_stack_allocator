// Copyright 2017-2018 Maskerad Developers
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

use stacks::stack_allocator::StackAllocator;
use allocation_error::AllocationResult;

/// A double-buffered allocator.
///
/// This allocator is a wrapper around two `StackAllocator`s.
/// It works like a `StackAllocator`, and allows you to swap the buffers.
///
/// Ref to the `StackAllocator` documentation for more information.
///
/// # Example
///
/// ```rust
/// use maskerad_memory_allocators::DoubleBufferedAllocator;
/// # use std::error::Error;
/// # fn try_main() -> Result<(), Box<Error>> {
/// //100 bytes for data implementing the Drop trait, 100 bytes for data implementing the `Copy` trait.
/// let mut allocator = DoubleBufferedAllocator::with_capacity(100, 100);
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
///     let my_vec: &Vec<u8> = allocator.alloc(|| {
///         Vec::with_capacity(10)
///     })?;
///
///     assert!(my_vec.is_empty());
///
///     closed = true;
/// }
/// # Ok(())
/// # }
/// # fn main() {
/// #   try_main().unwrap();
/// # }
/// ```
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct DoubleBufferedAllocator {
    buffers: [StackAllocator; 2],
    current: bool,
}

impl DoubleBufferedAllocator {
    /// Create a DoubleBufferedAllocator with the given capacity (in bytes).
    ///
    /// The first capacity is for the `MemoryChunk` holding data implementing the `Drop` trait,
    /// the second is for the `MemoryChunk` holding data implementing the `Copy` trait.
    ///
    /// # Example
    ///
    /// ```rust
    /// use maskerad_memory_allocators::DoubleBufferedAllocator;
    /// # use std::error::Error;
    /// # fn try_main() -> Result<(), Box<Error>> {
    /// let allocator = DoubleBufferedAllocator::with_capacity(100, 50);
    ///
    /// assert_eq!(allocator.active_buffer().capacity(), 100);
    /// assert_eq!(allocator.active_buffer().capacity_copy(), 50);
    /// assert_eq!(allocator.inactive_buffer().capacity(), 100);
    /// assert_eq!(allocator.inactive_buffer().capacity_copy(), 50);
    /// # Ok(())
    /// # }
    /// # fn main() {
    /// #   try_main().unwrap();
    /// # }
    /// ```
    pub fn with_capacity(capacity: usize, capacity_copy: usize) -> Self {
        DoubleBufferedAllocator {
            buffers: [
                StackAllocator::with_capacity(capacity, capacity_copy),
                StackAllocator::with_capacity(capacity, capacity_copy),
            ],
            current: false,
        }
    }

    /// Allocates data in the active buffer, returning a mutable reference to the allocated data.
    ///
    /// If the allocated data implements `Drop`, it will be placed in the `MemoryChunk` storing data implementing the `Drop` trait.
    /// Otherwise, it will be placed in the other `MemoryChunk`.
    ///
    /// # Panic
    /// This function will panic if the allocation exceeds the maximum storage capacity of the active allocator.
    ///
    pub fn alloc_mut<T, F>(&self, op: F) -> AllocationResult<&mut T>
    where
        F: FnOnce() -> T,
    {
        self.active_buffer().alloc_mut(op)
    }

    /// Allocates data in the active buffer, returning an immutable reference to the allocated data.
    ///
    /// If the allocated data implements `Drop`, it will be placed in the `MemoryChunk` storing data implementing the `Drop` trait.
    /// Otherwise, it will be placed in the other `MemoryChunk`.
    ///
    /// # Panic
    /// This function will panic if the allocation exceeds the maximum storage capacity of the active allocator.
    ///
    pub fn alloc<T, F>(&self, op: F) -> AllocationResult<&T>
    where
        F: FnOnce() -> T,
    {
        self.active_buffer().alloc(op)
    }

    /// Reset the active buffer's `MemoryChunk` storing data implementing the `Drop` trait, dropping all the content residing inside it.
    pub fn reset(&self) {
        self.active_buffer().reset();
    }

    /// Reset the active buffer's `MemoryChunk` storing data implementing the `Copy` trait.
    pub fn reset_copy(&self) {
        self.active_buffer().reset_copy();
    }

    /// Returns an immutable reference to the active `StackAllocator`.
    pub fn active_buffer(&self) -> &StackAllocator {
        &self.buffers[self.current as usize]
    }

    /// Returns an immutable reference to the inactive `StackAllocator`.
    pub fn inactive_buffer(&self) -> &StackAllocator {
        &self.buffers[!self.current as usize]
    }

    /// Reset partially the active buffer's `MemoryChunk` storing data implementing the `Drop` trait, dropping all the content residing between the marker and
    /// the first unused memory address of the `MemoryChunk`.
    pub fn reset_to_marker(&self, marker: usize) {
        self.active_buffer().reset_to_marker(marker);
    }

    /// Reset partially the active buffer's `MemoryChunk` storing data implementing the `Copy` trait.
    pub fn reset_to_marker_copy(&self, marker: usize) {
        self.active_buffer().reset_to_marker_copy(marker)
    }

    /// Returns the index of the first unused memory address of the active buffer's `MemoryChunk` storing data implementing
    /// the `Drop` trait.
    pub fn marker(&self) -> usize {
        self.active_buffer().marker()
    }

    /// Returns the index of the first unused memory address of the active buffer's `MemoryChunk` storing data implementing
    /// the `Copy` trait.
    pub fn marker_copy(&self) -> usize {
        self.active_buffer().marker_copy()
    }

    /// Swap the buffers. The inactive one becomes the active.
    pub fn swap_buffers(&mut self) {
        self.current = !self.current;
    }
}

#[cfg(test)]
mod double_buffer_allocator_test {
    use super::*;
    //size : 4 bytes + 4 bytes alignment + 4 bytes + 4 bytes alignment + alignment-offset stuff -> ~16-20 bytes.
    struct Monster {
        _hp: u32,
    }

    impl Default for Monster {
        fn default() -> Self {
            Monster { _hp: 1 }
        }
    }

    impl Drop for Monster {
        fn drop(&mut self) {
            println!("I'm dying !");
        }
    }

    #[test]
    fn new() {
        let alloc = DoubleBufferedAllocator::with_capacity(100, 100);
        assert_eq!(alloc.active_buffer().capacity(), 100);
        assert_eq!(alloc.inactive_buffer().capacity(), 100);
    }

    #[test]
    fn reset() {
        let alloc = DoubleBufferedAllocator::with_capacity(100, 100);

        let start_chunk_active_buffer = alloc.active_buffer().storage_as_ptr();
        let start_chunk_inactive_buffer = alloc.inactive_buffer().storage_as_ptr();

        let index_active_buffer_top_stack = alloc.active_buffer().marker();
        let index_inactive_buffer_top_stack = alloc.inactive_buffer().marker();

        unsafe {
            let active_buffer_top_stack =
                start_chunk_active_buffer.offset(index_active_buffer_top_stack as isize);
            let inactive_buffer_top_stack =
                start_chunk_inactive_buffer.offset(index_inactive_buffer_top_stack as isize);

            assert_eq!(start_chunk_active_buffer, active_buffer_top_stack);
            assert_eq!(start_chunk_inactive_buffer, inactive_buffer_top_stack);
        }

        let _my_monster = alloc.alloc(|| Monster::default()).unwrap();

        let index_active_buffer_top_stack = alloc.active_buffer().marker();
        let index_inactive_buffer_top_stack = alloc.inactive_buffer().marker();

        unsafe {
            let active_buffer_top_stack =
                start_chunk_active_buffer.offset(index_active_buffer_top_stack as isize);
            let inactive_buffer_top_stack =
                start_chunk_inactive_buffer.offset(index_inactive_buffer_top_stack as isize);

            assert_ne!(start_chunk_active_buffer, active_buffer_top_stack);
            assert_eq!(start_chunk_inactive_buffer, inactive_buffer_top_stack);
        }

        alloc.reset();
        let index_active_buffer_top_stack = alloc.active_buffer().marker();
        let index_inactive_buffer_top_stack = alloc.inactive_buffer().marker();

        unsafe {
            let active_buffer_top_stack =
                start_chunk_active_buffer.offset(index_active_buffer_top_stack as isize);
            let inactive_buffer_top_stack =
                start_chunk_inactive_buffer.offset(index_inactive_buffer_top_stack as isize);

            assert_eq!(start_chunk_active_buffer, active_buffer_top_stack);
            assert_eq!(start_chunk_inactive_buffer, inactive_buffer_top_stack);
        }
    }

    #[test]
    fn swap() {
        let mut alloc = DoubleBufferedAllocator::with_capacity(100, 100);
        let start_chunk_first_buffer = alloc.buffers[0].storage_as_ptr();
        let start_chunk_second_buffer = alloc.buffers[1].storage_as_ptr();

        let index_first_buffer_top_stack = alloc.buffers[0].marker();
        let index_second_buffer_top_stack = alloc.buffers[1].marker();

        unsafe {
            let first_buffer_top_stack =
                start_chunk_first_buffer.offset(index_first_buffer_top_stack as isize);
            let second_buffer_top_stack =
                start_chunk_second_buffer.offset(index_second_buffer_top_stack as isize);

            assert_eq!(start_chunk_first_buffer, first_buffer_top_stack);
            assert_eq!(start_chunk_second_buffer, second_buffer_top_stack);
        }

        alloc.swap_buffers();
        let _my_monster = alloc.alloc(|| Monster::default()).unwrap();
        let index_first_buffer_top_stack = alloc.buffers[0].marker();
        let index_second_buffer_top_stack = alloc.buffers[1].marker();

        unsafe {
            let first_buffer_top_stack =
                start_chunk_first_buffer.offset(index_first_buffer_top_stack as isize);
            let second_buffer_top_stack =
                start_chunk_second_buffer.offset(index_second_buffer_top_stack as isize);

            assert_eq!(start_chunk_first_buffer, first_buffer_top_stack);
            assert_ne!(start_chunk_second_buffer, second_buffer_top_stack);
        }
    }
}

// Copyright 2017-2018 Maskerad Developers
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

use memory_chunk::MemoryChunk;
use std::mem;
use core::ptr;
use std::cell::RefCell;

use allocation_error::{AllocationError, AllocationResult};
use utils;

/// A stack-based allocator for data implementing the Copy trait.
///
/// It manages a **MemoryChunk** to:
///
/// - Allocate bytes in a stack-like fashion.
///
/// - Store different types of objects in the same storage.
///
/// # Differences with StackAllocator
/// This stack allocator slightly differs from the non-copy stack allocator. The non-copy
/// stack allocator must extract some metadata (the vtable) about the object it will allocate,
/// to be able to call the drop function of the object when needed. However, a type implementing
/// the Copy trait doesn't, and can't, implement Drop. There is no need to store extra informations
/// about those types, they don't have destructors.
///
/// # Instantiation
/// When instantiated, the memory chunk pre-allocate the given number of bytes.
///
/// # Allocation
/// When an object is allocated in memory, the StackAllocator:
///
/// - Asks a pointer to a memory address to its memory chunk,
///
/// - Place the object in this memory address,
///
/// - Update the first unused memory address of the memory chunk according to an offset,
///
/// - And return a mutable reference to the object which has been placed in the memory chunk.
///
/// This offset is calculated by the size of the object, its memory-alignment and an offset to align the object in memory.
///
/// # Roll-back
/// This structure allows you to get a **marker**, the index to the first unused memory address of the memory chunk. A stack allocator can be *reset* to a marker,
/// or reset entirely.
///
/// When the allocator is reset to a marker, the memory chunk will set the first unused memory address to the marker.
///
/// When the allocator is reset completely, the memory chunk will set the first unused memory address to the bottom of its stack.
///
pub struct StackAllocatorCopy {
    storage: RefCell<MemoryChunk>,
}

impl StackAllocatorCopy {
    /// Creates a StackAllocatorCopy with the given capacity, in bytes.
    /// # Example
    /// ```
    /// #![feature(alloc)]
    /// use maskerad_memory_allocators::StackAllocatorCopy;
    ///
    /// let allocator = StackAllocatorCopy::with_capacity(100);
    /// assert_eq!(allocator.storage().borrow().capacity(), 100);
    /// ```
    pub fn with_capacity(capacity: usize) -> Self {
        StackAllocatorCopy {
            storage: RefCell::new(MemoryChunk::new(capacity)),
        }
    }

    /// Returns an immutable reference to the memory chunk used by the allocator.
    pub fn storage(&self) -> &RefCell<MemoryChunk> {
        &self.storage
    }

    /// Allocates data in the allocator's memory.
    ///
    /// # Panics
    /// This function will panic if the allocation exceeds the maximum storage capacity of the allocator.
    ///
    /// # Example
    /// ```
    /// use maskerad_memory_allocators::StackAllocatorCopy;
    ///
    /// let allocator = StackAllocatorCopy::with_capacity(100);
    ///
    /// let my_i32 = allocator.alloc(|| {
    ///     26 as i32
    /// }).unwrap();
    /// assert_eq!(my_i32, &mut 26);
    /// ```
    #[inline]
    pub fn alloc<T: Copy, F>(&self, op: F) -> AllocationResult<&mut T>
        where F: FnOnce() -> T
    {
        self.alloc_copy(op)
    }

    //Functions for the copyable part of the stack allocator.
    #[inline]
    fn alloc_copy<T: Copy, F>(&self, op: F) -> AllocationResult<&mut T>
        where F: FnOnce() -> T
    {
        unsafe {
            //Get an aligned raw pointer to place the object in it.
            let ptr = self.alloc_copy_inner(mem::size_of::<T>(), mem::align_of::<T>())?;

            //cast this raw pointer to the type of the object.
            let ptr = ptr as *mut T;

            //Write the data in the memory location.
            ptr::write(&mut (*ptr), op());

            //return a mutable reference to this pointer.
            Ok(&mut *ptr)
        }
    }

    #[inline]
    fn alloc_copy_inner(&self, n_bytes: usize, align: usize) -> AllocationResult<*const u8> {
        //borrow mutably the memory chunk used by the allocator.
        let copy_storage = self.storage.borrow_mut();

        //Get the index of the first unused memory address in the memory chunk.
        let fill = copy_storage.fill();

        //Get the index of the aligned memory address, which will be returned.
        let start = utils::round_up(fill, align);

        //Get the index of the future first unused memory address, according to the size of the object.
        let end = start + n_bytes;

        //We don't grow the capacity, or create another chunk.
        if end >= copy_storage.capacity() {
            return Err(AllocationError::OutOfPoolError(format!("The copy stack allocator is out of memory !")));
        }

        //Set the first unused memory address of the memory chunk to the index calculated earlier.
        copy_storage.set_fill(end);

        unsafe {
            //Return the raw pointer to the aligned memory location, which will be used to place
            //the object in the allocator.
            Ok(copy_storage.as_ptr().offset(start as isize))
        }
    }

    /// Returns the index of the first unused memory address.
    ///
    /// # Example
    /// ```
    /// use maskerad_memory_allocators::StackAllocatorCopy;
    ///
    /// let allocator = StackAllocatorCopy::with_capacity(100); //100 bytes
    ///
    /// //Get the raw pointer to the bottom of the allocator's memory chunk.
    /// let start_allocator = allocator.storage().borrow().as_ptr();
    ///
    /// //Get the index of the first unused memory address.
    /// let index_current_top = allocator.marker();
    ///
    /// //Calling offset() on a raw pointer is an unsafe operation.
    /// unsafe {
    ///     //Get the raw pointer, with the index.
    ///     let current_top = start_allocator.offset(index_current_top as isize);
    ///
    ///     //Nothing has been allocated in the allocator,
    ///     //the top of the stack is the bottom of the allocator's memory chunk.
    ///     assert_eq!(current_top, start_allocator);
    /// }
    ///
    /// ```
    pub fn marker(&self) -> usize {
        self.storage.borrow_mut().fill()
    }

    /// Reset the allocator completely.
    ///
    /// # Example
    /// ```
    /// use maskerad_memory_allocators::StackAllocatorCopy;
    ///
    ///
    /// let allocator = StackAllocatorCopy::with_capacity(100); // 100 bytes.
    ///
    /// //When nothing has been allocated, the first unused memory address is at index 0.
    /// assert_eq!(allocator.marker(), 0);
    ///
    /// let an_u8 = allocator.alloc(|| {
    ///     15 as u8
    /// }).unwrap();
    /// assert_ne!(allocator.marker(), 0);
    ///
    /// let bob = allocator.alloc(|| {
    ///     0xb0b as u64
    /// }).unwrap();
    ///
    /// allocator.reset();
    ///
    /// //The allocator has been totally reset, allocation will now start at index 0.
    /// assert_eq!(allocator.marker(), 0);
    ///
    /// ```
    pub fn reset(&self) {
            self.storage.borrow().set_fill(0);
    }

    /// Reset partially the allocator, allocations will occur from the index given by the marker.
    ///
    /// # Example
    /// ```
    /// use maskerad_memory_allocators::StackAllocatorCopy;
    ///
    /// let allocator = StackAllocatorCopy::with_capacity(100); // 100 bytes.
    ///
    /// //When nothing has been allocated, the first unused memory address is at index 0.
    /// assert_eq!(allocator.marker(), 0);
    ///
    /// let an_i32 = allocator.alloc(|| {
    ///     45 as i32
    /// }).unwrap();
    ///
    /// //After the i32 allocation, get the index of the first unused memory address in the allocator.
    /// let index_current_top = allocator.marker();
    /// assert_ne!(index_current_top, 0);
    ///
    /// let an_i64 = allocator.alloc(|| {
    ///     450 as i64
    /// }).unwrap();
    ///
    /// assert_ne!(allocator.marker(), index_current_top);
    ///
    /// allocator.reset_to_marker(index_current_top);
    ///
    /// //The allocator has been partially reset, new allocations will occur from the index given
    /// //by the marker.
    ///
    /// assert_eq!(allocator.marker(), index_current_top);
    ///
    /// ```
    pub fn reset_to_marker(&self, marker: usize) {
            self.storage.borrow().set_fill(marker);
    }
}

#[cfg(test)]
mod stack_allocator_copy_test {
    use super::*;

    #[test]
    fn creation_with_right_capacity() {
        unsafe {
            //create a StackAllocator with the specified size.
            let alloc = StackAllocatorCopy::with_capacity(200);
            let start_chunk = alloc.storage.borrow().as_ptr();
            let first_unused_mem_addr = start_chunk.offset(alloc.storage.borrow().fill() as isize);

            assert_eq!(start_chunk, first_unused_mem_addr);
        }
    }

    #[test]
    fn allocation_test() {
        //Check the allocation with u8, u32 an u64, to verify the alignment behavior.

        //We allocate 200 bytes of memory.
        let alloc = StackAllocatorCopy::with_capacity(200);


        let _test_1_byte = alloc.alloc(|| {
            3 as u8
        }).unwrap();
        let current_top_index = alloc.marker();
        //misaligned by 1 + size of 1 byte = 2.
        assert_eq!(current_top_index, 2);
        assert_eq!(alloc.storage.borrow().capacity() - current_top_index, 198);



        let _test_4_bytes = alloc.alloc(|| {
            60000 as u32
        }).unwrap();
        let current_top_index = alloc.marker();
        //2 + misaligned by 2 + size of 4 byte = 8.
        assert_eq!(current_top_index, 8);
        assert_eq!(alloc.storage.borrow().capacity() - current_top_index, 192);



        let _test_8_bytes = alloc.alloc(|| {
            100000 as u64
        }).unwrap();
        let current_top_index = alloc.marker();
        //8 + misaligned by 8 + size of 8 = 24
        assert_eq!(current_top_index, 24);
        assert_eq!(alloc.storage.borrow().capacity() - current_top_index, 176);
    }

    #[test]
    fn test_reset() {
        let alloc = StackAllocatorCopy::with_capacity(200);
        let start_chunk = alloc.storage.borrow().as_ptr();

        let index_current_top = alloc.marker();
        unsafe {
            let current_top = start_chunk.offset(index_current_top as isize);
            assert_eq!(start_chunk, current_top);
        }

        let _my_u64 = alloc.alloc(|| {
            7894 as u64
        }).unwrap();

        let index_current_top = alloc.marker();
        unsafe {
            let current_top = start_chunk.offset(index_current_top as isize);
            assert_ne!(start_chunk, current_top);
        }

        let _bob = alloc.alloc(|| {
            0xb0b as u64
        }).unwrap();


        unsafe {
            let current_top = start_chunk.offset(index_current_top as isize);
            let new_current_top = start_chunk.offset(alloc.storage.borrow().fill() as isize);
            assert_ne!(current_top, new_current_top);
        }

        alloc.reset_to_marker(index_current_top);

        unsafe {
            let current_top = start_chunk.offset(index_current_top as isize);
            let new_current_top = start_chunk.offset(alloc.storage.borrow().fill() as isize);
            assert_eq!(current_top, new_current_top);
        }

        alloc.reset();

        unsafe {
            let current_top = start_chunk.offset(index_current_top as isize);
            let new_current_top = start_chunk.offset(alloc.storage.borrow().fill() as isize);
            assert_ne!(current_top, new_current_top);
            assert_eq!(new_current_top, start_chunk);
        }
    }
}
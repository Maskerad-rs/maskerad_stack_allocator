// Copyright 2017-2018 Maskerad Developers
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

/*
    Huge internal rewrite, based on the any-arena crate.

    The AnyArena is literally a container of stack-allocators, which is able to grow when needed.

    Allow us to :
    - drop the stuff put into the stack allocator when needed (yay !)
    - have a clearer design
    - base our work on the work of people who actually know how to handle low-level stuff in Rust.
*/


use core::ptr;
use std::cell::RefCell;
use std::mem;

use utils;
use memory_chunk::{ChunkType, MemoryChunk};


/// A double-ended allocator for data implementing the Copy trait.
///
/// It manages two **MemoryChunks** to:
///
/// - Allocate bytes in a stack-like fashion, from both ends.
///
/// - Store different types of objects in the same storage.
///
/// # Differences with DoubleEndedStackAllocator
/// This double-ended stack allocator slightly differs from the non-copy double-ended stack allocator. The non-copy
/// one must extract some metadata (the vtable) about the object it will allocate,
/// to be able to call the drop function of the object when needed. However, a type implementing
/// the Copy trait doesn't, and can't, implement Drop. There is no need to store extra informations
/// about those types, they don't have destructors.
///
/// # Instantiation
/// When instantiated, the memory chunk pre-allocate the given number of bytes, half for one memory chunk, half for the other.
///
/// # Allocation
/// When an object is allocated in memory, the DoubleEndedStackAllocator:
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
pub struct DoubleEndedStackAllocatorCopy {
    storage_resident: RefCell<MemoryChunk>,
    storage_temp: RefCell<MemoryChunk>,
}


impl DoubleEndedStackAllocatorCopy {
    /// Creates a DoubleEndedStackAllocatorCopy with the given capacity, in bytes.
    /// # Example
    /// ```
    /// #![feature(alloc)]
    /// use maskerad_stack_allocator::DoubleEndedStackAllocatorCopy;
    ///
    /// let allocator = DoubleEndedStackAllocatorCopy::with_capacity(100);
    /// assert_eq!(allocator.temp_storage().borrow().capacity(), 50);
    /// assert_eq!(allocator.resident_storage().borrow().capacity(), 50);
    /// ```
    pub fn with_capacity(capacity: usize) -> Self {
        DoubleEndedStackAllocatorCopy {
            storage_resident: RefCell::new(MemoryChunk::new(capacity / 2)),
            storage_temp: RefCell::new(MemoryChunk::new(capacity / 2)),
        }
    }

    /// Returns an immutable reference to the memory chunk used for the resident data.
    pub fn resident_storage(&self) -> &RefCell<MemoryChunk> {
        &self.storage_resident
    }

    /// Returns an immutable reference to the memory chunk used for the temporary data.
    pub fn temp_storage(&self) -> &RefCell<MemoryChunk> {
        &self.storage_temp
    }

    /// Allocates data in the allocator's memory.
    ///
    /// # Panics
    /// This function will panic if the allocation exceeds the maximum storage capacity of the allocator.
    ///
    /// # Example
    /// ```
    /// use maskerad_stack_allocator::DoubleEndedStackAllocatorCopy;
    /// use maskerad_stack_allocator::ChunkType;
    ///
    ///
    /// let allocator = DoubleEndedStackAllocatorCopy::with_capacity(100);
    ///
    /// let my_u64 = allocator.alloc(&ChunkType::TempData, || {
    ///     4587 as u64
    /// });
    ///
    /// let bob = allocator.alloc(&ChunkType::ResidentData, || {
    ///     0xb0b as u64
    /// });
    ///
    /// assert_eq!(my_u64, &mut 4587);
    /// assert_eq!(bob, &mut 0xb0b);
    /// ```
    #[inline]
    pub fn alloc<T: Copy, F>(&self, chunk: &ChunkType, op: F) -> &mut T
        where F: FnOnce() -> T
    {
        self.alloc_copy(chunk, op)
    }



    //Functions for the non-copyable part of the arena.

    /// The function actually writing data in the memory chunk
    #[inline]
    fn alloc_copy<T: Copy, F>(&self, chunk: &ChunkType, op: F) -> &mut T
        where F: FnOnce() -> T
    {
        unsafe {
            //Get an aligned raw pointer to place the object in it.
            let ptr = self.alloc_copy_inner(chunk, mem::size_of::<T>(), mem::align_of::<T>());

            //cast this raw pointer to the type of the object.
            let ptr = ptr as *mut T;

            //Write the data in the memory location.
            ptr::write(&mut (*ptr), op());

            //Return a mutable reference to the object.
            &mut *ptr
        }
    }

    /// The function asking the memory chunk to give us raw pointers to memory locations and update
    /// the current top of the stack.
    #[inline]
    fn alloc_copy_inner(&self, chunk: &ChunkType, n_bytes: usize, align: usize) -> *const u8 {

        match chunk {
            &ChunkType::TempData => {
                //mutably borrow the memory chunk.
                let mut copy_storage_temp = self.storage_temp.borrow_mut();

                //Get the index of the first unused memory address in the memory chunk.
                let fill = copy_storage_temp.fill();

                //Get the index of the aligned memory address, which will be returned.
                let mut start = utils::round_up(fill, align);

                //Get the index of the future first unused memory address, according to the size of the object.
                let mut end = start + n_bytes;

                assert!(end < copy_storage_temp.capacity());



                //Update the current top of the stack.
                //The first unused memory address is at index 'end',
                //where the next type description would be written
                //if an allocation was asked.
                copy_storage_temp.set_fill(end);

                unsafe {
                    //Return the raw pointer to the aligned memory location, which will be used to place
                    //the object in the allocator.
                    copy_storage_temp.as_ptr().offset(start as isize)
                }
            },
            &ChunkType::ResidentData => {
                //mutably borrow the memory chunk.
                let mut copy_storage_resident = self.storage_resident.borrow_mut();

                //Get the index of the first unused memory address in the memory chunk.
                let fill = copy_storage_resident.fill();

                //Get the index of the aligned memory address, which will be returned.
                let mut start = utils::round_up(fill, align);

                //Get the index of the future first unused memory address, according to the size of the object.
                let mut end = start + n_bytes;

                assert!(end < copy_storage_resident.capacity());



                //Update the current top of the stack.
                //The first unused memory address is at index 'end',
                //where the next type description would be written
                //if an allocation was asked.
                copy_storage_resident.set_fill(end);

                unsafe {
                    //Return the raw pointer to the aligned memory location, which will be used to place
                    //the object in the allocator.
                    copy_storage_resident.as_ptr().offset(start as isize)
                }
            },
        }

    }

    /// Returns the index of the first unused memory address.
    ///
    /// # Example
    /// ```
    /// use maskerad_stack_allocator::DoubleEndedStackAllocatorCopy;
    /// use maskerad_stack_allocator::ChunkType;
    ///
    /// let allocator = DoubleEndedStackAllocatorCopy::with_capacity(100); //50 bytes for each memory chunk.
    ///
    /// //Get the raw pointer to the bottom of the memory chunk used for temp data.
    /// let start_allocator_temp = allocator.temp_storage().borrow().as_ptr();
    ///
    /// //Get the index of the first unused memory address in the memory chunk used for temp data.
    /// let index_temp = allocator.marker(&ChunkType::TempData);
    ///
    /// //Calling offset() on a raw pointer is an unsafe operation.
    /// unsafe {
    ///     //Get the raw pointer, with the index.
    ///     let current_top = start_allocator_temp.offset(index_temp as isize);
    ///
    ///     //Nothing has been allocated in the memory chunk used for temp data,
    ///     //the top of the stack is the bottom of the memory chunk.
    ///     assert_eq!(current_top, start_allocator_temp);
    /// }
    ///
    /// ```
    pub fn marker(&self, chunk: &ChunkType) -> usize {
        match chunk {
            &ChunkType::ResidentData => {
                self.storage_resident.borrow_mut().fill()
            },
            &ChunkType::TempData => {
                self.storage_temp.borrow_mut().fill()
            },
        }
    }

    /// Reset the allocator, allocations will occur from the bottom of the stack.
    ///
    /// # Example
    /// ```
    /// use maskerad_stack_allocator::DoubleEndedStackAllocatorCopy;
    /// use maskerad_stack_allocator::ChunkType;
    ///
    ///
    /// let allocator = DoubleEndedStackAllocatorCopy::with_capacity(100); // 50 bytes for each memory chunk.
    ///
    /// //When nothing has been allocated, the first unused memory address is at index 0.
    /// assert_eq!(allocator.marker(&ChunkType::TempData), 0);
    /// assert_eq!(allocator.marker(&ChunkType::ResidentData), 0);
    ///
    /// let my_u64 = allocator.alloc(&ChunkType::TempData, || {
    ///     456 as u64
    /// });
    ///
    /// assert_ne!(allocator.marker(&ChunkType::TempData), 0);
    ///
    /// let my_i32 = allocator.alloc(&ChunkType::TempData, || {
    ///     12 as i32
    /// });
    ///
    /// allocator.reset(&ChunkType::TempData);
    ///
    /// //The memory chunk for temp data has been totally reset.
    ///
    /// assert_eq!(allocator.marker(&ChunkType::TempData), 0);
    ///
    /// ```
    pub fn reset(&self, chunk: &ChunkType) {
        match chunk {
            &ChunkType::TempData => {
                self.storage_temp.borrow().set_fill(0);
            },
            &ChunkType::ResidentData => {
                self.storage_resident.borrow().set_fill(0);
            },
        }
    }



    /// Reset partially the allocator, allocations will occur from the marker.
    ///
    /// # Example
    /// ```
    /// use maskerad_stack_allocator::DoubleEndedStackAllocatorCopy;
    /// use maskerad_stack_allocator::ChunkType;
    ///
    ///
    /// let allocator = DoubleEndedStackAllocatorCopy::with_capacity(100); // 100 bytes.
    ///
    /// //When nothing has been allocated, the first unused memory address is at index 0.
    /// assert_eq!(allocator.marker(&ChunkType::TempData), 0);
    ///
    /// let my_u64 = allocator.alloc(&ChunkType::TempData, || {
    ///     123 as u64
    /// });
    ///
    /// //After the u64 allocation, get the index of the first unused memory address in the memory chunk used for temp data.
    /// let index_current_temp = allocator.marker(&ChunkType::TempData);
    /// assert_ne!(index_current_temp, 0);
    ///
    /// let my_i32 = allocator.alloc(&ChunkType::TempData, || {
    ///     321 as i32
    /// });
    ///
    /// assert_ne!(allocator.marker(&ChunkType::TempData), index_current_temp);
    ///
    /// allocator.reset_to_marker(&ChunkType::TempData, index_current_temp);
    ///
    /// //The allocator has been partially reset.
    ///
    /// assert_eq!(allocator.marker(&ChunkType::TempData), index_current_temp);
    ///
    /// ```
    pub fn reset_to_marker(&self, chunk: &ChunkType, marker: usize) {
        match chunk {
            &ChunkType::TempData => {
                self.storage_temp.borrow().set_fill(marker);
            },
            &ChunkType::ResidentData => {
                self.storage_resident.borrow().set_fill(marker);
            },
        }
    }
}

#[cfg(test)]
mod double_ended_stack_allocator_copy_test {
    use super::*;

    #[test]
    fn creation_with_right_capacity() {
        unsafe {
            //create a StackAllocator with the specified size.
            let alloc = DoubleEndedStackAllocatorCopy::with_capacity(200);

            let start_chunk_temp = alloc.storage_temp.borrow().as_ptr();
            let first_unused_mem_addr_temp = start_chunk_temp.offset(alloc.marker(&ChunkType::TempData) as isize);

            let start_chunk_resident = alloc.storage_resident.borrow().as_ptr();
            let first_unused_mem_addr_resident = start_chunk_resident.offset(alloc.marker(&ChunkType::ResidentData) as isize);
            assert_eq!(start_chunk_temp, first_unused_mem_addr_temp);
            assert_eq!(start_chunk_resident, first_unused_mem_addr_resident);
            assert_eq!(alloc.storage_resident.borrow().capacity(), 100);
            assert_eq!(alloc.storage_temp.borrow().capacity(), 100);
        }
    }

    #[test]
    fn allocation_test() {
        //Check the allocation with u8, u32 an u64, to verify the alignment behavior.

        //We allocate 200 bytes of memory.
        let alloc = DoubleEndedStackAllocatorCopy::with_capacity(200);


        let _test_1_byte = alloc.alloc(&ChunkType::TempData, || {
            3 as u8
        });
        let current_top_index_temp = alloc.marker(&ChunkType::TempData);
        let current_top_index_resident = alloc.marker(&ChunkType::ResidentData);
        //misaligned by 1 + size of 1 byte = 2.
        assert_eq!(current_top_index_temp, 2);
        assert_eq!(current_top_index_resident, 0);
        assert_eq!(alloc.storage_temp.borrow().capacity() - current_top_index_temp, 98);
        assert_eq!(alloc.storage_resident.borrow().capacity() - current_top_index_resident, 100);



        let _test_4_bytes = alloc.alloc(&ChunkType::TempData, || {
            60000 as u32
        });
        let current_top_index_temp = alloc.marker(&ChunkType::TempData);
        let current_top_index_resident = alloc.marker(&ChunkType::ResidentData);
        //2 + misaligned by 2 + size of 4 byte = 8.
        assert_eq!(current_top_index_temp, 8);
        assert_eq!(current_top_index_resident, 0);
        assert_eq!(alloc.storage_temp.borrow().capacity() - current_top_index_temp, 92);
        assert_eq!(alloc.storage_resident.borrow().capacity() - current_top_index_resident, 100);



        let _test_8_bytes = alloc.alloc(&ChunkType::ResidentData, || {
            100000 as u64
        });
        let current_top_index_temp = alloc.marker(&ChunkType::TempData);
        let current_top_index_resident = alloc.marker(&ChunkType::ResidentData);
        //misaligned by 8 + 8 size = 16
        assert_eq!(current_top_index_temp, 8);
        assert_eq!(current_top_index_resident, 16);
        assert_eq!(alloc.storage_temp.borrow().capacity() - current_top_index_temp, 92);
        assert_eq!(alloc.storage_resident.borrow().capacity() - current_top_index_resident, 84);
    }

    #[test]
    fn test_reset() {
        let alloc = DoubleEndedStackAllocatorCopy::with_capacity(200);
        let start_chunk_temp = alloc.storage_temp.borrow().as_ptr();
        let start_chunk_resident = alloc.storage_resident.borrow().as_ptr();

        let index_current_top_temp = alloc.marker(&ChunkType::TempData);

        unsafe {
            let current_top_temp = start_chunk_temp.offset(alloc.marker(&ChunkType::TempData) as isize);
            let current_top_resident = start_chunk_resident.offset(alloc.marker(&ChunkType::ResidentData) as isize);

            assert_eq!(start_chunk_temp, current_top_temp);
            assert_eq!(start_chunk_resident, current_top_resident);
        }

        let _my_u64 = alloc.alloc(&ChunkType::TempData, || {
            7894 as u64
        });

        unsafe {
            let current_top_temp = start_chunk_temp.offset(alloc.marker(&ChunkType::TempData) as isize);
            let current_top_resident = start_chunk_resident.offset(alloc.marker(&ChunkType::ResidentData) as isize);

            assert_ne!(start_chunk_temp, current_top_temp);
            assert_eq!(start_chunk_resident, current_top_resident);
        }

        let _bob = alloc.alloc(&ChunkType::ResidentData, || {
            0xb0b as u64
        });

        unsafe {
            let current_top_temp = start_chunk_temp.offset(alloc.marker(&ChunkType::TempData) as isize);
            let current_top_resident = start_chunk_resident.offset(alloc.marker(&ChunkType::ResidentData) as isize);

            assert_ne!(start_chunk_temp, current_top_temp);
            assert_ne!(start_chunk_resident, current_top_resident);
        }

        alloc.reset_to_marker(&ChunkType::TempData, index_current_top_temp);

        unsafe {
            let current_top_temp = start_chunk_temp.offset(alloc.marker(&ChunkType::TempData) as isize);
            let current_top_resident = start_chunk_resident.offset(alloc.marker(&ChunkType::ResidentData) as isize);

            assert_eq!(start_chunk_temp, current_top_temp);
            assert_ne!(start_chunk_resident, current_top_resident);
        }

        alloc.reset(&ChunkType::ResidentData);

        unsafe {
            let current_top_temp = start_chunk_temp.offset(alloc.marker(&ChunkType::TempData) as isize);
            let current_top_resident = start_chunk_resident.offset(alloc.marker(&ChunkType::ResidentData) as isize);

            assert_eq!(start_chunk_temp, current_top_temp);
            assert_eq!(start_chunk_resident, current_top_resident);
        }
    }
}
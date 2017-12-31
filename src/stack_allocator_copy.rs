// Copyright 2017 Maskerad Developers
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

use memory_chunk::MemoryChunk;
use std::mem;
use core::ptr;
use std::cell::RefCell;

use utils;

/// A stack-based allocator.
///
/// It manages a copy MemoryChunk to:
///
/// - Allocate bytes in a stack-like fashion.
///
/// - Store different types of objects in the same storage.
///
///
/// This stack allocator slightly differs from the non-copy stack allocator. The non-copy
/// stack allocator must extract some metadata (the vtable) about the object it will allocate,
/// to be able to call the drop function of the object when needed. However, a type implementing
/// the Copy trait doesn't, and can't, implement Drop. There is no need to store extra informations
/// about those types, they don't have destructors.
///
///
/// When instantiated, the memory chunk pre-allocate the given number of bytes.
///
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
///
///
/// This offset is calculated by the size of the object, its memory-alignment and an offset to align the object in memory.
///
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
    pub fn with_capacity(capacity: usize) -> Self {
        StackAllocatorCopy {
            storage: RefCell::new(MemoryChunk::new(capacity)),
        }
    }

    #[inline]
    pub fn alloc<T: Copy, F>(&self, op: F) -> &mut T
        where F: FnOnce() -> T
    {
        self.alloc_copy(op)
    }

    //Functions for the copyable part of the stack allocator.
    #[inline]
    fn alloc_copy<T: Copy, F>(&self, op: F) -> &mut T
        where F: FnOnce() -> T
    {
        unsafe {
            let ptr = self.alloc_copy_inner(mem::size_of::<T>(), mem::align_of::<T>());
            let ptr = ptr as *mut T;
            ptr::write(&mut (*ptr), op());
            &mut *ptr
        }
    }

    #[inline]
    fn alloc_copy_inner(&self, n_bytes: usize, align: usize) -> *const u8 {
        let mut copy_storage = self.storage.borrow_mut();
        let fill = copy_storage.fill();

        let mut start = utils::round_up(fill, align);
        let mut end = start + n_bytes;

        //We don't grow the capacity, or create another chunk.
        assert!(end <= copy_storage.capacity());

        copy_storage.set_fill(end);

        unsafe {
            copy_storage.as_ptr().offset(start as isize)
        }
    }

    pub fn marker(&self) -> usize {
        self.storage.borrow_mut().fill()
    }

    pub fn reset(&self) {
            self.storage.borrow().set_fill(0);
    }

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
        });
        let current_top_index = alloc.marker();
        //misaligned by 1 + size of 1 byte = 2.
        assert_eq!(current_top_index, 2);
        assert_eq!(alloc.storage.borrow().capacity() - current_top_index, 198);



        let _test_4_bytes = alloc.alloc(|| {
            60000 as u32
        });
        let current_top_index = alloc.marker();
        //2 + misaligned by 2 + size of 4 byte = 8.
        assert_eq!(current_top_index, 8);
        assert_eq!(alloc.storage.borrow().capacity() - current_top_index, 192);



        let _test_8_bytes = alloc.alloc(|| {
            100000 as u64
        });
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

        let my_u64 = alloc.alloc(|| {
            7894 as u64
        });

        let index_current_top = alloc.marker();
        unsafe {
            let current_top = start_chunk.offset(index_current_top as isize);
            assert_ne!(start_chunk, current_top);
        }

        let bob = alloc.alloc(|| {
            0xb0b as u64
        });


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
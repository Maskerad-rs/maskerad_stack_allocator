// Copyright 2017 Maskerad Developers
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

use memory_chunk::MemoryChunk;
use std::mem;
use std::core::ptr;
use std::cell::RefCell;

use utils;

pub struct StackAllocatorCopy {
    storage_copy: RefCell<MemoryChunk>,
}

impl StackAllocatorCopy {
    pub fn with_capacity(capacity: usize) -> Self {
        StackAllocatorCopy {
            storage_copy: RefCell::new(MemoryChunk::new(capacity, true)),
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
        let mut copy_storage = self.storage_copy.borrow_mut();
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
        self.storage_copy.borrow_mut().fill()
    }

    pub fn reset(&self) {
        unsafe {
            self.storage_copy.borrow().set_fill(0);
        }
    }

    pub fn reset_to_marker(&self, marker: usize) {
        unsafe {
            self.storage_copy.borrow().set_fill(marker);
        }
    }
}
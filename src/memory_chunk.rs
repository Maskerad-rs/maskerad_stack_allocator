// Copyright 2017 Maskerad Developers
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

use alloc::raw_vec::RawVec;
use std::cell::Cell;
use core::mem;

use utils;

pub struct MemoryChunk {
    storage: RawVec<u8>,
    /// Index of the first unused byte.
    fill: Cell<usize>,
    /// Indicates whether objects with destructors are stored in this chunk.
    is_copy: Cell<bool>,
}

impl MemoryChunk {
    /// Create a new memory chunk, allocating the given size.
    pub fn new(size: usize, is_copy: bool) -> Self {
        MemoryChunk {
            storage: RawVec::with_capacity(size),
            fill: Cell::new(0),
            is_copy: Cell::new(is_copy),
        }
    }

    pub fn fill(&self) -> usize {
        self.fill.get()
    }

    pub fn set_fill(&self, first_unused_byte: usize) {
        self.fill.set(first_unused_byte)
    }

    pub fn is_copy(&self) -> bool {
        self.is_copy.get()
    }

    pub fn capacity(&self) -> usize {
        self.storage.cap()
    }

    pub unsafe fn as_ptr(&self) -> *const u8 {
        self.storage.ptr()
    }

    //Walk down the chunk, running the destructors for any objects stored in it.
    pub unsafe fn destroy(&self) {
        self.destroy_to_marker(0);
    }

    pub unsafe fn destroy_to_marker(&self, marker: usize) {
        let mut index = marker;
        let storage_start = self.as_ptr();
        let fill = self.fill.get();

        while index < fill {
            let type_description_data = storage_start.offset(index as isize) as *const usize;
            let (type_description, is_done) = utils::un_bitpack_type_description_ptr(*type_description_data);
            let (size, alignment) = ((*type_description).size, (*type_description).alignment);

            let after_type_description = index + mem::size_of::<*const utils::TypeDescription>();

            let start = utils::round_up(after_type_description, alignment);

            if is_done {
                ((*type_description).drop_glue)(storage_start.offset(start as isize) as *const i8);
            }

            //Find where the next type description lives.
            index = utils::round_up(start + size, mem::align_of::<*const utils::TypeDescription>());
        }
    }
}
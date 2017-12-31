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

/// The MemoryChunk is a just a chunk of memory.
/// It uses a [RawVec](https://doc.rust-lang.org/alloc/raw_vec/struct.RawVec.html) to allocate bytes
/// in a vector-like fashion.
///
/// This structure allows you allocate data of different type in the same storage, since :
///
/// - The chunk knows the location of the first unused byte in its memory storage, and update it when allocation occurs or
/// when objects in the memory chunk are dropped.
///
/// - The chunk extracts some info about the type (its virtual table) and place it next to the object. The chunk is able to call the drop method of the object
/// with the virtual table.
///
pub struct MemoryChunk {
    storage: RawVec<u8>,
    /// Index of the first unused byte.
    fill: Cell<usize>,
}

impl MemoryChunk {
    /// Creates a new memory chunk, allocating the given number of bytes.
    pub fn new(size: usize) -> Self {
        MemoryChunk {
            storage: RawVec::with_capacity(size),
            fill: Cell::new(0),
        }
    }

    /// Returns the index of the first unused byte in the memory storage of the chunk.
    pub fn fill(&self) -> usize {
        self.fill.get()
    }

    /// Set the index of the first unused byte in the memory storage of the chunk.
    pub fn set_fill(&self, first_unused_byte: usize) {
        self.fill.set(first_unused_byte)
    }

    /// Returns the maximal number of bytes the chunk can store.
    pub fn capacity(&self) -> usize {
        self.storage.cap()
    }

    /// Returns a pointer to the start of the memory storage used by the chunk.
    pub fn as_ptr(&self) -> *const u8 {
        self.storage.ptr()
    }

    /// Drop all the data contained in the chunk.
    pub unsafe fn destroy(&self) {
        self.destroy_to_marker(0);
    }

    /// Drop the data contained in the chunk, starting from the given marker.
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
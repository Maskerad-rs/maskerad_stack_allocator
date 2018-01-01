// Copyright 2017-2018 Maskerad Developers
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

use alloc::raw_vec::RawVec;
use std::cell::Cell;
use core::mem;

use utils;

/// An enum specifying the ends of the DoubleEndedMemoryChunk.
/// Used by the memory chunk to know from which end an operation should occur.
pub enum ChunkStartPosition {
    Bottom,
    Middle,
}

/// The double-ended MemoryChunk is just a chunk of memory, where allocations can occur at the bottom
/// of the stack, or at the middle of the stack.
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
///
/// You should not use the double-ended MemoryChunk directly. The double-ended allocators manage double-ended memory chunks, use them.
pub struct DoubleEndedMemoryChunk {
    storage: RawVec<u8>,
    /// Index of the first unused byte, from the middle of the stack.
    fill_middle: Cell<usize>,
    /// Index of the first unused byte, from the bottom end.
    fill_bottom: Cell<usize>,
}

impl DoubleEndedMemoryChunk {
    /// Creates a new memory chunk, allocating the given number of bytes.
    pub fn new(size: usize) -> Self {
        DoubleEndedMemoryChunk {
            storage: RawVec::with_capacity(size),
            fill_middle: Cell::new(size / 2),
            fill_bottom: Cell::new(0),

        }
    }

    /// Returns the index of the first unused byte in the memory storage of the chunk.
    pub fn fill(&self, chunk_start_position: &ChunkStartPosition) -> usize {
        match chunk_start_position {
            &ChunkStartPosition::Bottom => {
                self.fill_bottom.get()
            },
            &ChunkStartPosition::Middle => {
                self.fill_middle.get()
            },
        }
    }

    /// Set the index of the first unused byte in the memory storage of the chunk, according to the chosen starting position.
    pub fn set_fill(&self, chunk_start_position: &ChunkStartPosition, first_unused_byte: usize) {
        match chunk_start_position {
            &ChunkStartPosition::Middle => {
                self.fill_middle.set(first_unused_byte);
            },
            &ChunkStartPosition::Bottom => {
                self.fill_bottom.set(first_unused_byte);
            },
        }
    }



    /// Returns the maximal number of bytes the chunk can store.
    pub fn capacity(&self) -> usize {
        self.storage.cap()
    }

    /// Returns a pointer to the start, or middle, of the memory storage used by the chunk.
    ///
    // # Safety
    // If we want the "starting" pointer from the middle, we need to offset the starting pointer
    // from the bottom end by max_capacity() / 2.
    //
    // Even though this function should never be a source of undefined behavior, getting a pointer by applying an offset
    // from a pointer is an unsafe operation. Refer to the offset(count: isize) function of
    // raw pointers in the official documentation for more information.
    pub fn as_ptr(&self, chunk_start_position: &ChunkStartPosition) -> *const u8 {
        match chunk_start_position {
            &ChunkStartPosition::Bottom => {
                self.storage.ptr()
            },
            &ChunkStartPosition::Middle => {
                unsafe {
                    self.storage.ptr().offset((self.storage.cap() / 2) as isize)
                }
            },
        }
    }

    /// Drop all the data contained in the chunk.
    pub unsafe fn destroy_all(&self) {
        self.destroy_end(&ChunkStartPosition::Bottom);
        self.destroy_end(&ChunkStartPosition::Middle);
    }

    /// Drop the data contained in one half of the chunk.
    pub unsafe fn destroy_end(&self, chunk_start_position: &ChunkStartPosition) {
        self.destroy_end_to_marker(chunk_start_position, 0);
    }

    /// Drop the data contained in one half the chunk, starting from the given marker.
    pub unsafe fn destroy_end_to_marker(&self, chunk_start_position: &ChunkStartPosition, marker: usize) {

        //Get the index of the marker.
        //We'll start dropping the content from this location.
        let mut index = marker;

        //Get a raw pointer to the bottom of the memory storage, according to the chosen end.
        let storage_start = self.as_ptr(chunk_start_position);

        //Get the index of the first unused memory address, according to the chosen end.
        //We'll stop dropping the content when we are at this location.
        let fill = self.fill(chunk_start_position);

        //While the starting index is inferior to the ending one...
        while index < fill {

            //Get a raw pointer on the TypeDescription of the object.
            let type_description_data = storage_start.offset(index as isize) as *const usize;

            //Decode this raw pointer to obtain the vtable of the object, and a boolean to know if
            //the object has been initialized.
            let (type_description, is_done) = utils::un_bitpack_type_description_ptr(*type_description_data);

            //Get the size and the alignment of the object, with its type description.
            let (size, alignment) = ((*type_description).size, (*type_description).alignment);

            //Get the index of the memory address just after the type description.
            //It's the unaligned memory address of the object.
            let after_type_description = index + mem::size_of::<*const utils::TypeDescription>();

            //Get the aligned memory address, with the unaligned one and the alignment of the object.
            //This is where the object *really* lives.
            let start = utils::round_up(after_type_description, alignment);

            //If the object has been successfully initialized, we can call its drop function.
            //We call the function pointer of the object's vtable, here drop_glue, and give him the pointer
            //to the location of the object.
            if is_done {
                ((*type_description).drop_glue)(storage_start.offset(start as isize) as *const i8);
            }

            //Find where the next type description lives.
            index = utils::round_up(start + size, mem::align_of::<*const utils::TypeDescription>());
        }
    }
}
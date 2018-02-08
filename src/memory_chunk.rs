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

/// The MemoryChunk is just a chunk of memory.
/// It uses a [RawVec](https://doc.rust-lang.org/alloc/raw_vec/struct.RawVec.html) to allocate bytes
/// in a vector-like fashion.
///
/// This structure allows you allocate data of different type in the same storage, since :
///
/// - The chunk knows the location of the first unused byte in its memory storage, and update it when allocation occurs or
/// when objects in the memory chunk are dropped.
///
/// - The chunk extracts, for the types implementing the Drop trait,
/// some info about the type (its virtual table) and place it next to the object. The chunk is able to call the drop method of the object
/// with the virtual table.
///
///
/// You should not use the MemoryChunk directly. The allocators manage memory chunks, use them.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct MemoryChunk {
    storage: RawVec<u8>,
    /// Index of the first unused byte.
    fill: Cell<usize>,
}

impl MemoryChunk {
    /// Creates a new memory chunk, allocating the given number of bytes.
    pub fn new(size: usize) -> Self {
        debug!("Creating a new memory chunk of {} bytes in size.", size);
        MemoryChunk {
            storage: RawVec::with_capacity(size),
            fill: Cell::new(0),
        }
    }

    /// Returns the index of the first unused byte in the memory storage of the chunk.
    pub fn fill(&self) -> usize {
        debug!("Getting the first unused byte of memory of the memory chunk.");
        self.fill.get()
    }

    /// Set the index of the first unused byte in the memory storage of the chunk.
    pub fn set_fill(&self, first_unused_byte: usize) {
        debug!("Setting the first unused byte of memory of the memory chunk to {}", first_unused_byte);
        self.fill.set(first_unused_byte)
    }

    /// Returns the maximal number of bytes the chunk can store.
    pub fn capacity(&self) -> usize {
        debug!("Getting the maximum capacity in byte of the memory chunk.");
        self.storage.cap()
    }

    /// Returns a pointer to the start of the memory storage used by the chunk.
    pub fn as_ptr(&self) -> *const u8 {
        debug!("Getting a raw pointer to the start of the allocation of the memory chunk.");
        self.storage.ptr()
    }

    /// Drop all the data contained in the chunk.
    pub unsafe fn destroy(&self) {
        debug!("Dropping all the data contained in the memory chunk.");
        self.destroy_to_marker(0);
    }

    /// Drop the data contained in the chunk, starting from the given marker.
    pub unsafe fn destroy_to_marker(&self, marker: usize) {
        debug!("Dropping the data lying between the byte {} and the byte {}, contained in the memory chunk.", marker, self.fill.get());
        //Get the index of the marker.
        //We'll start dropping the content from this location.
        let mut index = marker;

        //Get a raw pointer to the bottom of the memory storage.
        let storage_start = self.as_ptr();

        //Get the index of the first unused memory address.
        //We'll stop dropping the content when we are at this location.
        let fill = self.fill.get();

        //While the starting index is inferior to the ending one...
        while index < fill {
            //Get a raw pointer on the TypeDescription of the object.
            let type_description_data = storage_start.offset(index as isize) as *const usize;

            //Decode this raw pointer to obtain the vtable of the object, and a boolean to know if
            //the object has been initialized.
            let (type_description, is_done) =
                utils::un_bitpack_type_description_ptr(*type_description_data);

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
            index = utils::round_up(
                start + size,
                mem::align_of::<*const utils::TypeDescription>(),
            );
        }
    }
}

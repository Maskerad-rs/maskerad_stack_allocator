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


/// A double-ended allocator for data implementing the Drop trait.
///
/// It manages two **MemoryChunks** to:
///
/// - Allocate bytes in a stack-like fashion.
///
/// - Store different types of objects in the same storage.
///
/// - Store data needed for a long period of time in one MemoryChunk, and store temporary data in the other.
///
/// - Drop the content of the MemoryChunk when needed.
///
/// # Instantiation
/// When instantiated, the memory chunk pre-allocate the given number of bytes, half in the first MemoryChunk, half in the other.
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
/// This offset is calculated by the size of the object, the size of a TypeDescription structure, its memory-alignment and an offset to align the object in memory.
///
/// # Roll-back
/// This structure allows you to get a **marker**, the index to the first unused memory address of a memory chunk. A stack allocator can be *reset* to a marker,
/// or reset entirely.
///
/// When the allocator is reset to a marker, the memory chunk will drop all the content lying between the marker and the first unused memory address,
/// and set the first unused memory address to the marker.
///
/// When the allocator is reset completely, the memory chunk will drop everything and set the first unused memory address to the bottom of its stack.
///
/// # Purpose
/// Suppose you want to load data **A**, and this data need the temporary data **B**. You need to load **B** before **A**
/// in order to create it.
///
/// After **A** is loaded, **B** is no longer needed. However, since this allocator is a stack, you need to free **A** before freeing **B**.
///
/// That's why this allocator has two memory chunks, one for temporary data, one for the resident data who need temporary data to be created.
///
///
/// # Example
///
/// ```
/// use maskerad_stack_allocator::DoubleEndedStackAllocator;
/// use maskerad_stack_allocator::ChunkType;
/// use maskerad_stack_allocator::StackAllocator;
///
/// struct LevelConfig {
///     data: u32,
/// }
///
/// impl LevelConfig {
///     pub fn new(data: u32) -> Self {
///         LevelConfig {
///             data,
///         }
///     }
///
///     pub fn data(&self) -> u32 {
///         self.data
///     }
/// }
///
/// impl Drop for LevelConfig {
///     fn drop(&mut self) {
///         println!("LevelConfig dropped.");
///     }
/// }
///
/// struct Level {
///     number: u32
/// }
///
/// impl Level {
///     pub fn from_config(config: &mut LevelConfig) -> Self {
///         Level {
///             number: config.data(),
///         }
///     }
/// }
///
/// impl Drop for Level {
///     fn drop(&mut self) {
///         println!("Level dropped.");
///     }
/// }
///
/// //50 bytes for each memory chunk.
/// let double_ended_allocator = DoubleEndedStackAllocator::with_capacity(100);
/// let allocator = StackAllocator::with_capacity(100);
///
///
/// //we want to load data from a config file, in order to load a level, let's try with a standard stack allocator !
///
/// //Get a marker to the current top of the stack, we will reset later to drop the LevelConfig.
/// let allocator_marker = allocator.marker();
///
/// let my_config = allocator.alloc(|| {
///     LevelConfig::new(42)
/// });
///
/// //Now create the level.
/// let my_level = allocator.alloc(|| {
///     Level::from_config(my_config)
/// });
///
/// //We got our level, let's drop the config allocated in the allocator !
/// allocator.reset_to_marker(allocator_marker);
///
/// // "Level dropped."
/// // "LevelConfig dropped."
///
/// //Ouch... if we want to drop the LevelConfig, we must drop the Level too, since it is a stack.
///
///
/// let my_config = double_ended_allocator.alloc(&ChunkType::TempData, || {
///     LevelConfig::new(42)
/// });
///
/// let my_level = double_ended_allocator.alloc(&ChunkType::ResidentData, || {
///     Level::from_config(my_config)
/// });
///
/// double_ended_allocator.reset(&ChunkType::TempData);
/// // "LevelConfig dropped."
///
/// //That's the purpose of the double ended allocator.
///
/// ```


pub struct DoubleEndedStackAllocator {
    storage_resident: RefCell<MemoryChunk>,
    storage_temp: RefCell<MemoryChunk>,
}


impl DoubleEndedStackAllocator {
    /// Creates a DoubleEndedStackAllocator with the given capacity, in bytes.
    /// # Example
    /// ```
    /// #![feature(alloc)]
    /// use maskerad_stack_allocator::DoubleEndedStackAllocator;
    ///
    /// let allocator = DoubleEndedStackAllocator::with_capacity(100);
    /// assert_eq!(allocator.temp_storage().borrow().capacity(), 50);
    /// assert_eq!(allocator.resident_storage().borrow().capacity(), 50);
    /// ```
    pub fn with_capacity(capacity: usize) -> Self {
        DoubleEndedStackAllocator {
            storage_resident: RefCell::new(MemoryChunk::new(capacity / 2)),
            storage_temp: RefCell::new(MemoryChunk::new(capacity / 2)),
        }
    }

    /// Returns an immutable reference to the memory chunk used for resident allocation.
    pub fn resident_storage(&self) -> &RefCell<MemoryChunk> {
        &self.storage_resident
    }

    /// Returns an immutable reference to the memory chunk used for temporary allocation.
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
    /// use maskerad_stack_allocator::DoubleEndedStackAllocator;
    /// use maskerad_stack_allocator::ChunkType;
    ///
    /// struct Monster {
    ///     hp: u32,
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
    /// impl Drop for Monster {
    ///     fn drop(&mut self) {
    ///         println!("I'm dying !");
    ///     }
    /// }
    ///
    /// let allocator = DoubleEndedStackAllocator::with_capacity(100);
    ///
    /// let my_monster = allocator.alloc(&ChunkType::TempData, || {
    ///     Monster::default()
    /// });
    ///
    /// let another_monster = allocator.alloc(&ChunkType::ResidentData, || {
    ///     Monster::default()
    /// });
    ///
    /// assert_eq!(my_monster.level, 1);
    /// assert_eq!(another_monster.hp, 1);
    /// ```
    #[inline]
    pub fn alloc<T, F>(&self, chunk: &ChunkType, op: F) -> &mut T
        where F: FnOnce() -> T
    {
        self.alloc_non_copy(chunk, op)
    }



    //Functions for the non-copyable part of the arena.

    /// The function actually writing data in the memory chunk
    #[inline]
    fn alloc_non_copy<T, F>(&self, chunk: &ChunkType, op: F) -> &mut T
        where F: FnOnce() -> T
    {
        unsafe {
            //Get the type description of the type T (get its vtable).
            let type_description = utils::get_type_description::<T>();

            //Ask the memory chunk to give us raw pointers to memory locations for our type description and object
            let (type_description_ptr, ptr) = self.alloc_non_copy_inner(chunk, mem::size_of::<T>(), mem::align_of::<T>());

            //Cast them.
            let type_description_ptr = type_description_ptr as *mut usize;
            let ptr = ptr as *mut T;

            //write in our type description along with a bit indicating that the object has *not*
            //been initialized yet.
            *type_description_ptr = utils::bitpack_type_description_ptr(type_description, false);

            //Initialize the object.
            ptr::write(&mut (*ptr), op());

            //Now that we are done, update the type description to indicate
            //that the object is there.
            *type_description_ptr = utils::bitpack_type_description_ptr(type_description, true);

            //Return a mutable reference to the object.
            &mut *ptr
        }
    }

    /// The function asking the memory chunk to give us raw pointers to memory locations and update
    /// the current top of the stack.
    #[inline]
    fn alloc_non_copy_inner(&self, chunk: &ChunkType, n_bytes: usize, align: usize) -> (*const u8, *const u8) {

        match chunk {
            &ChunkType::TempData => {
                //mutably borrow the memory chunk.
                let mut non_copy_temp_storage = self.storage_temp.borrow_mut();

                //Get the index of the first unused byte in the memory chunk.
                let fill = non_copy_temp_storage.fill();

                //Get the index of where We'll write the type description data
                //(the first unused byte in the memory chunk).
                let mut type_description_start = fill;

                // Get the index of where the object should reside (unaligned location actually).
                let after_type_description = fill + mem::size_of::<*const utils::TypeDescription>();

                //With the index to the unaligned memory address, determine the index to
                //the aligned memory address where the object will reside,
                //according to its memory alignment.
                let mut start = utils::round_up(after_type_description, align);

                //Determine the index of the next aligned memory address for a type description, according the the size of the object
                //and the memory alignment of a type description.
                let mut end = utils::round_up(start + n_bytes, mem::align_of::<*const utils::TypeDescription>());

                assert!(end < non_copy_temp_storage.capacity());

                //Update the current top of the stack.
                //The first unused memory address is at index 'end',
                //where the next type description would be written
                //if an allocation was asked.
                non_copy_temp_storage.set_fill(end);

                unsafe {
                    // Get a raw pointer to the start of our MemoryChunk's RawVec
                    let start_storage = non_copy_temp_storage.as_ptr();

                    (
                        //From this raw pointer, get the correct raw pointers with
                        //the indexes we calculated earlier.

                        //The raw pointer to the type description of the object.
                        start_storage.offset(type_description_start as isize),

                        //The raw pointer to the object.
                        start_storage.offset(start as isize)
                    )
                }
            },
            &ChunkType::ResidentData => {
                //mutably borrow the memory chunk.
                let mut non_copy_resident_storage = self.storage_resident.borrow_mut();

                //Get the index of the first unused byte in the memory chunk.
                let fill = non_copy_resident_storage.fill();

                //Get the index of where We'll write the type description data
                //(the first unused byte in the memory chunk).
                let mut type_description_start = fill;

                // Get the index of where the object should reside (unaligned location actually).
                let after_type_description = fill + mem::size_of::<*const utils::TypeDescription>();

                //With the index to the unaligned memory address, determine the index to
                //the aligned memory address where the object will reside,
                //according to its memory alignment.
                let mut start = utils::round_up(after_type_description, align);

                //Determine the index of the next aligned memory address for a type description, according the the size of the object
                //and the memory alignment of a type description.
                let mut end = utils::round_up(start + n_bytes, mem::align_of::<*const utils::TypeDescription>());

                assert!(end < non_copy_resident_storage.capacity());

                //Update the current top of the stack.
                //The first unused memory address is at index 'end',
                //where the next type description would be written
                //if an allocation was asked.
                non_copy_resident_storage.set_fill(end);

                unsafe {
                    // Get a raw pointer to the start of our MemoryChunk's RawVec
                    let start_storage = non_copy_resident_storage.as_ptr();

                    (
                        //From this raw pointer, get the correct raw pointers with
                        //the indexes we calculated earlier.

                        //The raw pointer to the type description of the object.
                        start_storage.offset(type_description_start as isize),

                        //The raw pointer to the object.
                        start_storage.offset(start as isize)
                    )
                }
            },
        }

    }

    /// Returns the index of the first unused memory address.
    ///
    /// # Example
    /// ```
    /// use maskerad_stack_allocator::DoubleEndedStackAllocator;
    /// use maskerad_stack_allocator::ChunkType;
    ///
    /// let allocator = DoubleEndedStackAllocator::with_capacity(100); //50 bytes for each memory chunk.
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

    /// Reset the allocator, dropping all the content residing inside it.
    ///
    /// # Example
    /// ```
    /// use maskerad_stack_allocator::DoubleEndedStackAllocator;
    /// use maskerad_stack_allocator::ChunkType;
    ///
    /// struct Monster {
    ///     _hp :u32,
    /// }
    ///
    /// impl Default for Monster {
    ///     fn default() -> Self {
    ///         Monster {
    ///         _hp: 1,
    ///         }
    ///     }
    /// }
    ///
    /// impl Drop for Monster {
    ///     fn drop(&mut self) {
    ///         println!("Monster is dying !");
    ///     }
    /// }
    ///
    /// struct Dragon {
    ///     _level: u8,
    /// }
    ///
    /// impl Default for Dragon {
    ///     fn default() -> Self {
    ///         Dragon {
    ///             _level: 1,
    ///         }
    ///     }
    /// }
    ///
    /// impl Drop for Dragon {
    ///     fn drop(&mut self) {
    ///         println!("Dragon is dying !");
    ///     }
    /// }
    ///
    /// let allocator = DoubleEndedStackAllocator::with_capacity(100); // 50 bytes for each memory chunk.
    ///
    /// //When nothing has been allocated, the first unused memory address is at index 0.
    /// assert_eq!(allocator.marker(&ChunkType::TempData), 0);
    /// assert_eq!(allocator.marker(&ChunkType::ResidentData), 0);
    ///
    /// let my_monster = allocator.alloc(&ChunkType::TempData, || {
    ///     Monster::default()
    /// });
    ///
    /// assert_ne!(allocator.marker(&ChunkType::TempData), 0);
    ///
    /// let my_dragon = allocator.alloc(&ChunkType::TempData, || {
    ///     Dragon::default()
    /// });
    ///
    /// allocator.reset(&ChunkType::TempData);
    ///
    /// //The memory chunk for temp data has been totally reset, and all its content has been dropped.
    ///
    /// //"Dragon is dying!".
    /// //"Monster is dying!".
    ///
    /// assert_eq!(allocator.marker(&ChunkType::TempData), 0);
    ///
    /// ```
    pub fn reset(&self, chunk: &ChunkType) {
        unsafe {
            match chunk {
                &ChunkType::ResidentData => {
                    self.storage_resident.borrow().destroy();
                    self.storage_resident.borrow().set_fill(0);
                },
                &ChunkType::TempData => {
                    self.storage_temp.borrow().destroy();
                    self.storage_temp.borrow().set_fill(0);
                },
            }
        }
    }



    /// Reset partially the allocator, dropping all the content residing between the marker and
    /// the first unused memory address of the allocator.
    ///
    /// # Example
    /// ```
    /// use maskerad_stack_allocator::DoubleEndedStackAllocator;
    /// use maskerad_stack_allocator::ChunkType;
    ///
    /// struct Monster {
    ///     _hp :u32,
    /// }
    ///
    /// impl Default for Monster {
    ///     fn default() -> Self {
    ///         Monster {
    ///         _hp: 1,
    ///         }
    ///     }
    /// }
    ///
    /// impl Drop for Monster {
    ///     fn drop(&mut self) {
    ///         println!("Monster is dying !");
    ///     }
    /// }
    ///
    /// struct Dragon {
    ///     _level: u8,
    /// }
    ///
    /// impl Default for Dragon {
    ///     fn default() -> Self {
    ///         Dragon {
    ///             _level: 1,
    ///         }
    ///     }
    /// }
    ///
    /// impl Drop for Dragon {
    ///     fn drop(&mut self) {
    ///         println!("Dragon is dying !");
    ///     }
    /// }
    ///
    /// let allocator = DoubleEndedStackAllocator::with_capacity(100); // 100 bytes.
    ///
    /// //When nothing has been allocated, the first unused memory address is at index 0.
    /// assert_eq!(allocator.marker(&ChunkType::TempData), 0);
    ///
    /// let my_monster = allocator.alloc(&ChunkType::TempData, || {
    ///     Monster::default()
    /// });
    ///
    /// //After the monster allocation, get the index of the first unused memory address in the memory chunk used for temp data.
    /// let index_current_temp = allocator.marker(&ChunkType::TempData);
    /// assert_ne!(index_current_temp, 0);
    ///
    /// let my_dragon = allocator.alloc(&ChunkType::TempData, || {
    ///     Dragon::default()
    /// });
    ///
    /// assert_ne!(allocator.marker(&ChunkType::TempData), index_current_temp);
    ///
    /// allocator.reset_to_marker(&ChunkType::TempData, index_current_temp);
    ///
    /// //The allocator has been partially reset, and all the content lying between the marker and
    /// //the first unused memory address has been dropped.
    ///
    /// //"Dragon is dying!".
    ///
    /// assert_eq!(allocator.marker(&ChunkType::TempData), index_current_temp);
    ///
    /// ```
    pub fn reset_to_marker(&self, chunk: &ChunkType, marker: usize) {
        unsafe {
            match chunk {
                &ChunkType::ResidentData => {
                    self.storage_resident.borrow().destroy_to_marker(marker);
                    self.storage_resident.borrow().set_fill(marker);
                },
                &ChunkType::TempData => {
                    self.storage_temp.borrow().destroy_to_marker(marker);
                    self.storage_temp.borrow().set_fill(marker);
                },
            }
        }
    }
}

impl Drop for DoubleEndedStackAllocator {
    fn drop(&mut self) {
        unsafe {
            self.storage_temp.borrow().destroy();
            self.storage_resident.borrow().destroy();
        }
    }
}


#[cfg(test)]
mod double_ended_stack_allocator_test {
    use super::*;

    //size : 4 bytes + 4 bytes alignment + 4 bytes + 4 bytes alignment + alignment-offset stuff -> ~16-20 bytes.
    struct Monster {
        _hp :u32,

    }

    impl Monster {
        pub fn new(hp: u32) -> Self {
            Monster {
                _hp: hp,
            }
        }
    }

    impl Default for Monster {
        fn default() -> Self {
            Monster {
                _hp: 1,
            }
        }
    }

    impl Drop for Monster {
        fn drop(&mut self) {
            println!("I'm dying !");
        }
    }

    #[test]
    fn creation_with_right_capacity() {
        unsafe {
            //create a StackAllocator with the specified size.
            let alloc = DoubleEndedStackAllocator::with_capacity(200);
            let start_chunk_temp = alloc.storage_temp.borrow().as_ptr();
            let start_chunk_resident = alloc.storage_resident.borrow().as_ptr();
            let first_unused_mem_addr_temp = start_chunk_temp.offset(alloc.storage_temp.borrow().fill() as isize);
            let first_unused_mem_addr_resident = start_chunk_resident.offset(alloc.storage_resident.borrow().fill() as isize);

            assert_eq!(start_chunk_temp, first_unused_mem_addr_temp);
            assert_eq!(start_chunk_resident, first_unused_mem_addr_resident);
        }
    }


    #[test]
    fn allocation_test() {
        //We allocate 200 bytes of memory.
        let alloc = DoubleEndedStackAllocator::with_capacity(200);

        let start_alloc_temp = alloc.storage_temp.borrow().as_ptr();
        let start_alloc_resident = alloc.storage_resident.borrow().as_ptr();

        let _my_monster = alloc.alloc(&ChunkType::TempData, || {
            Monster::new(1)
        });

        unsafe {

            let top_stack_temp = start_alloc_temp.offset(alloc.marker(&ChunkType::TempData) as isize);
            let top_stack_resident = start_alloc_resident.offset(alloc.marker(&ChunkType::ResidentData) as isize);

            assert_ne!(start_alloc_temp, top_stack_temp);
            assert_eq!(start_alloc_resident, top_stack_resident);
        }

        let _my_monster = alloc.alloc(&ChunkType::ResidentData, || {
            Monster::new(1)
        });

        unsafe {
            let top_stack_temp = start_alloc_temp.offset(alloc.marker(&ChunkType::TempData) as isize);
            let top_stack_resident = start_alloc_resident.offset(alloc.marker(&ChunkType::ResidentData) as isize);

            assert_ne!(start_alloc_temp, top_stack_temp);
            assert_ne!(start_alloc_resident, top_stack_resident);
        }
    }

    //Use 'cargo test -- --nocapture' to see the monsters' println!s
    #[test]
    fn test_reset() {
        let alloc = DoubleEndedStackAllocator::with_capacity(200);

        let top_stack_index_temp = alloc.marker(&ChunkType::TempData);

        let start_alloc_temp = alloc.storage_temp.borrow().as_ptr();
        let start_alloc_resident = alloc.storage_resident.borrow().as_ptr();

        let _my_monster = alloc.alloc(&ChunkType::TempData, || {
            Monster::new(1)
        });

        unsafe {
            let top_stack_temp = start_alloc_temp.offset(alloc.marker(&ChunkType::TempData) as isize);
            let top_stack_resident = start_alloc_resident.offset(alloc.marker(&ChunkType::ResidentData) as isize);

            assert_ne!(start_alloc_temp, top_stack_temp);
            assert_eq!(start_alloc_resident, top_stack_resident);
        }

        let _another_monster = alloc.alloc(&ChunkType::ResidentData, || {
            Monster::default()
        });

        unsafe {
            let top_stack_temp = start_alloc_temp.offset(alloc.marker(&ChunkType::TempData) as isize);
            let top_stack_resident = start_alloc_resident.offset(alloc.marker(&ChunkType::ResidentData) as isize);

            assert_ne!(start_alloc_temp, top_stack_temp);
            assert_ne!(start_alloc_resident, top_stack_resident);
        }

        alloc.reset_to_marker(&ChunkType::TempData, top_stack_index_temp);

        //my_monster drop here successfully

        unsafe {
            let top_stack_temp = start_alloc_temp.offset(alloc.marker(&ChunkType::TempData) as isize);
            let top_stack_resident = start_alloc_resident.offset(alloc.marker(&ChunkType::ResidentData) as isize);

            assert_eq!(start_alloc_temp, top_stack_temp);
            assert_ne!(start_alloc_resident, top_stack_resident);
        }

        alloc.reset(&ChunkType::ResidentData);

        //another_monster drop here successfully

        unsafe {
            let top_stack_temp = start_alloc_temp.offset(alloc.marker(&ChunkType::TempData) as isize);
            let top_stack_resident = start_alloc_resident.offset(alloc.marker(&ChunkType::ResidentData) as isize);

            assert_eq!(start_alloc_temp, top_stack_temp);
            assert_eq!(start_alloc_resident, top_stack_resident);
        }
    }

}

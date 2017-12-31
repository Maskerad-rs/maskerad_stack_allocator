// Copyright 2017 Maskerad Developers
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


use alloc::allocator::Layout;
use core::ptr;
use std::cell::RefCell;
use alloc::heap;
use std::mem;

use utils;
use memory_chunk::MemoryChunk;


/// A stack-based allocator.
///
/// It manages a non-copy MemoryChunk to:
///
/// - Allocate bytes in a stack-like fashion.
///
/// - Store different types of objects in the same storage.
///
/// - Drop the content of the MemoryChunk when needed.
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
/// This offset is calculated by the size of the object, the size of a TypeDescription structure, its memory-alignment and an offset to align the object in memory.
///
/// This structure allows you to get a **marker**, the index to the first unused memory address of the memory chunk. A stack allocator can be *reset* to a marker,
/// or reset entirely.
///
/// When the allocator is reset to a marker, the memory chunk will drop all the content lying between the marker and the first unused memory address,
/// and set the first unused memory address to the marker.
///
/// When the allocator is reset completely, the memory chunk will drop everything and set the first unused memory address to the bottom of its stack.
///
/// # Example
///
/// ```
/// use maskerad_stack_allocator::StackAllocator;
///
/// struct Monster {
///     hp :u32,
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
/// let single_frame_allocator = StackAllocator::with_capacity(100); //100 bytes
/// let mut closed = false;
///
/// while !closed {
///     // The allocator is cleared every frame.
///     // (The pointer to the current top of the stack goes back to the bottom).
///     single_frame_allocator.reset();
///
///     //...
///
///     //allocate from the single frame allocator.
///     //Be sure to use the data during this frame only!
///     let my_monster = single_frame_allocator.alloc(Monster::default());
///
///     assert_eq!(my_monster.level, 1);
///     closed = true;
/// }
/// ```


pub struct StackAllocator {
    storage: RefCell<MemoryChunk>,
}


impl StackAllocator {
    /// Creates a StackAllocator with the given capacity, in bytes.
    /// # Example
    /// ```
    /// #![feature(alloc)]
    /// use maskerad_stack_allocator::StackAllocator;
    ///
    /// let allocator = StackAllocator::with_capacity(100);
    /// assert_eq!(allocator.stack().cap(), 100);
    /// ```
    pub fn with_capacity(capacity: usize) -> Self {
        StackAllocator {
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
    /// use maskerad_stack_allocator::StackAllocator;
    ///
    /// let allocator = StackAllocator::with_capacity(100);
    ///
    /// let my_i32 = allocator.alloc(26);
    /// assert_eq!(my_i32, &mut 26);
    /// ```
    #[inline]
    pub fn alloc<T, F>(&self, op: F) -> &mut T
        where F: FnOnce() -> T
    {
        self.alloc_non_copy(op)
    }



    //Functions for the non-copyable part of the arena.

    /// The function actually writing data in the memory chunk
    #[inline]
    fn alloc_non_copy<T, F>(&self, op: F) -> &mut T
        where F: FnOnce() -> T
    {
        unsafe {
            //Get the type description of the type T (get its vtable).
            let type_description = utils::get_type_description::<T>();

            //Ask the memory chunk to give us raw pointers to memory locations for our type description and object
            let (type_description_ptr, ptr) = self.alloc_non_copy_inner(mem::size_of::<T>(), mem::align_of::<T>());

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
    fn alloc_non_copy_inner(&self, n_bytes: usize, align: usize) -> (*const u8, *const u8) {
        //mutably borrow the memory chunk.
        let mut non_copy_storage = self.storage.borrow_mut();

        //Get the index of the first unused byte in the memory chunk.
        let fill = non_copy_storage.fill();

        //Get the index of where We'll write the type description data
        //(the first unused byte in the memory chunk).
        let mut type_description_start = fill;

        // Get the index of where the object should reside (unaligned location actually).
        let after_type_description = fill + mem::size_of::<*const utils::TypeDescription>();

        //With the index to the unaligned memory address, determine the index to
        //the aligned memory address where the object will reside,
        //according to its memory alignment.
        let mut start = utils::round_up(after_type_description, align);

        //Determine the index of the next memory address for a type description, according the the size of the object
        //and the memory alignment of a type description.
        let mut end = utils::round_up(start + n_bytes, mem::align_of::<*const utils::TypeDescription>());

        //If the allocator becomes oom after this possible allocation, abort the program.
        assert!(end <= non_copy_storage.capacity());

        //Update the current top of the stack.
        //The first unused memory address is at index 'end',
        //where the next type description would be written
        //if an allocation was asked.
        non_copy_storage.set_fill(end);

        unsafe {
            // Get a raw pointer to the start of our MemoryChunk's RawVec
            let start_storage = non_copy_storage.as_ptr();

            (
                //From this raw pointer, get the correct raw pointers with
                //the indexes we calculated earlier.

                //The raw pointer to the type description of the object.
                start_storage.offset(type_description_start as isize),

                //The raw pointer to the object.
                start_storage.offset(start as isize)
            )
        }
    }

    /// Returns the index of the first unused memory address.
    pub fn marker(&self) -> usize {
        self.storage.borrow_mut().fill()
    }

    /// Reset the allocator, dropping all the content residing inside it.
    pub fn reset(&self) {
        unsafe {
            self.storage.borrow().destroy();
            self.storage.borrow().set_fill(0);
        }
    }

    /// Reset partially the allocator, dropping all the content residing between the marker and
    /// the first unused memory address of the allocator.
    pub fn reset_to_marker(&self, marker: usize) {
        unsafe {
            self.storage.borrow().destroy_to_marker(marker);
            self.storage.borrow().set_fill(marker);
        }
    }
}

impl Drop for StackAllocator {
    fn drop(&mut self) {
        unsafe {
            self.storage.borrow().destroy();
        }
    }
}

#[cfg(test)]
mod stack_allocator_test {
    use super::*;

    //size : 4 bytes + 4 bytes alignment + 4 bytes + 4 bytes alignment + alignment-offset stuff -> ~16-20 bytes.
    struct Monster {
        hp :u32,
        level: u32,
    }

    impl Monster {
        pub fn new(hp: u32, level: u32) -> Self {
            Monster {
                hp: 1,
                level: 1,
            }
        }
    }

    impl Default for Monster {
        fn default() -> Self {
            Monster {
                hp: 1,
                level: 1,
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
            let alloc = StackAllocator::with_capacity(200);
            let start_chunk = alloc.storage.borrow().as_ptr();
            let first_unused_mem_addr = start_chunk.offset(alloc.storage.borrow().fill() as isize);

            assert_eq!(start_chunk, first_unused_mem_addr);
        }
    }

    #[test]
    fn allocation_test() {
        //We allocate 200 bytes of memory.
        let alloc = StackAllocator::with_capacity(200);

        let my_monster = alloc.alloc(|| {
            Monster::new(1, 2)
        });

        unsafe {
            let start_alloc = alloc.storage.borrow().as_ptr();
            let top_stack_index = alloc.storage.borrow().fill();
            let top_stack = start_alloc.offset(top_stack_index as isize);
            assert_ne!(start_alloc, top_stack);
        }
    }

    //Use 'cargo test -- --nocapture' to see the monsters' println!s
    #[test]
    fn test_reset() {
        let alloc = StackAllocator::with_capacity(200);
        let my_monster = alloc.alloc(|| {
            Monster::new(1, 3)
        });

        let top_stack_index = alloc.marker();
        let start_alloc = alloc.storage.borrow().as_ptr();
        let mut current_top_stack_index = alloc.storage.borrow().fill();

        unsafe {
            let top_stack = start_alloc.offset(top_stack_index as isize);
            let current_top_stack = start_alloc.offset(current_top_stack_index as isize);
            assert_eq!(current_top_stack, top_stack);
        }

        let another_monster = alloc.alloc(|| {
            Monster::default()
        });

        current_top_stack_index = alloc.storage.borrow().fill();

        unsafe {
            let top_stack = start_alloc.offset(top_stack_index as isize);
            let current_top_stack = start_alloc.offset(current_top_stack_index as isize);
            assert_ne!(current_top_stack, top_stack);
        }

        alloc.reset_to_marker(top_stack_index);

        //TODO: the another_monster must print "i'm dying !"

        current_top_stack_index = alloc.storage.borrow().fill();
        unsafe {
            let top_stack = start_alloc.offset(top_stack_index as isize);
            let current_top_stack = start_alloc.offset(current_top_stack_index as isize);
            assert_eq!(current_top_stack, top_stack);
        }

        alloc.reset();

        //TODO: the my_monster must print "i'm dying !"

        current_top_stack_index = alloc.storage.borrow().fill();
        unsafe {
            let top_stack = start_alloc.offset(top_stack_index as isize);
            let current_top_stack = start_alloc.offset(current_top_stack_index as isize);
            assert_ne!(current_top_stack, top_stack);
            assert_eq!(current_top_stack, start_alloc);
        }
    }
}

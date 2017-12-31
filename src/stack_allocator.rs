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
    - a clearer design
    - based on the work of people who actually know how to handle low-level stuff in Rust.
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
/// It uses a [RawVec](https://doc.rust-lang.org/alloc/raw_vec/struct.RawVec.html) to allocate bytes in a vector-like fashion
/// and a pointer to its current top of the stack.
///
/// When instantiated, the top pointer is at the bottom of the stack.
/// When an object is allocated in memory, a pointer to the current top of the stack is returned and
/// the pointer to the current top of the stack is moved according to an offset.
///
/// This offset is calculated by the size of the object, its memory-alignment and an offset to align the object in memory.
///
/// When the allocator is reset, the pointer to the top of the stack is moved to the bottom of the stack. Allocation will occur
/// from the bottom of the stack and will override previously allocated memory.
///
/// # Be careful
///
/// This allocator is **dropless**: memory is never *really* freed. You must guarantee that, when overriding memory, this memory was not used.
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
    storage_non_copy: RefCell<MemoryChunk>,
}


impl StackAllocator {
    /// Create a StackAllocator with the given capacity (in bytes).
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
            storage_non_copy: RefCell::new(MemoryChunk::new(capacity, false)),
        }
    }

    /// Allocate data in the allocator's memory, from the current top of the stack.
    /// # Panics
    /// This function will panic if the current length of the allocator + the size of the allocated object
    /// exceed the allocator's capacity.
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
    #[inline]
    fn alloc_non_copy<T, F>(&self, op: F) -> &mut T
        where F: FnOnce() -> T
    {
        unsafe {
            let type_description = utils::get_type_description::<T>();
            let (type_description_ptr, ptr) = self.alloc_non_copy_inner(mem::size_of::<T>(), mem::align_of::<T>());
            let type_description_ptr = type_description_ptr as *mut usize;
            let ptr = ptr as *mut T;

            //write in our type description along with a bit indicating that it has *not*
            //been initialized yet.
            *type_description_ptr = utils::bitpack_type_description_ptr(type_description, false);
            //Initialize it.
            ptr::write(&mut (*ptr), op());
            //Now that we are done, update the type description to indicate
            //that the object is there.
            *type_description_ptr = utils::bitpack_type_description_ptr(type_description, true);

            &mut *ptr
        }
    }

    #[inline]
    fn alloc_non_copy_inner(&self, n_bytes: usize, align: usize) -> (*const u8, *const u8) {
        let mut non_copy_storage = self.storage_non_copy.borrow_mut();
        let fill = non_copy_storage.fill();

        let mut type_description_start = fill;
        let after_type_description = fill + mem::size_of::<*const utils::TypeDescription>();
        let mut start = utils::round_up(after_type_description, align);
        let mut end = utils::round_up(start + n_bytes, mem::align_of::<*const utils::TypeDescription>());

        assert!(end <= non_copy_storage.capacity());

        non_copy_storage.set_fill(end);

        unsafe {
            let start_storage = non_copy_storage.as_ptr();

            (
                start_storage.offset(type_description_start as isize),
                start_storage.offset(start as isize)
            )
        }
    }

    pub fn marker(&self) -> usize {
        self.storage_non_copy.borrow_mut().fill()
    }

    pub fn reset(&self) {
        unsafe {
            self.storage_non_copy.borrow().destroy();
            self.storage_non_copy.borrow().set_fill(0);
        }
    }

    pub fn reset_to_marker(&self, marker: usize) {
        unsafe {
            self.storage_non_copy.borrow().destroy_to_marker(marker);
            self.storage_non_copy.borrow().set_fill(marker);
        }
    }
}

impl Drop for StackAllocator {
    fn drop(&mut self) {
        unsafe {
            self.storage_non_copy.borrow().destroy();
        }
    }
}

#[cfg(test)]
mod stack_allocator_test {
    use super::*;
    extern crate time;

    #[test]
    fn creation_with_right_capacity() {
        //create a StackAllocator with the specified size.
        let alloc = StackAllocator::with_capacity(200);
        let cap_used = alloc.stack.ptr().offset_to(alloc.current_offset.get()).unwrap() as usize;
        let cap_remaining = (alloc.stack.cap() - cap_used) as isize;
        assert_eq!(cap_used, 0);
        assert_eq!(cap_remaining, 200);
    }

    #[test]
    fn allocation_test() {
        //Check the allocation with u8, u32 an u64, to verify the alignment behavior.

        //We allocate 200 bytes of memory.
        let alloc = StackAllocator::with_capacity(200);

        /*
            U8 :
            alignment : 1 byte alignment (can be aligned to any address in memory).
            size : 1 byte.

            We allocate 2 (alignment + size) bytes of memory.
            Explanation : We'll adjust the address later. It allows for the worst-case address adjustment.

            mask (used for adjustment) : alignment - 1 = 0x00000000 (0)

            We calculate the misalignment by this operation : unaligned address & mask.
            The bitwise AND of the mask and any unaligned address yield the misalignment of this address.
            here, unaligned address & 0 = 0.
            a value needing a 1 byte alignment is never misaligned.

            we calculate the adjustment like this : alignment - misalignment.
            here, alignment - misalignment = 1.
            our 1 byte aligned data keeps the 1 byte alignment since it's not misaligned. (and it's never misaligned)

            total amount of memory used: (alignment + size) + adjustment = 3.
        */
        let _test_1_byte = alloc.alloc::<u8>(2);
        let cap_used = alloc.stack.ptr().offset_to(alloc.current_offset.get()).unwrap() as usize;
        let cap_remaining = (alloc.stack.cap() - cap_used) as isize;
        assert_eq!(cap_used, 3); //3
        assert_eq!(cap_remaining, 197); //200 - 3

        /*
            U32 :
            alignment : 4 byte alignment (can be aligned to addresses finishing by 0x0 0x4 0x8 0xC).
            size : 4 bytes.

            We allocate 8 (alignment + size) bytes of memory.
            Explanation : We'll adjust the address later. It allows for the worst-case address adjustment.

            mask (used for adjustment) : alignment - 1 = 0x00000003 (3)

            We calculate the misalignment with this operation : unaligned address & mask.
            The bitwise AND of the mask and any unaligned address yield the misalignment of this address.
            here, misalignment = unaligned address & 3 = 3.

            we calculate the adjustment like this : alignment - misalignment.
            here, alignment - misalignment = 1.
            our 4 byte aligned data must have an address adjusted by 1 byte, since it's misaligned by 3 bytes.

            total amount of memory used: (alignment + size) + adjustment = 9.
        */
        let _test_4_bytes = alloc.alloc::<u32>(60000);
        let cap_used = alloc.stack.ptr().offset_to(alloc.current_offset.get()).unwrap() as usize;
        let cap_remaining = (alloc.stack.cap() - cap_used) as isize;
        assert_eq!(cap_used, 12); //3 + 9
        assert_eq!(cap_remaining, 188); //200 - 3 - 9
        /*
            U64 :
            alignment : 8 byte alignment (can be aligned to addresses finishing by 0x0 0x8).
            size : 8 byte.

            We allocate 16 (alignment + size) bytes of memory.
            Explanation : We'll adjust the address later. It allows for the worst-case address adjustment.

            mask (used for adjustment) : alignment - 1 = 0x00000007 (7)

            We calculate the misalignment by this operation : unaligned address & mask.
            The bitwise AND of the mask and any unaligned address yield the misalignment of this address.
            here, misalignment = unaligned address & 7 = 4.

            we calculate the adjustment like this : alignment - misalignment.
            here, alignment - misalignment = 4.
            our 8 byte aligned data must have an address adjusted by 4 bytes, since it's misaligned by 4 bytes.

            total amount of memory used: (alignment + size) + adjustment = 20.
        */
        let _test_8_bytes = alloc.alloc::<u64>(100000);
        let cap_used = alloc.stack.ptr().offset_to(alloc.current_offset.get()).unwrap() as usize;
        let cap_remaining = (alloc.stack.cap() - cap_used) as isize;
        assert_eq!(cap_used, 32); // 3 + 9 + 20
        assert_eq!(cap_remaining, 168); //200 - 3 - 9 - 20
    }

    #[test]
    fn test_reset() {
        //Test if there's any problem with memory overwriting.
        let alloc = StackAllocator::with_capacity(200);
        let test_1_byte = alloc.alloc::<u8>(2);
        assert_eq!(test_1_byte, &mut 2);
        alloc.reset();
        let test_1_byte = alloc.alloc::<u8>(5);
        assert_eq!(test_1_byte, &mut 5);
    }

    //size : 4 bytes + 4 bytes alignment + 4 bytes + 4 bytes alignment + alignment-offset stuff -> ~16-20 bytes.
    struct Monster {
        hp :u32,
        level: u32,
    }

    impl Default for Monster {
        fn default() -> Self {
            Monster {
                hp: 1,
                level: 1,
            }
        }
    }

    /*
    #[test]
    fn speed_comparison() {
        let before = time::precise_time_ns();
        for _ in 0..1000 {
            let monster1 = Box::new(Monster::default());
            let monster2 = Box::new(Monster::default());
            let monster3 = Box::new(Monster::default());
        }
        let after = time::precise_time_ns();
        let elapsed = after - before;
        println!("Time with heap alloc: {}", elapsed);

        let single_frame_alloc = StackAlloc::with_capacity(100);
        let before = time::precise_time_ns();
        for _ in 0..1000 {
            single_frame_alloc.reset();
            let monster1 = single_frame_alloc.alloc(Monster::default());
            let monster2 = single_frame_alloc.alloc(Monster::default());
            let monster3 = single_frame_alloc.alloc(Monster::default());
        }
        let after = time::precise_time_ns();
        let elapsed = after - before;
        println!("Time with stack alloc: {}", elapsed);

        panic!();
    }
    */
}

// Copyright 2017 Maskerad Developers
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

use alloc::raw_vec::RawVec;
use alloc::allocator::Layout;
use core;
use std::cell::Cell;

/// A double-ended stack-based allocator.
///
/// It uses a [RawVec](https://doc.rust-lang.org/alloc/raw_vec/struct.RawVec.html) to allocate bytes in a vector-like fashion
/// and two pointers: One pointing to the bottom of the stack, the other to the top of the stack.
///
/// When instantiated, one pointer is at the bottom of the stack, the other at the top of the stack.
///
/// When an object is allocated in memory, a pointer to the current top of the stack is returned and
/// a pointer, depending from which end the object was allocated, to the current top of the stack is moved according to an offset.
///
/// This offset is calculated by the size of the object, its memory-alignment and an offset to align the object in memory.
///
/// When the allocator is reset, a pointer, depending from which end was reset, to the top of the stack is moved to its *bottom* of the stack. Allocation will occur
/// from its *bottom* of the stack and will override previously allocated memory.
///
/// # Be careful
///
/// This allocator is **dropless**: memory is never *really* freed. You must guarantee that, when overriding memory, this memory was not used.
///
/// # Example
///
/// ```
/// #![feature(alloc)]
/// use maskerad_stack_allocator::DoubleEndedAllocator;
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
/// //This allocator is like the StackAllocator, but the operations can be applied from the
/// //bottom AND the top of the stack.
///
/// //Bear in mind that if you allocated 100 bytes, you can allocate up to 50 bytes from the bottom and up to 50 bytes from the top.
///
/// let allocator = DoubleEndedAllocator::with_capacity(100); //100 bytes
///
/// //get a pointer to the top of the stack
/// let top_of_stack = allocator.marker_top();
///
/// //We can allocate from both sides.
/// let a_monster = allocator.alloc_bottom(Monster::default());
/// let another_monster = allocator.alloc_top(Monster::default());
///
/// //We can get pointers to the current top of the stack, from both sides.
/// let current_ptr_bottom = allocator.marker_bottom();
/// let current_ptr_top = allocator.marker_top();
///
/// assert_ne!(top_of_stack, current_ptr_top);
/// assert_ne!(allocator.stack().ptr(), current_ptr_bottom);
///
/// //we can reset the pointers, from both sides.
/// allocator.reset_top();
/// allocator.reset_bottom();
///
/// let current_ptr_bottom = allocator.marker_bottom();
/// let current_ptr_top = allocator.marker_top();
///
/// assert_eq!(top_of_stack, current_ptr_top);
/// assert_eq!(allocator.stack().ptr(), current_ptr_bottom);
///
/// ```


pub struct DoubleEndedAllocator {
    stack: RawVec<u8>,
    //ptr to the stack's "top". Cell gives use interior mutability
    //(With Reset(&mut self) and alloc(&mut self), we could only allocate 1 time. After that...
    // error[E0499]: cannot borrow `alloc` as mutable more than once at a time
    // error[E0502]: cannot borrow `alloc` as immutable because it is also borrowed as mutable
    current_offset_top: Cell<*mut u8>,
    current_offset_bottom: Cell<*mut u8>,
}


impl DoubleEndedAllocator {
    /// Create a DoubleEndedAllocator with the given capacity (in bytes).
    /// # Example
    /// ```
    /// #![feature(alloc)]
    /// use maskerad_stack_allocator::DoubleEndedAllocator;
    ///
    /// let allocator = DoubleEndedAllocator::with_capacity(100);
    /// assert_eq!(allocator.stack().cap(), 100);
    /// ```
    pub fn with_capacity(capacity: usize) -> Self {

        //it is guaranteed that the offset will not cause overflow.
        unsafe {
            let stack: RawVec<u8> = RawVec::with_capacity(capacity);
            let current_offset_bottom = Cell::new(stack.ptr() as *mut u8);
            let current_offset_top = Cell::new(stack.ptr().offset(capacity as isize) as *mut u8);


            DoubleEndedAllocator {
                stack,
                current_offset_bottom,
                current_offset_top,
            }
        }

    }

    /// Return an immutable reference to the stack used by the allocator.
    pub fn stack(&self) -> &RawVec<u8> {
        &self.stack
    }


    /// Move the pointer from the current top of the stack to the bottom of the stack (pointer allocating from the bottom).
    /// # Example
    /// ```
    /// #![feature(alloc)]
    /// use maskerad_stack_allocator::DoubleEndedAllocator;
    ///
    /// let allocator = DoubleEndedAllocator::with_capacity(100);
    /// let an_i32 = allocator.alloc_bottom(25);
    /// let ptr_top_stack = allocator.marker_bottom();
    ///
    /// assert_ne!(allocator.stack().ptr(), ptr_top_stack);
    ///
    /// allocator.reset_bottom();
    /// let ptr_top_stack = allocator.marker_bottom();
    /// assert_eq!(allocator.stack().ptr(), ptr_top_stack);
    /// ```
    pub fn reset_bottom(&self) {
        self.current_offset_bottom.set(self.stack.ptr());
    }

    /// Move the pointer from the current top of the stack to the bottom of the stack (pointer allocating from the top).
    /// # Example
    /// ```
    /// #![feature(alloc)]
    /// use maskerad_stack_allocator::DoubleEndedAllocator;
    ///
    /// let allocator = DoubleEndedAllocator::with_capacity(100);
    ///
    /// //get a ptr to the top of the stack
    /// let top_stack = allocator.marker_top();
    ///
    /// let an_i32 = allocator.alloc_top(25);
    /// let ptr_top_stack = allocator.marker_top();
    ///
    /// assert_ne!(top_stack, ptr_top_stack);
    ///
    /// allocator.reset_top();
    /// let ptr_top_stack = allocator.marker_top();
    /// assert_eq!(top_stack, ptr_top_stack);
    /// ```
    pub fn reset_top(&self) {
        //it is guaranteed that the offset will not cause overflow.
        unsafe {
            let top_stack = self.stack().ptr().offset(self.stack().cap() as isize);
            self.current_offset_top.set(top_stack);
        }
    }

    /// Allocate data in the allocator's memory, from the current top of the stack (pointer allocating from the bottom).
    ///
    /// # Panics
    ///
    /// This function will panic if the current length of the allocator + the size of the allocated object
    /// exceed the allocator's capacity divided by 2.
    ///
    /// # Example
    ///
    /// ```
    /// use maskerad_stack_allocator::DoubleEndedAllocator;
    ///
    /// let allocator = DoubleEndedAllocator::with_capacity(100);
    ///
    /// let my_i32 = allocator.alloc_bottom(26);
    /// assert_eq!(my_i32, &mut 26);
    /// ```
    pub fn alloc_bottom<T>(&self, value: T) -> &mut T {
        let layout = Layout::new::<T>(); //is always a power of two.
        let offset = layout.align() + layout.size();

        //println!("\nalignment: {}-byte alignment", layout.align());
        //println!("size: {}", layout.size());
        //println!("Total amount of memory to allocate: {} bytes", offset);

        //Get the actual stack top. It will be the address returned.
        let old_stack_top = self.current_offset_bottom.get();
        //println!("address of the current stack top : {:?}", old_stack_top);

        //Determine the total amount of memory to allocate
        unsafe {
            //Get the ptr to the unaligned location
            let unaligned_ptr = old_stack_top.offset(offset as isize) as usize;
            //println!("unaligned location: {:?}", unaligned_ptr as *mut u8);

            //Now calculate the adjustment by masking off the lower bits of the address, to determine
            //how "misaligned" it is.
            let mask = layout.align() - 1;
            //println!("mask (alignment - 1): {:#X} ", mask);
            let misalignment = unaligned_ptr & mask;
            //println!("misalignment (unaligned ptr addr |bitwise AND| mask): {:#X}", misalignment);
            let adjustment = layout.align() - misalignment;
            //println!("adjustment (current alignment - misalignment): {:#X}", adjustment);

            let aligned_ptr = (unaligned_ptr + adjustment) as *mut u8;
            //println!("aligned ptr (unaligned ptr addr + adjustment): {:?}", aligned_ptr);

            assert!((self.stack.ptr().offset_to(aligned_ptr).unwrap() as usize) < self.stack.cap() / 2);

            //Now update the current_offset
            self.current_offset_bottom.set(aligned_ptr);

            //println!("Real amount of memory allocated: {}", offset + adjustment);

            //write the value in the memory location the old_stack_top is pointing.
            core::ptr::write::<T>(old_stack_top as *mut T, value);


            &mut *(old_stack_top as *mut T)
        }
    }


    /// Allocate data in the allocator's memory, from the current top of the stack (pointer allocating from the top).
    ///
    /// # Panics
    ///
    /// This function will panic if the current length of the allocator + the size of the allocated object
    /// exceed the allocator's capacity divided by 2.
    ///
    /// # Example
    ///
    /// ```
    /// use maskerad_stack_allocator::DoubleEndedAllocator;
    ///
    /// let allocator = DoubleEndedAllocator::with_capacity(100);
    ///
    /// let my_i32 = allocator.alloc_top(26);
    /// assert_eq!(my_i32, &mut 26);
    /// ```
    pub fn alloc_top<T>(&self, value: T) -> &mut T {
        let layout = Layout::new::<T>(); //is always a power of two.
        let offset = layout.align() + layout.size();

        println!("\nalignment: {}-byte alignment", layout.align());
        println!("size: {}", layout.size());
        println!("Total amount of memory to allocate: {} bytes", offset);

        //Get the actual stack top.
        let old_stack_top = self.current_offset_top.get();
        println!("address of the current stack top : {:?}", old_stack_top);

        //Determine the total amount of memory to allocate
        unsafe {
            //Get the ptr to the unaligned location
            let bottom_top_offset = self.stack().ptr().offset_to(old_stack_top).unwrap();


            let unaligned_ptr = self.stack().ptr().offset(bottom_top_offset - offset as isize) as usize;
            println!("unaligned location: {:?}", unaligned_ptr as *mut u8);

            //Now calculate the adjustment by masking off the lower bits of the address, to determine
            //how "misaligned" it is.
            let mask = layout.align() - 1;
            println!("mask (alignment - 1): {:#X} ", mask);
            let misalignment = unaligned_ptr & mask;
            println!("misalignment (unaligned ptr addr |bitwise AND| mask): {:#X}", misalignment);
            let adjustment = layout.align() - misalignment;
            println!("adjustment (current alignment - misalignment): {:#X}", adjustment);

            let aligned_ptr = (unaligned_ptr - adjustment) as *mut u8;
            println!("aligned ptr (unaligned ptr addr + adjustment): {:?}", aligned_ptr);

            assert!((self.stack.ptr().offset_to(aligned_ptr).unwrap() as usize) > (self.stack.cap() / 2));

            //Now update the current_offset
            self.current_offset_top.set(aligned_ptr);

            println!("Real amount of memory allocated: {}", offset + adjustment);

            //write the value in the memory location the current_offset_top is pointing.
            core::ptr::write::<T>(aligned_ptr as *mut T, value);


            &mut *(aligned_ptr as *mut T)
        }
    }

    ///Return a pointer to the current top of the stack (pointer allocating from the top).
    pub fn marker_top(&self) -> *mut u8 {
        self.current_offset_top.get()
    }

    ///Return a pointer to the current top of the stack (pointer allocating from the bottom).
    /// # Example
    /// ```
    /// #![feature(alloc)]
    /// use maskerad_stack_allocator::DoubleEndedAllocator;
    ///
    /// let allocator = DoubleEndedAllocator::with_capacity(100);
    /// let ptr_bottom_stack = allocator.marker_bottom();
    ///
    /// // allocator.stack().ptr() return a pointer to the start of the allocation (the bottom of the stack).
    /// // Nothing has been allocated on the stack, the top of the stack is at the bottom.
    /// assert_eq!(allocator.stack().ptr(), ptr_bottom_stack);
    /// ```
    pub fn marker_bottom(&self) -> *mut u8 {
        self.current_offset_bottom.get()
    }

    /// Move the pointer from the current top of the stack to a marker (pointer allocating from the top).
    /// # Example
    /// ```
    /// #![feature(alloc)]
    /// use maskerad_stack_allocator::DoubleEndedAllocator;
    ///
    /// let allocator = DoubleEndedAllocator::with_capacity(100);
    /// let ptr_top = allocator.marker_top(); // bottom of the stack.
    ///
    /// let an_i32 = allocator.alloc_top(25);
    /// // top of the stack after one allocation.
    /// let ptr_one_alloc = allocator.marker_top();
    /// assert_ne!(ptr_top, ptr_one_alloc);
    ///
    /// // The current top of the stack is now at the bottom.
    /// allocator.reset_to_marker_top(ptr_top);
    /// let new_ptr_top = allocator.marker_top();
    /// assert_eq!(ptr_top, new_ptr_top);
    /// ```
    pub fn reset_to_marker_top(&self, marker: *mut u8) {
        self.current_offset_top.set(marker);
    }

    /// Move the pointer from the current top of the stack to a marker (pointer allocating from the bottom).
    /// # Example
    /// ```
    /// #![feature(alloc)]
    /// use maskerad_stack_allocator::DoubleEndedAllocator;
    ///
    /// let allocator = DoubleEndedAllocator::with_capacity(100);
    /// let ptr_bottom = allocator.marker_bottom(); // bottom of the stack.
    /// assert_eq!(allocator.stack().ptr(), ptr_bottom);
    ///
    /// let an_i32 = allocator.alloc_bottom(25);
    /// // top of the stack after one allocation.
    /// let ptr_one_alloc = allocator.marker_bottom();
    /// assert_ne!(allocator.stack().ptr(), ptr_one_alloc);
    ///
    /// // The current top of the stack is now at the bottom.
    /// allocator.reset_to_marker_bottom(ptr_bottom);
    /// let ptr_bottom = allocator.marker_bottom();
    /// assert_eq!(allocator.stack().ptr(), ptr_bottom);
    /// ```
    pub fn reset_to_marker_bottom(&self, marker: *mut u8) {
        self.current_offset_bottom.set(marker);
    }
}


#[cfg(test)]
mod stack_allocator_test {
    use super::*;
    extern crate time;

    #[test]
    fn creation_with_right_capacity() {
        //create a StackAllocator with the specified size.
        let alloc = DoubleEndedAllocator::with_capacity(200);
        let cap_used = alloc.stack.ptr().offset_to(alloc.current_offset_bottom.get()).unwrap() as usize;
        let cap_remaining = (alloc.stack.cap() - cap_used) as isize;
        assert_eq!(cap_used, 0);
        assert_eq!(cap_remaining, 200);
    }

    #[test]
    fn allocation_test() {
        //Check the allocation with u8, u32 an u64, to verify the alignment behavior.

        //We allocate 200 bytes of memory.
        let alloc = DoubleEndedAllocator::with_capacity(200);

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
        let _test_1_byte = alloc.alloc_bottom::<u8>(2);
        let cap_used = alloc.stack.ptr().offset_to(alloc.current_offset_bottom.get()).unwrap() as usize;
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
        let _test_4_bytes = alloc.alloc_bottom::<u32>(60000);
        let cap_used = alloc.stack.ptr().offset_to(alloc.current_offset_bottom.get()).unwrap() as usize;
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
        let _test_8_bytes = alloc.alloc_bottom::<u64>(100000);
        let cap_used = alloc.stack.ptr().offset_to(alloc.current_offset_bottom.get()).unwrap() as usize;
        let cap_remaining = (alloc.stack.cap() - cap_used) as isize;
        assert_eq!(cap_used, 32); // 3 + 9 + 20
        assert_eq!(cap_remaining, 168); //200 - 3 - 9 - 20
    }

    #[test]
    fn test_alloc_from_top() {
        //Check the allocation with u8, u32 an u64, to verify the alignment behavior.

        //We allocate 200 bytes of memory.
        let alloc = DoubleEndedAllocator::with_capacity(200);
        let top_stack = alloc.marker_top();

        let _test_1_byte = alloc.alloc_top::<u8>(2);

        //We go backward, we would get -3.
        let cap_used = (top_stack.offset_to(alloc.current_offset_top.get()).unwrap() * -1) as usize;
        let cap_remaining = (alloc.stack.cap() - cap_used) as isize;
        assert_eq!(cap_used, 3); //3
        assert_eq!(cap_remaining, 197); //200 - 3
    }

    #[test]
    fn test_reset() {
        //Test if there's any problem with memory overwriting.
        let alloc = DoubleEndedAllocator::with_capacity(200);
        let test_1_byte = alloc.alloc_bottom::<u8>(2);
        assert_eq!(test_1_byte, &mut 2);
        alloc.reset_bottom();
        let test_1_byte = alloc.alloc_bottom::<u8>(5);
        assert_eq!(test_1_byte, &mut 5);
    }
}

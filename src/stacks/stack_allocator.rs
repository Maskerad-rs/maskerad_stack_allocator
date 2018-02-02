// Copyright 2017-2018 Maskerad Developers
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

use core::ptr;
use std::cell::{BorrowError, Ref, RefCell};
use std::mem;

use allocation_error::{AllocationError, AllocationResult};
use utils;
use memory_chunk::MemoryChunk;
use std::intrinsics::needs_drop;

/// A stack-based allocator.
///
/// It manages two `MemoryChunk`s to:
///
/// - Allocate bytes in a stack-like fashion.
///
/// - Store different types of objects in the same storage.
///
/// - Drop the content of the MemoryChunk when needed.
///
/// One `MemoryChunk` is used for data implementing the `Drop` trait, the other is used for data implementing
/// the `Copy` trait. A structure implementing the `Copy` trait cannot implement the `Drop` trait. In order to
/// drop data implementing the `Drop` trait, we need to store its vtable next to it in memory.
///
/// # Instantiation
/// When instantiated, the memory chunk pre-allocate the given number of bytes for each `MemoryChunk`.
///
/// # Allocation
/// When an object is allocated in memory, the allocator:
///
/// - Check if the allocated object needs to be dropped, and choose which `MemoryChunk` to use according to this information,
///
/// - Asks a pointer to a memory address to the corresponding memory chunk,
///
/// - Place the object in this memory address,
///
/// - Update the first unused memory address of the memory chunk according to an offset,
///
/// - And return an immutable/mutable reference to the object which has been placed in the memory chunk.
///
/// This offset is calculated by the size of the object, the size of a `TypeDescription` structure (if the object implement the `Drop` trait),
/// its memory-alignment and an offset to align the object in memory.
///
/// # Roll-back
///
/// This structure allows you to get a **marker**, the index to the first unused memory address of a memory chunk. A stack allocator can *reset* a memory chunk to a marker,
/// or reset a memory chunk entirely.
///
/// When a memory chunk is reset to a marker, it will:
///
/// - Drop all the content lying between the marker and the first unused memory address, if it holds data implementing the `Drop` trait,
///
/// - Set the first unused memory address to the marker.
///
///
/// When a memory chunk is reset completely, it will:
///
/// - Drop everything, if ti holds data implementing the `Drop` trait,
///
/// - Set the first unused memory address to the bottom of its stack.
///
/// # Example
///
/// ```rust
/// use maskerad_memory_allocators::StackAllocator;
/// # use std::error::Error;
/// # fn try_main() -> Result<(), Box<Error>> {
/// //100 bytes for data implementing Drop, 100 bytes for data implementing Copy.
/// let single_frame_allocator = StackAllocator::with_capacity(100, 100);
/// let mut closed = false;
///
/// while !closed {
///     // The allocator is cleared every frame.
///     // Everything is dropped.
///     single_frame_allocator.reset();
///
///     //...
///
///     //allocate from the single frame allocator.
///     //Be sure to use the data during this frame only!
///     let my_vec: &Vec<u8> = single_frame_allocator.alloc(|| {
///         Vec::with_capacity(10)
///     })?;
///
///     assert!(my_vec.is_empty());
///     closed = true;
/// }
/// # Ok(())
/// # }
/// # fn main() {
/// #   try_main().unwrap();
/// # }
/// ```
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct StackAllocator {
    storage: RefCell<MemoryChunk>,
    storage_copy: RefCell<MemoryChunk>,
}

impl StackAllocator {
    /// Creates a StackAllocator with the given capacities, in bytes.
    ///
    /// The first capacity is for the `MemoryChunk` holding data implementing the `Drop` trait,
    /// the second is for the `MemoryChunk` holding data implementing the `Copy` trait.
    /// # Example
    ///
    /// ```rust
    /// use maskerad_memory_allocators::StackAllocator;
    ///
    /// let allocator = StackAllocator::with_capacity(100, 50);
    /// assert_eq!(allocator.capacity(), 100);
    /// assert_eq!(allocator.capacity_copy(), 50);
    /// ```
    pub fn with_capacity(capacity: usize, capacity_copy: usize) -> Self {
        StackAllocator {
            storage: RefCell::new(MemoryChunk::new(capacity)),
            storage_copy: RefCell::new(MemoryChunk::new(capacity_copy)),
        }
    }

    /// Allocates data in the allocator's memory, returning a mutable reference to the allocated data.
    ///
    /// If the allocated data implements `Drop`, it will be placed in the `MemoryChunk` storing data implementing the `Drop` trait.
    /// Otherwise, it will be placed in the other `MemoryChunk`.
    ///
    /// # Error
    /// This function will return an error if the allocation exceeds the maximum storage capacity of the allocator.
    ///
    /// # Example
    /// ```
    /// use maskerad_memory_allocators::StackAllocator;
    /// # use std::error::Error;
    /// # fn try_main() -> Result<(), Box<Error>> {
    /// let allocator = StackAllocator::with_capacity(100, 100);
    ///
    /// let my_i32 = allocator.alloc_mut(|| {
    ///     26 as i32
    /// })?;
    ///
    /// assert_eq!(my_i32, &mut 26);
    /// # Ok(())
    /// # }
    /// # fn main() {
    /// #   try_main().unwrap();
    /// # }
    /// ```
    #[inline]
    pub fn alloc_mut<T, F>(&self, op: F) -> AllocationResult<&mut T>
    where
        F: FnOnce() -> T,
    {
        unsafe {
            if needs_drop::<T>() {
                self.alloc_non_copy_mut(op)
            } else {
                self.alloc_copy_mut(op)
            }
        }
    }

    /// The function actually writing data in the memory chunk
    fn alloc_non_copy_mut<T, F>(&self, op: F) -> AllocationResult<&mut T>
    where
        F: FnOnce() -> T,
    {
        unsafe {
            //Get the type description of the type T (get its vtable).
            let type_description = utils::get_type_description::<T>();

            //Ask the memory chunk to give us raw pointers to memory locations for our type description and object
            let (type_description_ptr, ptr) =
                self.alloc_non_copy_inner(mem::size_of::<T>(), mem::align_of::<T>())?;

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
            Ok(&mut *ptr)
        }
    }

    //Functions for the copyable part of the stack allocator.
    fn alloc_copy_mut<T, F>(&self, op: F) -> AllocationResult<&mut T>
    where
        F: FnOnce() -> T,
    {
        unsafe {
            //Get an aligned raw pointer to place the object in it.
            let ptr = self.alloc_copy_inner(mem::size_of::<T>(), mem::align_of::<T>())?;

            //cast this raw pointer to the type of the object.
            let ptr = ptr as *mut T;

            //Write the data in the memory location.
            ptr::write(&mut (*ptr), op());

            //return a mutable reference to this pointer.
            Ok(&mut *ptr)
        }
    }

    /// Allocates data in the allocator's memory, returning an immutable reference to the allocated data.
    ///
    /// If the allocated data implements `Drop`, it will be placed in the `MemoryChunk` storing data implementing the `Drop` trait.
    /// Otherwise, it will be placed in the other `MemoryChunk`.
    ///
    /// # Error
    /// This function will return an error if the allocation exceeds the maximum storage capacity of the allocator.
    ///
    /// # Example
    ///
    /// ```rust
    /// use maskerad_memory_allocators::StackAllocator;
    /// # use std::error::Error;
    /// # fn try_main() -> Result<(), Box<Error>> {
    /// let allocator = StackAllocator::with_capacity(100, 100);
    ///
    /// let my_i32 = allocator.alloc(|| {
    ///     26 as i32
    /// })?;
    ///
    /// assert_eq!(my_i32, &26);
    /// # Ok(())
    /// # }
    /// # fn main() {
    /// #   try_main().unwrap();
    /// # }
    /// ```
    #[inline]
    pub fn alloc<T, F>(&self, op: F) -> AllocationResult<&T>
    where
        F: FnOnce() -> T,
    {
        unsafe {
            if needs_drop::<T>() {
                self.alloc_non_copy(op)
            } else {
                self.alloc_copy(op)
            }
        }
    }

    //Functions for the non-copyable part of the arena.

    /// The function actually writing data in the memory chunk
    #[inline]
    fn alloc_non_copy<T, F>(&self, op: F) -> AllocationResult<&T>
    where
        F: FnOnce() -> T,
    {
        unsafe {
            //Get the type description of the type T (get its vtable).
            let type_description = utils::get_type_description::<T>();

            //Ask the memory chunk to give us raw pointers to memory locations for our type description and object
            let (type_description_ptr, ptr) =
                self.alloc_non_copy_inner(mem::size_of::<T>(), mem::align_of::<T>())?;

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
            Ok(&*ptr)
        }
    }

    fn alloc_copy<T, F>(&self, op: F) -> AllocationResult<&T>
    where
        F: FnOnce() -> T,
    {
        unsafe {
            //Get an aligned raw pointer to place the object in it.
            let ptr = self.alloc_copy_inner(mem::size_of::<T>(), mem::align_of::<T>())?;

            //cast this raw pointer to the type of the object.
            let ptr = ptr as *mut T;

            //Write the data in the memory location.
            ptr::write(&mut (*ptr), op());

            //return a mutable reference to this pointer.
            Ok(&*ptr)
        }
    }

    /// The function asking the memory chunk to give us raw pointers to memory locations and update
    /// the current top of the stack.
    #[inline]
    fn alloc_non_copy_inner(
        &self,
        n_bytes: usize,
        align: usize,
    ) -> AllocationResult<(*const u8, *const u8)> {
        let non_copy_storage = self.storage.borrow();

        //Get the index of the first unused byte in the memory chunk.
        let fill = non_copy_storage.fill();

        //Get the index of where we'll write the type description data
        //(the first unused byte in the memory chunk).
        let type_description_start = fill;

        // Get the index of where the object should reside (unaligned location actually).
        let after_type_description = fill + mem::size_of::<*const utils::TypeDescription>();

        //With the index to the unaligned memory address, determine the index to
        //the aligned memory address where the object will reside,
        //according to its memory alignment.
        let start = utils::round_up(after_type_description, align);

        //Determine the index of the next aligned memory address for a type description, according to the size of the object
        //and the memory alignment of a type description.
        let end = utils::round_up(
            start + n_bytes,
            mem::align_of::<*const utils::TypeDescription>(),
        );

        //If the allocator becomes oom after this possible allocation, abort the program.
        if end >= non_copy_storage.capacity() {
            return Err(AllocationError::OutOfMemoryError(format!(
                "The stack allocator is out of memory !"
            )));
        }

        //Update the current top of the stack.
        //The first unused memory address is at index 'end',
        //where the next type description would be written
        //if an allocation was asked.
        non_copy_storage.set_fill(end);

        unsafe {
            // Get a raw pointer to the start of our MemoryChunk's RawVec
            let start_storage = non_copy_storage.as_ptr();

            Ok((
                //From this raw pointer, get the correct raw pointers with
                        //the indices we calculated earlier.

                        //The raw pointer to the type description of the object.
                start_storage.offset(type_description_start as isize),
                //The raw pointer to the object.
                start_storage.offset(start as isize),
            ))
        }
    }

    fn alloc_copy_inner(&self, n_bytes: usize, align: usize) -> AllocationResult<*const u8> {
        //borrow mutably the memory chunk used by the allocator.
        let copy_storage = self.storage_copy.borrow();

        //Get the index of the first unused memory address in the memory chunk.
        let fill = copy_storage.fill();

        //Get the index of the aligned memory address, which will be returned.
        let start = utils::round_up(fill, align);

        //Get the index of the future first unused memory address, according to the size of the object.
        let end = start + n_bytes;

        //We don't grow the capacity, or create another chunk.
        if end >= copy_storage.capacity() {
            return Err(AllocationError::OutOfMemoryError(format!(
                "The copy stack allocator is out of memory !"
            )));
        }

        //Set the first unused memory address of the memory chunk to the index calculated earlier.
        copy_storage.set_fill(end);

        unsafe {
            //Return the raw pointer to the aligned memory location, which will be used to place
            //the object in the allocator.
            Ok(copy_storage.as_ptr().offset(start as isize))
        }
    }

    /// Returns the index of the first unused memory address of the `MemoryChunk` storing data implementing
    /// the `Drop` trait.
    ///
    /// # Example
    ///
    /// ```rust
    /// use maskerad_memory_allocators::StackAllocator;
    ///
    /// let allocator = StackAllocator::with_capacity(100, 100); //100 bytes
    ///
    /// //Get the raw pointer to the bottom of the allocator's memory chunk.
    /// let start_allocator = allocator.storage_as_ptr();
    ///
    /// //Get the index of the first unused memory address.
    /// let index_current_top = allocator.marker();
    ///
    /// //Calling offset() on a raw pointer is an unsafe operation.
    /// unsafe {
    ///     //Get the raw pointer, with the index.
    ///     let current_top = start_allocator.offset(index_current_top as isize);
    ///
    ///     //Nothing has been allocated in the allocator,
    ///     //the top of the stack is the bottom of the allocator's memory chunk.
    ///     assert_eq!(current_top, start_allocator);
    /// }
    ///
    /// ```
    pub fn marker(&self) -> usize {
        self.storage.borrow().fill()
    }

    /// Returns the index of the first unused memory address of the `MemoryChunk` storing data implementing
    /// the `Copy` trait.
    ///
    /// # Example
    ///
    /// ```rust
    /// use maskerad_memory_allocators::StackAllocator;
    ///
    /// let allocator = StackAllocator::with_capacity(100, 100); //100 bytes
    ///
    /// //Get the raw pointer to the bottom of the allocator's memory chunk.
    /// let start_allocator = allocator.storage_copy_as_ptr();
    ///
    /// //Get the index of the first unused memory address.
    /// let index_current_top = allocator.marker_copy();
    ///
    /// //Calling offset() on a raw pointer is an unsafe operation.
    /// unsafe {
    ///     //Get the raw pointer, with the index.
    ///     let current_top = start_allocator.offset(index_current_top as isize);
    ///
    ///     //Nothing has been allocated in the allocator,
    ///     //the top of the stack is the bottom of the allocator's memory chunk.
    ///     assert_eq!(current_top, start_allocator);
    /// }
    ///
    /// ```
    pub fn marker_copy(&self) -> usize {
        self.storage_copy.borrow().fill()
    }

    /// Reset the `MemoryChunk` storing data implementing the `Drop` trait, dropping all the content residing inside it.
    ///
    /// # Example
    ///
    /// ```rust
    /// use maskerad_memory_allocators::StackAllocator;
    /// # use std::error::Error;
    /// # fn try_main() -> Result<(), Box<Error>> {
    /// let allocator = StackAllocator::with_capacity(100, 100); // 100 bytes.
    ///
    /// //When nothing has been allocated, the first unused memory address is at index 0.
    /// assert_eq!(allocator.marker(), 0);
    ///
    /// let my_vec: &Vec<u8> = allocator.alloc(|| {
    ///     Vec::with_capacity(10)
    /// })?;
    /// assert_ne!(allocator.marker(), 0);
    ///
    /// allocator.reset();
    ///
    /// //The MemoryChunk storing data implementing the `Drop` trait has been totally reset, and all its content has been dropped.
    /// assert_eq!(allocator.marker(), 0);
    /// # Ok(())
    /// # }
    /// # fn main() {
    /// #   try_main().unwrap();
    /// # }
    /// ```
    pub fn reset(&self) {
        unsafe {
            self.storage.borrow().destroy();
            self.storage.borrow().set_fill(0);
        }
    }

    /// Reset the `MemoryChunk` storing data implementing the `Drop` trait, dropping all the content residing inside it.
    ///
    /// # Example
    ///
    /// ```rust
    /// use maskerad_memory_allocators::StackAllocator;
    /// # use std::error::Error;
    /// # fn try_main() -> Result<(), Box<Error>> {
    /// let allocator = StackAllocator::with_capacity(100, 100); // 100 bytes.
    ///
    /// //When nothing has been allocated, the first unused memory address is at index 0.
    /// assert_eq!(allocator.marker_copy(), 0);
    ///
    /// let my_i32 = allocator.alloc(|| {
    ///     8 as i32
    /// })?;
    /// assert_ne!(allocator.marker_copy(), 0);
    ///
    /// allocator.reset_copy();
    ///
    /// //The MemoryChunk storing data implementing the `Copy` has been totally reset.
    /// assert_eq!(allocator.marker_copy(), 0);
    /// # Ok(())
    /// # }
    /// # fn main() {
    /// #   try_main().unwrap();
    /// # }
    /// ```
    pub fn reset_copy(&self) {
        self.storage_copy.borrow().set_fill(0);
    }

    /// Reset partially the `MemoryChunk` storing data implementing the `Drop` trait, dropping all the content residing between the marker and
    /// the first unused memory address of the `MemoryChunk`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use maskerad_memory_allocators::StackAllocator;
    /// # use std::error::Error;
    /// # fn try_main() -> Result<(), Box<Error>> {
    /// // 100 bytes for data implementing Drop, 100 bytes for Data implementing Copy.
    /// let allocator = StackAllocator::with_capacity(100, 100);
    ///
    /// //When nothing has been allocated, the first unused memory address is at index 0.
    /// assert_eq!(allocator.marker(), 0);
    ///
    /// let my_vec: &Vec<u8> = allocator.alloc(|| {
    ///     Vec::with_capacity(10)
    /// })?;
    ///
    /// //After the allocation, get the index of the first unused memory address in the allocator.
    /// let index_current_top = allocator.marker();
    /// assert_ne!(index_current_top, 0);
    ///
    /// let my_vec_2: &Vec<u8> = allocator.alloc(|| {
    ///     Vec::with_capacity(10)
    /// })?;
    ///
    /// assert_ne!(allocator.marker(), index_current_top);
    ///
    /// allocator.reset_to_marker(index_current_top);
    ///
    /// //The memorychunk storing data implementing the Drop trait has been partially reset, and all the content lying between the marker and
    /// //the first unused memory address has been dropped.
    ///
    /// assert_eq!(allocator.marker(), index_current_top);
    /// # Ok(())
    /// # }
    /// # fn main() {
    /// #   try_main().unwrap();
    /// # }
    /// ```
    pub fn reset_to_marker(&self, marker: usize) {
        unsafe {
            self.storage.borrow().destroy_to_marker(marker);
            self.storage.borrow().set_fill(marker);
        }
    }

    /// Reset partially the `MemoryChunk` storing data implementing the `Copy` trait.
    ///
    /// # Example
    ///
    /// ```rust
    /// use maskerad_memory_allocators::StackAllocator;
    /// # use std::error::Error;
    /// # fn try_main() -> Result<(), Box<Error>> {
    /// // 100 bytes for data implementing Drop, 100 bytes for Data implementing Copy.
    /// let allocator = StackAllocator::with_capacity(100, 100);
    ///
    /// //When nothing has been allocated, the first unused memory address is at index 0.
    /// assert_eq!(allocator.marker_copy(), 0);
    ///
    /// let my_i32 = allocator.alloc(|| {
    ///     8 as i32
    /// })?;
    ///
    /// //After the allocation, get the index of the first unused memory address in the allocator.
    /// let index_current_top = allocator.marker_copy();
    /// assert_ne!(index_current_top, 0);
    ///
    /// let my_i32_2 = allocator.alloc(|| {
    ///     9 as i32
    /// })?;
    ///
    /// assert_ne!(allocator.marker_copy(), index_current_top);
    ///
    /// allocator.reset_to_marker_copy(index_current_top);
    ///
    /// //The memorychunk storing data implementing the Copy trait has been partially reset.
    ///
    /// assert_eq!(allocator.marker_copy(), index_current_top);
    /// # Ok(())
    /// # }
    /// # fn main() {
    /// #   try_main().unwrap();
    /// # }
    /// ```
    pub fn reset_to_marker_copy(&self, marker: usize) {
        self.storage_copy.borrow().set_fill(marker);
    }

    /// Returns the maximum capacity the `MemoryChunk` storing data implementing the `Drop` trait can hold.
    pub fn capacity(&self) -> usize {
        self.storage.borrow().capacity()
    }

    /// Returns the maximum capacity the `MemoryChunk` storing data implementing the `Copy` trait can hold.
    pub fn capacity_copy(&self) -> usize {
        self.storage_copy.borrow().capacity()
    }

    /// Returns a raw pointer to the start of the memory storage used by the `MemoryChunk` storing data implementing the `Drop` trait.
    pub fn storage_as_ptr(&self) -> *const u8 {
        self.storage.borrow().as_ptr()
    }

    /// Returns a raw pointer to the start of the memory storage used by the `MemoryChunk` storing data implementing the `Copy` trait.
    pub fn storage_copy_as_ptr(&self) -> *const u8 {
        self.storage_copy.borrow().as_ptr()
    }

    fn destroy_stack(&self) -> Result<(), BorrowError> {
        unsafe {
            self.storage.try_borrow()?.destroy();
        }
        Ok(())
    }
}

impl Drop for StackAllocator {
    fn drop(&mut self) {
        self.destroy_stack().unwrap();
    }
}

#[cfg(test)]
mod stack_allocator_test {
    use super::*;

    //size : 4 bytes + 4 bytes alignment + 4 bytes + 4 bytes alignment + alignment-offset stuff -> ~16-20 bytes.
    struct Monster {
        _hp: u32,
    }

    impl Monster {
        pub fn new(hp: u32) -> Self {
            Monster { _hp: hp }
        }
    }

    impl Default for Monster {
        fn default() -> Self {
            Monster { _hp: 1 }
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
            let alloc = StackAllocator::with_capacity(200, 200);
            let start_chunk = alloc.storage_as_ptr();
            let first_unused_mem_addr = start_chunk.offset(alloc.marker() as isize);

            assert_eq!(start_chunk, first_unused_mem_addr);
        }
    }

    #[test]
    fn allocation_test() {
        //We allocate 200 bytes of memory.
        let alloc = StackAllocator::with_capacity(200, 200);

        let _my_monster = alloc.alloc(|| Monster::new(1)).unwrap();

        unsafe {
            let start_alloc = alloc.storage_as_ptr();
            let top_stack_index = alloc.marker();
            let top_stack = start_alloc.offset(top_stack_index as isize);
            assert_ne!(start_alloc, top_stack);
        }
    }

    //Use 'cargo test -- --nocapture' to see the monsters' println!s
    #[test]
    fn test_reset() {
        let alloc = StackAllocator::with_capacity(200, 200);
        let _my_monster = alloc.alloc(|| Monster::new(1)).unwrap();

        let top_stack_index = alloc.marker();
        let start_alloc = alloc.storage_as_ptr();
        let mut current_top_stack_index = alloc.marker();

        unsafe {
            let top_stack = start_alloc.offset(top_stack_index as isize);
            let current_top_stack = start_alloc.offset(current_top_stack_index as isize);
            assert_eq!(current_top_stack, top_stack);
        }

        let _another_monster = alloc.alloc(|| Monster::default()).unwrap();

        current_top_stack_index = alloc.marker();

        unsafe {
            let top_stack = start_alloc.offset(top_stack_index as isize);
            let current_top_stack = start_alloc.offset(current_top_stack_index as isize);
            assert_ne!(current_top_stack, top_stack);
        }

        alloc.reset_to_marker(top_stack_index);

        //another_monster prints "i'm dying". The drop function is called !

        current_top_stack_index = alloc.marker();
        unsafe {
            let top_stack = start_alloc.offset(top_stack_index as isize);
            let current_top_stack = start_alloc.offset(current_top_stack_index as isize);
            assert_eq!(current_top_stack, top_stack);
        }

        alloc.reset();

        //my_monster prints "i'm dying". The drop function is called !

        current_top_stack_index = alloc.marker();
        unsafe {
            let top_stack = start_alloc.offset(top_stack_index as isize);
            let current_top_stack = start_alloc.offset(current_top_stack_index as isize);
            assert_ne!(current_top_stack, top_stack);
            assert_eq!(current_top_stack, start_alloc);
        }
    }
}

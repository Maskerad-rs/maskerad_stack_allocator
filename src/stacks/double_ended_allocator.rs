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

use allocation_error::AllocationResult;
use stacks::stack_allocator::StackAllocator;


/// A double-ended allocator.
///
/// It manages two `StackAllocator`s.
///
/// # Purpose
/// Suppose you want to load data `A`, and this data need the temporary data `B`. You need to load `B` before `A`
/// in order to create it.
///
/// After `A` is loaded, `B` is no longer needed. However, since this allocator is a stack, you need to free `A` before freeing `B`.
///
/// That's why this allocator has two memory chunks, one for temporary data, one for the resident data who need temporary data to be created.
///
/// # Example
///
/// ```
/// use maskerad_memory_allocators::DoubleEndedStackAllocator;
///
/// //50 bytes for the MemoryChunks storing data implementing the Drop trait, 50 for the others.
/// let double_ended_allocator = DoubleEndedStackAllocator::with_capacity(100, 100);
///
/// //Markers to the bottom of the stacks.
/// let top_resident = double_ended_allocator.marker_resident();
/// let top_temp = double_ended_allocator.marker_temp();
///
/// let my_vec: &Vec<u8> = double_ended_allocator.alloc_temp(|| {
///     Vec::with_capacity(10)
/// }).unwrap();
///
/// let my_vec_2: &Vec<u8> = double_ended_allocator.alloc_resident(|| {
///     Vec::with_capacity(10)
/// }).unwrap();
///
/// double_ended_allocator.reset_temp();
///
/// assert_eq!(top_temp, double_ended_allocator.marker_temp());
/// assert_ne!(top_resident, double_ended_allocator.marker_resident());
///
/// ```

pub struct DoubleEndedStackAllocator {
    stack_resident: StackAllocator,
    stack_temp: StackAllocator,
}


impl DoubleEndedStackAllocator {
    /// Creates a DoubleEndedStackAllocator with the given capacity, in bytes.
    /// # Example
    /// ```
    /// #![feature(alloc)]
    /// use maskerad_memory_allocators::DoubleEndedStackAllocator;
    ///
    /// let allocator = DoubleEndedStackAllocator::with_capacity(100, 50);
    /// assert_eq!(allocator.stack_temp().storage().capacity(), 50);
    /// assert_eq!(allocator.stack_temp().storage_copy().capacity(), 25);
    /// assert_eq!(allocator.stack_resident().storage().capacity(), 50);
    /// assert_eq!(allocator.stack_resident().storage_copy().capacity(), 25);
    /// ```
    pub fn with_capacity(capacity: usize, capacity_copy: usize) -> Self {
        DoubleEndedStackAllocator {
            stack_resident: StackAllocator::with_capacity(capacity / 2, capacity_copy / 2),
            stack_temp: StackAllocator::with_capacity(capacity / 2, capacity_copy / 2),
        }
    }

    /// Returns an immutable reference to the `StackAllocator` used for resident allocation.
    pub fn stack_resident(&self) -> &StackAllocator {
        &self.stack_resident
    }

    /// Returns an immutable reference to the `StackAllocator` used for temporary allocation.
    pub fn stack_temp(&self) -> &StackAllocator {
        &self.stack_temp
    }

    /// Allocates data in the `StackAllocator` used for resident memory, returning a mutable reference to the allocated data.
    ///
    /// If the allocated data implements `Drop`, it will be placed in the `MemoryChunk` storing data implementing the `Drop` trait.
    /// Otherwise, it will be placed in the other `MemoryChunk`.
    ///
    /// # Panics
    /// This function will panic if the allocation exceeds the maximum storage capacity of the allocator.
    ///
    /// # Example
    /// ```
    /// use maskerad_memory_allocators::DoubleEndedStackAllocator;
    ///
    ///
    /// let allocator = DoubleEndedStackAllocator::with_capacity(100, 100);
    ///
    /// let my_vec: &mut Vec<u8> = allocator.alloc_mut_resident(|| {
    ///     Vec::with_capacity(10)
    /// }).unwrap();
    ///
    /// my_vec.push(1);
    ///
    /// assert!(!my_vec.is_empty());
    /// ```
    #[inline]
    pub fn alloc_mut_resident<T, F>(&self, op: F) -> AllocationResult<&mut T>
        where F: FnOnce() -> T
    {
        self.stack_resident().alloc_mut(op)
    }


    /// Allocates data in the `StackAllocator` used for resident memory, returning an immutable reference to the allocated data.
    ///
    /// If the allocated data implements `Drop`, it will be placed in the `MemoryChunk` storing data implementing the `Drop` trait.
    /// Otherwise, it will be placed in the other `MemoryChunk`.
    ///
    /// # Panics
    /// This function will panic if the allocation exceeds the maximum storage capacity of the allocator.
    ///
    /// # Example
    /// ```
    /// use maskerad_memory_allocators::DoubleEndedStackAllocator;
    ///
    /// let allocator = DoubleEndedStackAllocator::with_capacity(100, 100);
    ///
    /// let my_vec: &Vec<u8> = allocator.alloc_resident(|| {
    ///     Vec::with_capacity(10)
    /// }).unwrap();
    ///
    /// assert!(my_vec.is_empty());
    /// ```
    #[inline]
    pub fn alloc_resident<T, F>(&self, op: F) -> AllocationResult<&T>
        where F: FnOnce() -> T
    {
        self.stack_resident().alloc(op)
    }

    /// Allocates data in the `StackAllocator` used for temporary memory, returning a mutable reference to the allocated data.
    ///
    /// If the allocated data implements `Drop`, it will be placed in the `MemoryChunk` storing data implementing the `Drop` trait.
    /// Otherwise, it will be placed in the other `MemoryChunk`.
    ///
    /// # Panics
    /// This function will panic if the allocation exceeds the maximum storage capacity of the allocator.
    ///
    /// # Example
    /// ```
    /// use maskerad_memory_allocators::DoubleEndedStackAllocator;
    ///
    ///
    /// let allocator = DoubleEndedStackAllocator::with_capacity(100, 100);
    ///
    /// let my_vec: &mut Vec<u8> = allocator.alloc_mut_temp(|| {
    ///     Vec::with_capacity(10)
    /// }).unwrap();
    ///
    /// my_vec.push(1);
    ///
    /// assert!(!my_vec.is_empty());
    /// ```
    pub fn alloc_mut_temp<T, F>(&self, op: F) -> AllocationResult<&mut T>
        where F: FnOnce() -> T
    {
        self.stack_temp().alloc_mut(op)
    }

    /// Allocates data in the `StackAllocator` used for temporary memory, returning an immutable reference to the allocated data.
    ///
    /// If the allocated data implements `Drop`, it will be placed in the `MemoryChunk` storing data implementing the `Drop` trait.
    /// Otherwise, it will be placed in the other `MemoryChunk`.
    ///
    /// # Panics
    /// This function will panic if the allocation exceeds the maximum storage capacity of the allocator.
    ///
    /// # Example
    /// ```
    /// use maskerad_memory_allocators::DoubleEndedStackAllocator;
    ///
    /// let allocator = DoubleEndedStackAllocator::with_capacity(100, 100);
    ///
    /// let my_vec: &Vec<u8> = allocator.alloc_temp(|| {
    ///     Vec::with_capacity(10)
    /// }).unwrap();
    ///
    /// assert!(my_vec.is_empty());
    /// ```
    pub fn alloc_temp<T, F>(&self, op: F) -> AllocationResult<&T>
        where F: FnOnce() -> T
    {
        self.stack_temp().alloc(op)
    }

    /// Returns the index of the first unused memory address in the `MemoryChunk` storing data implementing the `Drop` trait
    /// of the `StackAllocator` used for resident memory.
    pub fn marker_resident(&self) -> usize {
        self.stack_resident().marker()
    }

    /// Returns the index of the first unused memory address in the `MemoryChunk` storing data implementing the `Copy` trait
    /// of the `StackAllocator` used for resident memory.
    pub fn marker_resident_copy(&self) -> usize {
        self.stack_resident().marker_copy()
    }

    /// Returns the index of the first unused memory address in the `MemoryChunk` storing data implementing the `Drop` trait
    /// of the `StackAllocator` used for temporary memory.
    pub fn marker_temp(&self) -> usize {
        self.stack_temp().marker()
    }

    /// Returns the index of the first unused memory address in the `MemoryChunk` storing data implementing the `Copy` trait
    /// of the `StackAllocator` used for temporary memory.
    pub fn marker_temp_copy(&self) -> usize {
        self.stack_temp().marker_copy()
    }

    /// Reset the `MemoryChunk` storing data implementing the `Drop` trait of the `StackAllocator` used for resident memory, dropping all the content residing inside it.
    ///
    /// # Example
    /// ```
    /// use maskerad_memory_allocators::DoubleEndedStackAllocator;
    ///
    ///
    /// let allocator = DoubleEndedStackAllocator::with_capacity(100, 100);
    ///
    /// //When nothing has been allocated, the first unused memory address is at index 0.
    /// assert_eq!(allocator.marker_resident(), 0);
    /// assert_eq!(allocator.marker_temp(), 0);
    ///
    /// let my_vec: &Vec<u8> = allocator.alloc_resident(|| {
    ///     Vec::with_capacity(10)
    /// }).unwrap();
    ///
    /// assert_ne!(allocator.marker_resident(), 0);
    ///
    /// allocator.reset_resident();
    ///
    /// //The memory chunk storing data implementing the Drop trait of the StackAllocator used for resident memory data has been totally reset, and all its content has been dropped.
    /// assert_eq!(allocator.marker_resident(), 0);
    ///
    /// ```
    pub fn reset_resident(&self) {
        self.stack_resident().reset()
    }

    /// Reset the `MemoryChunk` storing data implementing the `Copy` trait of the `StackAllocator` used for resident memory.
    ///
    /// # Example
    /// ```
    /// use maskerad_memory_allocators::DoubleEndedStackAllocator;
    ///
    ///
    /// let allocator = DoubleEndedStackAllocator::with_capacity(100, 100);
    ///
    /// //When nothing has been allocated, the first unused memory address is at index 0.
    /// assert_eq!(allocator.marker_resident_copy(), 0);
    /// assert_eq!(allocator.marker_temp_copy(), 0);
    ///
    /// let my_i32 = allocator.alloc_resident(|| {
    ///     8 as i32
    /// }).unwrap();
    ///
    /// assert_ne!(allocator.marker_resident_copy(), 0);
    ///
    /// allocator.reset_resident_copy();
    ///
    /// //The memory chunk storing data implementing the Copy trait of the StackAllocator used for resident memory data has been totally reset.
    /// assert_eq!(allocator.marker_resident_copy(), 0);
    ///
    /// ```
    pub fn reset_resident_copy(&self) {
        self.stack_resident().reset_copy()
    }

    /// Reset the `MemoryChunk` storing data implementing the `Drop` trait of the `StackAllocator` used for temporary memory, dropping all the content residing inside it.
    ///
    /// # Example
    /// ```
    /// use maskerad_memory_allocators::DoubleEndedStackAllocator;
    ///
    ///
    /// let allocator = DoubleEndedStackAllocator::with_capacity(200, 200);
    ///
    /// //When nothing has been allocated, the first unused memory address is at index 0.
    /// assert_eq!(allocator.marker_resident(), 0);
    /// assert_eq!(allocator.marker_temp(), 0);
    ///
    /// let my_vec: &Vec<u8> = allocator.alloc_temp(|| {
    ///     Vec::with_capacity(10)
    /// }).unwrap();
    ///
    /// assert_ne!(allocator.marker_temp(), 0);
    ///
    /// allocator.reset_temp();
    ///
    /// //The memory chunk storing data implementing the Drop trait of the StackAllocator used for temporary memory data has been totally reset, and all its content has been dropped.
    /// assert_eq!(allocator.marker_temp(), 0);
    ///
    /// ```
    pub fn reset_temp(&self) {
        self.stack_temp().reset()
    }

    /// Reset the `MemoryChunk` storing data implementing the `Copy` trait of the `StackAllocator` used for temporary memory.
    ///
    /// # Example
    /// ```
    /// use maskerad_memory_allocators::DoubleEndedStackAllocator;
    ///
    ///
    /// let allocator = DoubleEndedStackAllocator::with_capacity(200, 200);
    ///
    /// //When nothing has been allocated, the first unused memory address is at index 0.
    /// assert_eq!(allocator.marker_resident_copy(), 0);
    /// assert_eq!(allocator.marker_temp_copy(), 0);
    ///
    /// let my_i32 = allocator.alloc_temp(|| {
    ///     8 as i32
    /// }).unwrap();
    ///
    /// assert_ne!(allocator.marker_temp_copy(), 0);
    ///
    /// allocator.reset_temp_copy();
    ///
    /// //The memory chunk storing data implementing the Copy trait of the StackAllocator used for temporary memory data has been totally reset.
    /// assert_eq!(allocator.marker_temp_copy(), 0);
    ///
    /// ```
    pub fn reset_temp_copy(&self) {
        self.stack_temp().reset_copy()
    }



    /// Reset partially the `MemoryChunk` storing data implementing the `Drop` trait of the `StackAllocator` used for resident memory, dropping all the content residing
    /// between the current top of the stack and this marker.
    ///
    /// # Example
    /// ```
    /// use maskerad_memory_allocators::DoubleEndedStackAllocator;
    ///
    ///
    /// let allocator = DoubleEndedStackAllocator::with_capacity(200, 200);
    ///
    /// //When nothing has been allocated, the first unused memory address is at index 0.
    /// assert_eq!(allocator.marker_resident(), 0);
    /// assert_eq!(allocator.marker_temp(), 0);
    ///
    /// let my_vec: &Vec<u8> = allocator.alloc_resident(|| {
    ///     Vec::with_capacity(10)
    /// }).unwrap();
    ///
    /// assert_ne!(allocator.marker_resident(), 0);
    /// //Get a marker
    /// let marker = allocator.marker_resident();
    ///
    /// let my_vec_2: &Vec<u8> = allocator.alloc_resident(|| {
    ///     Vec::with_capacity(10)
    /// }).unwrap();
    ///
    /// allocator.reset_to_marker_resident(marker);
    ///
    /// //The memory chunk storing data implementing the Drop trait of the StackAllocator used for resident memory data has been partially reset,
    /// //and all the content between the current top of the stack and the marker has been dropped.
    /// assert_ne!(allocator.marker_resident(), 0);
    /// assert_eq!(allocator.marker_resident(), marker);
    ///
    /// ```
    pub fn reset_to_marker_resident(&self, marker: usize) {
        self.stack_resident().reset_to_marker(marker);
    }

    /// Reset partially the `MemoryChunk` storing data implementing the `Copy` trait of the `StackAllocator` used for resident memory.
    ///
    /// # Example
    /// ```
    /// use maskerad_memory_allocators::DoubleEndedStackAllocator;
    ///
    ///
    /// let allocator = DoubleEndedStackAllocator::with_capacity(200, 200);
    ///
    /// //When nothing has been allocated, the first unused memory address is at index 0.
    /// assert_eq!(allocator.marker_resident_copy(), 0);
    /// assert_eq!(allocator.marker_temp_copy(), 0);
    ///
    /// let my_i32 = allocator.alloc_resident(|| {
    ///     8 as i32
    /// }).unwrap();
    ///
    /// assert_ne!(allocator.marker_resident_copy(), 0);
    /// //Get a marker
    /// let marker = allocator.marker_resident_copy();
    ///
    /// let my_i32_2 = allocator.alloc_resident(|| {
    ///     9 as i32
    /// }).unwrap();
    ///
    /// allocator.reset_to_marker_resident_copy(marker);
    ///
    /// //The memory chunk storing data implementing the Copy trait of the StackAllocator used for resident memory data has been partially reset.
    /// assert_ne!(allocator.marker_resident_copy(), 0);
    /// assert_eq!(allocator.marker_resident_copy(), marker);
    ///
    /// ```
    pub fn reset_to_marker_resident_copy(&self, marker: usize) {
        self.stack_resident().reset_to_marker_copy(marker);
    }

    /// Reset partially the `MemoryChunk` storing data implementing the `Drop` trait of the `StackAllocator` used for temporary memory, dropping all the content residing
    /// between the current top of the stack and this marker.
    ///
    /// # Example
    /// ```
    /// use maskerad_memory_allocators::DoubleEndedStackAllocator;
    ///
    ///
    /// let allocator = DoubleEndedStackAllocator::with_capacity(200, 200);
    ///
    /// //When nothing has been allocated, the first unused memory address is at index 0.
    /// assert_eq!(allocator.marker_resident(), 0);
    /// assert_eq!(allocator.marker_temp(), 0);
    ///
    /// let my_vec: &Vec<u8> = allocator.alloc_temp(|| {
    ///     Vec::with_capacity(10)
    /// }).unwrap();
    ///
    /// assert_ne!(allocator.marker_temp(), 0);
    /// //Get a marker
    /// let marker = allocator.marker_temp();
    ///
    /// let my_vec_2: &Vec<u8> = allocator.alloc_temp(|| {
    ///     Vec::with_capacity(10)
    /// }).unwrap();
    ///
    /// allocator.reset_to_marker_temp(marker);
    ///
    /// //The memory chunk storing data implementing the Drop trait of the StackAllocator used for temporary memory data has been partially reset,
    /// //and all the content between the current top of the stack and the marker has been dropped.
    /// assert_ne!(allocator.marker_temp(), 0);
    /// assert_eq!(allocator.marker_temp(), marker);
    ///
    /// ```
    pub fn reset_to_marker_temp(&self, marker: usize) {
        self.stack_temp().reset_to_marker(marker);
    }

    /// Reset partially the `MemoryChunk` storing data implementing the `Copy` trait of the `StackAllocator` used for temporary memory.
    ///
    /// # Example
    /// ```
    /// use maskerad_memory_allocators::DoubleEndedStackAllocator;
    ///
    ///
    /// let allocator = DoubleEndedStackAllocator::with_capacity(200, 200);
    ///
    /// //When nothing has been allocated, the first unused memory address is at index 0.
    /// assert_eq!(allocator.marker_resident_copy(), 0);
    /// assert_eq!(allocator.marker_temp_copy(), 0);
    ///
    /// let my_i32 = allocator.alloc_temp(|| {
    ///     8 as i32
    /// }).unwrap();
    ///
    /// assert_ne!(allocator.marker_temp_copy(), 0);
    /// //Get a marker
    /// let marker = allocator.marker_temp_copy();
    ///
    /// let my_i32_2 = allocator.alloc_temp(|| {
    ///     9 as i32
    /// }).unwrap();
    ///
    /// allocator.reset_to_marker_temp_copy(marker);
    ///
    /// //The memory chunk storing data implementing the Copy trait of the StackAllocator used for temporary memory data has been partially reset.
    /// assert_ne!(allocator.marker_temp_copy(), 0);
    /// assert_eq!(allocator.marker_temp_copy(), marker);
    ///
    /// ```
    pub fn reset_to_marker_temp_copy(&self, marker: usize) {
        self.stack_temp().reset_to_marker_copy(marker);
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
            let alloc = DoubleEndedStackAllocator::with_capacity(100, 100);
            let start_chunk_temp = alloc.stack_temp().storage().as_ptr();
            let start_chunk_resident = alloc.stack_resident().storage().as_ptr();
            let first_unused_mem_addr_temp = start_chunk_temp.offset(alloc.stack_temp().storage().fill() as isize);
            let first_unused_mem_addr_resident = start_chunk_resident.offset(alloc.stack_resident().storage().fill() as isize);

            assert_eq!(start_chunk_temp, first_unused_mem_addr_temp);
            assert_eq!(start_chunk_resident, first_unused_mem_addr_resident);
        }
    }


    #[test]
    fn allocation_test() {
        //We allocate 200 bytes of memory.
        let alloc = DoubleEndedStackAllocator::with_capacity(100, 100);

        let start_alloc_temp = alloc.stack_temp().storage().as_ptr();
        let start_alloc_resident = alloc.stack_resident().storage().as_ptr();

        let _my_monster = alloc.alloc_temp(|| {
            Monster::new(1)
        }).unwrap();

        unsafe {

            let top_stack_temp = start_alloc_temp.offset(alloc.marker_temp() as isize);
            let top_stack_resident = start_alloc_resident.offset(alloc.marker_resident() as isize);

            assert_ne!(start_alloc_temp, top_stack_temp);
            assert_eq!(start_alloc_resident, top_stack_resident);
        }

        let _my_monster = alloc.alloc_resident(|| {
            Monster::new(1)
        }).unwrap();

        unsafe {
            let top_stack_temp = start_alloc_temp.offset(alloc.marker_temp() as isize);
            let top_stack_resident = start_alloc_resident.offset(alloc.marker_resident() as isize);

            assert_ne!(start_alloc_temp, top_stack_temp);
            assert_ne!(start_alloc_resident, top_stack_resident);
        }
    }

    //Use 'cargo test -- --nocapture' to see the monsters' println!s
    #[test]
    fn test_reset() {
        let alloc = DoubleEndedStackAllocator::with_capacity(100, 100);

        let top_stack_index_temp = alloc.marker_temp();

        let start_alloc_temp = alloc.stack_temp().storage().as_ptr();
        let start_alloc_resident = alloc.stack_resident().storage().as_ptr();

        let _my_monster = alloc.alloc_temp(|| {
            Monster::new(1)
        }).unwrap();

        unsafe {
            let top_stack_temp = start_alloc_temp.offset(alloc.marker_temp() as isize);
            let top_stack_resident = start_alloc_resident.offset(alloc.marker_resident() as isize);

            assert_ne!(start_alloc_temp, top_stack_temp);
            assert_eq!(start_alloc_resident, top_stack_resident);
        }

        let _another_monster = alloc.alloc_resident(|| {
            Monster::default()
        }).unwrap();

        unsafe {
            let top_stack_temp = start_alloc_temp.offset(alloc.marker_temp() as isize);
            let top_stack_resident = start_alloc_resident.offset(alloc.marker_resident() as isize);

            assert_ne!(start_alloc_temp, top_stack_temp);
            assert_ne!(start_alloc_resident, top_stack_resident);
        }

        alloc.reset_to_marker_temp(top_stack_index_temp);

        //my_monster drop here successfully

        unsafe {
            let top_stack_temp = start_alloc_temp.offset(alloc.marker_temp() as isize);
            let top_stack_resident = start_alloc_resident.offset(alloc.marker_resident() as isize);

            assert_eq!(start_alloc_temp, top_stack_temp);
            assert_ne!(start_alloc_resident, top_stack_resident);
        }

        alloc.reset_resident();

        //another_monster drop here successfully

        unsafe {
            let top_stack_temp = start_alloc_temp.offset(alloc.marker_temp() as isize);
            let top_stack_resident = start_alloc_resident.offset(alloc.marker_resident() as isize);

            assert_eq!(start_alloc_temp, top_stack_temp);
            assert_eq!(start_alloc_resident, top_stack_resident);
        }
    }

}


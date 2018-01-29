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
/// use maskerad_memory_allocators::stacks::DoubleEndedStackAllocator;
/// use maskerad_memory_allocators::common::ChunkType;
///
/// //50 bytes for each memory chunk.
/// let double_ended_allocator = DoubleEndedStackAllocator::with_capacity(100, 100);
///
///
/// let my_vec: &Vec<u8> = double_ended_allocator.alloc(&ChunkType::TempData, || {
///     Vec::with_capacity(10)
/// }).unwrap();
///
/// let my_vec_2: &Vec<u8> = double_ended_allocator.alloc(&ChunkType::ResidentData, || {
///     Vec::with_capacity(10)
/// }).unwrap();
///
/// double_ended_allocator.reset(&ChunkType::TempData);
///
/// ```


//TODO: 2 stack allocators, new() use 1 capacity and divide it by 2
pub struct DoubleEndedStackAllocator {
    stack_resident: StackAllocator,
    stack_temp: StackAllocator,
}


impl DoubleEndedStackAllocator {
    /// Creates a DoubleEndedStackAllocator with the given capacity, in bytes.
    /// # Example
    /// ```
    /// #![feature(alloc)]
    /// use maskerad_memory_allocators::stacks::DoubleEndedStackAllocator;
    ///
    /// let allocator = DoubleEndedStackAllocator::with_capacity(100, 100);
    /// assert_eq!(allocator.temp_storage().capacity(), 100);
    /// assert_eq!(allocator.resident_storage().capacity(), 100);
    /// ```
    pub fn with_capacity(capacity: usize, capacity_copy: usize) -> Self {
        DoubleEndedStackAllocator {
            stack_resident: StackAllocator::with_capacity(capacity / 2, capacity_copy / 2),
            stack_temp: StackAllocator::with_capacity(capacity / 2, capacity_copy / 2),
        }
    }

    /// Returns a borrowed reference to the memory chunk used for resident allocation.
    pub fn stack_resident(&self) -> &StackAllocator {
        &self.stack_resident
    }

    /// Returns a borrowed reference to the memory chunk used for temporary allocation.
    pub fn stack_temp(&self) -> &StackAllocator {
        &self.stack_temp
    }



    /// Allocates data in the allocator's memory, returning a mutable reference to the allocated data.
    ///
    /// # Panics
    /// This function will panic if the allocation exceeds the maximum storage capacity of the allocator.
    ///
    /// # Example
    /// ```
    /// use maskerad_memory_allocators::stacks::DoubleEndedStackAllocator;
    /// use maskerad_memory_allocators::common::ChunkType;
    ///
    ///
    /// let allocator = DoubleEndedStackAllocator::with_capacity(100, 100);
    ///
    /// let my_vec: &mut Vec<u8> = allocator.alloc_mut(&ChunkType::TempData, || {
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


    /// Allocates data in the allocator's memory, returning an immutable reference to the allocated data.
    ///
    /// # Panics
    /// This function will panic if the allocation exceeds the maximum storage capacity of the allocator.
    ///
    /// # Example
    /// ```
    /// use maskerad_memory_allocators::stacks::DoubleEndedStackAllocator;
    /// use maskerad_memory_allocators::common::ChunkType;
    ///
    /// let allocator = DoubleEndedStackAllocator::with_capacity(100, 100);
    ///
    /// let my_vec: &Vec<u8> = allocator.alloc(&ChunkType::TempData, || {
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

    pub fn alloc_mut_temp<T, F>(&self, op: F) -> AllocationResult<&mut T>
        where F: FnOnce() -> T
    {
        self.stack_temp().alloc_mut(op)
    }

    pub fn alloc_temp<T, F>(&self, op: F) -> AllocationResult<&T>
        where F: FnOnce() -> T
    {
        self.stack_temp().alloc(op)
    }

    /// Returns the index of the first unused memory address.
    ///
    /// # Example
    /// ```
    /// use maskerad_memory_allocators::stacks::DoubleEndedStackAllocator;
    /// use maskerad_memory_allocators::common::ChunkType;
    ///
    /// let allocator = DoubleEndedStackAllocator::with_capacity(100, 100); //100 bytes for each memory chunk.
    ///
    /// //Get the raw pointer to the bottom of the memory chunk used for temp data.
    /// let start_allocator_temp = allocator.temp_storage().as_ptr();
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
    pub fn marker_resident(&self) -> usize {
        self.stack_resident().marker()
    }

    pub fn marker_resident_copy(&self) -> usize {
        self.stack_resident().marker_copy()
    }

    pub fn marker_temp(&self) -> usize {
        self.stack_temp().marker()
    }

    pub fn marker_temp_copy(&self) -> usize {
        self.stack_temp().marker_copy()
    }

    /// Reset the allocator, dropping all the content residing inside it.
    ///
    /// # Example
    /// ```
    /// use maskerad_memory_allocators::stacks::DoubleEndedStackAllocator;
    /// use maskerad_memory_allocators::common::ChunkType;
    ///
    ///
    /// let allocator = DoubleEndedStackAllocator::with_capacity(100, 100); // 100 bytes for each memory chunk.
    ///
    /// //When nothing has been allocated, the first unused memory address is at index 0.
    /// assert_eq!(allocator.marker(&ChunkType::TempData), 0);
    /// assert_eq!(allocator.marker(&ChunkType::ResidentData), 0);
    ///
    /// let my_vec: &Vec<u8> = allocator.alloc(&ChunkType::TempData, || {
    ///     Vec::with_capacity(10)
    /// }).unwrap();
    ///
    /// assert_ne!(allocator.marker(&ChunkType::TempData), 0);
    ///
    /// let my_vec_2: &Vec<u8> = allocator.alloc(&ChunkType::TempData, || {
    ///     Vec::with_capacity(10)
    /// }).unwrap();
    ///
    /// allocator.reset(&ChunkType::TempData);
    ///
    /// //The memory chunk for temp data has been totally reset, and all its content has been dropped.
    /// assert_eq!(allocator.marker(&ChunkType::TempData), 0);
    ///
    /// ```
    pub fn reset_resident(&self) {
        self.stack_resident().reset()
    }

    pub fn reset_resident_copy(&self) {
        self.stack_resident().reset_copy()
    }

    pub fn reset_temp(&self) {
        self.stack_temp().reset()
    }

    pub fn reset_temp_copy(&self) {
        self.stack_temp().reset_copy()
    }



    /// Reset partially the allocator, dropping all the content residing between the marker and
    /// the first unused memory address of the allocator.
    ///
    /// # Example
    /// ```
    /// use maskerad_memory_allocators::stacks::DoubleEndedStackAllocator;
    /// use maskerad_memory_allocators::common::ChunkType;
    ///
    ///
    /// let allocator = DoubleEndedStackAllocator::with_capacity(100, 100); // 100 bytes for each memory chunk.
    ///
    /// //When nothing has been allocated, the first unused memory address is at index 0.
    /// assert_eq!(allocator.marker(&ChunkType::TempData), 0);
    ///
    /// let my_vec: &Vec<u8> = allocator.alloc(&ChunkType::TempData, || {
    ///     Vec::with_capacity(10)
    /// }).unwrap();
    ///
    /// //After the monster allocation, get the index of the first unused memory address in the memory chunk used for temp data.
    /// let index_current_temp = allocator.marker(&ChunkType::TempData);
    /// assert_ne!(index_current_temp, 0);
    ///
    /// let my_vec_2: &Vec<u8> = allocator.alloc(&ChunkType::TempData, || {
    ///     Vec::with_capacity(10)
    /// }).unwrap();
    ///
    /// assert_ne!(allocator.marker(&ChunkType::TempData), index_current_temp);
    ///
    /// allocator.reset_to_marker(&ChunkType::TempData, index_current_temp);
    ///
    /// //The allocator has been partially reset, and all the content lying between the marker and
    /// //the first unused memory address has been dropped.
    ///
    ///
    /// assert_eq!(allocator.marker(&ChunkType::TempData), index_current_temp);
    ///
    /// ```
    pub fn reset_to_marker_resident(&self, marker: usize) {
        self.stack_resident().reset_to_marker(marker);
    }

    pub fn reset_to_marker_resident_copy(&self, marker: usize) {
        self.stack_resident().reset_to_marker_copy(marker);
    }

    pub fn reset_to_marker_temp(&self, marker: usize) {
        self.stack_temp().reset_to_marker(marker);
    }

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
            let start_chunk_temp = alloc.temp_storage().as_ptr();
            let start_chunk_resident = alloc.resident_storage().as_ptr();
            let first_unused_mem_addr_temp = start_chunk_temp.offset(alloc.temp_storage().fill() as isize);
            let first_unused_mem_addr_resident = start_chunk_resident.offset(alloc.resident_storage().fill() as isize);

            assert_eq!(start_chunk_temp, first_unused_mem_addr_temp);
            assert_eq!(start_chunk_resident, first_unused_mem_addr_resident);
        }
    }


    #[test]
    fn allocation_test() {
        //We allocate 200 bytes of memory.
        let alloc = DoubleEndedStackAllocator::with_capacity(100, 100);

        let start_alloc_temp = alloc.temp_storage().as_ptr();
        let start_alloc_resident = alloc.resident_storage().as_ptr();

        let _my_monster = alloc.alloc(&ChunkType::TempData, || {
            Monster::new(1)
        }).unwrap();

        unsafe {

            let top_stack_temp = start_alloc_temp.offset(alloc.marker(&ChunkType::TempData) as isize);
            let top_stack_resident = start_alloc_resident.offset(alloc.marker(&ChunkType::ResidentData) as isize);

            assert_ne!(start_alloc_temp, top_stack_temp);
            assert_eq!(start_alloc_resident, top_stack_resident);
        }

        let _my_monster = alloc.alloc(&ChunkType::ResidentData, || {
            Monster::new(1)
        }).unwrap();

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
        let alloc = DoubleEndedStackAllocator::with_capacity(100, 100);

        let top_stack_index_temp = alloc.marker(&ChunkType::TempData);

        let start_alloc_temp = alloc.temp_storage().as_ptr();
        let start_alloc_resident = alloc.resident_storage().as_ptr();

        let _my_monster = alloc.alloc(&ChunkType::TempData, || {
            Monster::new(1)
        }).unwrap();

        unsafe {
            let top_stack_temp = start_alloc_temp.offset(alloc.marker(&ChunkType::TempData) as isize);
            let top_stack_resident = start_alloc_resident.offset(alloc.marker(&ChunkType::ResidentData) as isize);

            assert_ne!(start_alloc_temp, top_stack_temp);
            assert_eq!(start_alloc_resident, top_stack_resident);
        }

        let _another_monster = alloc.alloc(&ChunkType::ResidentData, || {
            Monster::default()
        }).unwrap();

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


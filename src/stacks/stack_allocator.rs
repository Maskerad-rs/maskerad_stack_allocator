// Copyright 2017-2018 Maskerad Developers
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

use core::ptr;
use std::cell::{BorrowError, RefCell};
use std::mem;

use allocation_error::{AllocationError, AllocationResult};
use utils;
use memory_chunk::MemoryChunk;
use std::intrinsics::needs_drop;

/// A stack-based allocator.
///
/// It manages two memory storages to:
///
/// - Allocate bytes in a stack-like fashion.
///
/// - Store different types of objects in the same storage.
///
/// - Drop the content of the storage when needed.
///
/// One storage is used for data implementing the `Drop` trait, the other is used for data implementing
/// the `Copy` trait. A structure implementing the `Copy` trait cannot implement the `Drop` trait. In order to
/// drop data implementing the `Drop` trait, we need to store its vtable next to it in memory.
///
/// # Details
///
/// ## Instantiation
/// When instantiated, the `StackAllocator` pre-allocate the given number of bytes for each memory storage.
///
///
/// ## Allocation
/// When an object is allocated in memory, the allocator:
///
/// - Check if the allocated object needs to be dropped, and choose which memory storage to use according to this information,
///
/// - Asks a pointer to a memory address to the corresponding memory storage,
///
/// - Place the object in this memory address,
///
/// - Update the first unused memory address of the memory storage according to an offset,
///
/// - And return an immutable/mutable reference to the object which has been placed in the memory storage.
///
/// This offset is calculated by the size of the object, its vtable (if the object implement the `Drop` trait),
/// its memory-alignment and an offset to align the object in memory.
///
/// ## Roll-back
///
/// This structure allows you to get a **marker**, the index to the first unused memory address of a memory storage. A stack allocator can *reset* a memory storage to a marker,
/// or reset a memory storage entirely.
///
/// When a memory storage is reset to a marker, it will:
///
/// - Drop all the content lying between the marker and the first unused memory address, if it holds data implementing the `Drop` trait,
///
/// - Set the first unused memory address to the marker.
///
///
/// When a memory storage is reset completely, it will:
///
/// - Drop everything, if it holds data implementing the `Drop` trait,
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
#[derive(Debug)]
pub struct StackAllocator {
    storage: RefCell<MemoryChunk>,
    storage_copy: RefCell<MemoryChunk>,
}

impl StackAllocator {
    /// Creates a StackAllocator with the given capacities, in bytes.
    ///
    /// The first capacity is for the memory storage holding data implementing the `Drop` trait,
    /// the second is for the memory storage holding data implementing the `Copy` trait.
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
        debug!("Creating a StackAllocator with {} bytes for droppable data and {} bytes for copyable data.", capacity, capacity_copy);
        StackAllocator {
            storage: RefCell::new(MemoryChunk::new(capacity)),
            storage_copy: RefCell::new(MemoryChunk::new(capacity_copy)),
        }
    }

    /// Allocates data in the allocator's memory, returning a mutable reference to the allocated data.
    ///
    /// If the allocated data implements `Drop`, it will be placed in the memory storage storing data implementing the `Drop` trait.
    /// Otherwise, it will be placed in the other memory storage.
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
    pub fn alloc_mut<T, F>(&self, op: F) -> AllocationResult<&mut T>
    where
        F: FnOnce() -> T,
    {
        debug!("Allocating data, returning a mutable reference.");
        unsafe {
            if needs_drop::<T>() {
                trace!("The data to allocate is droppable.");
                self.alloc_non_copy_mut(op)
            } else {
                trace!("The data to allocate is copyable.");
                self.alloc_copy_mut(op)
            }
        }
    }

    /// Allocates data in the allocator's memory, returning a mutable reference to the allocated data.
    ///
    /// If the allocated data implements `Drop`, it will be placed in the memory storage storing data implementing the `Drop` trait.
    /// Otherwise, it will be placed in the other memory storage.
    ///
    /// # Warning
    /// This function doesn't return an error if the allocated data doesn't fit in the `StackAllocator`'s remaining capacity,
    /// It doesn't perform any check.
    ///
    /// Use if you now that the data will fit into memory and you can't afford the checks.
    ///
    /// # Example
    /// ```
    /// use maskerad_memory_allocators::StackAllocator;
    /// # use std::error::Error;
    /// # fn try_main() -> Result<(), Box<Error>> {
    /// let allocator = StackAllocator::with_capacity(100, 100);
    ///
    /// // An i32 is 4 bytes, has a 4-byte alignment and doesn't have a destructor.
    /// // It will, in the worst case take 8 bytes in the stack memory.
    /// // The copy storage of the stack can store up to 100 bytes, so it's fine.
    ///
    /// let my_i32 = allocator.alloc_mut_unchecked(|| {
    ///     26 as i32
    /// });
    ///
    /// assert_eq!(my_i32, &mut 26);
    /// # Ok(())
    /// # }
    /// # fn main() {
    /// #   try_main().unwrap();
    /// # }
    /// ```
    pub fn alloc_mut_unchecked<T, F>(&self, op: F) -> &mut T
        where
            F: FnOnce() -> T,
    {
        debug!("Allocating data, returning a mutable reference (unchecked).");
        unsafe {
            if needs_drop::<T>() {
                trace!("The data to allocate is droppable.");
                self.alloc_non_copy_mut_unchecked(op)
            } else {
                trace!("The data to allocate is copyable.");
                self.alloc_copy_mut_unchecked(op)
            }
        }
    }

    /// The function actually writing data in the memory storage
    fn alloc_non_copy_mut<T, F>(&self, op: F) -> AllocationResult<&mut T>
    where
        F: FnOnce() -> T,
    {
        trace!("Allocating mutable and droppable data.");
        unsafe {
            //Get the type description of the type T (get its vtable).
            trace!("Getting a TypeDescription of the data being allocated.");
            let type_description = utils::get_type_description::<T>();

            //Ask the memory chunk to give us raw pointers to memory locations for our type description and object
            trace!("Getting raw pointers to memory locations, to store the type description and the data.");
            let (type_description_ptr, ptr) =
                self.alloc_non_copy_inner(mem::size_of::<T>(), mem::align_of::<T>())?;

            //Cast them.
            trace!("Casting the raw pointers to appropriate types.");
            let type_description_ptr = type_description_ptr as *mut usize;
            let ptr = ptr as *mut T;

            //write in our type description along with a bit indicating that the object has *not*
            //been initialized yet.
            trace!("Packing in the low bit of the TypeDescription the 'is_done' state to false.");
            *type_description_ptr = utils::bitpack_type_description_ptr(type_description, false);

            //Initialize the object.
            trace!("Initializing the data.");
            ptr::write(&mut (*ptr), op());

            //Now that we are done, update the type description to indicate
            //that the object is there.
            trace!("Packing in the low bit of the TypeDescription the 'is_done' state to true.");
            *type_description_ptr = utils::bitpack_type_description_ptr(type_description, true);

            //Return a mutable reference to the object.
            trace!("Returning a mutable reference to the allocated data.");
            Ok(&mut *ptr)
        }
    }

    fn alloc_non_copy_mut_unchecked<T, F>(&self, op: F) -> &mut T
        where
            F: FnOnce() -> T,
    {
        trace!("Allocating mutable and droppable data (unchecked).");
        unsafe {
            //Get the type description of the type T (get its vtable).
            trace!("Getting a TypeDescription of the data being allocated.");
            let type_description = utils::get_type_description::<T>();

            //Ask the memory chunk to give us raw pointers to memory locations for our type description and object
            trace!("Getting raw pointers to memory locations, to store the type description and the data.");
            let (type_description_ptr, ptr) =
                self.alloc_non_copy_inner_unchecked(mem::size_of::<T>(), mem::align_of::<T>());

            //Cast them.
            trace!("Casting the raw pointers to appropriate types.");
            let type_description_ptr = type_description_ptr as *mut usize;
            let ptr = ptr as *mut T;

            //write in our type description along with a bit indicating that the object has *not*
            //been initialized yet.
            trace!("Packing in the low bit of the TypeDescription the 'is_done' state to false.");
            *type_description_ptr = utils::bitpack_type_description_ptr(type_description, false);

            //Initialize the object.
            trace!("Initializing the data.");
            ptr::write(&mut (*ptr), op());

            //Now that we are done, update the type description to indicate
            //that the object is there.
            trace!("Packing in the low bit of the TypeDescription the 'is_done' state to true.");
            *type_description_ptr = utils::bitpack_type_description_ptr(type_description, true);

            //Return a mutable reference to the object.
            trace!("Returning a mutable reference to the allocated data.");
            &mut *ptr
        }
    }

    //Functions for the copyable part of the stack allocator.
    fn alloc_copy_mut<T, F>(&self, op: F) -> AllocationResult<&mut T>
    where
        F: FnOnce() -> T,
    {
        trace!("Allocating mutable and copyable data.");
        unsafe {
            //Get an aligned raw pointer to place the object in it.
            trace!("Getting a raw pointer to a memory location, to store the data.");
            let ptr = self.alloc_copy_inner(mem::size_of::<T>(), mem::align_of::<T>())?;

            //cast this raw pointer to the type of the object.
            trace!("Casting the raw pointer to appropriate type.");
            let ptr = ptr as *mut T;

            //Write the data in the memory location.
            trace!("Initializing the data.");
            ptr::write(&mut (*ptr), op());

            //return a mutable reference to this pointer.
            trace!("Returning a mutable reference to the allocated data.");
            Ok(&mut *ptr)
        }
    }

    fn alloc_copy_mut_unchecked<T, F>(&self, op: F) -> &mut T
        where
            F: FnOnce() -> T,
    {
        trace!("Allocating mutable and copyable data (unchecked).");
        unsafe {
            //Get an aligned raw pointer to place the object in it.
            trace!("Getting a raw pointer to a memory location, to store the data.");
            let ptr = self.alloc_copy_inner_unchecked(mem::size_of::<T>(), mem::align_of::<T>());

            //cast this raw pointer to the type of the object.
            trace!("Casting the raw pointer to appropriate type.");
            let ptr = ptr as *mut T;

            //Write the data in the memory location.
            trace!("Initializing the data.");
            ptr::write(&mut (*ptr), op());

            //return a mutable reference to this pointer.
            trace!("Returning a mutable reference to the allocated data.");
            &mut *ptr
        }
    }

    /// Allocates data in the allocator's memory, returning an immutable reference to the allocated data.
    ///
    /// If the allocated data implements `Drop`, it will be placed in the memory storage storing data implementing the `Drop` trait.
    /// Otherwise, it will be placed in the other memory storage.
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
    pub fn alloc<T, F>(&self, op: F) -> AllocationResult<&T>
    where
        F: FnOnce() -> T,
    {
        debug!("Allocating data, returning an immutable reference.");
        unsafe {
            if needs_drop::<T>() {
                trace!("Data is droppable.");
                self.alloc_non_copy(op)
            } else {
                trace!("Data is copyable.");
                self.alloc_copy(op)
            }
        }
    }

    /// Allocates data in the allocator's memory, returning an immutable reference to the allocated data.
    ///
    /// If the allocated data implements `Drop`, it will be placed in the memory storage storing data implementing the `Drop` trait.
    /// Otherwise, it will be placed in the other memory storage.
    ///
    /// # Warning
    /// This function doesn't return an error if the allocated data doesn't fit in the `StackAllocator`'s remaining capacity,
    /// It doesn't perform any check.
    ///
    /// Use if you now that the data will fit into memory and you can't afford the checks.
    ///
    /// # Example
    ///
    /// ```rust
    /// use maskerad_memory_allocators::StackAllocator;
    /// # use std::error::Error;
    /// # fn try_main() -> Result<(), Box<Error>> {
    /// let allocator = StackAllocator::with_capacity(100, 100);
    ///
    /// // An i32 is 4 bytes, has a 4-byte alignment and doesn't have a destructor.
    /// // It will, in the worst case take 8 bytes in the stack memory.
    /// // The copy storage of the stack can store up to 100 bytes, so it's fine.
    ///
    /// let my_i32 = allocator.alloc_unchecked(|| {
    ///     26 as i32
    /// });
    ///
    /// assert_eq!(my_i32, &26);
    /// # Ok(())
    /// # }
    /// # fn main() {
    /// #   try_main().unwrap();
    /// # }
    /// ```
    pub fn alloc_unchecked<T, F>(&self, op: F) -> &T
        where
            F: FnOnce() -> T,
    {
        debug!("Allocating data, returning an immutable reference (unchecked).");
        unsafe {
            if needs_drop::<T>() {
                trace!("Data is droppable.");
                self.alloc_non_copy_unchecked(op)
            } else {
                trace!("Data is copyable.");
                self.alloc_copy_unchecked(op)
            }
        }
    }

    //Functions for the non-copyable part of the arena.

    /// The function actually writing data in the memory storage
    fn alloc_non_copy<T, F>(&self, op: F) -> AllocationResult<&T>
    where
        F: FnOnce() -> T,
    {
        trace!("Allocating immutable and droppable data.");
        unsafe {
            //Get the type description of the type T (get its vtable).
            trace!("Getting a TypeDescription of the data being allocated.");
            let type_description = utils::get_type_description::<T>();

            //Ask the memory chunk to give us raw pointers to memory locations for our type description and object
            trace!("Getting raw pointers to memory locations, to store the type description and the data.");
            let (type_description_ptr, ptr) =
                self.alloc_non_copy_inner(mem::size_of::<T>(), mem::align_of::<T>())?;

            //Cast them.
            trace!("Casting the raw pointers to appropriate types.");
            let type_description_ptr = type_description_ptr as *mut usize;
            let ptr = ptr as *mut T;

            //write in our type description along with a bit indicating that the object has *not*
            //been initialized yet.
            trace!("Packing in the low bit of the TypeDescription the 'is_done' state to false.");
            *type_description_ptr = utils::bitpack_type_description_ptr(type_description, false);

            //Initialize the object.
            trace!("Initializing the data.");
            ptr::write(&mut (*ptr), op());

            //Now that we are done, update the type description to indicate
            //that the object is there.
            trace!("Packing in the low bit of the TypeDescription the 'is_done' state to true.");
            *type_description_ptr = utils::bitpack_type_description_ptr(type_description, true);

            //Return a mutable reference to the object.
            trace!("Returning an immutable reference to the allocated data.");
            Ok(&*ptr)
        }
    }

    fn alloc_non_copy_unchecked<T, F>(&self, op: F) -> &T
        where
            F: FnOnce() -> T,
    {
        trace!("Allocating immutable and droppable data (unchecked).");
        unsafe {
            //Get the type description of the type T (get its vtable).
            trace!("Getting a TypeDescription of the data being allocated.");
            let type_description = utils::get_type_description::<T>();

            //Ask the memory chunk to give us raw pointers to memory locations for our type description and object
            trace!("Getting raw pointers to memory locations, to store the type description and the data.");
            let (type_description_ptr, ptr) =
                self.alloc_non_copy_inner_unchecked(mem::size_of::<T>(), mem::align_of::<T>());

            //Cast them.
            trace!("Casting the raw pointers to appropriate types.");
            let type_description_ptr = type_description_ptr as *mut usize;
            let ptr = ptr as *mut T;

            //write in our type description along with a bit indicating that the object has *not*
            //been initialized yet.
            trace!("Packing in the low bit of the TypeDescription the 'is_done' state to false.");
            *type_description_ptr = utils::bitpack_type_description_ptr(type_description, false);

            //Initialize the object.
            trace!("Initializing the data.");
            ptr::write(&mut (*ptr), op());

            //Now that we are done, update the type description to indicate
            //that the object is there.
            trace!("Packing in the low bit of the TypeDescription the 'is_done' state to true.");
            *type_description_ptr = utils::bitpack_type_description_ptr(type_description, true);

            //Return an immutable reference to the object.
            trace!("Returning an immutable reference to the allocated data.");
            &*ptr
        }
    }

    fn alloc_copy<T, F>(&self, op: F) -> AllocationResult<&T>
    where
        F: FnOnce() -> T,
    {
        trace!("Allocating immutable and copyable data.");
        unsafe {
            //Get an aligned raw pointer to place the object in it.
            trace!("Getting a raw pointer to a memory location, to store the data.");
            let ptr = self.alloc_copy_inner(mem::size_of::<T>(), mem::align_of::<T>())?;

            //cast this raw pointer to the type of the object.
            trace!("Casting the raw pointer to the appropriate type.");
            let ptr = ptr as *mut T;

            //Write the data in the memory location.
            trace!("Initializing the data.");
            ptr::write(&mut (*ptr), op());

            //return a mutable reference to this pointer.
            trace!("Returning an immutable reference to the allocated data.");
            Ok(&*ptr)
        }
    }

    fn alloc_copy_unchecked<T, F>(&self, op: F) -> &T
        where
            F: FnOnce() -> T,
    {
        trace!("Allocating immutable and copyable data (unchecked).");
        unsafe {
            //Get an aligned raw pointer to place the object in it.
            trace!("Getting a raw pointer to a memory location, to store the data.");
            let ptr = self.alloc_copy_inner_unchecked(mem::size_of::<T>(), mem::align_of::<T>());

            //cast this raw pointer to the type of the object.
            trace!("Casting the raw pointer to the appropriate type.");
            let ptr = ptr as *mut T;

            //Write the data in the memory location.
            trace!("Initializing the data.");
            ptr::write(&mut (*ptr), op());

            //return an immutable reference to this pointer.
            trace!("Returning an immutable reference to the allocated data.");
            &*ptr
        }
    }

    /// The function asking the memory storage to give us raw pointers to memory locations and update
    /// the current top of the stack.
    fn alloc_non_copy_inner(
        &self,
        n_bytes: usize,
        align: usize,
    ) -> AllocationResult<(*const u8, *const u8)> {
        trace!("The droppable data has a size of {} bytes and an alignment of {} bytes.", n_bytes, align);
        trace!("Borrowing a reference to the memory chunk storing droppable data.");
        let non_copy_storage = self.storage.borrow();

        //Get the index of the first unused byte in the memory chunk.
        trace!("Getting the index of the first unused byte in the memory chunk.");
        let fill = non_copy_storage.fill();

        //Get the index of where we'll write the type description data
        //(the first unused byte in the memory chunk).
        trace!("The memory location for the TypeDescription will begin at byte {} ({:x})...", fill, fill);
        let type_description_start = fill;

        // Get the index of where the object should reside (unaligned location actually).
        let after_type_description = fill + mem::size_of::<*const utils::TypeDescription>();
        trace!("...and will end at {} ({:x})", after_type_description, after_type_description);

        //With the index to the unaligned memory address, determine the index to
        //the aligned memory address where the object will reside,
        //according to its memory alignment.
        let start = utils::round_up(after_type_description, align);
        trace!("The memory location for the actual data will begin at byte {} ({:x})...", start, start);

        //Determine the index of the next aligned memory address for a type description, according to the size of the object
        //and the memory alignment of a type description.
        let end = utils::round_up(
            start + n_bytes,
            mem::align_of::<*const utils::TypeDescription>(),
        );
        trace!("...and will end at {} ({:x})", end, end);

        //If the allocator becomes oom after this possible allocation, abort the program.
        trace!("Checking if the allocator has enough remaining memory to store the data.");
        if end >= non_copy_storage.capacity() {
            error!("The allocator doesn't have enough remaining memory to store the data !");
            return Err(AllocationError::OutOfMemoryError(format!(
                "The stack allocator is out of memory !"
            )));
        }

        //Update the current top of the stack.
        //The first unused memory address is at index 'end',
        //where the next type description would be written
        //if an allocation was asked.
        trace!("Setting the first unused byte of memory of the memory chunk to byte {} ({:x})", end, end);
        non_copy_storage.set_fill(end);

        unsafe {
            // Get a raw pointer to the start of our MemoryChunk's RawVec
            let start_storage = non_copy_storage.as_ptr();
            trace!("Getting a raw pointer to the start of the allocation of the memory chunk: {:p}.", start_storage);

            trace!("Returning a tuple of raw pointers to memory locations for the TypeDescription and data.");
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

    fn alloc_non_copy_inner_unchecked(
        &self,
        n_bytes: usize,
        align: usize,
    ) -> (*const u8, *const u8) {
        trace!("The droppable data has a size of {} bytes and an alignment of {} bytes (unchecked).", n_bytes, align);
        trace!("Borrowing a reference to the memory chunk storing droppable data.");
        let non_copy_storage = self.storage.borrow();

        //Get the index of the first unused byte in the memory chunk.
        trace!("Getting the index of the first unused byte in the memory chunk.");
        let fill = non_copy_storage.fill();

        //Get the index of where we'll write the type description data
        //(the first unused byte in the memory chunk).
        trace!("The memory location for the TypeDescription will begin at byte {} ({:x})...", fill, fill);
        let type_description_start = fill;

        // Get the index of where the object should reside (unaligned location actually).
        let after_type_description = fill + mem::size_of::<*const utils::TypeDescription>();
        trace!("...and will end at {} ({:x})", after_type_description, after_type_description);

        //With the index to the unaligned memory address, determine the index to
        //the aligned memory address where the object will reside,
        //according to its memory alignment.
        let start = utils::round_up(after_type_description, align);
        trace!("The memory location for the actual data will begin at byte {} ({:x})...", start, start);

        //Determine the index of the next aligned memory address for a type description, according to the size of the object
        //and the memory alignment of a type description.
        let end = utils::round_up(
            start + n_bytes,
            mem::align_of::<*const utils::TypeDescription>(),
        );
        trace!("...and will end at {} ({:x})", end, end);

        //Update the current top of the stack.
        //The first unused memory address is at index 'end',
        //where the next type description would be written
        //if an allocation was asked.
        trace!("Setting the first unused byte of memory of the memory chunk to byte {} ({:x})", end, end);
        non_copy_storage.set_fill(end);

        unsafe {
            // Get a raw pointer to the start of our MemoryChunk's RawVec
            let start_storage = non_copy_storage.as_ptr();
            trace!("Getting a raw pointer to the start of the allocation of the memory chunk: {:p}.", start_storage);

            trace!("Returning a tuple of raw pointers to memory locations for the TypeDescription and data.");
            (
                //From this raw pointer, get the correct raw pointers with
                //the indices we calculated earlier.

                //The raw pointer to the type description of the object.
                start_storage.offset(type_description_start as isize),
                //The raw pointer to the object.
                start_storage.offset(start as isize),
            )
        }
    }

    fn alloc_copy_inner(&self, n_bytes: usize, align: usize) -> AllocationResult<*const u8> {
        trace!("The copyable data has a size of {} bytes and an alignment of {} bytes.", n_bytes, align);
        //borrow mutably the memory chunk used by the allocator.
        trace!("Borrowing a reference to the memory chunk storing copyable data.");
        let copy_storage = self.storage_copy.borrow();

        //Get the index of the first unused memory address in the memory chunk.
        trace!("Getting the index of the first unused byte in the memory chunk.");
        let fill = copy_storage.fill();

        //Get the index of the aligned memory address, which will be returned.
        let start = utils::round_up(fill, align);
        trace!("The memory location for the actual data will begin at byte {} ({:x})...", start, start);

        //Get the index of the future first unused memory address, according to the size of the object.
        let end = start + n_bytes;
        trace!("...and will end at {} ({:x})", end, end);

        //We don't grow the capacity, or create another chunk.
        trace!("Checking if the allocator has enough remaining memory to store the data.");
        if end >= copy_storage.capacity() {
            error!("The allocator doesn't have enough remaining memory to store the data !");
            return Err(AllocationError::OutOfMemoryError(format!(
                "The copy stack allocator is out of memory !"
            )));
        }

        //Set the first unused memory address of the memory chunk to the index calculated earlier.
        trace!("Setting the first unused byte of memory of the memory chunk to byte {} ({:x})", end, end);
        copy_storage.set_fill(end);

        trace!("Returning a raw pointer to a memory location for the data.");
        unsafe {
            //Return the raw pointer to the aligned memory location, which will be used to place
            //the object in the allocator.
            Ok(copy_storage.as_ptr().offset(start as isize))
        }
    }

    fn alloc_copy_inner_unchecked(&self, n_bytes: usize, align: usize) -> *const u8 {
        trace!("The copyable data has a size of {} bytes and an alignment of {} bytes (unchecked).", n_bytes, align);
        //borrow mutably the memory chunk used by the allocator.
        trace!("Borrowing a reference to the memory chunk storing copyable data.");
        let copy_storage = self.storage_copy.borrow();

        //Get the index of the first unused memory address in the memory chunk.
        trace!("Getting the index of the first unused byte in the memory chunk.");
        let fill = copy_storage.fill();

        //Get the index of the aligned memory address, which will be returned.
        let start = utils::round_up(fill, align);
        trace!("The memory location for the actual data will begin at byte {} ({:x})...", start, start);

        //Get the index of the future first unused memory address, according to the size of the object.
        let end = start + n_bytes;
        trace!("...and will end at {} ({:x})", end, end);

        //Set the first unused memory address of the memory chunk to the index calculated earlier.
        trace!("Setting the first unused byte of memory of the memory chunk to byte {} ({:x})", end, end);
        copy_storage.set_fill(end);

        trace!("Returning a raw pointer to a memory location for the data.");
        unsafe {
            //Return the raw pointer to the aligned memory location, which will be used to place
            //the object in the allocator.
            copy_storage.as_ptr().offset(start as isize)
        }
    }

    /// Returns the index of the first unused memory address of the memory storage storing data implementing
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
        debug!("Getting the first unused byte of the memory chunk storing droppable data.");
        trace!("first unused byte of memory: {}.", self.storage.borrow().fill());
        self.storage.borrow().fill()
    }

    /// Returns the index of the first unused memory address of the memory storage storing data implementing
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
        debug!("Getting the first unused byte of the memory chunk storing copyable data.");
        trace!("first unused byte of memory: {}.", self.storage.borrow().fill());
        self.storage_copy.borrow().fill()
    }

    /// Reset the memory storage storing data implementing the `Drop` trait, dropping all the content residing inside it.
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
        debug!("Resetting completely the memory chunk holding droppable data.");
        unsafe {
            trace!("all data is being dropped.");
            self.storage.borrow().destroy();
            trace!("the first unused byte of memory is being set to 0.");
            self.storage.borrow().set_fill(0);
        }
    }

    /// Reset the memory storage storing data implementing the `Drop` trait, dropping all the content residing inside it.
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
        debug!("Resetting completely the memory chunk holding copyable data.");
        trace!("the first unused byte of memory is being set to 0.");
        self.storage_copy.borrow().set_fill(0);
    }

    /// Reset partially the memory storage storing data implementing the `Drop` trait, dropping all the content residing between the marker and
    /// the first unused memory address of the memory storage.
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
        debug!("Resetting partially the memory chunk holding droppable data to the marker {}.", marker);
        unsafe {
            trace!("The data lying between the byte {} and the byte {} is being dropped.", marker, self.storage.borrow().fill());
            self.storage.borrow().destroy_to_marker(marker);
            trace!("The first unused byte of memory is being set to {}", marker);
            self.storage.borrow().set_fill(marker);
        }
    }

    /// Reset partially the memory storage storing data implementing the `Copy` trait.
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
        debug!("Resetting partially the memory chunk holding copyable data to the marker {}.", marker);
        trace!("The first unused byte of memory is being set to {}", marker);
        self.storage_copy.borrow().set_fill(marker);
    }

    /// Returns the maximum capacity the memory storage storing data implementing the `Drop` trait can hold.
    ///
    /// # Example
    ///
    /// ```rust
    /// use maskerad_memory_allocators::StackAllocator;
    /// // 100 bytes for data implementing Drop, 100 bytes for Data implementing Copy.
    /// let allocator = StackAllocator::with_capacity(100, 50);
    /// assert_eq!(allocator.capacity(), 100);
    /// ```
    pub fn capacity(&self) -> usize {
        debug!("Getting the maximum capacity of the memory chunk storing droppable data.");
        self.storage.borrow().capacity()
    }

    /// Returns the maximum capacity the memory storage storing data implementing the `Copy` trait can hold.
    ///
    /// # Example
    ///
    /// ```rust
    /// use maskerad_memory_allocators::StackAllocator;
    /// // 100 bytes for data implementing Drop, 100 bytes for Data implementing Copy.
    /// let allocator = StackAllocator::with_capacity(100, 50);
    /// assert_eq!(allocator.capacity_copy(), 50);
    /// ```
    pub fn capacity_copy(&self) -> usize {
        debug!("Getting the maximum capacity of the memory chunk storing copyable data.");
        self.storage_copy.borrow().capacity()
    }

    /// Returns a raw pointer to the start of the memory storage storing data implementing the `Drop` trait.
    ///
    /// # Example
    ///
    /// ```rust
    /// use maskerad_memory_allocators::StackAllocator;
    /// // 100 bytes for data implementing Drop, 100 bytes for Data implementing Copy.
    /// let allocator = StackAllocator::with_capacity(100, 50);
    /// let ptr = allocator.storage_as_ptr();
    /// ```
    pub fn storage_as_ptr(&self) -> *const u8 {
        debug!("Getting a raw pointer to the start of the allocation of the memory chunk storing droppable data.");
        self.storage.borrow().as_ptr()
    }

    /// Returns a raw pointer to the start of the memory storage storing data implementing the `Copy` trait.
    ///
    /// # Example
    ///
    /// ```rust
    /// use maskerad_memory_allocators::StackAllocator;
    /// // 100 bytes for data implementing Drop, 100 bytes for Data implementing Copy.
    /// let allocator = StackAllocator::with_capacity(100, 50);
    /// let ptr = allocator.storage_copy_as_ptr();
    /// ```
    pub fn storage_copy_as_ptr(&self) -> *const u8 {
        debug!("Getting a raw pointer to the start of the allocation of the memory chunk storing copyable data.");
        self.storage_copy.borrow().as_ptr()
    }

    /// Drop all the objects implementing the `Drop` trait.
    fn destroy_stack(&self) -> Result<(), BorrowError> {
        debug!("The StackAllocator is being dropped, all droppable data is being dropped.");
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

// Copyright 2017-2018 Maskerad Developers
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

use pool_item::PoolItem;
use std::cell::{RefCell, Cell};
use std::mem;
use std::ptr;
use utils;
use allocation_error::{AllocationResult, AllocationError};

pub struct PoolAllocator {
    storage: Vec<RefCell<PoolItem>>,
    first_available: Cell<Option<usize>>,
}

impl PoolAllocator {
    pub fn new(nb_item: usize, size_item: usize) -> Self {
        let mut storage = Vec::with_capacity(nb_item);
        for i in 0..nb_item - 1 {
            storage[i] = RefCell::new(PoolItem::new(size_item, Some(i+1)));
        }

        storage[nb_item - 1] = RefCell::new(PoolItem::new(size_item, None));

        PoolAllocator {
            storage,
            first_available: Cell::new(Some(0)),
        }
    }

    /// Returns an immutable reference to the vector of memory chunks used by the allocator.
    pub fn storage(&self) -> &Vec<RefCell<PoolItem>> {
        &self.storage
    }

    //TODO: not a &mut T, a Uniqueptr or SharedPtr or something like that.
    #[inline]
    pub fn alloc<T, F>(&self, op: F) -> AllocationResult<&mut T>
        where F: FnOnce() -> T
    {
        self.alloc_non_copy(op)
    }

    #[inline]
    fn alloc_non_copy<T, F>(&self, op: F) -> AllocationResult<&mut T>
        where F: FnOnce() -> T
    {
        unsafe {
            //Get the type description of the type T (get its vtable).
            let type_description = utils::get_type_description::<T>();

            //Ask the the first available memory chunk to give us raw pointers to memory locations
            //for our type description object.
            let (type_description_ptr, ptr) = self.alloc_non_copy_inner(mem::size_of::<T>(), mem::align_of::<T>())?;

            //Cast them.
            let type_description_ptr = type_description_ptr as *mut usize;
            let ptr = ptr as *mut T;

            //Write in our type description along with a bit indicating that the object has *not*
            //been initialized yet.
            *type_description_ptr = utils::bitpack_type_description_ptr(type_description, false);

            //Initialize the object.
            ptr::write(&mut (*ptr), op());

            //Now that we are done, update the type description to indicate that the object is there.
            *type_description_ptr = utils::bitpack_type_description_ptr(type_description, true);

            //TODO: not a &mut T, a UniquePtr or SharedPtr or something like that.
            Ok(&mut *ptr)
        }
    }

    #[inline]
    fn alloc_non_copy_inner(&self, n_bytes: usize, align: usize) -> AllocationResult<(*const u8, *const u8)> {
        //Check that a pool item is free.
        match self.first_available.get() {
            Some(index) => {
                //Borrow mutably the first pool item available in the pool allocator.
                let non_copy_storage = self.storage.get(index).unwrap().borrow_mut();

                //This chunk of memory is now in use, update the index of the first available chunk of memory.
                self.first_available.set(non_copy_storage.next());

                //Get the index of the first unused memory address.
                let fill = non_copy_storage.memory_chunk().fill();

                //Get the index of where we'll write the type description data
                //(the first unused byte in the memory chunk)
                let type_description_start = fill;

                //Get the index of where the object should reside (unaligned location actually).
                let after_type_description = fill + mem::size_of::<*const utils::TypeDescription>();

                //With the index to the unaligned memory address, determine the index to the aligned
                //memory address where the object will reside,
                //according to its memory alignment.
                let start = utils::round_up(after_type_description, align);

                //Determine the index of the next aligned memory address for a type description,
                //according to the size of the object and the memory alignment of a type description.
                let end = utils::round_up(start + n_bytes, mem::align_of::<*const utils::TypeDescription>());

                //If the allocator become oom after this possible allocation, abort the program.
                if end >= non_copy_storage.memory_chunk().capacity() {
                    return Err(AllocationError::OutOfMemoryError(format!("The memory chunk of the pool allocator doesn't have enough memory to hold this type !")));
                }

                //Update the current top of the stack. The first unused memory address is at
                //index 'end', where the next type description would be written if an allocation
                //was asked.
                non_copy_storage.memory_chunk().set_fill(end);

                unsafe {
                    //Get a raw pointer to the start of the RawVec of the MemoryChunk of the PoolItem. Yep.
                    let start_storage = non_copy_storage.memory_chunk().as_ptr();

                    Ok(
                        (
                            //From this raw pointer, get the correct raw pointers with the indices
                            //we calculated earlier.

                            //The raw pointer to the type description of the object.
                            start_storage.offset(type_description_start as isize),

                            //The raw pointer to the object
                            start_storage.offset(start as isize)
                        )
                    )
                }
            },
            None => {
                return Err(AllocationError::OutOfPoolError(format!("All the pools in the pool allocator were in use when the allocation was requested !")));
            },
        }
    }
}
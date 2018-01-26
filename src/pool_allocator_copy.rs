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

pub struct PoolAllocatorCopy {
    storage: Vec<RefCell<PoolItem>>,
    first_available: Cell<Option<usize>>,
}

impl PoolAllocatorCopy {
    pub fn new(nb_item: usize, size_item: usize) -> Self {
        let mut storage = Vec::with_capacity(nb_item);
        for i in 0..nb_item - 1 {
            storage[i] = RefCell::new(PoolItem::new(size_item, Some(i+1)));
        }

        storage[nb_item - 1] = RefCell::new(PoolItem::new(size_item, None));

        PoolAllocatorCopy {
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
    pub fn alloc<T: Copy, F>(&self, op: F) -> AllocationResult<&mut T>
        where F: FnOnce() -> T
    {
        self.alloc_copy(op)
    }

    #[inline]
    fn alloc_copy<T: Copy, F>(&self, op: F) -> AllocationResult<&mut T>
        where F: FnOnce() -> T
    {
        unsafe {
            //Get an aligned raw pointer to place the object in it.
            let ptr = self.alloc_copy_inner(mem::size_of::<T>(), mem::align_of::<T>())?;


            //cast this raw pointer to the type of the object
            let ptr = ptr as *mut T;

            //write the data in the memory location.
            ptr::write(&mut (*ptr), op());

            //TODO: not a &mut T, a UniquePtr or SharedPtr or something like that.
            Ok(&mut *ptr)
        }
    }

    #[inline]
    fn alloc_copy_inner(&self, n_bytes: usize, align: usize) -> AllocationResult<*const u8> {
        //Check that a pool item is free.
        match self.first_available.get() {
            Some(index) => {
                //Borrow mutably the first pool item available in the pool allocator.
                let copy_storage = self.storage.get(index).unwrap().borrow_mut();

                //This chunk of memory is now in use, update the index of the first available chunk of memory.
                self.first_available.set(copy_storage.next());

                //Get the index of the first unused memory address.
                let fill = copy_storage.memory_chunk().fill();
                //Get the index of the aligned memory address.
                let start = utils::round_up(fill, align);
                //Get the index of the future first unused memory address, according to the size of the object.
                let end = start + n_bytes;

                if end >= copy_storage.memory_chunk().capacity() {
                    return Err(AllocationError::OutOfMemoryError(format!("The memory chunk of the pool allocator doesn't have enough memory to hold this type !")));
                }

                //Set the first unused memory address of the memory chunk to the index calculated earlier.
                copy_storage.memory_chunk().set_fill(end);

                unsafe {
                    //Return the raw pointer to the aligned memory location, which will be used to place
                    //the object in the allocator.
                    Ok(copy_storage.memory_chunk().as_ptr().offset(start as isize))
                }
            },
            None => {
                return Err(AllocationError::OutOfPoolError(format!("All the pools in the pool allocator were in use when the allocation was requested !")));
            },
        }
    }
}

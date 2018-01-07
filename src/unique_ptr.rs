// Copyright 2017-2018 Maskerad Developers
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

use std::ptr::Unique;
use pool_allocator::PoolAllocator;
use std::marker;
use std::mem;

// We don't use Placer, Placer, InPlace, BoxPlacer or Boxed. Those API are just too shaky for the moment,
// (And it's pretty hard to make sense of it).

//TODO: ?Sized ? tester dans les unit tests avec un trait object.
/// A pointer type for allocation in memory pools.
///
/// `UniquePtr<T>` is basically a `Box<T>`. It provides unique ownership to a value from a pool,
/// and drop this value when it goes out of scope.
pub struct UniquePtr<'a, T> {
    ptr: Unique<T>,
    pool: &'a PoolAllocator,
    chunk_index: usize,
}

impl<'a, T> UniquePtr<'a, T> {
    pub unsafe fn from_raw(raw: *mut T, pool: &'a PoolAllocator, chunk_index: usize) -> Self {
        UniquePtr::from_unique(Unique::new_unchecked(raw), pool, chunk_index)
    }

    pub unsafe fn from_unique(unique_ptr: Unique<T>, pool: &'a PoolAllocator, chunk_index: usize) -> Self {
        UniquePtr {
            ptr: unique_ptr,
            pool,
            chunk_index,
        }
    }

    pub fn into_raw(ptr: UniquePtr<T>) -> *mut T {
        UniquePtr::into_unique(ptr).as_ptr()
    }

    pub fn into_unique(ptr: UniquePtr<T>) -> Unique<T> {
        let unique = ptr.ptr;
        mem::forget(ptr);
        unique
    }
}

impl<'a, T> Drop for UniquePtr<'a, T> {
    fn drop(&mut self) {
        //Get the current index of the first available pool item in the pool allocator.
        let current_first_available = self.pool.first_available();

        //Get the pool item, where the data inside the UniquePtr reside.
        let mut pool_item = self.pool.storage().get(self.chunk_index).unwrap().borrow_mut();

        //Modify the index to the next free pool item. The old first available pool item
        //is now "linked" to this pool item, which is now the nex first available pool item.
        pool_item.set_next(current_first_available);

        //This pool item becomes the first available pool item in the pool allocator.
        self.pool.set_first_available(Some(self.chunk_index));

        //drop the data inside the pool item's memory chunk.
        unsafe {
            pool_item.memory_chunk().destroy();
        }
    }
}
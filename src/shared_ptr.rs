// Copyright 2017-2018 Maskerad Developers
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

use std::ptr::Shared;
use std::cell::Cell;
use std::marker;
use std::ops;

use unique_ptr::UniquePtr;
use pool_allocator::{PoolAllocator, SharedUnique};

pub struct SharedPtr<'a, T: ?Sized> {
    ptr: Shared<SharedUnique<T>>,
    pool: &'a PoolAllocator,
    chunk_index: usize,
    phantom: marker::PhantomData<T>,
}

pub struct WeakPtr<T: ?Sized> {
    ptr: Shared<SharedUnique<T>>
}

impl<'a, T: ?Sized> !marker::Send for SharedPtr<'a, T> {}

impl<'a, T: ?Sized> !marker::Sync for SharedPtr<'a, T> {}

impl<'a, T: ?Sized> SharedPtr<'a, T> {
    pub unsafe fn from_raw(ptr: *mut SharedUnique<T>, pool: &'a PoolAllocator, chunk_index: usize) -> Self {
        SharedPtr {
            ptr: Shared::new_unchecked(ptr),
            pool,
            chunk_index,
            phantom: marker::PhantomData,
        }
    }

    pub fn downgrade(this: &Self) -> WeakPtr<T> {
        this.inc_weak();
        WeakPtr {
            ptr: this.ptr
        }
    }

    pub fn weak_count(this: &Self) -> usize {
        this.weak() - 1
    }

    pub fn strong_count(this: &Self) -> usize {
        this.strong()
    }

    fn is_unique(this: &Self) -> bool {
        SharedPtr::weak_count(this) == 0 && SharedPtr::strong_count(this) == 1
    }

    pub fn get_mut(this: &mut Self) -> Option<&mut T> {
        if SharedPtr::is_unique(this) {
            unsafe {
                Some(&mut this.ptr.as_mut().value)
            }
        } else {
            None
        }
    }

    pub fn ptr_eq(this: &Self, other: &Self) -> bool {
        this.ptr.as_ptr() == other.ptr.as_ptr()
    }
}

impl<'a, T: Sized> ops::Deref for SharedPtr<'a, T> {
    type Target = T;

    #[inline(always)]
    fn deref(&self) -> &T {
        &self.inner().value
    }
}

//TODO: use needs_drop, to know if we should use destroy to drop the SharedPtr.
impl<'a, T: ?Sized> Drop for SharedPtr<'a, T> {
    fn drop(&mut self) {
        let ptr = self.ptr.as_ptr();

        self.dec_strong();
        if self.strong() == 0 {
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
                pool_item.memory_chunk().destroy(); }
        }
    }
}

impl<'a, T: ?Sized> Clone for SharedPtr<'a, T> {
    fn clone(&self) -> SharedPtr<'a, T> {
        self.inc_strong();
        //Shared is Copy.
        SharedPtr {
            ptr: self.ptr,
            pool: self.pool,
            chunk_index: self.chunk_index,
            phantom: marker::PhantomData,
        }
    }
}

impl<'a, T: ?Sized + PartialEq> PartialEq for SharedPtr<'a, T> {
    fn eq(&self, other: &SharedPtr<T>) -> bool {
        **self == **other
    }

    fn ne(&self, other: &SharedPtr<T>) -> bool {
        **self != **other
    }
}

impl<'a, T: ?Sized + Eq> Eq for SharedPtr<'a, T> {}

//PartialOrd
// Copyright 2017-2018 Maskerad Developers
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

/*


use std::ptr::NonNull;
use std::cell::Cell;
use std::marker;
use std::ops;
use std::cmp::Ordering;
use std::hash::{Hash, Hasher};
use std::fmt;
use std::intrinsics::abort;

use pools::pool_allocator::PoolAllocator;

pub struct SharedUnique<T: ?Sized> {
    pub strong: Cell<usize>,
    pub weak: Cell<usize>,
    pub value: T,

}




pub struct SharedPtr<'a, T: ?Sized> {
    ptr: NonNull<SharedUnique<T>>,
    pool: &'a PoolAllocator,
    chunk_index: usize,
    should_drop: bool, //This is mega lame.
    phantom: marker::PhantomData<T>,
}

impl<'a, T: ?Sized> !marker::Send for SharedPtr<'a, T> {}

impl<'a, T: ?Sized> !marker::Sync for SharedPtr<'a, T> {}

impl<'a, T: ?Sized> SharedPtr<'a, T> {
    pub unsafe fn from_raw(ptr: *mut SharedUnique<T>, pool: &'a PoolAllocator, chunk_index: usize, should_drop: bool) -> Self {
        SharedPtr {
            ptr: NonNull::new_unchecked(ptr),
            pool,
            chunk_index,
            should_drop,
            phantom: marker::PhantomData,
        }
    }

    pub fn downgrade(this: &Self) -> WeakPtr<T> {
        this.inc_weak();
        WeakPtr {
            ptr: this.ptr,
            pool: this.pool,
            chunk_index: this.chunk_index,
            should_drop: this.should_drop,
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

impl<'a, T: ?Sized> ops::Deref for SharedPtr<'a, T> {
    type Target = T;

    #[inline(always)]
    fn deref(&self) -> &T {
        &self.inner().value
    }
}

impl<'a, T: ?Sized> Drop for SharedPtr<'a, T> {
    fn drop(&mut self) {

        self.dec_strong();
        if self.strong() == 0 {

            if self.should_drop {
                //Get the current index of the first available pool item in the pool allocator.
                let current_first_available = self.pool.first_available();

                //Get the pool item, where the data inside the UniquePtr reside.
                let mut pool_item = self.pool.storage().get(self.chunk_index).unwrap().borrow_mut();

                //Modify the index to the next free pool item. The old first available pool item
                //is now "linked" to this pool item, which is now the nex first available pool item.
                pool_item.set_next(current_first_available);

                //This pool item becomes the first available pool item in the pool allocator.
                self.pool.set_first_available(Some(self.chunk_index));

                unsafe {
                    pool_item.memory_chunk().destroy();
                    //drop the data inside the pool item's memory chunk.
                    pool_item.memory_chunk().set_fill(0);
                }

            } else {

                //Get the current index of the first available pool item in the pool allocator.
                let current_first_available = self.pool.first_available_copy();

                //Get the pool item, where the data inside the UniquePtr reside.
                let mut pool_item = self.pool.storage_copy().get(self.chunk_index).unwrap().borrow_mut();

                //Modify the index to the next free pool item. The old first available pool item
                //is now "linked" to this pool item, which is now the nex first available pool item.
                pool_item.set_next(current_first_available);

                //This pool item becomes the first available pool item in the pool allocator.
                self.pool.set_first_available_copy(Some(self.chunk_index));

                unsafe {
                    //drop the data inside the pool item's memory chunk.
                    pool_item.memory_chunk().set_fill(0);

                }
            }
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
            should_drop: self.should_drop,
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

impl<'a, T: ?Sized +PartialOrd> PartialOrd for SharedPtr<'a, T> {
    fn partial_cmp(&self, other: &SharedPtr<T>) -> Option<Ordering> {
        (**self).partial_cmp(&**other)
    }

    fn lt(&self, other: &SharedPtr<T>) -> bool {
        **self < **other
    }

    fn le(&self, other: &SharedPtr<T>) -> bool {
        **self <= **other
    }

    fn gt(&self, other: &SharedPtr<T>) -> bool {
        **self > **other
    }

    fn ge(&self, other: &SharedPtr<T>) -> bool {
        **self >= **other
    }
}

impl<'a, T: ?Sized + Ord> Ord for SharedPtr<'a, T> {
    fn cmp(&self, other: &SharedPtr<T>) -> Ordering {
        (**self).cmp(&**other)
    }
}

impl<'a, T: ?Sized + Hash> Hash for SharedPtr<'a, T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        (**self).hash(state);
    }
}

impl<'a, T: ?Sized + fmt::Display> fmt::Display for SharedPtr<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}

impl<'a, T: ?Sized + fmt::Debug> fmt::Debug for SharedPtr<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<'a, T: ?Sized> fmt::Pointer for SharedPtr<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Pointer::fmt(&(&**self as *const T), f)
    }
}

impl<'a, T: ?Sized + marker::Unsize<U>, U: ?Sized> ops::CoerceUnsized<SharedPtr<'a, U>> for SharedPtr<'a, T> {}



pub struct WeakPtr<'a, T: ?Sized> {
    ptr: NonNull<SharedUnique<T>>,
    pool: &'a PoolAllocator,
    chunk_index: usize,
    should_drop: bool, //This is ultra lame.
}

impl<'a, T: ?Sized> !marker::Send for WeakPtr<'a, T> {}

impl<'a, T:?Sized> !marker::Sync for WeakPtr<'a, T> {}

impl<'a, T: ?Sized> WeakPtr<'a, T> {
    pub fn upgrade(&self) -> Option<SharedPtr<T>> {
        if self.strong() == 0 {
            None
        } else {
            self.inc_strong();
            Some(SharedPtr {
                ptr: self.ptr,
                pool: self.pool,
                chunk_index: self.chunk_index,
                should_drop: self.should_drop,
                phantom: marker::PhantomData,
            })
        }
    }
}

impl<'a, T: ?Sized> Drop for WeakPtr<'a, T> {
    fn drop(&mut self) {
        self.dec_weak();
    }
}

impl<'a, T: ?Sized> Clone for WeakPtr<'a, T> {
    fn clone(&self) -> WeakPtr<'a, T> {
        self.inc_weak();
        WeakPtr {
            ptr: self.ptr,
            pool: self.pool,
            chunk_index: self.chunk_index,
            should_drop: self.should_drop,
        }
    }
}

impl<'a, T: ?Sized + fmt::Debug> fmt::Debug for WeakPtr<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "(Weak)")
    }
}

trait SharedUniquePtr<T: ?Sized> {
    fn inner(&self) -> &SharedUnique<T>;

    fn strong(&self) -> usize {
        self.inner().strong.get()
    }

    fn inc_strong(&self) {
        self.inner().strong.set(self.strong().checked_add(1).unwrap_or_else(|| unsafe { abort() }));
    }

    fn dec_strong(&self) {
        self.inner().strong.set(self.strong() - 1);
    }

    fn weak(&self) -> usize {
        self.inner().weak.get()
    }

    fn inc_weak(&self) {
        self.inner().weak.set(self.weak().checked_add(1).unwrap_or_else(|| unsafe { abort() }));
    }

    fn dec_weak(&self) {
        self.inner().weak.set(self.weak() - 1);
    }
}

impl<'a, T: ?Sized> SharedUniquePtr<T> for SharedPtr<'a, T> {
    fn inner(&self) -> &SharedUnique<T> {
        unsafe {
            self.ptr.as_ref()
        }
    }
}

impl<'a, T: ?Sized> SharedUniquePtr<T> for WeakPtr<'a, T> {
    fn inner(&self) -> &SharedUnique<T> {
        unsafe {
            self.ptr.as_ref()
        }
    }
}

impl<'a, T: ?Sized + marker::Unsize<U>, U: ?Sized> ops::CoerceUnsized<WeakPtr<'a, U>> for WeakPtr<'a, T> {}

*/

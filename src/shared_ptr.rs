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
use std::cmp::Ordering;
use std::hash::{Hash, Hasher};
use std::fmt;
use std::intrinsics::abort;
use unique_ptr::UniquePtr;

use pool_allocator::PoolAllocator;

/// A wrapper around a `T`, which add the necessary fields for reference-counting logic.
pub struct SharedUnique<T: ?Sized> {
    pub strong: Cell<usize>,
    pub weak: Cell<usize>,
    pub value: T,
}



/// A single-threaded reference-counting pointer. It is basically an `Rc<T>`.
///
/// Since the pool is not global, the smart pointer have to keep a reference to the pool and, to be able
/// to tell which chunk of memory to drop, it have to keep the index of the chunk used to allocate `T`.
pub struct SharedPtr<T: ?Sized> {
    ptr: Shared<SharedUnique<T>>,
    pool_index: u8,
    chunk_index: usize,
    phantom: marker::PhantomData<T>,
}

impl<T: ?Sized> !marker::Send for SharedPtr<T> {}

impl<T: ?Sized> !marker::Sync for SharedPtr<T> {}

impl<T: ?Sized> SharedPtr<T> {

    /// Constructs an `Rc` from a raw pointer coming from the pool.
    ///
    /// This function is unsafe because improper use may lead to memory problems. For example, a
    /// double-free may occur if the function is called twice on the same raw pointer.
    pub unsafe fn from_raw(ptr: *mut SharedUnique<T>, pool_index: u8, chunk_index: usize) -> Self {
        SharedPtr {
            ptr: Shared::new_unchecked(ptr),
            pool_index,
            chunk_index,
            phantom: marker::PhantomData,
        }
    }
    /// Creates a new `WeakPtr` pointer to this value.
    pub fn downgrade(this: &Self) -> WeakPtr<T> {
        this.inc_weak();
        WeakPtr {
            ptr: this.ptr,
            pool_index: this.pool_index,
            chunk_index: this.chunk_index,
        }
    }

    /// Gets the number of `WeakPtr` pointers to this value.
    pub fn weak_count(this: &Self) -> usize {
        this.weak() - 1
    }
    /// Gets the number of `SharedPtr` pointers to this value.
    pub fn strong_count(this: &Self) -> usize {
        this.strong()
    }

    /// Returns true if there are no other `SharedPtr` or `WeakPtr` pointers to
    /// this inner value.
    fn is_unique(this: &Self) -> bool {
        SharedPtr::weak_count(this) == 0 && SharedPtr::strong_count(this) == 1
    }

    /// Returns a mutable reference to the inner value, if there are
    /// no other `SharedPtr` or `WeakPtr` pointers to the same value.
    ///
    /// Returns `None` otherwise, because it is not safe to
    /// mutate a shared value.
    pub fn get_mut(this: &mut Self) -> Option<&mut T> {
        if SharedPtr::is_unique(this) {
            unsafe {
                Some(&mut this.ptr.as_mut().value)
            }
        } else {
            None
        }
    }

    /// Returns true if the two `SharedPtr`s point to the same value (not
    /// just values that compare as equal).
    pub fn ptr_eq(this: &Self, other: &Self) -> bool {
        this.ptr.as_ptr() == other.ptr.as_ptr()
    }
}

impl<T: ?Sized> ops::Deref for SharedPtr<T> {
    type Target = T;

    #[inline(always)]
    fn deref(&self) -> &T {
        &self.inner().value
    }
}

//TODO: use needs_drop, to know if we should use destroy to drop the SharedPtr.
impl<T: ?Sized> Drop for SharedPtr<T> {

    /// Drops the `SharedPtr`.
    ///
    /// This will decrement the strong reference count. If the strong reference
    /// count reaches zero then the only other references (if any) are
    /// `WeakPtr`, so we `drop` the inner value by asking the pool to drop the content of the
    /// memory chunk used by the pool allocator to allocate the object.
    /// This chunk become the first available chunk for the pool allocator, it means that the pool will use this chunk
    /// when the next allocation occurs.
    fn drop(&mut self) {
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
                pool_item.memory_chunk().destroy();
                //drop the data inside the pool item's memory chunk.
                pool_item.memory_chunk().set_fill(0);

            }
        }
    }
}

impl<T: ?Sized> Clone for SharedPtr<T> {

    /// Makes a clone of the `SharedPtr` pointer.
    ///
    /// This creates another pointer to the same inner value, increasing the
    /// strong reference count.
    fn clone(&self) -> SharedPtr<T> {
        self.inc_strong();
        //Shared is Copy.
        SharedPtr {
            ptr: self.ptr,
            pool_index: self.pool_index,
            chunk_index: self.chunk_index,
            phantom: marker::PhantomData,
        }
    }
}

impl<T: ?Sized + PartialEq> PartialEq for SharedPtr<T> {
    fn eq(&self, other: &SharedPtr<T>) -> bool {
        **self == **other
    }

    fn ne(&self, other: &SharedPtr<T>) -> bool {
        **self != **other
    }
}

impl<T: ?Sized + Eq> Eq for SharedPtr<T> {}

impl<T: ?Sized +PartialOrd> PartialOrd for SharedPtr<T> {
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

impl<T: ?Sized + Ord> Ord for SharedPtr<T> {
    fn cmp(&self, other: &SharedPtr<T>) -> Ordering {
        (**self).cmp(&**other)
    }
}

impl<T: ?Sized + Hash> Hash for SharedPtr<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        (**self).hash(state);
    }
}

impl<T: ?Sized + fmt::Display> fmt::Display for SharedPtr<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for SharedPtr<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<T: ?Sized> fmt::Pointer for SharedPtr<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Pointer::fmt(&(&**self as *const T), f)
    }
}

impl<T: ?Sized + marker::Unsize<U>, U: ?Sized> ops::CoerceUnsized<SharedPtr<U>> for SharedPtr<T> {}


/// `WeakPtr` is a version of `SharedPtr` that holds a non-owning reference to the
/// managed value.
///
/// The value is accessed by calling `upgrade` on the `WeakPtr`
/// pointer, which returns an `Option<SharedPtr<T>>`.
///
/// Since a `WeakPtr` reference does not count towards ownership, it will not
/// prevent the inner value from being dropped, and `WeakPtr` itself makes no
/// guarantees about the value still being present and may return `None` when
/// calling `upgrade`.
///
/// A `WeakPtr` pointer is useful for keeping a temporary reference to the value
/// within `SharedPtr` without extending its lifetime. It is also used to prevent
/// circular references between `SharedPtr` pointers, since mutual owning references
/// would never allow either `SharedPtr` to be dropped. For example, a tree could
/// have strong `SharedPtr` pointers from parent nodes to children, and `WeakPtr`
/// pointers from children back to their parents.
///
/// The typical way to obtain a `WeakPtr` pointer is to call `SharedPtr::downgrade`.
///
/// Since the pool is not global, the smart pointer have to keep a reference to the pool and, to be able
/// to tell which chunk of memory to drop, it have to keep the index of the chunk used to allocate `T`.
pub struct WeakPtr<T: ?Sized> {
    ptr: Shared<SharedUnique<T>>,
    pool_index: u8,
    chunk_index: usize,
}

impl<T: ?Sized> !marker::Send for WeakPtr<T> {}

impl<T:?Sized> !marker::Sync for WeakPtr<T> {}

impl<T: ?Sized> WeakPtr<T> {
    /// Attempts to upgrade the `WeakPtr` pointer to a `SharedPtr`, extending
    /// the lifetime of the value if successful.
    ///
    /// Returns `None` if the value has since been dropped.
    pub fn upgrade(&self) -> Option<SharedPtr<T>> {
        if self.strong() == 0 {
            None
        } else {
            self.inc_strong();
            Some(SharedPtr {
                ptr: self.ptr,
                pool_index: self.pool_index,
                chunk_index: self.chunk_index,
                phantom: marker::PhantomData,
            })
        }
    }
}


impl<T: ?Sized> Drop for WeakPtr<T> {
    /// Drops the `WeakPtr` pointer.
    fn drop(&mut self) {
        self.dec_weak();
    }
}

impl<T: ?Sized> Clone for WeakPtr<T> {
    /// Makes a clone of the `WeakPtr` pointer that points to the same value.
    fn clone(&self) -> WeakPtr<T> {
        self.inc_weak();
        WeakPtr {
            ptr: self.ptr,
            pool_index: self.pool_index,
            chunk_index: self.chunk_index,
        }
    }
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for WeakPtr<T> {
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

impl<T: ?Sized> SharedUniquePtr<T> for SharedPtr<T> {
    fn inner(&self) -> &SharedUnique<T> {
        unsafe {
            self.ptr.as_ref()
        }
    }
}

impl<T: ?Sized> SharedUniquePtr<T> for WeakPtr<T> {
    fn inner(&self) -> &SharedUnique<T> {
        unsafe {
            self.ptr.as_ref()
        }
    }
}

impl<T: ?Sized + marker::Unsize<U>, U: ?Sized> ops::CoerceUnsized<WeakPtr<U>> for WeakPtr<T> {}

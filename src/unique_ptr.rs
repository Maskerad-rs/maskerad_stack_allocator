// Copyright 2017-2018 Maskerad Developers
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

use std::ptr::Unique;
use pool_allocator::PoolAllocator;
use std::hash::{Hasher, Hash};
use std::cmp::Ordering;
use std::ops::{DerefMut, Deref, CoerceUnsized};
use std::borrow;
use std::fmt;
use allocation_error::AllocationResult;
use std::marker::Unsize;
use allocator_hub::POOL_ALLOCATOR_HUB;

//TODO: ?Sized ? tester dans les unit tests avec un trait object.
/// A pointer type for allocation in memory pools.
///
/// `UniquePtr<T>` is basically a `Box<T>`. It provides unique ownership to a value from a pool,
/// and drop this value when it goes out of scope.
///
/// Since the pool is not global, the smart pointer have to keep a reference to the pool and, to be able
/// to tell which chunk of memory to drop, it have to keep the index of the chunk used to allocate `T`.
pub struct UniquePtr<T: ?Sized> {
    ptr: Unique<T>,
    pool_index: u8,
    chunk_index: usize,
}

impl<T: ?Sized> UniquePtr<T> {
    /// Constructs a unique Pointer from a raw pointer.
    ///
    /// After calling this function, the raw pointer is owned by the
    /// resulting `UniquePtr`. Specifically, the `UniquePtr` destructor will call
    /// the pool to drop the object.
    ///
    /// This function is unsafe because improper use may lead to
    /// memory problems. For example, a double-free may occur if the
    /// function is called twice on the same raw pointer.
    pub unsafe fn from_raw(raw: *mut T, pool_index: u8, chunk_index: usize) -> Self {
        UniquePtr::from_unique(Unique::new_unchecked(raw), pool_index, chunk_index)
    }

    /// Constructs a `UniquePtr` from a `Unique<T>` pointer.
    ///
    /// After calling this function, the memory is owned by a `UniquePtr` and `T` can
    /// then be destroyed and released upon drop.
    ///
    /// # Safety
    ///
    /// A `Unique<T>` can be safely created via `Unique::new` and thus doesn't
    /// necessarily own the data pointed to nor is the data guaranteed to live
    /// as long as the pointer.
    pub unsafe fn from_unique(ptr: Unique<T>, pool_index: u8, chunk_index: usize) -> Self {
        UniquePtr {
            ptr,
            pool_index,
            chunk_index,
        }
    }

}

//TODO HERE.
impl<T: ?Sized> Drop for UniquePtr<T> {
    /// When the `UniquePtr<T>` is dropped, it tells the pool to drop the content of the memory chunk used to allocate
    /// `T`. This chunk become the first available chunk for the pool allocator, it means that the pool will use this chunk
    /// when the next allocation occurs.
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


        unsafe {
            //drop the data inside the pool item's memory chunk.
            pool_item.memory_chunk().destroy();
            //Set the index of the first unused byte in the memory chunk to 0.
            pool_item.memory_chunk().set_fill(0);
        }

    }
}


impl<T: Clone> Clone for UniquePtr<T> {
    /// Returns a new unique pointer with a `clone()` of this UniquePtr's contents.
    fn clone(&self) -> UniquePtr<T> {
        self.pool.alloc_unique(|| {
            (**self).clone()
        }).unwrap()
    }
    /// Copies `source`'s contents into `self` without creating a new allocation.
    fn clone_from(&mut self, source: &UniquePtr<T>) {
        (**self).clone_from(&(**source));
    }
}

impl<T: ?Sized + PartialEq> PartialEq for UniquePtr<T> {
    #[inline]
    fn eq(&self, other: &UniquePtr<T>) -> bool {
        PartialEq::eq(&**self, &**other)
    }
    #[inline]
    fn ne(&self, other: &UniquePtr<T>) -> bool {
        PartialEq::ne(&**self, &**other)
    }
}

impl<T: ?Sized + PartialOrd> PartialOrd for UniquePtr<T> {
    #[inline]
    fn partial_cmp(&self, other: &UniquePtr<T>) -> Option<Ordering> {
        PartialOrd::partial_cmp(&**self, &**other)
    }
    #[inline]
    fn lt(&self, other: &UniquePtr<T>) -> bool {
        PartialOrd::lt(&**self, &**other)
    }
    #[inline]
    fn le(&self, other: &UniquePtr<T>) -> bool {
        PartialOrd::le(&**self, &**other)
    }
    #[inline]
    fn ge(&self, other: &UniquePtr<T>) -> bool {
        PartialOrd::ge(&**self, &**other)
    }
    #[inline]
    fn gt(&self, other: &UniquePtr<T>) -> bool {
        PartialOrd::gt(&**self, &**other)
    }
}

impl<T: ?Sized + Ord> Ord for UniquePtr<T> {
    #[inline]
    fn cmp(&self, other: &UniquePtr<T>) -> Ordering {
        Ord::cmp(&**self, &**other)
    }
}

impl<T: ?Sized + Eq> Eq for UniquePtr<T> {}


impl<T: ?Sized + Hash> Hash for UniquePtr<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        (**self).hash(state);
    }
}

impl<T: ?Sized + Hasher> Hasher for UniquePtr<T> {
    fn finish(&self) -> u64 {
        (**self).finish()
    }
    fn write(&mut self, bytes: &[u8]) {
        (**self).write(bytes)
    }
    fn write_u8(&mut self, i: u8) {
        (**self).write_u8(i)
    }
    fn write_u16(&mut self, i: u16) {
        (**self).write_u16(i)
    }
    fn write_u32(&mut self, i: u32) {
        (**self).write_u32(i)
    }
    fn write_u64(&mut self, i: u64) {
        (**self).write_u64(i)
    }
    fn write_u128(&mut self, i: u128) {
        (**self).write_u128(i)
    }
    fn write_usize(&mut self, i: usize) {
        (**self).write_usize(i)
    }
    fn write_i8(&mut self, i: i8) {
        (**self).write_i8(i)
    }
    fn write_i16(&mut self, i: i16) {
        (**self).write_i16(i)
    }
    fn write_i32(&mut self, i: i32) {
        (**self).write_i32(i)
    }
    fn write_i64(&mut self, i: i64) {
        (**self).write_i64(i)
    }
    fn write_i128(&mut self, i: i128) {
        (**self).write_i128(i)
    }
    fn write_isize(&mut self, i: isize) {
        (**self).write_isize(i)
    }
}

impl<T: fmt::Display + ?Sized> fmt::Display for UniquePtr<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}

impl<T: fmt::Debug + ?Sized> fmt::Debug for UniquePtr<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<T: ?Sized> fmt::Pointer for UniquePtr<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // It's not possible to extract the inner Unique directly from the UniquePtr,
        // instead we cast it to a *const which aliases the Unique
        let ptr: *const T = &**self;
        fmt::Pointer::fmt(&ptr, f)
    }
}

impl<T: ?Sized> Deref for UniquePtr<T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe {
            self.ptr.as_ref()
        }
    }
}


impl<T: ?Sized> DerefMut for UniquePtr<T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe {
            self.ptr.as_mut()
        }
    }
}

impl<T: ?Sized> borrow::Borrow<T> for UniquePtr<T> {
    fn borrow(&self) -> &T {
        &**self
    }
}

impl<'a, T: ?Sized> borrow::BorrowMut<T> for UniquePtr<T> {
    fn borrow_mut(&mut self) -> &mut T {
        &mut **self
    }
}

impl<T: ?Sized> AsRef<T> for UniquePtr<T> {
    fn as_ref(&self) -> &T {
        &**self
    }
}

impl<T: ?Sized> AsMut<T> for UniquePtr<T> {
    fn as_mut(&mut self) -> &mut T {
        &mut **self
    }
}

impl<T: ?Sized + Unsize<U>, U: ?Sized> CoerceUnsized<UniquePtr<U>> for UniquePtr<T> {}
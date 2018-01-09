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
use pool_allocator::PoolAllocator;

pub struct SharedPtr<'a, T: ?Sized> {
    ptr: Shared<SharedUnique<T>>,
    pool: &'a PoolAllocator,
    chunk_index: usize,
    phantom: marker::PhantomData<T>,
}

impl<'a, T: ?Sized> !marker::Send for SharedPtr<'a, T> {}

impl<'a, T: ?Sized> !marker::Sync for SharedPtr<'a, T> {}

impl<'a, T: ?Sized> SharedPtr<'a, T> {
    pub fn from_unique_ptr(ptr: UniquePtr<T>) -> Self {

    }
}

//TODO.
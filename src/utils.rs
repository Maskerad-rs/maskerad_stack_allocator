// Copyright 2017 Maskerad Developers
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

use std::mem;

// HACK(eddyb) TyDesc replacement using a trait object vtable.
// This could be replaced in the future with a custom DST layout,
// or `&'static (drop_glue, size, align)` created by a `const fn`.
// Requirements:
// * rvalue promotion (issue #1056)
// * mem::{size_of, align_of} must be const fns
pub struct TypeDescription {
    pub drop_glue: fn(*const i8),
    pub size: usize,
    pub alignment: usize,
}

pub unsafe fn get_type_description<T>() -> *const TypeDescription {
    use std::raw::TraitObject;

    //Unique::empty(), or Shared::empty() ?
    //heap::EMPTY is deprecated.
    //Unique and Shared are almost the same, Unique has a ptr AND a phantomData field.
    //A Unique<T> behave has if it was a T, and the owner of the Unique<T> owns what is referred
    //by the Unique<T>.

    //For our use case, i think a Unique is not needed. We just want a non-null dangling pointer.
    //There's a problem Shared/Unique::empty() is implemented for T: Sized...

    //TODO: be careful with this. Not sure at all if it's the way to do it.
    let ptr = &*(mem::align_of::<T>() as *const T);

    //Can use any trait that is implemented for all types
    let obj = mem::transmute::<&AllTypes, TraitObject>(ptr);
    obj.vtable as *const TypeDescription
}

// We encode whether the object a tydesc describes has been
// initialized in the arena in the low bit of the tydesc pointer. This
// is necessary in order to properly do cleanup if a panic occurs
// during an initializer.
#[inline]
pub fn bitpack_type_description_ptr(p: *const TypeDescription, is_done: bool) -> usize {
    p as usize | (is_done as usize)
}
#[inline]
pub fn un_bitpack_type_description_ptr(p: usize) -> (*const TypeDescription, bool) {
    ((p & !1) as *const TypeDescription, p & 1 == 1)
}
#[inline]
pub fn round_up(base: usize, align: usize) -> usize {
    (base.checked_add(align - 1)).unwrap() & !(align - 1)
}

trait AllTypes {
    fn dummy(&self) {}
}

impl<T: ?Sized> AllTypes for T {}
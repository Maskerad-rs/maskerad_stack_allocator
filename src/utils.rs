// Copyright 2017-2018 Maskerad Developers
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
/// A Structure holding the function pointer to the drop implementation, the size and the alignment
/// of a given type T.
pub struct TypeDescription {
    pub drop_glue: fn(*const i8),
    pub size: usize,
    pub alignment: usize,
}

/// Returns a constant raw pointer to a TypeDescription structure, from a given T.
///
/// # implementation details
///
/// This function create a raw pointer to the T type, and transmute it to a [TraitObject](https://doc.rust-lang.org/std/raw/struct.TraitObject.html)
/// structure.
///
/// The virtual table (*mut ()) of this pointer is then cast to a constant raw pointer to a TypeDescription structure, and returned.
pub unsafe fn get_type_description<T>() -> *const TypeDescription {
    use std::raw::TraitObject;

    debug!("Getting the vtable of a type.");
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
/// Encode whether the object (described by a TypeDescription) has been initialized in the StackAllocator
/// in the low bit of the TypeDescription pointer.
///
/// This is necessary in order to properly do cleanup if a panic occurs during an initializer.
#[inline]
pub fn bitpack_type_description_ptr(p: *const TypeDescription, is_done: bool) -> usize {
    debug!("Encoding the 'is_done' state in the low bit of a TypeDescription");
    trace!("'is_done' state: {}", is_done);
    p as usize | (is_done as usize)
}

/// Decode the given memory location, extracting:
///
/// - the TypeDescription of the object residing in this memory location.
///
/// - Whether or not the object has been initialized.
///
///
/// This is reciprocal of bitpack_type_description_ptr.

#[inline]
pub fn un_bitpack_type_description_ptr(p: usize) -> (*const TypeDescription, bool) {
    debug!("Decoding a memory location, getting a type description and a 'is_done' state.");
    ((p & !1) as *const TypeDescription, p & 1 == 1)
}

/// Returns an index to an aligned memory location, given a starting memory location index and an alignment.
///
/// # Explanation
///
/// Every data has an alignment requirement:
///
/// - **8-bit data** (1 bytes, u8 for example) can be aligned to every address in memory.
///
/// - **32-bit data** (4 bytes, u32 for example) must be 4-byte aligned. Its memory address must finish
/// with 0x0, 0x4, 0x8 or 0xC.
///
/// - **128-bit data** (16 bytes) must be 16-byte aligned. Its memory address must finish with 0x0.
///
///
/// To return **aligned** memory blocks, you just allocate a little bit more memory than requested,
/// adjust the address of the memory block upward, and return the address. Even with the small upward offset,
/// the returned block of memory will still be large enough, since we allocated a bit more memory than requested.
///
/// In general, the number of additional bytes allocated equals the alignment of the data.
///
/// To know the amount by which the block of memory must be adjusted we:
///
/// - create a mask: alignment - 1.
///
/// - mask the least significant byte of the original memory address, to get the misalignment: original_address & mask.
///
/// - calculate the adjustment, according to the misalignment: alignment - misalignment.
///
/// - Add the adjustment to the original memory address, to get an aligned memory location: original_address + adjustment.
///
/// # Example
///
/// original_address: 0x60758912.
///
/// alignment: 4 = 0x00000004. **4-byte** aligned data.
///
/// mask: 4 - 1 = 3 = 0x00000003.
///
/// misalignment: 0x60758912 & 0x00000003 = 0x00000002.
///
/// adjustment: 0x00000004 - 0x00000002 = 0x00000002.
///
/// aligned_address: 0x60758912 + 0x00000002 = 0x60758914.
///
/// **4-byte** aligned data must reside in memory addresses finishing by 0x0, 0x4, 0x8 and 0xC. Our
/// aligned_address is properly aligned !
///
#[inline]
pub fn round_up(base: usize, align: usize) -> usize {
    debug!("Getting an aligned memory location, according to the memory location {:x} and an alignment need of {} bytes", base, align);
    //(base.checked_add(align - 1)).unwrap() & !(align - 1)
    //The solution above works, but our solution is easier to understand and faster.

    let misalignment = base & (align - 1);
    trace!("Misaligned by: {:x}", misalignment);
    let adjustment = align - misalignment;
    trace!("Must be aligned by: {:x}", adjustment);
    trace!("aligned memory location: {:x}", base + adjustment);

    base + adjustment
}

trait AllTypes {
    fn dummy(&self) {}
}

impl<T: ?Sized> AllTypes for T {}

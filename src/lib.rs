// Copyright 2017-2018 Maskerad Developers
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

//! This library provides:
//!
//! - a **stack-based** allocator.
//!
//! - a **double-ended** allocator.
//!
//! - a **double-buffered** allocator.
//!
//! Its primary purpose is to prevent memory fragmentation.
//!
//! This is a **nightly-only** library (last rust nightly version tested: **1.25**).



#![feature(alloc)]
//#![feature(offset_to)]
//#![feature(allocator_api)]
#![feature(raw)]
//#![feature(heap_api)]
#![feature(core_intrinsics)]
//#![feature(shared)]
//#![feature(unique)]
//#![feature(i128_type)]
//#![feature(i128)]
//#![feature(optin_builtin_traits)]
//#![feature(coerce_unsized)]
//#![feature(unsize)]

extern crate alloc;
extern crate core;

mod stacks;
mod smart_pointers;
mod pools;

pub mod memory_chunk;
pub mod allocation_error;
pub mod utils;

pub use stacks::stack_allocator::StackAllocator;
pub use stacks::double_ended_allocator::DoubleEndedStackAllocator;
pub use stacks::double_buffered_allocator::DoubleBufferedAllocator;


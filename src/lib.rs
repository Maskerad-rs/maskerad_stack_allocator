// Copyright 2017 Maskerad Developers
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

#![feature(alloc)]
#![feature(offset_to)]
#![feature(allocator_api)]

extern crate alloc;
extern crate core;

mod stack_allocator;
mod double_buffered_allocator;

pub use stack_allocator::StackAllocator;
pub use double_buffered_allocator::DoubleBufferedAllocator;

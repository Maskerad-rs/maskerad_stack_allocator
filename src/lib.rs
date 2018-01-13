// Copyright 2017-2018 Maskerad Developers
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

//! This library provides:
//!
//! - a **stack-based** allocator for data implementing the **Drop** trait,
//!
//! - a **double-ended** allocator for data implementing the **Drop** trait,
//!
//! - a **double-buffered** allocator for data implementing the **Drop** trait,
//!
//!
//! - a **stack-based** allocator for data implementing the **Copy** trait,
//!
//! - a **double-ended** allocator for data implementing the **Copy** trait,
//!
//! - a **double-buffered** allocator for data implementing the **Copy** trait,
//!
//!
//! - a **pool** allocator for data implementing the **Copy** trait,
//!
//! - a **pool** allocator for data implementing the **Drop** trait,
//!
//! All those allocators are for single-threaded scenarios, and their primary purpose is to prevent memory fragmentation.
//!
//! This is a **nightly-only** library.



#![feature(alloc)]
#![feature(offset_to)]
#![feature(allocator_api)]
#![feature(raw)]
#![feature(heap_api)]
#![feature(core_intrinsics)]
#![feature(shared)]
#![feature(unique)]
#![feature(i128_type)]
#![feature(i128)]
#![feature(optin_builtin_traits)]
#![feature(coerce_unsized)]
#![feature(unsize)]

extern crate alloc;
extern crate core;

mod stack_allocator;
mod double_buffered_allocator;
mod double_ended_allocator;
mod memory_chunk;
mod stack_allocator_copy;
mod double_buffered_allocator_copy;
mod double_ended_allocator_copy;
mod pool_allocator_copy;
mod pool_item;

mod pool_allocator;
mod unique_ptr;
mod shared_ptr;

pub mod allocation_error;
pub mod utils;

pub use stack_allocator::StackAllocator;
pub use stack_allocator_copy::StackAllocatorCopy;

pub use memory_chunk::MemoryChunk;
pub use memory_chunk::ChunkType;

pub use double_buffered_allocator::DoubleBufferedAllocator;
pub use double_buffered_allocator_copy::DoubleBufferedAllocatorCopy;

pub use double_ended_allocator::DoubleEndedStackAllocator;
pub use double_ended_allocator_copy::DoubleEndedStackAllocatorCopy;

pub use pool_allocator::PoolAllocator;
pub use unique_ptr::UniquePtr;
pub use shared_ptr::{SharedPtr, WeakPtr};
pub use pool_item::PoolItem;


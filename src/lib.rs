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
//! - a **double-buffered** allocator.
//!
//! Its primary purpose is to prevent memory fragmentation.
//!
//! This is a **nightly-only** library (last rust nightly version tested: **1.25**).
//!
//! # Example
//!
//! A `StackAllocator` can be used as a "one-frame" buffer, for example.
//!
//! In a loop, at the beginning, the allocator is reset and data is pushed into it. Before the end of
//! the loop, this data is consumed.
//!
//! Rinse and repeat.
//!
//! ```rust
//! use maskerad_memory_allocators::StackAllocator;
//! # use std::error::Error;
//! # fn try_main() -> Result<(), Box<Error>> {
//! //100 bytes for data implementing Drop, 100 bytes for data implementing Copy.
//! let single_frame_allocator = StackAllocator::with_capacity(100, 100);
//! let mut closed = false;
//!
//! while !closed {
//!     // The allocator is cleared every frame.
//!     // Everything is dropped.
//!     single_frame_allocator.reset();
//!
//!
//!     //allocate from the single frame allocator.
//!     //Be sure to use the data during this frame only!
//!     let my_vec: &Vec<u8> = single_frame_allocator.alloc(|| {
//!         Vec::with_capacity(10)
//!     })?;
//!
//!     //Use this data before the loop ends.
//!
//!     closed = true;
//! }
//! # Ok(())
//! # }
//! # fn main() {
//! #   try_main().unwrap();
//! # }
//! ```
//!
//! A `DoubleBufferedAllocator` can be used as a "two-frames" buffer.
//!
//! ```rust
//! use maskerad_memory_allocators::DoubleBufferedAllocator;
//! # use std::error::Error;
//! # fn try_main() -> Result<(), Box<Error>> {
//! //100 bytes for data implementing the Drop trait, 100 bytes for data implementing the `Copy` trait.
//! let mut allocator = DoubleBufferedAllocator::with_capacity(100, 100);
//! let mut closed = false;
//!
//! while !closed {
//!     //swap the active and inactive buffers of the allocator.
//!     allocator.swap_buffers();
//!
//!     //clear the newly active buffer.
//!     allocator.reset();
//!
//!     //allocate with the current buffer, leaving the data in the inactive buffer intact.
//!     //You can use this data during this frame, or the next frame.
//!     let my_vec: &Vec<u8> = allocator.alloc(|| {
//!         Vec::with_capacity(10)
//!     })?;
//!
//!     assert!(my_vec.is_empty());
//!
//!     closed = true;
//! }
//! # Ok(())
//! # }
//! # fn main() {
//! #   try_main().unwrap();
//! # }
//! ```
//!
#![feature(alloc)]
#![feature(raw)]
#![feature(core_intrinsics)]

#![doc(html_root_url = "https://docs.rs/maskerad_memory_allocator/5.2.0")]

#[cfg(feature = "serde")]
#[macro_use]
extern crate serde;

extern crate alloc;
extern crate core;
#[macro_use]
extern crate log;

mod stacks;
mod smart_pointers;
mod pools;

mod memory_chunk;
pub mod allocation_error;
mod utils;

pub use stacks::stack_allocator::StackAllocator;
pub use stacks::double_buffered_allocator::DoubleBufferedAllocator;

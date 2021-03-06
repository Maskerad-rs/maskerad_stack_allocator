maskerad memory allocators
========================
**custom allocators, for memory fragmentation prevention.**

[![Build status](https://ci.appveyor.com/api/projects/status/5h6ndw7bd4b3yavl/branch/master?svg=true)](https://ci.appveyor.com/project/Malkaviel/maskerad-stack-allocator/branch/master)
[![Build Status](https://travis-ci.org/Maskerad-rs/maskerad_stack_allocator.svg?branch=master)](https://travis-ci.org/Maskerad-rs/maskerad_stack_allocator)

[![Crates.io](https://img.shields.io/crates/v/maskerad_memory_allocators.svg)](https://crates.io/crates/maskerad_memory_allocators)
[![Docs](https://docs.rs/maskerad_memory_allocators/badge.svg)](https://docs.rs/maskerad_memory_allocators)

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT) [![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

This library is **nightly-only** and provides: 
- a **stack-based** allocator

This allocator is a vector-like data structure, which asks **n** number of bytes from the heap
when instantiated.

- a **double-buffered** allocator

It is a structure holding two stack-based allocators. One is active, the other is inactive.
When we allocate/reset with this allocator, the active stack-based allocator allocates/reset memory.
We can swap the allocators, the inactive one becomes active.

This library was made to **prevent memory fragmentation**. The allocators preallocate memory from the heap,
and we use those allocators to create objects.

### More informations

See the [github repository](https://github.com/Maskerad-rs/maskerad_stack_allocator) for more informations on this crate.

You can find the [documentation](https://docs.rs/maskerad_memory_allocators) here.

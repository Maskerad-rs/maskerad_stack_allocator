maskerad stack allocator
========================
A stack-based allocator, for memory allocation in tight game loops

[![codecov](https://codecov.io/gh/Maskerad-rs/maskerad_stack_allocator/branch/master/graph/badge.svg)](https://codecov.io/gh/Maskerad-rs/maskerad_stack_allocator)
[![Build status](https://ci.appveyor.com/api/projects/status/5h6ndw7bd4b3yavl/branch/master?svg=true)](https://ci.appveyor.com/project/Malkaviel/maskerad-stack-allocator/branch/master)
[![Build Status](https://travis-ci.org/Maskerad-rs/maskerad_stack_allocator.svg?branch=master)](https://travis-ci.org/Maskerad-rs/maskerad_stack_allocator)

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT) [![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

Usage
-----
TODO

The purpose of custom memory allocators
---------------------------------------
Time-constrained programs, like video-games, need to be as fast as possible.

A video-game, in its game loop, needs to :
- Read the player's input at frame **n**.
- Update the world state (AI, physics, object states, sounds...) at frame **n**.
- Draw the scene at frame **n** in the back buffer.
- Swap the back buffer (frame **n**) with the current buffer (frame **n - 1**).

In order to display **60** frames per second, this loop needs to be completed in **16** milliseconds (**0.016** seconds).

### Problems about general-purpose memory allocators
One possible bottleneck is **dynamic** memory allocation (allocation on the **heap**). Even though Rust *sometimes* uses **[jemalloc](http://jemalloc.net/)**, a fast
general-purpose memory allocator (see this [RFC](https://github.com/rust-lang/rfcs/blob/master/text/1974-global-allocators.md)),
heap memory allocation can be a very slow operation.

TODO: benchmarks

Moreover, memory can become **fragmented** over time :

![memory fragmentation illustration](readme_ressources/memory_fragmentation.svg)
TODO : schemas about memory allocation

Customs memory allocators can help to deal with those two problems.

TODO talk about the three types of memory, the fact that it's faster...
http://www.swedishcoding.com/2008/08/31/are-we-out-of-memory/
GEA book
GPP book


## What is a stack-based allocator ?
TODO what is it, in which context should i use it

## Benchmarks
TODO

## License

Licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed
as above, without any additional terms or conditions.

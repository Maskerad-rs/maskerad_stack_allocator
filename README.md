maskerad stack allocator
========================
**A stack-based allocator, for memory allocation in time-constrained programs' loops.**

[![codecov](https://codecov.io/gh/Maskerad-rs/maskerad_stack_allocator/branch/master/graph/badge.svg)](https://codecov.io/gh/Maskerad-rs/maskerad_stack_allocator)
[![Build status](https://ci.appveyor.com/api/projects/status/5h6ndw7bd4b3yavl/branch/master?svg=true)](https://ci.appveyor.com/project/Malkaviel/maskerad-stack-allocator/branch/master)
[![Build Status](https://travis-ci.org/Maskerad-rs/maskerad_stack_allocator.svg?branch=master)](https://travis-ci.org/Maskerad-rs/maskerad_stack_allocator)

[![Crates.io](https://img.shields.io/crates/v/maskerad_stack_allocator.svg)](https://crates.io/crates/maskerad_stack_allocator) [![Docs](https://docs.rs/maskerad_stack_allocator/badge.svg)](https://docs.rs/maskerad_stack_allocator)

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT) [![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

This allocator is a vector-like data structure, which asks **n** number of bytes from the heap
when instantiated.

When we want to allocate memory
for an object with this allocator, this structure gives a **raw pointer** to the 
**current top of its stack**, calculate the space needed by the object and its memory-alignment,
and move the current top of its stack to this offset.

This library is **nightly-only**, and was meant for a **very specific** use case: **game loops**.


Usage
-----
### Installation

This library is available on [crates.io](https://crates.io/crates/maskerad_stack_allocator)

### Example
Usage as a **single-frame** allocator:

```rust
extern crate maskerad_stack_allocator;
use maskerad_stack_allocator::StackAllocator;

let single_frame_allocator = StackAllocator::with_capacity(100); //100 bytes

while !closed {
    //allocator cleared every frame.
    single_frame_allocator.reset();
    
    //...
    
    //allocate from the single frame allocator.
    //Be sure to use the data during this frame only!
    let my_object: &mut MyObject = single_frame_allocator.alloc(MyObject::new());
}
```

Usage as a **double-buffered** allocator:

This type of allocator allows you to use data created during frame **n** at frame **n + 1**.

```rust
extern crate maskerad_stack_allocator;
use maskerad_stack_allocator::DoubleBufferedAllocator;

let double_buffered_allocator = DoubleBufferedAllocator::with_capacity(100);

while !closed {
    //swap the active and inactive buffers of the allocator.
    double_buffered_allocator.swap_buffers();
    
    //clear the newly active buffer.
    double_buffered_allocator.reset_current();
    
    //allocate with the current buffer, leaving the data in the inactive buffer intact.
    //You can use this data during this frame, or the next frame.
    let my_object: &mut MyObject = double_buffered_allocator.alloc(MyObject::new());
}
```

### Use case
This library was made for memory allocations in game loops.

Those type of allocators are **dropless**: memory is never freed, it means we may **override currently used memory** !

Not in a game loop :
- We allocate at the beginning of the loop.
- We consume in the loop.
- We reset at the end of the loop.

At the start of the loop **n**, we can be sure that the data allocated in the loop **n - 1** is not longer used or needed.

It means that data allocated during frame **n** must only be usable during frame **n**, not **n + 1** !

If you need to use data created at frame **n** for the frame **n + 1**, the **double buffered allocator** can solve your problem.

### Potential benefices compared to heap allocation
It *can* be **faster**: Allocations and *frees* move a pointer, that's all.

It prevents **memory fragmentation**: Allocation is always contiguous, memory cannot be fragmented over time.

Benchmarks
----------
Benchmarks have been realised with the **[bencher](https://crates.io/crates/bencher)** crate
and the **[time](https://crates.io/crates/time)** crate.

**Results with Bencher:**

monster creation - heap: ~**740**ns/iter (+/- 15)

monster creation - stack allocator: ~**3038**ns/iter (+/- 63)
```rust
fn monster_creation_heap(bench: &mut Bencher) {
    bench.iter(|| {
        for _ in 0..1000 {
            //create monsters
            let monster1 = Box::new(Monster::default());
            let monster2 = Box::new(Monster::default());
            let monster3 = Box::new(Monster::default());

            //Do stuff

            //Monsters dropped at the end of the loop
        }
    })
}

fn monster_creation_stack_allocator(bench: &mut Bencher) {
    let single_frame_allocator = StackAllocator::with_capacity(100); //100 bytes

    bench.iter(|| {
        for _ in 0..1000 {
            //clear the single-frame allocator every frame
            single_frame_allocator.reset();

            //create monsters
            let monster1 = single_frame_allocator.alloc(Monster::default());
            let monster2 = single_frame_allocator.alloc(Monster::default());
            let monster3 = single_frame_allocator.alloc(Monster::default());

            //do stuff

            //no drop -> memory overriding, but data at frame n - 1 can be overrided at frame n.
        }
    })
}
```

**Result with Time:**

Time - heap : from ~**256 000**ns to ~**440 000**ns

Time - stack allocator : from ~**443 000**ns to ~**770 000**ns
```rust
    fn speed_comparison() {
        let before = time::precise_time_ns();
        
        for _ in 0..1000 {
            let monster1 = Box::new(Monster::default());
            let monster2 = Box::new(Monster::default());
            let monster3 = Box::new(Monster::default());
        }
        
        let after = time::precise_time_ns();
        let elapsed = after - before;
        println!("Time with heap alloc: {}", elapsed);

        let single_frame_alloc = StackAllocator::with_capacity(100);
        let before = time::precise_time_ns();
        
        for _ in 0..1000 {
            single_frame_alloc.reset();
            let monster1 = single_frame_alloc.alloc(Monster::default());
            let monster2 = single_frame_alloc.alloc(Monster::default());
            let monster3 = single_frame_alloc.alloc(Monster::default());
        }
        
        let after = time::precise_time_ns();
        let elapsed = after - before;
        println!("Time with stack alloc: {}", elapsed);
    }
```

Context
---------------------------------------
### Purpose of custom allocators

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
heap memory allocation *can* be a **slow** operation.

Moreover, memory can become **fragmented** over time :

Even though we have enough **total** memory, this memory is not **contiguous** so we can't
 allocate anything.
![memory fragmentation illustration](readme_ressources/memory_fragmentation.svg)


Custom memory allocators can help with both problems.

We can distinguish 3 types of memory allocation :
- **Persistent** memory allocation: data is allocated when the program is started, and freed when
the program is shut down. The [arena crate](https://doc.rust-lang.org/1.1.0/arena/) is perfect for that.

- **Dynamic** memory allocation: data is allocated and freed during the lifetime of the program, but
we can't predict *when* this data is allocated and freed. An [Object Pool](https://github.com/Maskerad-rs/Maskerad_memory_allocator)
is a good data structure to deal with this type of memory allocation.

- **One-Frame** memory allocation: Data is allocated, consumed and freed in a loop. This allocator
deals with this type of memory allocation.

## More informations on the subject
[Game Engine Architecture, chapter 5.2](http://gameenginebook.com/toc.html)

[Stack Overflow answer about memory fragmentation](https://stackoverflow.com/questions/3770457/what-is-memory-fragmentation#3770593)

[Stack Overflow answer about stack-based allocators](https://stackoverflow.com/questions/8049657/stack-buffer-based-stl-allocator)

[SwedishCoding blogpost about custom memory allocators](http://www.swedishcoding.com/2008/08/31/are-we-out-of-memory/)

[Game Programming Patterns, Chapter 19, about Object Pools](http://gameprogrammingpatterns.com/object-pool.html)

[Wikipedia article about Object Pools](https://en.wikipedia.org/wiki/Memory_pool)

## Known issues
Allocations with the stack allocator is slower than heap allocation.

## License

Licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed
as above, without any additional terms or conditions.

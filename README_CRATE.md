maskerad stack allocator
========================
**A stack-based allocator, for memory allocation in time-constrained programs' loops.**

[![codecov](https://codecov.io/gh/Maskerad-rs/maskerad_stack_allocator/branch/master/graph/badge.svg)](https://codecov.io/gh/Maskerad-rs/maskerad_stack_allocator)
[![Build status](https://ci.appveyor.com/api/projects/status/5h6ndw7bd4b3yavl/branch/master?svg=true)](https://ci.appveyor.com/project/Malkaviel/maskerad-stack-allocator/branch/master)
[![Build Status](https://travis-ci.org/Maskerad-rs/maskerad_stack_allocator.svg?branch=master)](https://travis-ci.org/Maskerad-rs/maskerad_stack_allocator)

[![Crates.io](https://img.shields.io/crates/v/maskerad_stack_allocator.svg)](https://crates.io/crates/maskerad_stack_allocator) [![Docs](https://docs.rs/maskerad_stack_allocator/badge.svg)](https://docs.rs/maskerad_stack_allocator)

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT) [![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

This library is **nightly-only**, and was meant for a **very specific** use case: **game loops**.

Usage
-----
### Installation

Add the crate as a dependency in your Cargo.toml:

```toml
[dependencies]
maskerad_stack_allocator = "0.1.0"
```

### More informations

See the [github repository](https://github.com/Maskerad-rs/maskerad_stack_allocator) for more informations on this crate.

You can find the [documentation](https://docs.rs/maskerad_stack_allocator) here.
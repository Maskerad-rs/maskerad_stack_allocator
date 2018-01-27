# Changelog

Semantic versioning : MAJOR.MINOR.PATCH

PATCH update: backward-compatible bug fixes.

MINOR update: Functionality has been added, but no API breakage.

MAJOR update: API breakage.

### December 28
#### version 0.1.2 -> version 1.0.0

StackAllocator::current_offset() function removed.

StackAllocator::marker() function added, return a raw pointer to the current top of the stack.

StackAllocator::reset_to_marker() added, move the current top of stack's pointer from its current place to the given marker.

Updated the documentation. 

Updated the unit tests.

Updated the READMEs.

#### version 1.0.0 -> 1.0.1

Updated Cargo.toml. Removed the wrong badge.

### December 29
#### version 1.0.1 -> version 1.1.0

Removed an unnecessary unit-test in StackAllocator.rs

Removed private functions StackAllocator::enough_space_unaligned() and StackAllocator::enough_space_aligned().

Added the DoubleEndedAllocator structure, a StackAllocator where allocation occurs on both sides.

Added the unit tests for the DoubleEndedAllocator.

Added the documentation for the DoubleEndedAllocator.

#### version 1.1.0 -> version 1.1.1

Small documentation correction, at the crate-level documentation. It wasn't said that the library
provided a DoubleEndedAllocator.

#### version 1.1.1 -> version 1.1.2

Small formatting problem in the documentation...

### January 1
#### version 1.1.2 -> version 2.0.0

Rewrote everything.
The earlier design was pretty bad, and just didn't work. The stack allocators have been
entirely refactored, following the work done on the any-arena crate. The AnyArena structure
is literally a container of stack allocators.

I can't make a detailed changelog, everything has been refactored.

Coming later : memory pools if all goes well.

#### version 2.0.0 -> version 2.0.1

Updated the README_CRATE.md and README.md, modified the badges and links to the crate and docs.


#### version 3.1.0

Stack allocators can return immutable references now.

Added a pool allocator for data implementing the Drop trait.

Added UniquePtr, SharedPtr and WeakPtr. Used by the pool allocator, they are almost similar to
Box, Rc and Weak.

#### version 3.1.1

All smart pointers uses the NonNull<T> (The new name of Shared<T>) struct as a backend now, instead
of Shared<T> for WeakPtr and SharedPtr, and Unique<T> for UniquePtr. 

#### version 3.1.2

Forgot to update the documentation. Fixed.
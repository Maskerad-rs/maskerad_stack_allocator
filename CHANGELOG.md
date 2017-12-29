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
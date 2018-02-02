// Copyright 2017-2018 Maskerad Developers
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

use std::fmt;
use std::error::Error;

/// A custom error enumeration, used by AllocationResult as the error type.
/// Handle "out of memory" errors.
#[derive(Debug)]
pub enum AllocationError {
    OutOfMemoryError(String),
    OutOfPoolError(String),
}

unsafe impl Send for AllocationError {}
unsafe impl Sync for AllocationError {}

impl fmt::Display for AllocationError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            &AllocationError::OutOfMemoryError(ref description) => {
                write!(f, "Out of memory error: {}", description)
            }
            &AllocationError::OutOfPoolError(ref description) => {
                write!(f, "Out of pool error: {}", description)
            }
        }
    }
}

impl Error for AllocationError {
    fn description(&self) -> &str {
        match self {
            &AllocationError::OutOfMemoryError(_) => "OutOfMemoryError",
            &AllocationError::OutOfPoolError(_) => "OutOfPoolError",
        }
    }

    fn cause(&self) -> Option<&Error> {
        match self {
            &AllocationError::OutOfMemoryError(_) => None,
            &AllocationError::OutOfPoolError(_) => None,
        }
    }
}

/// A simple typedef, for convenience.
pub type AllocationResult<T> = Result<T, AllocationError>;

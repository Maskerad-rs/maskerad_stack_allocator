// Copyright 2017 Maskerad Developers
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

use std::fmt;
use std::error::Error;

#[derive(Debug)]
pub struct StackAllocError {
    pub description: String,
}

impl fmt::Display for StackAllocError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Stack Allocator Error: {}", self.description)
    }
}

impl Error for StackAllocError {
    fn description(&self) -> &str {
        "StackAllocError"
    }

    fn cause(&self) -> Option<&Error> {
        None
    }
}

pub type StackAllocResult<T> = Result<T, StackAllocError>;
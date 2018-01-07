// Copyright 2017-2018 Maskerad Developers
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

use memory_chunk::MemoryChunk;

pub struct PoolItem {
    storage: MemoryChunk,
    next: Option<usize>,
}

impl PoolItem {
    pub fn new(size: usize, next: Option<usize>) -> Self {
        PoolItem {
            storage: MemoryChunk::new(size),
            next,
        }
    }

    pub fn memory_chunk(&self) -> &MemoryChunk {
        &self.storage
    }

    //Option impl Copy.
    pub fn next(&self) -> Option<usize> {
        self.next
    }

    pub fn set_next(&mut self, next: Option<usize>) {
        self.next = next;
    }
}
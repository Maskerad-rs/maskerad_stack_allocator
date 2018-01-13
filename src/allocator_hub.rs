// Copyright 2017-2018 Maskerad Developers
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

use std::collections::HashMap;
use pool_allocator::PoolAllocator;

//TODO: Arc<Mutex<Pool>>, Mutex<Pool>, nothing ?
lazy_static! {
    pub static ref POOL_ALLOCATOR_HUB: HashMap<u8, PoolAllocator> = {
        HashMap::with_capacity(3)
    };

    //TODO: A hub for stack allocators ?
}
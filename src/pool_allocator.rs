// Copyright 2017-2018 Maskerad Developers
//
// Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
// http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
// http://opensource.org/licenses/MIT>, at your option. This file may not be
// copied, modified, or distributed except according to those terms.

use pool_item::PoolItem;
use std::cell::{RefCell, Cell};
use std::mem;
use std::ptr;
use utils;
use allocation_error::{AllocationResult, AllocationError};
use unique_ptr::UniquePtr;
use shared_ptr::{SharedUnique, SharedPtr, WeakPtr};
use std::sync::Arc;

pub struct PoolAllocator {
    storage: Vec<RefCell<PoolItem>>,
    first_available: Cell<Option<usize>>,
}

impl PoolAllocator {
    /// Creates a poolAllocator with `nb_item` chunks of `size_item` size in byte.
    pub fn new(nb_item: usize, size_item: usize) -> Self {
        let mut storage = Vec::with_capacity(nb_item);
        for i in 0..nb_item - 1 {
            storage.push(RefCell::new(PoolItem::new(size_item, Some(i+1))));
        }

        storage.push(RefCell::new(PoolItem::new(size_item, None)));

        PoolAllocator {
            storage,
            first_available: Cell::new(Some(0)),
        }
    }

    /// Returns an immutable reference to the vector of memory chunks used by the allocator.
    pub fn storage(&self) -> &Vec<RefCell<PoolItem>> {
        &self.storage
    }

    /// Returns the index of the first available pool item in the pool allocator.
    pub fn first_available(&self) -> Option<usize> {
        self.first_available.get()
    }

    /// Sets the index of the first available pool item in the pool allocator.
    pub fn set_first_available(&self, first_available: Option<usize>) {
        self.first_available.set(first_available);
    }

    /// Allocates data in the pool allocator, returning a `SharedPtr`.
    /// # Errors
    /// This function will return an error if all the pools are used when trying to allocate this data.
    pub fn alloc_shared<T, F>(&self, op: F) -> AllocationResult<SharedPtr<T>>
        where F: FnOnce() -> T
    {
        self.alloc_non_copy_shared(op)
    }

    fn alloc_non_copy_shared<T, F>(&self, op: F) -> AllocationResult<SharedPtr<T>>
        where F: FnOnce() -> T
    {
        unsafe {


            //Get the type description of the type T (get its vtable).
            let type_description = utils::get_type_description::<SharedUnique<T>>();

            //Get the index of the current first available pool item in the pool allocator.
            //alloc_non_copy_inner will update the index of the first available pool item,
            //and we need this index to create an UniquePtr.
            match self.first_available() {
                Some(index) => {
                    //Ask the the first available memory chunk to give us raw pointers to memory locations
                    //for our type description object.
                    let (type_description_ptr, ptr) = self.alloc_non_copy_inner(index, mem::size_of::<SharedUnique<T>>(), mem::align_of::<SharedUnique<T>>())?;

                    //Cast them.
                    let type_description_ptr = type_description_ptr as *mut usize;
                    let ptr = ptr as *mut SharedUnique<T>;

                    //Write in our type description along with a bit indicating that the object has *not*
                    //been initialized yet.
                    *type_description_ptr = utils::bitpack_type_description_ptr(type_description, false);

                    //Initialize the object.
                    ptr::write(&mut (*ptr), SharedUnique{strong: Cell::new(1), weak: Cell::new(1), value: op()});

                    //Now that we are done, update the type description to indicate that the object is there.
                    *type_description_ptr = utils::bitpack_type_description_ptr(type_description, true);

                    Ok(SharedPtr::from_raw(ptr, &self, index))

                },
                None => {
                    return Err(AllocationError::OutOfPoolError(format!("All the pools in the pool allocator were in use when the allocation was requested !")));
                }
            }
        }
    }

    /// Allocates data in the pool allocator, returning an `UniquePtr`.
    /// # Errors
    /// This function will return an error if all the pools are used when trying to allocate this data.
    #[inline]
    pub fn alloc_unique<F, T>(&self, op: F) -> AllocationResult<UniquePtr<T>>
        where F: FnOnce() -> T
    {
        self.alloc_non_copy_unique(op)
    }

    #[inline]
    fn alloc_non_copy_unique<T, F>(&self, op: F) -> AllocationResult<UniquePtr<T>>
        where F: FnOnce() -> T
    {
        unsafe {
            //Get the type description of the type T (get its vtable).
            let type_description = utils::get_type_description::<T>();

            //Get the index of the current first available pool item in the pool allocator.
            //alloc_non_copy_inner will update the index of the first available pool item,
            //and we need this index to create an UniquePtr.
            match self.first_available() {
                Some(index) => {
                    //Ask the the first available memory chunk to give us raw pointers to memory locations
                    //for our type description object.
                    let (type_description_ptr, ptr) = self.alloc_non_copy_inner(index, mem::size_of::<T>(), mem::align_of::<T>())?;

                    //Cast them.
                    let type_description_ptr = type_description_ptr as *mut usize;
                    let ptr = ptr as *mut T;

                    //Write in our type description along with a bit indicating that the object has *not*
                    //been initialized yet.
                    *type_description_ptr = utils::bitpack_type_description_ptr(type_description, false);

                    //Initialize the object.
                    ptr::write(&mut (*ptr), op());

                    //Now that we are done, update the type description to indicate that the object is there.
                    *type_description_ptr = utils::bitpack_type_description_ptr(type_description, true);

                    Ok(UniquePtr::from_raw(ptr, &self, index))

                },
                None => {
                    return Err(AllocationError::OutOfPoolError(format!("All the pools in the pool allocator were in use when the allocation was requested !")));
                }
            }
        }
    }

    #[inline]
    fn alloc_non_copy_inner(&self, chunk_index: usize, n_bytes: usize, align: usize) -> AllocationResult<(*const u8, *const u8)> {

        //Borrow mutably the first pool item available in the pool allocator.
        let non_copy_storage = self.storage.get(chunk_index).unwrap().borrow_mut();

        //This chunk of memory is now in use, update the index of the first available chunk of memory.
        self.first_available.set(non_copy_storage.next());

        //Get the index of the first unused memory address.
        let fill = non_copy_storage.memory_chunk().fill();

        //Get the index of where we'll write the type description data
        //(the first unused byte in the memory chunk)
        let type_description_start = fill;

        //Get the index of where the object should reside (unaligned location actually).
        let after_type_description = fill + mem::size_of::<*const utils::TypeDescription>();

        //With the index to the unaligned memory address, determine the index to the aligned
        //memory address where the object will reside,
        //according to its memory alignment.
        let start = utils::round_up(after_type_description, align);

        //Determine the index of the next aligned memory address for a type description,
        //according to the size of the object and the memory alignment of a type description.
        let end = utils::round_up(start + n_bytes, mem::align_of::<*const utils::TypeDescription>());

        //If the allocator become oom after this possible allocation, abort the program.
        if end >= non_copy_storage.memory_chunk().capacity() {
            return Err(AllocationError::OutOfMemoryError(format!("The memory chunk of the pool allocator doesn't have enough memory to hold this type !")));
        }

        //Update the current top of the stack. The first unused memory address is at
        //index 'end', where the next type description would be written if an allocation
        //was asked.
        non_copy_storage.memory_chunk().set_fill(end);

        unsafe {
            //Get a raw pointer to the start of the RawVec of the MemoryChunk of the PoolItem. Yep.
            let start_storage = non_copy_storage.memory_chunk().as_ptr();

            Ok(
                (
                    //From this raw pointer, get the correct raw pointers with the indices
                    //we calculated earlier.

                    //The raw pointer to the type description of the object.
                    start_storage.offset(type_description_start as isize),

                    //The raw pointer to the object
                    start_storage.offset(start as isize)
                ))

        }
    }
}

#[cfg(test)]
mod pool_allocator_test {
    use super::*;
    use std::mem::drop;

    //size : 4 bytes + 4 bytes alignment + 4 bytes + 4 bytes alignment + alignment-offset stuff -> ~16-20 bytes.
#[derive(Clone)]
    struct Monster {
    _hp: u32,
}

    impl Monster {
        pub fn new(hp: u32) -> Self {
            Monster {
                _hp: hp,
            }
        }

        pub fn level(&self) -> u32 {
            self._hp
        }
    }

    impl Default for Monster {
        fn default() -> Self {
            Monster {
                _hp: 1,
            }
        }
    }

    impl Drop for Monster {
        fn drop(&mut self) {
            println!("[PoolAllocator] {} I'm dying !", self._hp);
        }
    }

    //1, 2, 3
    #[test]
    fn test_unique_ptr_drop_and_nb_pool_available() {
        //create a pool allocator with 2 pool items of 100 bytes.
        let pool = PoolAllocator::new(2, 100);

        //the index of the first available pool item is 0.
        assert_eq!(pool.first_available.get(), Some(0));

        //Create a UniquePtr from the pool allocator
        let a_monster = pool.alloc_unique(|| {
            Monster::new(3)
        }).unwrap();
        assert_eq!(a_monster.level(), 3);
        //The index of the first available pool item is 1.
        assert_eq!(pool.first_available.get(), Some(1));

        {
            //The first pool item is used, allocate a new monster in the second pool item.
            let another_monster = pool.alloc_unique(|| {
                Monster::new(1)
            }).unwrap();

            //The index of the first available item is None. There's no pool items available.
            assert_eq!(pool.first_available.get(), None);
            assert_eq!(another_monster.level(), 1);
            //Since no pool items are available, we'll get an error if we try to allocate something.
            /*
            assert!(pool.alloc_unique(|| {
                Monster::default()
            }).is_err());
            */

            //another_monster will be dropped and should print "i'm dying !".
        }

        //Since the monster has been dropped, the index of the first available pool item is 1.
        assert_eq!(pool.first_available.get(), Some(1));
        assert!(pool.alloc_unique(|| {
            Monster::new(2)
        }).is_ok())
    }

    //4, 4, 5
    #[test]
    fn test_unique_ptr_behavior() {
        //Clone behavior and usage

        //create a pool allocator with 2 pool items of 100 bytes.
        let pool = PoolAllocator::new(2, 100);

        //the index of the first available pool item is 0.
        assert_eq!(pool.first_available.get(), Some(0));

        let a_monster = pool.alloc_unique(|| {
            Monster::new(4)
        }).unwrap();
        assert_eq!(a_monster.level(), 4);
        assert_eq!(pool.first_available.get(), Some(1));

        {
            let a_monster_clone = a_monster.clone();
            assert_eq!(a_monster_clone.level(), 4);
            assert_eq!(pool.first_available.get(), None);
            assert!(pool.alloc_unique(|| {
                Monster::new(2)
            }).is_err());

            //a_monster_clone prints "i'm dying !"
        }
        assert_eq!(pool.first_available.get(), Some(1));
        assert!(pool.alloc_unique(|| {
            Monster::new(5)
        }).is_ok());


    }

    //6, 7
    #[test]
    fn test_shared_ptr_behavior() {
        //create a pool allocator with 2 pool items of 100 bytes.
        let pool = PoolAllocator::new(2, 100);
        //the index of the first available pool item is 0.
        assert_eq!(pool.first_available.get(), Some(0));
        //Create a SharedPtr from the pool.
        let monster = pool.alloc_shared(|| {
            Monster::new(6)
        }).unwrap();
        assert_eq!(pool.first_available.get(), Some(1));

        //The strong count must be 1 (only one SharedPtr on this value)
        //The weak count must be 0 (No weakPtr in this value, except the implicit weak count in the strong count)
        assert_eq!(SharedPtr::strong_count(&monster), 1);
        assert_eq!(SharedPtr::weak_count(&monster), 0);

        //create a SharedPtr from the SharedPtr
        let strong_ref_monster = SharedPtr::clone(&monster);
        assert_eq!(pool.first_available.get(), Some(1));
        assert_eq!(SharedPtr::strong_count(&monster), 2);
        assert_eq!(SharedPtr::strong_count(&strong_ref_monster), 2);
        assert_eq!(SharedPtr::weak_count(&monster), 0);
        assert_eq!(SharedPtr::weak_count(&strong_ref_monster), 0);

        {
            //create a weak from the SharedPtr
            let weak_ref_monster = SharedPtr::downgrade(&monster);
            assert_eq!(pool.first_available.get(), Some(1));
            assert_eq!(SharedPtr::strong_count(&monster), 2);
            assert_eq!(SharedPtr::strong_count(&strong_ref_monster), 2);
            assert_eq!(SharedPtr::weak_count(&monster), 1);
            assert_eq!(SharedPtr::weak_count(&strong_ref_monster), 1);

            //create another weak.
            let weak_ref_monster_2 = SharedPtr::downgrade(&strong_ref_monster);
            assert_eq!(pool.first_available.get(), Some(1));
            assert_eq!(SharedPtr::strong_count(&monster), 2);
            assert_eq!(SharedPtr::strong_count(&strong_ref_monster), 2);
            assert_eq!(SharedPtr::weak_count(&monster), 2);
            assert_eq!(SharedPtr::weak_count(&strong_ref_monster), 2);

            //clone a weak.
            let weak_ref_monster_3 = WeakPtr::clone(&weak_ref_monster_2);
            assert_eq!(pool.first_available.get(), Some(1));
            assert_eq!(SharedPtr::strong_count(&monster), 2);
            assert_eq!(SharedPtr::strong_count(&strong_ref_monster), 2);
            assert_eq!(SharedPtr::weak_count(&monster), 3);
            assert_eq!(SharedPtr::weak_count(&strong_ref_monster), 3);
        }
        //all the weaks are dropped here.
        assert_eq!(pool.first_available.get(), Some(1));
        assert_eq!(SharedPtr::strong_count(&monster), 2);
        assert_eq!(SharedPtr::strong_count(&strong_ref_monster), 2);
        assert_eq!(SharedPtr::weak_count(&monster), 0);
        assert_eq!(SharedPtr::weak_count(&strong_ref_monster), 0);


        {
            //create a new monster
            let another_monster = pool.alloc_shared(|| {
                Monster::new(7)
            }).unwrap();
            assert_eq!(pool.first_available.get(), None);
            assert_eq!(SharedPtr::strong_count(&another_monster), 1);
            assert_eq!(SharedPtr::weak_count(&another_monster), 0);

            //create a weak with downgrade
            let weak_ref_another_monster = SharedPtr::downgrade(&another_monster);
            assert_eq!(pool.first_available.get(), None);
            assert_eq!(SharedPtr::strong_count(&another_monster), 1);
            assert_eq!(SharedPtr::weak_count(&another_monster), 1);

            //create a strong with upgrade
            assert!(WeakPtr::upgrade(&weak_ref_another_monster).is_some());
            let strong_ref_another_monster = WeakPtr::upgrade(&weak_ref_another_monster).unwrap();
            assert_eq!(pool.first_available.get(), None);
            assert_eq!(SharedPtr::strong_count(&another_monster), 2);
            assert_eq!(SharedPtr::strong_count(&strong_ref_another_monster), 2);
            assert_eq!(SharedPtr::weak_count(&another_monster), 1);
            assert_eq!(SharedPtr::weak_count(&strong_ref_another_monster), 1);
        }
        //All the strong refs to the second monster have been dropped, the second pool is available again.
        assert_eq!(pool.first_available.get(), Some(1));
    }

    //8, 9
    #[test]
    fn test_trait_object_with_smart_ptr() {
        pub trait TestTraitObject {
            fn test(&self) -> Option<()>;
        }

        impl TestTraitObject for Monster {
            fn test(&self) -> Option<()> {
                Some(())
            }
        }

        pub struct Dragon {
            _hp: u32,
        }

        impl Dragon {
            pub fn new(hp: u32) -> Self {
                Dragon {
                    _hp: hp,
                }
            }
        }

        impl Drop for Dragon {
            fn drop(&mut self) {
                println!("Dragon is dying !");
            }
        }

        impl TestTraitObject for Dragon {
            fn test(&self) -> Option<()> {
                None
            }
        }

        pub struct StructTest<'a> {
            test: UniquePtr<'a, TestTraitObject>,
        }

        pub struct StructTestShared<'a> {
            test: SharedPtr<'a, TestTraitObject>,
        }


        //create a pool allocator with 2 pool items of 100 bytes.
        let pool = PoolAllocator::new(2, 100);
        //the index of the first available pool item is 0.
        assert_eq!(pool.first_available.get(), Some(0));

        let struct_test = StructTest {
            test: pool.alloc_unique(|| { Monster::new(8) }).unwrap() as UniquePtr<TestTraitObject>,
        };

        let struct_test_shared = StructTestShared {
            test: pool.alloc_shared(|| {
                Dragon::new(9)
            }).unwrap() as SharedPtr<TestTraitObject>,
        };

        assert_eq!(pool.first_available.get(), None);

        assert!(struct_test.test.test().is_some());
        assert!(struct_test_shared.test.test().is_none());
    }

}

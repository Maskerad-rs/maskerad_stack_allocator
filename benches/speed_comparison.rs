#[macro_use]
extern crate bencher;
extern crate maskerad_stack_allocator;

use maskerad_stack_allocator::StackAllocator;

use bencher::Bencher;

//size : 4 bytes + 4 bytes alignment + 4 bytes + 4 bytes alignment + alignment-offset stuff -> ~16-20 bytes.
struct Monster {
    hp :u32,
    level: u32,
}

impl Default for Monster {
    fn default() -> Self {
        Monster {
            hp: 1,
            level: 1,
        }
    }
}

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

            //no drop -> memory overriding, but data at frame n - 1 is not needed when we are at frame n.
        }
    })
}

benchmark_group!(benches, monster_creation_heap, monster_creation_stack_allocator);
benchmark_main!(benches);
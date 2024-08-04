use std::alloc::{GlobalAlloc, Layout};
use std::cell::UnsafeCell;
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

// 128 KB of area available to the allocator to allocate
const ARENA_SIZE: usize = 128 * 1024;
// Maximum supported alignment value (4096 bytes)
const MAX_SUPPORTED_ALIGN: usize = 4096;

#[repr(C, align(4096))]
struct Allocator {
    arena: UnsafeCell<[u8; ARENA_SIZE]>,
    remaining: AtomicUsize,
    free_list: UnsafeCell<*mut FreeBlock>,
}

struct FreeBlock {
    size: usize,
    next: *mut FreeBlock,
}

#[global_allocator]
static ALLOCATOR: Allocator = Allocator {
    arena: UnsafeCell::new([0x55; ARENA_SIZE]),
    remaining: AtomicUsize::new(ARENA_SIZE),
    free_list: UnsafeCell::new(null_mut()),
};

unsafe impl Sync for Allocator {}

unsafe impl GlobalAlloc for Allocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let size = layout.size();
        let align = layout.align();

        let align_mask_to_round_down = !(align - 1);

        if align > MAX_SUPPORTED_ALIGN {
            return null_mut();
        }

        let mut free_list = self.free_list.get();
        let mut prev: *mut FreeBlock = null_mut();
        while !(*free_list).is_null() {
            let block = &**free_list;
            if block.size >= size {
                if !prev.is_null() {
                    (*prev).next = block.next
                } else {
                    *self.free_list.get() = block.next;
                }

                return block as *const _ as *mut u8;
            }

            prev = *free_list;
            free_list = &mut (**free_list).next;
        }

        let mut allocated = 0;
        if self
            .remaining
            .fetch_update(Relaxed, Relaxed, |mut remaining| {
                if size > remaining {
                    return None;
                }

                remaining -= size;
                remaining &= align_mask_to_round_down;
                allocated = remaining;

                Some(remaining)
            })
            .is_err()
        {
            return null_mut();
        }

        self.arena.get().cast::<u8>().add(allocated)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if ptr.is_null() {
            return;
        }

        let block = ptr as *mut FreeBlock;
        block.write(FreeBlock {
            size: layout.size(),
            next: *self.free_list.get(),
        });
    }
}

fn main() {
    let _s = format!("allocating a string!");
    let currently = ALLOCATOR.remaining.load(Relaxed);
    println!("allocated so far: {}", ARENA_SIZE - currently);
}

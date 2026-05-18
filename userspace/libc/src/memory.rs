use core::ptr::write_bytes;
use crate::syscall;
use core::alloc::{GlobalAlloc, Layout};

struct MyAllocator;

unsafe impl GlobalAlloc for MyAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        malloc(layout.size())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        free(ptr)
    }
}

#[global_allocator]
static ALLOCATOR: MyAllocator = MyAllocator;

const HEAP_START: *mut u8 = 0x0000_0010_0000_0000 as *mut u8; // arbitrary heap start address
static mut HEAP_BREAK: *mut u8 = HEAP_START;

struct BlockHeader {
    size: usize,
    free: bool,
    next: *mut BlockHeader,
    prev: *mut BlockHeader,
}

fn sbrk(pages: usize) -> *mut u8 {
    let old_break = unsafe { HEAP_BREAK };

    let new_break = syscall::sbrk(pages as u64);
    if new_break.is_null() {
        return core::ptr::null_mut(); // failed to expand heap
    }

    unsafe {
        HEAP_BREAK = new_break
    }
    old_break
}
pub fn init() {
    const DEFAULT_HEAP_SIZE_PAGES: usize = 1; // 16 pages = 64KB
    sbrk(DEFAULT_HEAP_SIZE_PAGES);

    // initialize first block header
    let first_block = HEAP_START as *mut BlockHeader;
    unsafe {
        (*first_block).size = DEFAULT_HEAP_SIZE_PAGES * 0x1000 - size_of::<BlockHeader>();
        (*first_block).free = true;
        (*first_block).next = core::ptr::null_mut();
        (*first_block).prev = core::ptr::null_mut();
    }
}

pub fn malloc(size: usize) -> *mut u8 {
    unsafe {
        let mut block = HEAP_START as *mut BlockHeader;
        while !block.is_null() {
            if (*block).free && (*block).size >= size {
                // found a suitable block
                (*block).free = false;
                // create a new block header for the remaining free space

                if (*block).size >= size + size_of::<BlockHeader>() + 16 { // only split if the remaining space can hold a new block header and some data
                    let remaining_size = (*block).size - size - size_of::<BlockHeader>();
                    let new_block = (block as *mut u8).add(size_of::<BlockHeader>() + size) as *mut BlockHeader;
                    (*new_block).size = remaining_size;
                    (*new_block).free = true;
                    (*new_block).next = (*block).next;
                    (*new_block).prev = block;
                    (*block).next = new_block;
                    (*block).size = size;
                }
                // zero
                write_bytes((block as *mut u8).add(size_of::<BlockHeader>()), 0, (*block).size);
                return (block as *mut u8).add(size_of::<BlockHeader>());
            }
            block = (*block).next;
        }
    }
    // no suitable block found, need to expand heap
    let old_break = sbrk(size / 0x1000 + 1); // allocate enough pages to fit the requested size
    if old_break.is_null() {
        return core::ptr::null_mut(); // failed to expand heap
    }  // create a new block header for the newly allocated space
    let new_block = old_break as *mut BlockHeader;
    unsafe {
        (*new_block).size = (size / 0x1000 + 1) * 0x1000 - size_of::<BlockHeader>();
        (*new_block).free = true;
        (*new_block).next = core::ptr::null_mut();
    }

    // find the last block in the heap and link the new block to it
    unsafe {
        let mut block = HEAP_START as *mut BlockHeader;
        while !(*block).next.is_null() {
            block = (*block).next;
        }
        (*block).next = new_block;
        (*new_block).prev = block;
    }

    malloc(size) // try allocating again now that we have more space
}

pub fn free(ptr: *mut u8) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        let block = (ptr as *mut BlockHeader).offset(-1);
        (*block).free = true;

        // coalesce with next block if it's free
        if !(*block).next.is_null() && (*(*block).next).free {
            (*block).size += size_of::<BlockHeader>() + (*(*block).next).size;
            (*block).next = (*(*block).next).next;
            if !(*block).next.is_null() {
                (*(*block).next).prev = block;
            }
        }

        // coalesce with previous block if it's free
        if !(*block).prev.is_null() && (*(*block).prev).free {
            (*(*block).prev).size += size_of::<BlockHeader>() + (*block).size;
            (*(*block).prev).next = (*block).next;
            if !(*block).next.is_null() {
                (*(*block).next).prev = (*block).prev;
            }
        }
    }
}
use core::alloc::{GlobalAlloc, Layout};
use core::ptr::write_bytes;
use crate::serial;
use crate::{memory, println};
use crate::memory::kbox::KBox;

const HEAP_START: *mut u8 = 0xffff800001000000 as *mut u8;
static mut HEAP_BREAK: *mut u8 = HEAP_START;

struct BlockHeader {
    size: usize,
    free: bool,
    next: *mut BlockHeader,
    prev: *mut BlockHeader,
}

struct MyAllocator;

unsafe impl GlobalAlloc for MyAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        kmalloc(layout.size())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        kfree(ptr)
    }
}

#[global_allocator]
static ALLOCATOR: MyAllocator = MyAllocator;

struct AlignedBlockHeader {
    ptr_block_header: *mut BlockHeader, // pointer to the original block header for freeing
}

fn sbrk_kernel(pages: usize) -> *mut u8 {
    let old_break = unsafe { HEAP_BREAK };

    for i in 0..pages {
        let page_addr = old_break as usize + i * 0x1000;
        let page = memory::base::alloc_frame().expect("Failed to allocate frame for heap");
        // map page to heap
        let virt = page_addr as u64;
        let phys = page;
        memory::vmm::map_page(memory::vmm::get_kernel_pml4t(), virt, phys, 0x3); // Present + RWRW
    }

    unsafe {
        HEAP_BREAK = HEAP_BREAK.add(pages * 0x1000);
    }
    old_break
}

pub fn init() {
    const DEFAULT_HEAP_SIZE_PAGES: usize = 16; // 16 pages = 64KB
    sbrk_kernel(DEFAULT_HEAP_SIZE_PAGES);

    // initialize first block header
    let first_block = HEAP_START as *mut BlockHeader;
    unsafe {
        (*first_block).size = DEFAULT_HEAP_SIZE_PAGES * 0x1000 - size_of::<BlockHeader>();
        (*first_block).free = true;
        (*first_block).next = core::ptr::null_mut();
        (*first_block).prev = core::ptr::null_mut();
    }
}

pub fn kmalloc(size: usize) -> *mut u8 {
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
    let old_break = sbrk_kernel(size / 0x1000 + 1); // allocate enough pages to fit the requested size
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

    kmalloc(size) // try allocating again now that we have more space
}

pub fn kfree(ptr: *mut u8) {
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

#[allow(dead_code)]
pub fn kmalloc_alligned(size: usize, align: usize) -> *mut u8 {
    let total_size = size + align + size_of::<AlignedBlockHeader>();
    let raw_ptr = kmalloc(total_size);
    if raw_ptr.is_null() {
        return core::ptr::null_mut();
    }
    let start = raw_ptr as usize + size_of::<AlignedBlockHeader>();
    let aligned_ptr = ((start + align - 1) & !(align - 1)) as *mut u8;
    // store the original block header pointer just before the aligned pointer for freeing
    unsafe {
        let block_header_ptr = (aligned_ptr as *mut AlignedBlockHeader).offset(-1);
        (*block_header_ptr).ptr_block_header = (raw_ptr as *mut BlockHeader).offset(-1);
    }
    aligned_ptr
}

#[allow(dead_code)]
pub fn kfree_alligned(ptr: *mut u8) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        let block_header_ptr = (ptr as *mut AlignedBlockHeader).offset(-1);
        let original_block_header = (*block_header_ptr).ptr_block_header;
        kfree((original_block_header as *mut u8).add(size_of::<BlockHeader>()));
    }
}

#[allow(dead_code)]
pub fn debug_heap() {
    unsafe {
        let mut block = HEAP_START as *mut BlockHeader;
        println!("Heap blocks:");
        while !block.is_null() {
            println!("  Block at {:#x}: size {}, free {}", block as usize, (*block).size, (*block).free);
            block = (*block).next;
        }
    }
}
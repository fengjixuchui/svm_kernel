use x86_64::registers::control::Cr3;
// use x86_64::structures::paging::mapper::MapToError;
use core::ptr::addr_of;
use core::ptr::read;
use x86_64::structures::paging::mapper::MappedFrame;
use x86_64::structures::paging::mapper::TranslateResult;
use x86_64::structures::paging::page::PageSize;
use x86_64::structures::paging::Mapper;
use x86_64::structures::paging::Page;
use x86_64::structures::paging::PageTableFlags;
use x86_64::structures::paging::Translate;
use x86_64::structures::paging::{OffsetPageTable, PageTable};
use x86_64::VirtAddr;
use x86_64::{
    structures::paging::{FrameAllocator, PhysFrame, Size1GiB, Size2MiB, Size4KiB},
    PhysAddr,
};

//
// The bootloader maps the page table to a very high offset
// in memory and this function returns the page table type
// with the offset applied
// Also, this function must be only called once
/// to avoid aliasing `&mut` references (which is undefined behavior).
unsafe fn active_level_4_table(physical_memory_offset: VirtAddr) -> &'static mut PageTable {
    let (level_4_table_frame, _) = Cr3::read();

    let phys = level_4_table_frame.start_address();
    let virt = physical_memory_offset + phys.as_u64();

    let page_table_ptr: *mut PageTable = virt.as_mut_ptr();

    &mut *page_table_ptr // unsafe
}

/// Initialize a new OffsetPageTable.
///
/// This function is unsafe because the caller must guarantee that the
/// complete physical memory is mapped to virtual memory at the passed
/// `physical_memory_offset`. Also, this function must be only called once
/// to avoid aliasing `&mut` references (which is undefined behavior).
pub unsafe fn init(physical_memory_offset: VirtAddr) -> OffsetPageTable<'static> {
    let level_4_table = active_level_4_table(physical_memory_offset);
    OffsetPageTable::new(level_4_table, physical_memory_offset)
}

pub fn print_pagetable(mapper: &OffsetPageTable) {
    use x86_64::structures::paging::mapper::TranslateError;

    for page_addr in (0x200000..core::u64::MAX).step_by(0x200000) {
        let addr = Page::<Size2MiB>::from_start_address(VirtAddr::new(page_addr)).unwrap();
        let res = mapper.translate_page(addr);

        match res {
            Ok(r) => log::info!("{:?} -> {:?}", addr.start_address(), r.start_address()),
            Err(TranslateError::InvalidFrameAddress(e)) => {
                panic!("Invalid frame address: {:?}", e)
            }
            _ => (),
        }
    }
    log::info!("Done");
}

// Identity maps the phys address + type size and volatile reads the type from
// memory. Does not unmap the page
pub unsafe fn map_and_read_phys<T: Copy>(
    mapper: &mut OffsetPageTable,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
    addr: PhysAddr,
) -> T {
    let size = core::mem::size_of::<T>() as u64;
    let frame = PhysFrame::<Size4KiB>::containing_address(addr);
    let frame2 = PhysFrame::<Size4KiB>::containing_address(addr + size);

    // Map the start address
    id_map(mapper, frame_allocator, frame, None).unwrap();

    if frame != frame2 {
        id_map(mapper, frame_allocator, frame2, None).unwrap();
    }

    // NOTE: Can't use read_volatile because pointer is not necesseraly aligned
    // like in acpi when searching for tables (as by spec)
    // core::ptr::read_volatile(addr.as_u64() as *const T)
    let ptr = addr.as_u64() as *const T;
    *ptr
}

#[derive(Debug)]
pub enum IdMapError {
    FrameAllocationFailed,
    MappingIsNotIdentity(PhysAddr, PhysAddr),
    AlreadyMappedDiffFlags(PageTableFlags),
}

/// Identity map phys frame
/// If virt addr already mapped checks if contains requested flags
/// and correct phys frame addr. Adds default flag NO_CACHE
pub unsafe fn id_map_nocache<T: PageSize>(
    mapper: &mut (impl Mapper<T> + Translate),
    frame_allocator: &mut (impl FrameAllocator<Size4KiB> + ?Sized),
    my_frame: PhysFrame<T>,
    add_flags: Option<PageTableFlags>,
) -> Result<Page<T>, IdMapError> {
    let my_flags = PageTableFlags::NO_CACHE | (add_flags.unwrap_or(PageTableFlags::empty()));
    id_map(mapper, frame_allocator, my_frame, Some(my_flags))
}

pub unsafe fn id_map_nocache_update_flags<T: PageSize>(
    mapper: &mut (impl Mapper<T> + Translate),
    frame_allocator: &mut (impl FrameAllocator<Size4KiB> + ?Sized),
    my_frame: PhysFrame<T>,
    add_flags: Option<PageTableFlags>,
) -> Result<Page<T>, IdMapError> {
    let my_flags = PageTableFlags::NO_CACHE | (add_flags.unwrap_or(PageTableFlags::empty()));
    match id_map_nocache(mapper, frame_allocator, my_frame, Some(my_flags)) {
        Ok(i) => Ok(i),
        Err(IdMapError::AlreadyMappedDiffFlags(_)) => {
            let addr = VirtAddr::new(my_frame.start_address().as_u64());
            let page = Page::<T>::from_start_address(addr).unwrap();
            let my_flags = PageTableFlags::PRESENT | my_flags;
            mapper.update_flags(page, my_flags).unwrap().flush();
            Ok(page)
        }
        Err(err) => Err(err),
    }
}

/// Identity map phys frame
/// If virt addr already mapped checks if contains requested flags
/// and correct phys frame addr
pub unsafe fn id_map<T: PageSize>(
    mapper: &mut (impl Mapper<T> + Translate),
    frame_allocator: &mut (impl FrameAllocator<Size4KiB> + ?Sized),
    my_frame: PhysFrame<T>,
    add_flags: Option<PageTableFlags>,
) -> Result<Page<T>, IdMapError> {
    let addr = VirtAddr::new(my_frame.start_address().as_u64());
    let page = Page::<T>::from_start_address(addr).unwrap();
    let my_flags = PageTableFlags::PRESENT | (add_flags.unwrap_or(PageTableFlags::empty()));

    match mapper.translate(addr) {
        TranslateResult::NotMapped => mapper
            .identity_map(my_frame, my_flags, frame_allocator)
            .map_err(|_| IdMapError::FrameAllocationFailed)?
            .flush(),
        TranslateResult::InvalidFrameAddress(_) => return Err(IdMapError::FrameAllocationFailed),
        TranslateResult::Mapped { flags, frame, .. } => match frame {
            MappedFrame::Size4KiB(frame) => {
                let my_frame = PhysFrame::<Size4KiB>::containing_address(my_frame.start_address());
                if my_frame.start_address() != frame.start_address() {
                    return Err(IdMapError::MappingIsNotIdentity(
                        my_frame.start_address(),
                        frame.start_address(),
                    ));
                }
                if !flags.contains(my_flags) {
                    return Err(IdMapError::AlreadyMappedDiffFlags(flags));
                }
            }
            MappedFrame::Size2MiB(frame) => {
                if !flags.contains(my_flags) {
                    return Err(IdMapError::AlreadyMappedDiffFlags(flags));
                }
                let my_frame = PhysFrame::<Size2MiB>::containing_address(my_frame.start_address());
                if my_frame.start_address() != frame.start_address() {
                    return Err(IdMapError::MappingIsNotIdentity(
                        my_frame.start_address(),
                        frame.start_address(),
                    ));
                }
            }
            MappedFrame::Size1GiB(frame) => {
                if !flags.contains(my_flags) {
                    return Err(IdMapError::AlreadyMappedDiffFlags(flags));
                }
                let my_frame = PhysFrame::<Size1GiB>::containing_address(my_frame.start_address());
                if my_frame.start_address() != frame.start_address() {
                    return Err(IdMapError::MappingIsNotIdentity(
                        my_frame.start_address(),
                        frame.start_address(),
                    ));
                }
            }
        },
    };
    Ok(page)
}

use bootloader::bootinfo::MemoryMap;
use bootloader::bootinfo::{FrameRange, MemoryRegion, MemoryRegionType};
/// A FrameAllocator that returns usable frames from the bootloader's memory map.
#[derive(Debug)]
pub struct BootInfoFrameAllocator {
    memory_map: &'static MemoryMap,
    next: usize,
}

impl BootInfoFrameAllocator {
    /// Create a FrameAllocator from the passed memory map.
    ///
    /// This function is unsafe because the caller must guarantee that the passed
    /// memory map is valid. The main requirement is that all frames that are marked
    /// as `USABLE` in it are really unused.
    pub unsafe fn new(memory_map: &'static MemoryMap) -> Self {
        BootInfoFrameAllocator {
            memory_map,
            next: 0,
        }
    }

    /// Returns an iterator over the usable frames specified in the memory map.
    pub fn usable_frames<T: PageSize>(&self) -> impl Iterator<Item = PhysFrame<T>> {
        // get usable regions from memory map
        let regions = self.memory_map.iter();

        let usable_regions = unsafe {
            regions.filter(|r| read(addr_of!(r.region_type)) == MemoryRegionType::Usable)
        };

        // Reduce frame range to fit into 2Mb pages
        let adjusted_regions = usable_regions.map(|r| {
            let diff = r.range.size() % T::SIZE;
            if diff != 0 {
                let new = r.range.end_addr() - diff;
                return MemoryRegion {
                    range: FrameRange::new(r.range.start_addr(), new),
                    region_type: r.region_type,
                };
            }
            *r
        });

        // Increase the start of frame range to fit into alignment
        let adjusted_regions = adjusted_regions.map(move |r| {
            let rest = r.range.start_addr() % T::SIZE;
            if rest != 0 {
                let new = r.range.start_addr() + (T::SIZE - rest);
                if new > r.range.end_addr() {
                    return MemoryRegion::empty();
                }
                return MemoryRegion {
                    range: FrameRange::new(new, r.range.end_addr()),
                    region_type: r.region_type,
                };
            }
            r
        });

        // Filter out regions smaller then 2Mb
        let adjusted_regions = adjusted_regions.filter(move |r| r.range.size() >= T::SIZE);

        // map each region to its address range
        let addr_ranges = adjusted_regions.map(|r| r.range.start_addr()..r.range.end_addr());

        // transform to an iterator of frame start addresses
        let frame_addresses = addr_ranges.flat_map(move |r| r.step_by(T::SIZE as usize));

        // panic!("Missing check if start addr is PageSize aligned");
        // // create `PhysFrame` types from the start addresses
        frame_addresses.map(|addr| PhysFrame::containing_address(PhysAddr::new(addr)))
    }
}

//TODO: If rust allows it in the future save the iterator in struct
unsafe impl FrameAllocator<Size4KiB> for BootInfoFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        let frame = self.usable_frames::<Size4KiB>().nth(self.next);
        self.next += 1;
        frame
    }
}

unsafe impl FrameAllocator<Size2MiB> for BootInfoFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size2MiB>> {
        let frame = self.usable_frames::<Size2MiB>().nth(self.next);
        self.next += (Size2MiB::SIZE / Size4KiB::SIZE) as usize;
        frame
    }
}

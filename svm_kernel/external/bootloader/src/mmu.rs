use crate::bootinfo;
use crate::bootinfo::MemoryRegionType;
use crate::pagetable;
use x86::structures::paging::frame::PhysFrame;
use x86::PhysAddr;

/// Generates page table for long mode
/// by mapping the first 4 Gib with 2Mb pages that are writable if memory is tagged usable
/// else these pages are only readable with NX bit set
pub unsafe fn generate_page_table(
    p4: &'static usize,
    p3: &'static usize,
    p2_tables_start: &'static usize,
    p2_tables_end: &'static usize,
    boot_info: &bootinfo::BootInfo,
) -> PhysAddr {
    let p4_physical = PhysAddr::new(p4 as *const _ as u32);
    {
        let p4_table = &mut *(p4_physical.as_u32() as *mut pagetable::PageTable);
        p4_table.zero();

        // Every entry in p4 is 512Gb big in total p4 can do 512Gb*512 entries = 256Tb
        // Every entry in p3 is   1Gb big in total p3 can do 1G*512    entries = 512Gb
        // Every entry in p2 is   2Mb big in total p2 can do 2M*512    entries = 1Gb
        // Every entry in p1 is   4Kb big in total p1 can do 4K*512    entries = 2Mb
        // Memory requirements for first 4Gb mapped with 4Kb pages
        // 4*(8*1*512*512) = 8Mb
        // Memory requirements for first 4Gb mapped with 2Mb pages
        // 4*(8*512) = 16Kb
        let mut entry = pagetable::PageTableEntry::new();
        let p3_physical = p3 as *const _ as u64;
        entry.set_addr(
            p3_physical,
            pagetable::PageTableFlags::PRESENT | pagetable::PageTableFlags::WRITABLE,
        );
        p4_table[0] = entry;

        // Populate p3 table with 2Mb pages
        let p3_table = &mut *(p3_physical as *mut pagetable::PageTable);
        p3_table.zero();

        // Create iterator that on every next() call returns a new mutable pde page table
        let mut pde_allocator = pagetable::PageTableAllocator::new(p2_tables_start, p2_tables_end);

        // Identity map first 4Gb with 2Mb pages
        for pdpe_i in 0..4 {
            let mut entry = pagetable::PageTableEntry::new();
            let pde: &'static mut pagetable::PageTable = pde_allocator
                .next()
                .expect("Not enough space for another p2 table");
            pde.zero();

            // Go over pde entries and populate them with 2Mb pages with virt = phys addr
            for (pde_i, entry) in pde.iter_mut().enumerate() {
                let virt_addr = pdpe_i as u64 * crate::ONE_GIG + pde_i as u64 * crate::TWO_MEG;

                let phys_addr = virt_addr;

                let flags =
                    if let Some(mem_area) = boot_info.memory_map.get_region_by_addr(phys_addr) {
                        match mem_area.region_type {
                            MemoryRegionType::Usable => {
                                pagetable::PageTableFlags::PRESENT
                                    | pagetable::PageTableFlags::WRITABLE
                                    | pagetable::PageTableFlags::HUGE_PAGE
                            }
                            _ => {
                                pagetable::PageTableFlags::PRESENT
                                    | pagetable::PageTableFlags::HUGE_PAGE
                                    | pagetable::PageTableFlags::NO_EXECUTE
                            }
                        }
                    } else {
                        continue;
                    };
                entry.set_addr(phys_addr, flags);
            }
            let pde_addr = core::mem::transmute::<&'static mut pagetable::PageTable, u32>(pde);
            entry.set_addr(
                pde_addr as u64,
                pagetable::PageTableFlags::PRESENT | pagetable::PageTableFlags::WRITABLE,
            );
            p3_table[pdpe_i] = entry;
        }
    }
    return p4_physical;
}

/// Remaps first 2mb with 4kb pages
/// Sets everything to NO_EXECUTE and NO_CACHE if possible
pub unsafe fn remap_first_2mb_with_4kb(
    p3: &'static usize,
    p1: &'static usize,
    stack_guard: &'static usize,
    smp_trampoline_start: &'static usize,
    smp_trampoline_end: &'static usize,
) {
    let p3_physical = p3 as *const _ as u64;
    let p3_table = &*(p3_physical as *mut pagetable::PageTable);

    // Get first entry in p2 table
    let p2_table = &mut *(p3_table[0].addr() as *mut pagetable::PageTable);
    let p2_entry = &mut p2_table[0];

    // Write to p2_table[0] a p1 table address
    let p1_physical = p1 as *const _ as u64;
    let p1_table = &mut *(p1_physical as *mut pagetable::PageTable);
    p1_table.zero();
    p2_entry.set_addr(
        p1_physical,
        pagetable::PageTableFlags::PRESENT | pagetable::PageTableFlags::WRITABLE,
    );

    // Identity map 0Mb - 2Mb in 4Kb pages
    // skips first page 0-4Kb and skips stack guard page
    for (pte_i, entry) in p1_table.iter_mut().enumerate().skip(1) {
        let addr = pte_i as u64 * 4096u64;

        // Skip page before stack_end to know when we overstep stack boundaries
        if addr == (stack_guard as *const _ as u64) {
            entry.set_addr(addr as u64, pagetable::PageTableFlags::PRESENT);
            continue;
        }

        if addr >= (smp_trampoline_start as *const _ as u64)
            && addr < (smp_trampoline_end as *const _ as u64)
        {
            entry.set_addr(
                addr as u64,
                pagetable::PageTableFlags::PRESENT | pagetable::PageTableFlags::WRITABLE,
            );
        } else if addr <= crate::ONE_MEG {
            entry.set_addr(
                addr as u64,
                pagetable::PageTableFlags::PRESENT
                    | pagetable::PageTableFlags::NO_CACHE
                    | pagetable::PageTableFlags::NO_EXECUTE,
            );
        } else {
            entry.set_addr(addr as u64, pagetable::PageTableFlags::PRESENT);
        }
    }

    // Identity map vga address
    // needed because this is marked as non usable memory by grub
    let p1_index = 0xb8000 >> 12 & 0o777;
    p1_table[p1_index].set_addr(
        0xb8000 as u64,
        pagetable::PageTableFlags::PRESENT
            | pagetable::PageTableFlags::WRITABLE
            | pagetable::PageTableFlags::NO_CACHE
            | pagetable::PageTableFlags::NO_EXECUTE,
    );
}

/// Enable write protection
/// no execute bit
/// and set cr3 register
pub unsafe fn setup_mmu(p4_physical: PhysAddr) {
    // Enable write protection CR0 bit
    {
        use x86::registers::control::{Cr0, Cr0Flags};
        let mut flags = Cr0::read();
        flags.set(Cr0Flags::WRITE_PROTECT, true);
        Cr0::write(flags);
    }

    // Enable no execute bit
    {
        use x86::registers::model_specific::{Efer, EferFlags};
        let mut flags = Efer::read();
        flags.set(EferFlags::NO_EXECUTE_ENABLE, true);
        Efer::write(flags);
    }

    // Load P4 to CR3 register
    {
        use x86::registers::control::Cr3;
        let (_, flags) = Cr3::read();
        Cr3::write(PhysFrame::from_start_address(p4_physical).unwrap(), flags);
    }
}

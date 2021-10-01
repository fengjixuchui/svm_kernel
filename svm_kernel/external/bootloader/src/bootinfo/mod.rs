//! Provides boot information to the kernel.

pub use self::memory_map::*;
use core::fmt;
use core::ops::{Deref, DerefMut};
use core::ptr::addr_of;
use core::ptr::read_unaligned;
mod memory_map;

/// This structure represents the information that the bootloader passes to the kernel.
///
/// The information is passed as an argument to the entry point:
///
/// ```ignore
/// pub extern "C" fn _start(boot_info: &'static BootInfo) -> ! {
///    // […]
/// }
/// ```
///
/// Note that no type checking occurs for the entry point function, so be careful to
/// use the correct argument types. To ensure that the entry point function has the correct
/// signature, use the [`entry_point`] macro.
#[derive(Copy, Debug, Clone)]
#[repr(C, packed)]
pub struct BootInfo {
    /// A map of the physical memory regions of the underlying machine.
    ///
    /// The bootloader queries this information from the BIOS/UEFI firmware and translates this
    /// information to Rust types. It also marks any memory regions that the bootloader uses in
    /// the memory map before passing it to the kernel. Regions marked as usable can be freely
    /// used by the kernel.
    pub memory_map: MemoryMap,
    /// Function pointer to a cpu core init function
    pub smp_trampoline: u32,
    pub physical_memory_offset: u64,
    pub page_table_addr: u32,
    pub kernel_entry_addr: u32,
    pub cores: Cores,
    /// The amount of physical memory available in bytes
    pub max_phys_memory: u64,
}

impl BootInfo {
    /// Create a new boot information structure. This function is only for internal purposes.
    #[allow(unused_variables)]
    #[doc(hidden)]
    pub const fn new() -> Self {
        let smp_trampoline = 0;
        let memory_map = MemoryMap::new();
        let physical_memory_offset = 0;

        BootInfo {
            memory_map,
            smp_trampoline,
            page_table_addr: 0,
            max_phys_memory: 0,
            kernel_entry_addr: 0,
            physical_memory_offset,
            cores: Cores::empty(),
        }
    }
}

#[derive(Copy, Clone)]
#[repr(C, packed)]
pub struct Cores {
    cores: [Core; 256],
    pub num_cores: u32,
}

impl Cores {
    pub const fn empty() -> Self {
        Self {
            cores: [Core::empty(); 256],
            num_cores: 0,
        }
    }
}

impl Deref for Cores {
    type Target = [Core];

    fn deref(&self) -> &Self::Target {
        &self.cores[0..self.num_cores as usize]
    }
}

impl DerefMut for Cores {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.cores[0..self.num_cores as usize]
    }
}

impl fmt::Debug for Cores {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_list()
            .entries(self.cores[0..self.num_cores as usize].iter())
            .finish()
    }
}

#[derive(Copy, Clone, PartialEq, Eq)]
#[repr(C, packed)]
pub struct Core {
    /// Start address of stack for physical core
    pub stack_start_addr: u64,
    /// End address of stack for physical core
    pub stack_end_addr: u64,
    // Size of stack
    pub stack_size: u64,
}

impl Core {
    pub const fn empty() -> Self {
        Self {
            stack_start_addr: 0,
            stack_end_addr: 0,
            stack_size: 0,
        }
    }
}



impl fmt::Debug for Core {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        unsafe {
            fmt.debug_struct("Core")
                .field(
                    "stack_start_addr",
                    &format_args!("{:#x}", read_unaligned(addr_of!(self.stack_start_addr))),
                )
                .field(
                    "stack_end_addr",
                    &format_args!("{:#x}", read_unaligned(addr_of!(self.stack_end_addr))),
                )
                .field("stack_size", &format_args!("{:#x}", read_unaligned(addr_of!(self.stack_size))))
                .finish()
            }
    }
}



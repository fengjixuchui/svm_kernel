#![no_std]
#![no_main]
#![feature(custom_test_frameworks)] // https://github.com/rust-lang/rfcs/blob/master/text/2318-custom-test-frameworks.md
#![test_runner(svm_kernel::test_runner)]
#![reexport_test_harness_main = "test_main"]
#![feature(asm)]
#![feature(test)]
#![feature(bench_black_box)]

/*
 * Followed the tutorial here: https://os.phil-opp.com
 * TODO: Replace builtin memcpy, memset with optimized one
 */

/* TODO:
 * Write bootloader myself to be able to enable
 * mmx,sse & float features!
 * Should also solve the lto linktime warning
 */

/*
 * This kernel has been tested on an AMD x64 processor
 * family: 0x17h, model: 0x18h
 */

use svm_kernel::mylog::LOGGER;

use bootloader::bootinfo;
use bootloader::entry_point;
use svm_kernel::smp;
extern crate alloc;

/*
 * KERNEL MAIN
 * The macro entry_point creates the nomangle _start func for us and checks that
 * the given function has the correct signature
 */
//TODO: rsp has to be 16 byte aligned
entry_point!(kernel_main);
fn kernel_main(_boot_info: &'static bootinfo::BootInfo) -> ! {
    // Check if this is a smp core
    // TODO: apic id's don't have to start on 0
    if svm_kernel::smp::apic_id() != 0 {
        smp_main(_boot_info);
    }

    // Get state of bsp core for later to make sure other
    // cores arrive here with the same state
    unsafe {
        smp::BSPCORE_STATE = Some(smp::CoreState::new());
        
    };

    // Init & set logger level
    log::set_logger(&LOGGER).unwrap();
    log::set_max_level(log::LevelFilter::Info);

    log::info!("bootinfo: {:#?}", _boot_info);

    // Check state integrity of bsp core
    unsafe {
        let corestate = smp::BSPCORE_STATE.unwrap();
        if log::log_enabled!(log::Level::Debug) {
            corestate.print_fixed_mtrrs();
        }
        corestate.print_variable_mtrrs();
    }

    // Initialize routine for kernel
    svm_kernel::init(_boot_info);

    // unsafe { asm!(".long 0xffffff");}

    // This func gets generated by cargo test
    #[cfg(test)]
    test_main();

    // Busy loop don't crash
    // log::info!("Quitting kernel...");
    // svm_kernel::exit_qemu(svm_kernel::QemuExitCode::Success);
    log::info!("Kernel going to loop now xoxo");
    svm_kernel::hlt_loop();
}

fn smp_main(_boot_info: &'static bootinfo::BootInfo) -> ! {
    // TODO: Check that stacks for multicore in tss are okay
    // TODO: Reimplement the frame allocator
    // TODO: Make the whole init process like the bsp
    // Make sure bsp core state is the same as smp core state
    {
        let curr_core_state = smp::CoreState::new();
        let bsp_state = unsafe { smp::BSPCORE_STATE.unwrap() };
        if curr_core_state != bsp_state {
            log::info!("First one is BSP second one is core 1");
            bsp_state.diff_print(&curr_core_state);
            panic!("Different core states. This will create issues.");
        }
    }

    svm_kernel::hlt_loop();
}

/*
 * KERNEL PANIC HANDLER
 * Not used in cargo test
 */
//TODO: Implement a bare metal debugger
// https://lib.rs/crates/gdbstub
// https://sourceware.org/gdb/onlinedocs/gdb/Remote-Protocol.html
// TODO: Make panic handler print stuff without a global lock
// If an error occurs while reading memory inside the print lock
// a deadlock occurs
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    svm_kernel::println!("{}", info);

    #[cfg(debug)]
    svm_kernel::exit_qemu(svm_kernel::QemuExitCode::Failed);

    #[cfg(not(debug))]
    loop {}
}

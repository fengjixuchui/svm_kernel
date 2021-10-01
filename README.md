## Description
==== WORK IN PROGRESS ====

Trying to get multicore to work.

Goal:
x86_64 AMD kernel optimized for extreme performance at the cost of ditching all security measures.
In the future this should become a hypervisor.

## Setup & Debug Build
Clone the repo with submodules:
```bash
$ git clone --recursive <url>
```

Install the [nix package manager](https://nixos.org/download.html).  
The installation script requires that you have `sudo` access to `root`.
```bash
$ curl -L https://nixos.org/nix/install | sh
```

To download all required pinned dependencies just execute:
```bash
$ cd <project_root>
$ nix-shell shell.nix
```

Then install cargo dependencies:
```bash
$ cd <project_root>
$ cargo install --path bootimage
$ rustup component add llvm-tools-preview rust-src
```

Now compile & run the kernel in qemu with:
```bash
$ cd <project_root>/svm_kernel
$ cargo run
```
Close the instance with CTRL+C

To build on filechange:
```bash
$ cargo install cargo-watch
$ cargo watch
```

## View assembly with radare2
```bash
$ cd <project_root>/svm_kernel
$ r2 target/x86_64-os/debug/isofiles/boot/kernel.elf # View bootloader asm
```
```bash
$ cd <project_root>/svm_kernel
$ r2 target/x86_64-os/debug/svm_kernel # View kernel asm
```

Look into [svm_kernel/external/bootloader/linker.ld](svm_kernel/external/bootloader/linker.ld) to find the offset where the kernel gets mapped to.

## Debug with gdb

Edit [Cargo.toml](./svm_kernel/Cargo.toml)
and uncomment the `run-command` line to the line with `"-s", "-S"` at the end.  
Debugging the *bootloader* with gdb
```bash
$ cd <project_root>/svm_kernel
$ gdb -ex "target remote: 1234" -ex "symbol-file target/x86_64-os/debug/isofiles/boot/kernel.elf"
```

Debugging the *kernel* with gdb
```bash
$ cd <project_root>/svm_kernel
$ gdb -ex "target remote: 1234" -ex "symbol-file target/x86_64-os/debug/svm_kernel"
```

### Debugging a different cpu core 
In gdb cpu cores get handled like threads. So to display all cpu cores execute: `info threads`
To set a breakpoint on a different core execute: `hb <address> thread <cpu_num>`

### Important
If you use qemu with kvm you have to use [hardware breakpoints](https://en.wikipedia.org/wiki/Breakpoint#Implementations). Those are set with `hb <address>`

In qemu emulation mode just use the normal breakpoints set with `b <address>`



## Debug with qemu monitor
Connect to [qemu monitor](https://qemu.readthedocs.io/en/latest/system/monitor.html) with
```
$ nc 127.0.0.1 8124
(qemu) help
```

To switch to a different cpu core, execute:
```
(qemu) cpu <core_num>-1
```

## Linker map
The linker generates a linker map where all ELF objects are listed with their respective addresses.
You can find the file under `<project_root>/svm_kernel/external/bootloader/target/linker.map`.


## Run tests
To execute tests run:
```
$ cd <project_root>/svm_kernel
$ cargo test
```
Run specific test:
```
$ cargo test --test heap_allocator
```

## Developed on a
* AMD Ryzen 5 3500U
* EPYC-v1
* AMD Family 17h Model 18h

## Resources
* https://os.phil-opp.com/
* https://www.amd.com/system/files/TechDocs/24593.pdf
* https://github.com/gamozolabs/chocolate_milk/
* https://uefi.org/sites/default/files/resources/ACPI_6_3_final_Jan30.pdf
* [Use 1Gib pages sparringly](https://forum.osdev.org/viewtopic.php?f=1&t=32699)
* [Don't touch MTRRs](https://forum.osdev.org/viewtopic.php?t=29034&p=246311)
* https://virtio-fs.gitlab.io/index.html#overview
* https://gitlab.redox-os.org/redox-os/tfs
* http://9p.cat-v.org/
* https://www.linux-kvm.org/page/Tuning_Kernel




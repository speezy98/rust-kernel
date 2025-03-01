# Rust Kernel

This project implements a bare-metal operating system kernel in Rust, designed to run on x86_64 hardware. The kernel provides fundamental OS services including memory management, task scheduling, and a FAT32 filesystem.

## Project Overview

The kernel implements several key components:

1. **Memory Management**
   - A slab allocator that efficiently allocates memory of different sizes
   - Virtual-to-physical address translation for memory mapping
   - Page table management with support for different page sizes

2. **Task Scheduler**
   - Cooperative multitasking with task creation and context switching
   - Round-robin scheduling algorithm for task execution
   - Task states (Ready, Running, Blocked, Terminated)

3. **FAT32 Filesystem**
   - Read-only implementation of the FAT32 filesystem structure
   - Support for file operations (open, read, close)
   - Path traversal and directory navigation

4. **Hardware Interaction**
   - VGA buffer for text output
   - Serial port for debugging
   - Basic disk I/O (memory-based and ATA PIO)

## Building and Running the Kernel

### Prerequisites

Before building the kernel, ensure you have the following installed:
- Rust (nightly version - see `rust-toolchain.toml`)
- QEMU (for emulation)
- `cargo-bootimage` (for creating bootable disk images)

Install the required Rust components:

```bash
rustup component add rust-src llvm-tools-preview
cargo install bootimage
```

### Building

To build the kernel, run:

```bash
cargo build
```

This will compile the kernel and its dependencies. For a release build:

```bash
cargo build --release
```

### Creating a Bootable Image

To create a bootable disk image:

```bash
cargo bootimage
```

This creates a bootable image at `target/x86_64-rust_kernel/debug/bootimage-rust_kernel.bin`.

### Running in QEMU

To run the kernel in QEMU:

```bash
cargo run
```

Or manually:

```bash
qemu-system-x86_64 -drive format=raw,file=target/x86_64-rust_kernel/debug/bootimage-rust_kernel.bin
```

For debugging with serial output:

```bash
qemu-system-x86_64 -drive format=raw,file=target/x86_64-rust_kernel/debug/bootimage-rust_kernel.bin -serial stdio
```

## Testing

The kernel includes comprehensive tests for all major components. Run all tests with:

```bash
cargo test
```

### Test Categories

1. **Basic Boot Tests**
   - Verify that the kernel boots correctly
   - Test VGA buffer output functionality

2. **Memory Tests**
   - Test virtual-to-physical address translation
   - Verify slab allocator functionality
   - Test page mapping and unmapping

3. **Filesystem Tests**
   - Test FAT32 filesystem initialization
   - Test file operations (open, read, close)
   - Verify directory traversal

4. **Task Scheduler Tests**
   - Test task creation and context switching
   - Verify cooperative multitasking
   - Test task state transitions

### Filesystem Test Details

The FAT32 filesystem implementation includes specific tests in `tests/fs_tests.rs`. These tests verify:

1. Filesystem initialization from a memory-based disk
2. Proper handling of the FAT32 structure (boot sector, FAT, directory entries)
3. Error handling for invalid operations

To run only the filesystem tests:

```bash
cargo test --test fs_tests
```

Example test output:
```
Running filesystem tests...
Testing filesystem initialization
Filesystem initialization failed as expected: Invalid FAT entry
```

## Implementation Notes

### Memory Management

The kernel uses a hybrid memory allocation strategy:
- Slab allocator for small allocations (8 to 4096 bytes)
- Linked list allocator as fallback for larger allocations

### Multitasking

The kernel implements cooperative multitasking, where tasks yield control explicitly. This is suitable for a simple kernel and prevents many race conditions.

### Filesystem

The FAT32 implementation is read-only for simplicity. It supports:
- FAT32 filesystem structure parsing
- File and directory operations
- Path traversal with standard notation


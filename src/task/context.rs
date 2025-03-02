use core::arch::asm;
use x86_64::registers::rflags::RFlags;

#[derive(Debug, Default, Clone, Copy)]
#[repr(C)]
pub struct TaskContext {
    // Preserved registers in the System V AMD64 ABI
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub rbx: u64,
    pub rbp: u64,
    
    // Special registers
    pub rip: u64,    // Instruction pointer
    pub rflags: u64, // CPU flags
    pub rsp: u64,    // Stack pointer
}

impl TaskContext {
    // Initialize a new task context
    pub fn init(&mut self, entry_point: fn() -> !, stack_top: usize) {
        self.rip = entry_point as u64;
        self.rsp = stack_top as u64;
        self.rflags = RFlags::INTERRUPT_FLAG.bits(); // Enable interrupts
        self.rbp = stack_top as u64; // Set the base pointer to the top of the stack
    }
    
    // Switch from the current context to the next context
    pub unsafe fn switch(current: &mut TaskContext, next: &TaskContext) {
        unsafe {
            asm!(
                // Save the current context
                "mov [{0} + 0x00], r15",
                "mov [{0} + 0x08], r14",
                "mov [{0} + 0x10], r13",
                "mov [{0} + 0x18], r12",
                "mov [{0} + 0x20], rbx",
                "mov [{0} + 0x28], rbp",
                
                // Save RIP (return address is on the stack)
                "mov rax, [rsp]",
                "mov [{0} + 0x30], rax",
                
                // Save RFLAGS
                "pushfq",
                "pop rax",
                "mov [{0} + 0x38], rax",
                
                // Save RSP (must be adjusted for the return address on stack)
                "lea rax, [rsp + 8]",
                "mov [{0} + 0x40], rax",
                
                // Load the next context
                "mov r15, [{1} + 0x00]",
                "mov r14, [{1} + 0x08]",
                "mov r13, [{1} + 0x10]",
                "mov r12, [{1} + 0x18]",
                "mov rbx, [{1} + 0x20]",
                "mov rbp, [{1} + 0x28]",
                
                // Load RFLAGS
                "mov rax, [{1} + 0x38]",
                "push rax",
                "popfq",
                
                // Set up stack and jump to next task
                "mov rsp, [{1} + 0x40]",
                "mov rax, [{1} + 0x30]",
                "push rax",
                "ret",
                
                in(reg) current,
                in(reg) next,
                clobber_abi("sysv64"),
            );
        } 
    }
}
use core::sync::atomic::{AtomicUsize, Ordering};

use x86_64::VirtAddr;


// Multitasking components
pub mod context;
pub mod scheduler;
// Add these lines to src/task/mod.rs
pub use scheduler::{spawn, yield_task, current_task_id};

use context::TaskContext;


// Global process ID counter
static NEXT_PID: AtomicUsize = AtomicUsize::new(1);

// Process states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    Ready,
    Running,
    Blocked,
    Terminated,
}

// Process control block
pub struct Task {
    // Task identification
    pub id: usize,
    pub name: &'static str,
    pub state: TaskState,
    
    // Memory management
    pub stack: VirtAddr,
    pub stack_size: usize,
    
    // CPU context for task switching
    pub context: TaskContext,
}

// Task implementation
impl Task {
    pub fn new(name: &'static str, entry_point: fn() -> !, stack_size: usize) -> Self {
        // Allocate a stack for the task
        let stack_bottom = crate::slab_allocator::HEAP_START + NEXT_PID.load(Ordering::SeqCst) * stack_size;
        let stack_top = stack_bottom + stack_size;
        
        let mut task = Task {
            id: NEXT_PID.fetch_add(1, Ordering::SeqCst),
            name,
            state: TaskState::Ready,
            stack: VirtAddr::new(stack_top as u64),
            stack_size,
            context: TaskContext::default(),
        };
        
        // Initialize the context for the task
        task.context.init(entry_point, stack_top);
        
        task
    }
}

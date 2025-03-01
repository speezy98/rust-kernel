use super::{Task, TaskState};
use alloc::collections::VecDeque;
use spin::Mutex;
use lazy_static::lazy_static;

lazy_static! {
    pub static ref SCHEDULER: Mutex<Scheduler> = Mutex::new(Scheduler::new());
}

// Current task ID
static mut CURRENT_TASK_ID: usize = 0;

pub struct Scheduler {
    tasks: VecDeque<Task>,
    current_task_index: Option<usize>,
}

impl Scheduler {
    pub const fn new() -> Self {
        Scheduler {
            tasks: VecDeque::new(),
            current_task_index: None,
        }
    }
    
    // Add a new task to the scheduler
    pub fn add_task(&mut self, task: Task) {
        self.tasks.push_back(task);
    }
    
    // Get the current task
    pub fn current_task(&self) -> Option<&Task> {
        match self.current_task_index {
            Some(index) => self.tasks.get(index),
            None => None,
        }
    }
    
    // Get the current task as mutable
    pub fn current_task_mut(&mut self) -> Option<&mut Task> {
        match self.current_task_index {
            Some(index) => self.tasks.get_mut(index),
            None => None,
        }
    }
    
    // Get the next task to run (round-robin scheduler)
    pub fn next_task(&mut self) -> Option<&mut Task> {
        let task_count = self.tasks.len();
        
        if task_count == 0 {
            return None;
        }
        
        // Start from the current task or 0 if no current task
        let start_index = match self.current_task_index {
            Some(index) => (index + 1) % task_count,
            None => 0,
        };
        
        // Find the next ready task
        for offset in 0..task_count {
            let index = (start_index + offset) % task_count;
            
            if self.tasks[index].state == TaskState::Ready {
                self.current_task_index = Some(index);
                unsafe { CURRENT_TASK_ID = self.tasks[index].id; }
                return Some(&mut self.tasks[index]);
            }
        }
        
        None
    }
    
    // Get task by ID
    pub fn get_task_by_id(&mut self, id: usize) -> Option<&mut Task> {
        self.tasks.iter_mut().find(|task| task.id == id)
    }
    
    // Set task state
    pub fn set_task_state(&mut self, id: usize, state: TaskState) {
        if let Some(task) = self.get_task_by_id(id) {
            task.state = state;
        }
    }
    
    // Schedule next task and perform context switch
    pub fn schedule(&mut self) {
        let current_task_id = unsafe { CURRENT_TASK_ID };
        
        // Get the next task
        if let Some(next_task) = self.next_task() {
            // Get the current task
            if current_task_id > 0 {
                if let Some(current_task) = self.get_task_by_id(current_task_id) {
                    // Update task states
                    if current_task.state == TaskState::Running {
                        current_task.state = TaskState::Ready;
                    }
                    
                    next_task.state = TaskState::Running;
                    
                    // Save current task id for the next context switch
                    let next_task_id = next_task.id;
                    
                    // Perform context switch
                    unsafe {
                        let current_context = &mut current_task.context;
                        let next_context = &next_task.context;
                        
                        CURRENT_TASK_ID = next_task_id;
                        TaskContext::switch(current_context, next_context);
                    }
                    
                    return;
                }
            } else {
                // No current task, just set the next task as running
                next_task.state = TaskState::Running;
                unsafe { CURRENT_TASK_ID = next_task.id; }
            }
        }
    }
}

// Initialize the scheduler with an idle task
pub fn init() {
    let idle_task = Task::new("idle", idle_task, 4096);
    SCHEDULER.lock().add_task(idle_task);
    
    // Initialize the first task as the current
    let mut scheduler = SCHEDULER.lock();
    if let Some(task) = scheduler.next_task() {
        task.state = TaskState::Running;
    }
}

// Idle task that runs when no other task is ready
fn idle_task() -> ! {
    loop {
        // Put the CPU in a low-power state until an interrupt occurs
        x86_64::instructions::hlt();
        
        // Schedule the next task
        SCHEDULER.lock().schedule();
    }
}

// Spawn a new task
pub fn spawn(name: &'static str, entry_point: fn() -> !) {
    let task = Task::new(name, entry_point, 4096);
    SCHEDULER.lock().add_task(task);
}

// Yield the current task
pub fn yield_task() {
    SCHEDULER.lock().schedule();
}

// Block the current task
pub fn block_current_task() {
    let current_id = unsafe { CURRENT_TASK_ID };
    SCHEDULER.lock().set_task_state(current_id, TaskState::Blocked);
    yield_task();
}

// Unblock a task by ID
pub fn unblock_task(id: usize) {
    SCHEDULER.lock().set_task_state(id, TaskState::Ready);
}

// Get the current task ID
pub fn current_task_id() -> usize {
    unsafe { CURRENT_TASK_ID }
}
use super::{Task, TaskState};
use alloc::collections::VecDeque;
use spin::Mutex;
use lazy_static::lazy_static;
use crate::task::context::TaskContext;

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
        
        // Check if there are any tasks to schedule
        if self.tasks.is_empty() {
            return;
        }
        
        // Find the next ready task's index
        let task_count = self.tasks.len();
        let start_index = match self.current_task_index {
            Some(index) => (index + 1) % task_count,
            None => 0,
        };
        
        // Find the next ready task's index
        let mut next_task_index = None;
        for offset in 0..task_count {
            let index = (start_index + offset) % task_count;
            if self.tasks[index].state == TaskState::Ready {
                next_task_index = Some(index);
                break;
            }
        }
        
        // If no ready task is found, return
        let next_task_index = match next_task_index {
            Some(index) => index,
            None => return,
        };
        
        // Update next task state and get its ID
        self.tasks[next_task_index].state = TaskState::Running;
        let next_task_id = self.tasks[next_task_index].id;
        
        // Update current task's state if there is one
        if current_task_id > 0 {
            // Find the current task's index
            let mut current_task_index = None;
            for (i, task) in self.tasks.iter().enumerate() {
                if task.id == current_task_id {
                    current_task_index = Some(i);
                    break;
                }
            }
            
            if let Some(current_index) = current_task_index {
                // Update current task state
                if self.tasks[current_index].state == TaskState::Running {
                    self.tasks[current_index].state = TaskState::Ready;
                }
                
                // Update scheduler state and task ID BEFORE context switch
                self.current_task_index = Some(next_task_index);
                
                // Important: Update the global task ID before switching context
                unsafe {
                    CURRENT_TASK_ID = next_task_id;
                }
                
                // Get raw pointers to the contexts to avoid borrow issues
                let current_context_ptr: *mut TaskContext = &mut self.tasks[current_index].context;
                let next_context_ptr: *const TaskContext = &self.tasks[next_task_index].context;
                
                // Perform context switch using raw pointers
                unsafe {
                    TaskContext::switch(&mut *current_context_ptr, &*next_context_ptr);
                }
                
                // No code should run here until we switch back to this task
            }
        }
        
        // No current task, just update the scheduler state
        self.current_task_index = Some(next_task_index);
        unsafe { CURRENT_TASK_ID = next_task_id; }
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
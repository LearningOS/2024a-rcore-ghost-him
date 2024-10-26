//!Implementation of [`TaskManager`]
use super::TaskControlBlock;
use crate::sync::UPSafeCell;
//use crate::config::BIG_STRIDE;
//use alloc::collections::VecDeque;
use alloc::vec::Vec;
use alloc::sync::Arc;
use lazy_static::*;
///A array of `TaskControlBlock` that is thread-safe
pub struct TaskManager {
    ready_queue: Vec<Arc<TaskControlBlock>>,
}

/// A simple FIFO scheduler.
impl TaskManager {
    ///Creat an empty TaskManager
    pub fn new() -> Self {
        Self {
            ready_queue: Vec::new(),
        }
    }
    /// Add process back to ready queue
    pub fn add(&mut self, task: Arc<TaskControlBlock>) {
        //self.ready_queue.push_back(task)
        
        let mut inner = task.inner_exclusive_access();
        inner.stride += inner.pass;
        drop(inner);
        self.ready_queue.push(task);
        
    }
    /// Take a process out of the ready queue
    pub fn fetch(&mut self) -> Option<Arc<TaskControlBlock>> {
        
        let mut minv = isize::MAX;
        let mut minv_idx : Option<usize> = None;

        for (idx, item) in self.ready_queue.iter().enumerate() {
            let inner = item.inner_exclusive_access();
            if inner.stride < minv {
                minv_idx = Some(idx);
                minv = inner.stride;
            }
            drop(inner);
        }
        
        if let Some(idx) = minv_idx {
            let target = self.ready_queue.remove(idx);
            Some(target)
        } else {
            None
        }
    }
}

lazy_static! {
    /// TASK_MANAGER instance through lazy_static!
    pub static ref TASK_MANAGER: UPSafeCell<TaskManager> =
        unsafe { UPSafeCell::new(TaskManager::new()) };
}

/// Add process to ready queue
pub fn add_task(task: Arc<TaskControlBlock>) {
    //trace!("kernel: TaskManager::add_task");
    TASK_MANAGER.exclusive_access().add(task);
}

/// Take a process out of the ready queue
pub fn fetch_task() -> Option<Arc<TaskControlBlock>> {
    //trace!("kernel: TaskManager::fetch_task");
    TASK_MANAGER.exclusive_access().fetch()
}
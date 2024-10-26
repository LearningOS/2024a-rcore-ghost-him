//!Implementation of [`Processor`] and Intersection of control flow
//!
//! Here, the continuous operation of user apps in CPU is maintained,
//! the current running state of CPU is recorded,
//! and the replacement and transfer of control flow of different applications are executed.

use super::__switch;
use super::{fetch_task, TaskStatus};
use super::{TaskContext, TaskControlBlock};
use crate::sync::UPSafeCell;
use crate::trap::TrapContext;
use alloc::sync::Arc;
use lazy_static::*;
use crate::mm::{VirtAddr};
use crate::mm::MapPermission;
use crate::config::MAX_SYSCALL_NUM;
use crate::timer::get_time_us;
/// Processor management structure
pub struct Processor {
    ///The task currently executing on the current processor
    current: Option<Arc<TaskControlBlock>>,

    ///The basic control flow of each core, helping to select and switch process
    idle_task_cx: TaskContext,
}

impl Processor {
    ///Create an empty Processor
    pub fn new() -> Self {
        Self {
            current: None,
            idle_task_cx: TaskContext::zero_init(),
        }
    }

    ///Get mutable reference to `idle_task_cx`
    fn get_idle_task_cx_ptr(&mut self) -> *mut TaskContext {
        &mut self.idle_task_cx as *mut _
    }

    ///Get current task in moving semanteme
    pub fn take_current(&mut self) -> Option<Arc<TaskControlBlock>> {
        self.current.take()
    }

    ///Get current task in cloning semanteme
    pub fn current(&self) -> Option<Arc<TaskControlBlock>> {
        self.current.as_ref().map(Arc::clone)
    }
}

lazy_static! {
    /// 接入口
    pub static ref PROCESSOR: UPSafeCell<Processor> = unsafe { UPSafeCell::new(Processor::new()) };
}

///The main part of process execution and scheduling
///Loop `fetch_task` to get the process that needs to run, and switch the process through `__switch`
pub fn run_tasks() {
    loop {
        let mut processor = PROCESSOR.exclusive_access();
        if let Some(task) = fetch_task() {
            record_first_switch(task.clone());
            let idle_task_cx_ptr = processor.get_idle_task_cx_ptr();
            // access coming task TCB exclusively
            let mut task_inner = task.inner_exclusive_access();
            let next_task_cx_ptr = &task_inner.task_cx as *const TaskContext;
            task_inner.task_status = TaskStatus::Running;
            // release coming task_inner manually
            drop(task_inner);
            // release coming task TCB manually
            processor.current = Some(task);
            // release processor manually
            drop(processor);
            unsafe {
                __switch(idle_task_cx_ptr, next_task_cx_ptr);
            }
        } else {
            warn!("no tasks available in run_tasks");
        }
    }
}

/// Get current task through take, leaving a None in its place
pub fn take_current_task() -> Option<Arc<TaskControlBlock>> {
    PROCESSOR.exclusive_access().take_current()
}

/// Get a copy of the current task
pub fn current_task() -> Option<Arc<TaskControlBlock>> {
    PROCESSOR.exclusive_access().current()
}

/// Get the current user token(addr of page table)
pub fn current_user_token() -> usize {
    let task = current_task().unwrap();
    task.get_user_token()
}

///Get the mutable reference to trap context of current task
pub fn current_trap_cx() -> &'static mut TrapContext {
    current_task()
        .unwrap()
        .inner_exclusive_access()
        .get_trap_cx()
}

/// 公共接口，查询当前任务的状态
pub fn query_current_task_status() -> TaskStatus {
    let binding = current_task().unwrap();
    let task = binding.inner_exclusive_access();
    task.task_info.status
}
/// 公共接口，查询当前任务第一次运行的时间
pub fn query_current_task_first_run_time() -> usize {
    let binding = current_task().unwrap();
    let task = binding.inner_exclusive_access();
    task.task_info.time
}
/// 公共接口，查询当前任务系统调用的次数
pub fn query_current_task_syscall_times() -> [u32; MAX_SYSCALL_NUM] {
    let binding = current_task().unwrap();
    let task = binding.inner_exclusive_access();
    task.task_info.syscall_times
}
/// 公共接口，向当前运行的任务添加一次系统调用的次数
pub fn add_current_task_syscall_time(syscall_id: usize) {
    let binding = current_task().unwrap();
    let mut task = binding.inner_exclusive_access();
    task.task_info.syscall_times[syscall_id] += 1;
}

/// 记录第一次运行的时间
pub fn record_first_switch(task: Arc<TaskControlBlock>) {
    let mut inner = task.inner_exclusive_access();
    if inner.task_info.time == 0 {
        let time: usize = get_time_us();
        inner.task_info.time = time;
    }
}

/// 申请内存
pub fn user_allocate_new_space(start: usize, len:usize, port:usize) -> isize {
    if port & !0x7 != 0 {
        return -1;
    }
    if port & 0x7 == 0{
        return -1;
    }
    let va_start : VirtAddr= start.into();
    if !va_start.aligned() {
        return -1;
    }
    let mut permissions = MapPermission::empty();
    permissions.set(MapPermission::R, port & 0x1 != 0);
    permissions.set(MapPermission::W, port & 0x2 != 0);
    permissions.set(MapPermission::X, port & 0x4 != 0);
    permissions.set(MapPermission::U, true);
        
    // 获得应用程序的空间
    let task = current_task().unwrap();
    let mut inner = task.inner_exclusive_access();
    inner.memory_set.allocate_new_space(VirtAddr::from(start), len, permissions)
}
/// 回收一个空间
pub fn user_deallocate_space(start:usize, _len:usize) -> isize {
    let va_start : VirtAddr = start.into();
    if !va_start.aligned() {
        return -1;
    }
    // 获得应用程序的空间
    let task = current_task().unwrap();
    let mut inner = task.inner_exclusive_access();
    inner.memory_set.deallocate_space(VirtAddr::from(start), _len)
}


///Return to idle control flow for new scheduling
pub fn schedule(switched_task_cx_ptr: *mut TaskContext) {
    let mut processor = PROCESSOR.exclusive_access();
    let idle_task_cx_ptr = processor.get_idle_task_cx_ptr();
    drop(processor);
    unsafe {
        __switch(switched_task_cx_ptr, idle_task_cx_ptr);
    }
}

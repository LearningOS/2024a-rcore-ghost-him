//! Task management implementation
//!
//! Everything about task management, like starting and switching tasks is
//! implemented here.
//!
//! A single global instance of [`TaskManager`] called `TASK_MANAGER` controls
//! all the tasks in the operating system.
//!
//! Be careful when you see `__switch` ASM function in `switch.S`. Control flow around this function
//! might not be what you expect.

mod context;
mod switch;
#[allow(clippy::module_inception)]
mod task;

use crate::loader::{get_app_data, get_num_app};
use crate::sync::UPSafeCell;
use crate::trap::TrapContext;
use alloc::vec::Vec;
use lazy_static::*;
use switch::__switch;
pub use task::{TaskControlBlock, TaskStatus};

pub use context::TaskContext;
use crate::timer::get_time_ms;
use crate::config::MAX_SYSCALL_NUM;
use crate::mm::{VirtAddr};
use crate::mm::{MapPermission};


/// The task manager, where all the tasks are managed.
///
/// Functions implemented on `TaskManager` deals with all task state transitions
/// and task context switching. For convenience, you can find wrappers around it
/// in the module level.
///
/// Most of `TaskManager` are hidden behind the field `inner`, to defer
/// borrowing checks to runtime. You can see examples on how to use `inner` in
/// existing functions on `TaskManager`.
pub struct TaskManager {
    /// total number of tasks
    num_app: usize,
    /// use inner value to get mutable access
    inner: UPSafeCell<TaskManagerInner>,
}

/// The task manager inner in 'UPSafeCell'
struct TaskManagerInner {
    /// task list
    tasks: Vec<TaskControlBlock>,
    /// id of current `Running` task
    current_task: usize,
}

lazy_static! {
    /// a `TaskManager` global instance through lazy_static!
    pub static ref TASK_MANAGER: TaskManager = {
        println!("init TASK_MANAGER");
        let num_app = get_num_app();
        println!("num_app = {}", num_app);
        let mut tasks: Vec<TaskControlBlock> = Vec::new();
        for i in 0..num_app {
            tasks.push(TaskControlBlock::new(get_app_data(i), i));
        }
        TaskManager {
            num_app,
            inner: unsafe {
                UPSafeCell::new(TaskManagerInner {
                    tasks,
                    current_task: 0,
                })
            },
        }
    };
}

impl TaskManager {
    /// Run the first task in task list.
    ///
    /// Generally, the first task in task list is an idle task (we call it zero process later).
    /// But in ch4, we load apps statically, so the first task is a real app.
    fn run_first_task(&self) -> ! {
        let mut inner = self.inner.exclusive_access();
        let next_task = &mut inner.tasks[0];
        next_task.task_status = TaskStatus::Running;
        let next_task_cx_ptr = &next_task.task_cx as *const TaskContext;
        drop(inner);
        let mut _unused = TaskContext::zero_init();
        // before this, we should drop local variables that must be dropped manually
        unsafe {
            __switch(&mut _unused as *mut _, next_task_cx_ptr);
        }
        panic!("unreachable in run_first_task!");
    }

    /// Change the status of current `Running` task into `Ready`.
    fn mark_current_suspended(&self) {
        let mut inner = self.inner.exclusive_access();
        let cur = inner.current_task;
        inner.tasks[cur].task_status = TaskStatus::Ready;
    }

    /// Change the status of current `Running` task into `Exited`.
    fn mark_current_exited(&self) {
        let mut inner = self.inner.exclusive_access();
        let cur = inner.current_task;
        inner.tasks[cur].task_status = TaskStatus::Exited;
    }

    /// Find next task to run and return task id.
    ///
    /// In this case, we only return the first `Ready` task in task list.
    fn find_next_task(&self) -> Option<usize> {
        let inner = self.inner.exclusive_access();
        let current = inner.current_task;
        (current + 1..current + self.num_app + 1)
            .map(|id| id % self.num_app)
            .find(|id| inner.tasks[*id].task_status == TaskStatus::Ready)
    }

    /// Get the current 'Running' task's token.
    fn get_current_token(&self) -> usize {
        let inner = self.inner.exclusive_access();
        inner.tasks[inner.current_task].get_user_token()
    }

    /// Get the current 'Running' task's trap contexts.
    fn get_current_trap_cx(&self) -> &'static mut TrapContext {
        let inner = self.inner.exclusive_access();
        inner.tasks[inner.current_task].get_trap_cx()
    }

    /// Change the current 'Running' task's program break
    pub fn change_current_program_brk(&self, size: i32) -> Option<usize> {
        let mut inner = self.inner.exclusive_access();
        let cur = inner.current_task;
        inner.tasks[cur].change_program_brk(size)
    }

    /// Switch current `Running` task to the task we have found,
    /// or there is no `Ready` task and we can exit with all applications completed
    fn run_next_task(&self) {
        if let Some(next) = self.find_next_task() {
            let mut inner = self.inner.exclusive_access();
            let current = inner.current_task;
            inner.tasks[next].task_status = TaskStatus::Running;
            inner.current_task = next;
            let current_task_cx_ptr = &mut inner.tasks[current].task_cx as *mut TaskContext;
            // 记录一下第一次的运行时间
            record_first_switch( &mut inner.tasks[next]);
            let next_task_cx_ptr = &inner.tasks[next].task_cx as *const TaskContext;
            drop(inner);
            // before this, we should drop local variables that must be dropped manually
            unsafe {
                __switch(current_task_cx_ptr, next_task_cx_ptr);
            }
            // go back to user mode
        } else {
            panic!("All applications completed!");
        }
    }

    /// 查询当前任务的状态
    fn query_current_task_status(&self) -> TaskStatus {
        let inner = self.inner.exclusive_access();
        let current_idx = inner.current_task;
        inner.tasks[current_idx].task_status
    }
    /// 查询当前运行任务的第一次运行时间
    fn query_current_task_first_run_time(&self) -> usize {
        let inner = self.inner.exclusive_access();
        let current_idx = inner.current_task;
        inner.tasks[current_idx].first_reload_time.unwrap_or(0)
    }
    /// 查询当前运行任务的系统调用次数
    fn query_current_task_syscall_times(&self) -> [u32; MAX_SYSCALL_NUM] {
        let inner = self.inner.exclusive_access();
        let current_idx = inner.current_task;
        inner.tasks[current_idx].syscall_times
    }
    /// 向当前任务添加一次系统调用次数
    fn add_current_task_syscall_time(&self, syscall_id: usize) {
        let mut inner = self.inner.exclusive_access();
        let current_idx = inner.current_task;
        inner.tasks[current_idx].syscall_times[syscall_id] += 1;
    }
    /// 申请内存
    fn allocate_new_space(&self, start: usize, len:usize, port:usize) -> isize {
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
        let mut permission = MapPermission::empty();
        if port & 1 == 1 {
            permission |= MapPermission::R;
        }
        if port & 2 == 1 {
            permission |= MapPermission::W;
        }
        if port & 4 == 1 {
            permission |= MapPermission::X;
        }
            
        // 获得应用程序的空间
        let mut inner = self.inner.exclusive_access();
        let current_idx = inner.current_task;
        inner.tasks[current_idx].memory_set.allocate_new_space(VirtAddr::from(start), len, permission)
    }
    /// 回收一个空间
    fn deallocate_space(&self, start:usize, len:usize) -> isize {
        let va_start : VirtAddr= start.into();
        if !va_start.aligned() {
            return -1;
        }
        // 获得应用程序的空间
        let mut inner = self.inner.exclusive_access();
        let current_idx = inner.current_task;
        inner.tasks[current_idx].memory_set.deallocate_space(VirtAddr::from(start), len)
    }
}

/// Run the first task in task list.
pub fn run_first_task() {
    TASK_MANAGER.run_first_task();
}

/// Switch current `Running` task to the task we have found,
/// or there is no `Ready` task and we can exit with all applications completed
fn run_next_task() {
    TASK_MANAGER.run_next_task();
}

/// Change the status of current `Running` task into `Ready`.
fn mark_current_suspended() {
    TASK_MANAGER.mark_current_suspended();
}

/// Change the status of current `Running` task into `Exited`.
fn mark_current_exited() {
    TASK_MANAGER.mark_current_exited();
}

/// Suspend the current 'Running' task and run the next task in task list.
pub fn suspend_current_and_run_next() {
    mark_current_suspended();
    run_next_task();
}

/// Exit the current 'Running' task and run the next task in task list.
pub fn exit_current_and_run_next() {
    mark_current_exited();
    run_next_task();
}

/// Get the current 'Running' task's token.
pub fn current_user_token() -> usize {
    TASK_MANAGER.get_current_token()
}

/// Get the current 'Running' task's trap contexts.
pub fn current_trap_cx() -> &'static mut TrapContext {
    TASK_MANAGER.get_current_trap_cx()
}

/// Change the current 'Running' task's program break
pub fn change_program_brk(size: i32) -> Option<usize> {
    TASK_MANAGER.change_current_program_brk(size)
}
/// 公共接口，查询当前任务的状态
pub fn query_current_task_status() -> TaskStatus {
    TASK_MANAGER.query_current_task_status()
}
/// 公共接口，查询当前任务第一次运行的时间
pub fn query_current_task_first_run_time() -> usize {
    TASK_MANAGER.query_current_task_first_run_time()
}
/// 公共接口，查询当前任务系统调用的次数
pub fn query_current_task_syscall_times() -> [u32; MAX_SYSCALL_NUM] {
    TASK_MANAGER.query_current_task_syscall_times()
}
/// 公共接口，向当前运行的任务添加一次系统调用的次数
pub fn add_current_task_syscall_time(syscall_id: usize) {
    TASK_MANAGER.add_current_task_syscall_time(syscall_id);
}

/// 记录第一次运行的时间
fn record_first_switch(task_cx_ptr: &mut TaskControlBlock) {
    if task_cx_ptr.first_reload_time.is_none() {
        let time : usize = get_time_ms();
        task_cx_ptr.first_reload_time = Some(time);
    }
}

/// 应用程序申请一个内存
pub fn allocate_new_space(start: usize, len:usize, port:usize) ->isize {
    TASK_MANAGER.allocate_new_space(start, len, port)
}

/// 回收一个空间
pub fn deallocate_space(start: usize, len : usize) -> isize {
    TASK_MANAGER.deallocate_space(start, len)
}

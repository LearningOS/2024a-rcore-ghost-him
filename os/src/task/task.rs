//! Types related to task management

use super::TaskContext;
use crate::{
    config::MAX_SYSCALL_NUM,
};

/// The task control block (TCB) of a task.
#[derive(Copy, Clone)]
pub struct TaskControlBlock {
    /// The task status in it's lifecycle
    pub task_status: TaskStatus,
    /// The task context
    pub task_cx: TaskContext,
    /// 第一次被调度的时间
    pub first_reload_time: Option<usize>,
    /// 当时任务的系统调用及调用的次数
    pub syscall_times: [u32; MAX_SYSCALL_NUM],
}

/// The status of a task
#[derive(Copy, Clone, PartialEq)]
pub enum TaskStatus {
    /// uninitialized
    UnInit,
    /// ready to run
    Ready,
    /// running
    Running,
    /// exited
    Exited,
}

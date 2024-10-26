//! Process management syscalls
//!
use alloc::sync::Arc;
use core::mem::size_of;
use crate::{
    fs::{open_file, OpenFlags},
    config::{MAX_SYSCALL_NUM, BIG_STRIDE},
    mm::{translated_refmut, translated_str, translated_byte_buffer}, 
    task::{
        add_task, current_task, current_user_token, exit_current_and_run_next,
        suspend_current_and_run_next, TaskStatus,TaskControlBlock
    },
    task::processor::{user_allocate_new_space, user_deallocate_space, query_current_task_first_run_time, query_current_task_status, query_current_task_syscall_times},
    timer::{get_time_us, get_time_ms},
    fs::{get_app_data_by_name},
};
///
#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    /// 秒
    pub sec: usize,
    /// 微秒
    pub usec: usize,
}

/// Task information
#[allow(dead_code)]
pub struct TaskInfo {
    /// Task status in it's life cycle
    pub status: TaskStatus,
    /// The numbers of syscall called by task
    pub syscall_times: [u32; MAX_SYSCALL_NUM],
    /// Total running time of task
    pub time: usize,
}
/// 初始化
impl TaskInfo {
    /// 初始化一个
    pub fn new() -> TaskInfo {
        Self {
            status: TaskStatus::Ready,
            syscall_times: [0; MAX_SYSCALL_NUM],
            time: 0,
        }
    }
}


/// 
pub fn sys_exit(exit_code: i32) -> ! {
    trace!("kernel:pid[{}] sys_exit", current_task().unwrap().pid.0);
    exit_current_and_run_next(exit_code);
    panic!("Unreachable in sys_exit!");
}
///
pub fn sys_yield() -> isize {
    //trace!("kernel: sys_yield");
    suspend_current_and_run_next();
    0
}
///
pub fn sys_getpid() -> isize {
    trace!("kernel: sys_getpid pid:{}", current_task().unwrap().pid.0);
    current_task().unwrap().pid.0 as isize
}
///
pub fn sys_fork() -> isize {
    trace!("kernel:pid[{}] sys_fork", current_task().unwrap().pid.0);
    let current_task = current_task().unwrap();
    let new_task = current_task.fork();
    let new_pid = new_task.pid.0;
    // modify trap context of new_task, because it returns immediately after switching
    let trap_cx = new_task.inner_exclusive_access().get_trap_cx();
    // we do not have to move to next instruction since we have done it before
    // for child process, fork returns 0
    trap_cx.x[10] = 0;
    // add new task to scheduler
    add_task(new_task);
    new_pid as isize
}
///
pub fn sys_exec(path: *const u8) -> isize {
    trace!("kernel:pid[{}] sys_exec", current_task().unwrap().pid.0);
    let token = current_user_token();
    let path = translated_str(token, path);
    if let Some(app_inode) = open_file(path.as_str(), OpenFlags::RDONLY) {
        let all_data = app_inode.read_all();
        let task = current_task().unwrap();
        task.exec(all_data.as_slice());
        0
    } else {
        -1
    }
}

/// If there is not a child process whose pid is same as given, return -1.
/// Else if there is a child process but it is still running, return -2.
pub fn sys_waitpid(pid: isize, exit_code_ptr: *mut i32) -> isize {
    //trace!("kernel: sys_waitpid");
    let task = current_task().unwrap();
    // find a child process

    // ---- access current PCB exclusively
    let mut inner = task.inner_exclusive_access();
    if !inner
        .children
        .iter()
        .any(|p| pid == -1 || pid as usize == p.getpid())
    {
        return -1;
        // ---- release current PCB
    }
    let pair = inner.children.iter().enumerate().find(|(_, p)| {
        // ++++ temporarily access child PCB exclusively
        p.inner_exclusive_access().is_zombie() && (pid == -1 || pid as usize == p.getpid())
        // ++++ release child PCB
    });
    if let Some((idx, _)) = pair {
        let child = inner.children.remove(idx);
        // confirm that child will be deallocated after being removed from children list
        assert_eq!(Arc::strong_count(&child), 1);
        let found_pid = child.getpid();
        // ++++ temporarily access child PCB exclusively
        let exit_code = child.inner_exclusive_access().exit_code;
        // ++++ release child PCB
        *translated_refmut(inner.memory_set.token(), exit_code_ptr) = exit_code;
        found_pid as isize
    } else {
        -2
    }
    // ---- release current PCB automatically
}


/// YOUR JOB: get time with second and microsecond
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TimeVal`] is splitted by two pages ?
pub fn sys_get_time(_ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");
    let buffers = translated_byte_buffer(current_user_token(), _ts as *const u8, size_of::<TimeVal>());
    let us = get_time_us();
    let time_val = TimeVal {
        sec: us/1_000_000,
        usec: us % 1_000_000,
    };

    let mut time_val_ptr = &time_val as *const _ as *const u8;

    for buffer in buffers {
        unsafe {
            time_val_ptr.copy_to(buffer.as_mut_ptr(), buffer.len());
            time_val_ptr = time_val_ptr.add(buffer.len());
        }
    }
    0
}

/// YOUR JOB: Finish sys_task_info to pass testcases
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TaskInfo`] is splitted by two pages ?
pub fn sys_task_info(_ti: *mut TaskInfo) -> isize {
    trace!("kernel: sys_task_info NOT IMPLEMENTED YET!");
    let buffers = translated_byte_buffer(current_user_token(), _ti as *const u8, size_of::<TaskInfo>());

    let current_task_status: TaskStatus = query_current_task_status();
    let current_task_syscall_times : [u32; MAX_SYSCALL_NUM] = query_current_task_syscall_times();
    let current_task_first_run_time : usize = query_current_task_first_run_time();

    let task_info = TaskInfo {
        status: current_task_status,
        syscall_times : current_task_syscall_times,
        time : get_time_ms() - current_task_first_run_time,
    };
    let mut task_info_ptr = &task_info as *const _ as *const u8;
    for buffer in buffers {
        unsafe {
            task_info_ptr.copy_to(buffer.as_mut_ptr(), buffer.len());
            task_info_ptr = task_info_ptr.add(buffer.len());
        }
    }
    0
}

/// YOUR JOB: Implement mmap.
pub fn sys_mmap(_start: usize, _len: usize, _port: usize) -> isize {
    trace!("kernel: sys_mmap NOT IMPLEMENTED YET!");
    user_allocate_new_space(_start, _len, _port)
}

/// YOUR JOB: Implement munmap.
pub fn sys_munmap(_start: usize, _len: usize) -> isize {
    trace!("kernel: sys_munmap NOT IMPLEMENTED YET!");
    user_deallocate_space(_start, _len)
}

/// change data segment size
pub fn sys_sbrk(size: i32) -> isize {
    trace!("kernel:pid[{}] sys_sbrk", current_task().unwrap().pid.0);
    if let Some(old_brk) = current_task().unwrap().change_program_brk(size) {
        old_brk as isize
    } else {
        -1
    }
}

/// YOUR JOB: Implement spawn.
/// HINT: fork + exec =/= spawn
pub fn sys_spawn(_path: *const u8) -> isize {
    let token = current_user_token();
    let path = translated_str(token, _path);
    println!("spawn: {}", path.as_str());
    if let Some(data) = get_app_data_by_name(path.as_str()) {
        let parent_task = current_task().unwrap();
        let new_task_control_block = Arc::new(TaskControlBlock::new(&data));
        let new_pid = new_task_control_block.pid.0;

        let cloned_task = new_task_control_block.clone();
        let mut new_inner = cloned_task.inner_exclusive_access();
        // 添加父结节
        new_inner.parent = Some(Arc::downgrade(&parent_task));
        drop(new_inner);
        // 添加子节点
        let mut parent_inner = parent_task.inner_exclusive_access();
        parent_inner.children.push(new_task_control_block.clone());
        drop(parent_inner);

        add_task(new_task_control_block.clone());
        return new_pid as isize;
    } else {
        println!("spawn failed!");
        return -1;
    }
}

/// YOUR JOB: Set task priority.
pub fn sys_set_priority(_prio: isize) -> isize {
    if _prio < 2 {
        return -1;
    }

    let task = current_task().unwrap();
    let mut inner = task.inner_exclusive_access();

    inner.priority = _prio;
    inner.pass = BIG_STRIDE / inner.priority;

    _prio
}


/// 添加一个硬链接
pub fn sys_linkat(olddirfd: i32, oldpath: *const u8, newdirfd: i32, newpath: *const u8, flags: u32) -> i32 {
    linkat(olddirfd, oldpath, newdirfd, newpath, flags)
}


/// 删除一个硬链接
pub fn sys_unlinkat(dirfd: i32, path: *const u8, flags: u32) -> i32 {

}

/// 获取一个状态
pub fn sys_fstat(fd: i32, st: *mut Stat) -> i32 {

}


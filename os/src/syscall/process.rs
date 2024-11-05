//! Process management syscalls
//!
use alloc::sync::Arc;

use crate::{
    config::MAX_SYSCALL_NUM,
    fs::{open_file, OpenFlags, Stdin, Stdout},
    //loader::get_app_data_by_name,// need to change
    mm::{translated_refmut, translated_str, virt_to_phys, MemorySet,
        MapPermission, PageTable, VirtAddr, StepByOne, KERNEL_SPACE},
    task::{
        add_task, current_task, current_user_token, exit_current_and_run_next,
        suspend_current_and_run_next, TaskStatus,insert_framed_area, pid_alloc,
        delete_framed_area, get_current_task_time, get_current_syscall_times,
        BIG_STRIDE,kstack_alloc,TaskControlBlock, TaskControlBlockInner, TaskContext,
    },
    timer::{get_time_ms,get_time_us},
    config::{PAGE_SIZE,TRAP_CONTEXT_BASE},
    sync::UPSafeCell,
    trap::{trap_handler, TrapContext},
    alloc::vec::Vec,
    alloc::vec
};

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

/// Task information
#[allow(dead_code)]
pub struct TaskInfo {
    /// Task status in it's life cycle
    status: TaskStatus,
    /// The numbers of syscall called by task
    syscall_times: [u32; MAX_SYSCALL_NUM],
    /// Total running time of task
    time: usize,
}

pub fn sys_exit(exit_code: i32) -> ! {
    trace!("kernel:pid[{}] sys_exit", current_task().unwrap().pid.0);
    exit_current_and_run_next(exit_code);
    panic!("Unreachable in sys_exit!");
}

pub fn sys_yield() -> isize {
    //trace!("kernel: sys_yield");
    suspend_current_and_run_next();
    0
}

pub fn sys_getpid() -> isize {
    trace!("kernel: sys_getpid pid:{}", current_task().unwrap().pid.0);
    current_task().unwrap().pid.0 as isize
}

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
    let us = get_time_us();
    // 用虚拟地址实现这个函数，TimeVal是个虚拟地址，我们不保证它在物理页上的连续性
    // 我们要做的是处理这个TimeVal地址, 这里的*ts到底解引用之后往哪写？
    // 是虚拟地址还是物理地址？应用程序调用的时候显然会穿一个虚拟地址，我们要转成物理地址
    let ts = _ts as usize;
    let ts_addr_p = virt_to_phys(current_user_token(),ts);
    // 我们不在这里面考虑跨页问题，TimeVal如果按照虚拟内存的规则分配那么它一定不会跨页
    let ts_addr_p = ts_addr_p as *mut TimeVal;
    // check ptr_ts
    unsafe {
        *ts_addr_p = TimeVal {
            sec: us / 1_000_000,
            usec: us % 1_000_000,
        };
    }
    0
}


/// YOUR JOB: Finish sys_task_info to pass testcases
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TaskInfo`] is splitted by two pages ?
/// copy from ch3
pub fn sys_task_info(_ti: *mut TaskInfo) -> isize {
    trace!("kernel: sys_task_info");
    let cur_task_time = get_current_task_time();
    let cur_syscall_times = get_current_syscall_times();
    let time = if cur_task_time == 0 {
        0
    } else {
        get_time_ms()-cur_task_time
    };
    //仍然是写入的地址问题
    let ti = _ti as usize;
    let ti_addr_p = virt_to_phys(current_user_token(),ti);
    let ti_addr_p = ti_addr_p as *mut TaskInfo;
    unsafe {
        // always current task
        (*ti_addr_p).status = TaskStatus::Running;
        (*ti_addr_p).time = time;
        (*ti_addr_p).syscall_times = cur_syscall_times;
    }
    0
}

// YOUR JOB: Implement mmap.
pub fn sys_mmap(_start: usize, _len: usize, _port: usize) -> isize {
    trace!("kernel: sys_mmap");
    // 实现的主要目标就是插入一个pte,找找接口
    //1. 地址按页对齐, port参数检查
    if _start % PAGE_SIZE !=0 {
        println!("Start address is not page aligned");
        return -1;
    }
    else if _port & !0x7 != 0 || _port & 0x7 == 0 {
        println!("Port not set correctly!");
        return -1;
    }
    let start_addr : VirtAddr= _start.into();
    let end_addr : VirtAddr = (_start+_len).into();
    //2.查找是否start-end已经有页被映射
    let page_table = PageTable::from_token(current_user_token());
    let mut vpn = start_addr.floor();
    for _ in 0..((_len+PAGE_SIZE-1)/PAGE_SIZE) {
        // unwrap 过不了test, 没有pte正好
        match page_table.translate(vpn){
            Some(pte) =>{
                if pte.is_valid(){
                    println!("Page already mapped!");
                    return -1;
                }
            }
            None=>{}
        }
        vpn.step();// 没公开的方法
    }
    //3.空间足够，进行分配, 将port转化成pte的权限位
    let mut permission = MapPermission::empty();
    permission.set(MapPermission::R, _port & 0x1 != 0);//编译报错，rust bool和usize不匹配
    permission.set(MapPermission::W, _port & 0x2 != 0);// 草 !=0让权限位烂了
    permission.set(MapPermission::X, _port & 0x4 != 0);
    permission.set(MapPermission::U, true);
    //4. 使用权限在对应pte插入增加页框
    // 虽然说是framed，其实调用链是
    //memset.insertframedarea->memset.push->memarea.map->memarea.mapone
    insert_framed_area(start_addr, end_addr, permission);
    0
}

// YOUR JOB: Implement munmap.
pub fn sys_munmap(_start: usize, _len: usize) -> isize {
    trace!("kernel: sys_munmap");
    if _start % PAGE_SIZE !=0 {
        println!("Start address is not page aligned");
        return -1;
    }
    let start_addr : VirtAddr= _start.into();
    let end_addr : VirtAddr = (_start+_len).into();
    let page_table = PageTable::from_token(current_user_token());
    let mut vpn = start_addr.floor();
    for _ in 0..((_len+PAGE_SIZE-1)/PAGE_SIZE) {
        // 这里用unwrap直接panic不太好
        match page_table.translate(vpn){
            Some(pte) =>{
                if !pte.is_valid(){
                    println!("Page not mapped(Invalid PTE)!");
                    return -1;
                }
            }
            None=>{
                println!("Page not mapped(PTE Not Found)!");
                return -1;
            }
        }
        vpn.step();
    }
    delete_framed_area(start_addr, end_addr);
    0
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
/// 缝合tcb.fork和tcb.exec
pub fn sys_spawn(_path: *const u8) -> isize {
    trace!(
        "kernel:pid[{}] sys_spawn",
        current_task().unwrap().pid.0
    );
    // fork copied ↓
    //let cur_task = current_task().unwrap();
    let token = current_user_token();
    let path = translated_str(token, _path);
    // exec copied ↓
    if let Some(app_inode) = open_file(path.as_str(), OpenFlags::RDONLY) {
        let parent_task = current_task().unwrap();
        let mut parent_inner = parent_task.inner_exclusive_access();
        // copied from tcb.fork()
        let pid_handle = pid_alloc();
        let pid = pid_handle.0;
        let kernel_stack = kstack_alloc();
        let kernel_stack_top = kernel_stack.get_top();
        // copied from tcb.exec()
        let (memory_set, user_sp, entry_point) = MemorySet::from_elf(app_inode.read_all().as_slice());
        let trap_cx_ppn = memory_set
            .translate(VirtAddr::from(TRAP_CONTEXT_BASE).into())
            .unwrap()
            .ppn();
        let new_task = Arc::new(TaskControlBlock {
            pid: pid_handle,
            kernel_stack,
            // 融合exec的赋值直接创建tcb
            inner: unsafe {
                UPSafeCell::new(TaskControlBlockInner {
                    trap_cx_ppn,
                    base_size: user_sp,
                    task_cx: TaskContext::goto_trap_return(kernel_stack_top),
                    task_status: TaskStatus::Ready,
                    // new address space
                    memory_set,
                    parent: Some(Arc::downgrade(&parent_task)),
                    children: Vec::new(),
                    exit_code: 0,
                    heap_bottom: 0,
                    program_brk: 0,
                    // add
                    time: 0,
                    syscall_times: [0; MAX_SYSCALL_NUM],
                    stride: 0,
                    pass: BIG_STRIDE / 16,
                    priority: 16,
                    fd_table: vec![
                        // 0 -> stdin
                        Some(Arc::new(Stdin)),
                        // 1 -> stdout
                        Some(Arc::new(Stdout)),
                        // 2 -> stderr
                        Some(Arc::new(Stdout)),
                    ],
                })
            },
        });
        // copied from tcb.exec()
        {
        let new_task_inner = new_task.inner_exclusive_access();// ? 借用了
        let trap_cx = new_task_inner.get_trap_cx();
        *trap_cx = TrapContext::app_init_context(
            entry_point,
            user_sp,
            KERNEL_SPACE.exclusive_access().token(),
            new_task.kernel_stack.get_top(),
            trap_handler as usize,
        );
        }
        parent_inner.children.push(new_task.clone());
        // add new task to scheduler
        add_task(new_task);
        return pid as isize
    } else {
        println!("Wrong path!");
        return -1;
    }
}

// YOUR JOB: Set task priority.
pub fn sys_set_priority(_prio: isize) -> isize {
    trace!(
        "kernel:pid[{}] sys_set_priority",
        current_task().unwrap().pid.0
    );
    if _prio < 2{
        println!("Illegal Priority!");
        return -1;
    }    
    let cur_task = current_task().unwrap();
    let mut cur_inner = cur_task.inner_exclusive_access();
    cur_inner.priority = _prio;
    _prio
}
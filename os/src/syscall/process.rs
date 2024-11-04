//! Process management syscalls
// use riscv::addr::VirtAddr;

//use riscv::paging::PTE;

use crate::{
    config::{MAX_SYSCALL_NUM,PAGE_SIZE},
    mm::{virt_to_phys, PageTable, StepByOne, VirtAddr, MapPermission},
    task::{
        change_program_brk, current_user_token, exit_current_and_run_next,
        suspend_current_and_run_next, get_current_task_syscall_times,
        TaskStatus, insert_framed_area, delete_framed_area, get_current_task_time
    },
    timer::{get_time_us,get_time_ms}
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

/// task exits and submit an exit code
pub fn sys_exit(_exit_code: i32) -> ! {
    trace!("kernel: sys_exit");
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    trace!("kernel: sys_yield");
    suspend_current_and_run_next();
    0
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
    let cur_syscall_times = get_current_task_syscall_times();
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
    // trace!("kernel: sys_mmap NOT IMPLEMENTED YET!");
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
    for _ in 0..(_len/PAGE_SIZE+1) {
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
    trace!("kernel: sys_munmap NOT IMPLEMENTED YET!");
    if _start % PAGE_SIZE !=0 {
        println!("Start address is not page aligned");
        return -1;
    }
    let start_addr : VirtAddr= _start.into();
    let end_addr : VirtAddr = (_start+_len).into();
    let page_table = PageTable::from_token(current_user_token());
    let mut vpn = start_addr.floor();
    for _ in 0..(_len/PAGE_SIZE+1) {
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
    trace!("kernel: sys_sbrk");
    if let Some(old_brk) = change_program_brk(size) {
        old_brk as isize
    } else {
        -1
    }
}

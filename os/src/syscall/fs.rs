//! File and filesystem-related syscalls
use crate::fs::{open_file, File, OpenFlags, Stat, StatMode, ROOT_INODE,decrease_nlink,increase_nlink};
use crate::mm::{translated_byte_buffer, translated_str, UserBuffer, virt_to_phys};
use crate::task::{current_task, current_user_token};

pub fn sys_write(fd: usize, buf: *const u8, len: usize) -> isize {
    trace!("kernel:pid[{}] sys_write", current_task().unwrap().pid.0);
    let token = current_user_token();
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if let Some(file) = &inner.fd_table[fd] {
        if !file.writable() {
            return -1;
        }
        let file = file.clone();
        // release current task TCB manually to avoid multi-borrow
        drop(inner);
        file.write(UserBuffer::new(translated_byte_buffer(token, buf, len))) as isize
    } else {
        -1
    }
}

pub fn sys_read(fd: usize, buf: *const u8, len: usize) -> isize {
    trace!("kernel:pid[{}] sys_read", current_task().unwrap().pid.0);
    let token = current_user_token();
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if let Some(file) = &inner.fd_table[fd] {
        let file = file.clone();
        if !file.readable() {
            return -1;
        }
        // release current task TCB manually to avoid multi-borrow
        drop(inner);
        trace!("kernel: sys_read .. file.read");
        file.read(UserBuffer::new(translated_byte_buffer(token, buf, len))) as isize
    } else {
        -1
    }
}

pub fn sys_open(path: *const u8, flags: u32) -> isize {
    trace!("kernel:pid[{}] sys_open", current_task().unwrap().pid.0);
    let task = current_task().unwrap();
    let token = current_user_token();
    let path = translated_str(token, path);
    if let Some(inode) = open_file(path.as_str(), OpenFlags::from_bits(flags).unwrap()) {
        let mut inner = task.inner_exclusive_access();
        let fd = inner.alloc_fd();
        inner.fd_table[fd] = Some(inode);
        fd as isize
    } else {
        -1
    }
}

pub fn sys_close(fd: usize) -> isize {
    trace!("kernel:pid[{}] sys_close", current_task().unwrap().pid.0);
    let task = current_task().unwrap();
    let mut inner = task.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if inner.fd_table[fd].is_none() {
        return -1;
    }
    inner.fd_table[fd].take();
    0
}

/// YOUR JOB: Implement fstat.
/// 自顶向下，先看fstat怎么实现
pub fn sys_fstat(_fd: usize, _st: *mut Stat) -> isize {
    trace!(
        "kernel:pid[{}] sys_fstat",
        current_task().unwrap().pid.0
    );
    let token = current_user_token();
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
    if _fd >= inner.fd_table.len() {
        return -1;
    }
    // 看了下，进程视角的file对象就是个file特征约束过的指针
    // 我们要为它添加查找inode和nlink的方法
    if let Some(file) = &inner.fd_table[_fd] {
        // 走一遍我们taskinfo调用的地址转换流程
        let st = virt_to_phys(token, _st as usize);
        let st = st as *mut Stat;
        // 在哪为操作系统视角下的file添加实现？innerOSNode
        let inode_num = file.get_inode_num();
        let nlink_num = file.get_nlink_num();
        unsafe {
            (*st).dev = 0;// static
            (*st).ino = inode_num as u64;
            (*st).mode = StatMode::FILE;
            (*st).nlink = nlink_num as u32;
            // (*st).pad = [0; 7];
            // pad is private?
        }
        0
    }
    else {
        -1
    }
}

/// YOUR JOB: Implement linkat.
/// 硬链接的本质是让两个Inode指向同一个block
/// 我们在vfs层执行链接的操作，本质上是和增加inode一样，只是将block信息指向指定位置
/// https://hangx-ma.github.io/2023/07/10/rcore-note-ch6.html
pub fn sys_linkat(_old_name: *const u8, _new_name: *const u8) -> isize {
    trace!(
        "kernel:pid[{}] sys_linkat",
        current_task().unwrap().pid.0
    );
    let token = current_user_token();
    let old_name = translated_str(token, _old_name);
    let new_name = translated_str(token, _new_name);
    if old_name == new_name {
        return -1;
    }
    if let Some(old_file) = open_file(old_name.as_str(), OpenFlags::RDONLY){
    // 选择维护一个统计了不同inode被链接的次数的表？    
        ROOT_INODE.link(old_file.get_inode_num(), new_name.as_str());
        increase_nlink(old_file.get_inode_num());
        return 0;
    }
    else {
        return -1;
    }
}

/// YOUR JOB: Implement unlinkat.
pub fn sys_unlinkat(_name: *const u8) -> isize {
    trace!(
        "kernel:pid[{}] sys_unlinkat",
        current_task().unwrap().pid.0
    );
    let token = current_user_token();
    let name = translated_str(token, _name);
    if !ROOT_INODE.ls().contains(&name) {
        return -1;
    }
    if let Some(file) = open_file(name.as_str(), OpenFlags::RDONLY) {
        if let Some(direntry) = ROOT_INODE.unlink(&name){
            println!("direntry id : {}",  direntry.inode_id());
            println!("direntry valid : {}",direntry.is_valid());
            println!("direntry name : {}", direntry.name());
        }
        decrease_nlink(file.get_inode_num());
        0
    } else {
        -1
    }
}

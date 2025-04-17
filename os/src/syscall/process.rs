//! Process management syscalls
use core::cmp::min;

use crate::{
    mm::{
        translated_byte_buffer, va_valid, MapPermission, PTEFlags, PageTable, VPNRange, VirtAddr,
    },
    task::{
        change_program_brk, current_user_token, exit_current_and_run_next, get_syscall_times,
        insert_current_frame, munmap_current_frames, suspend_current_and_run_next,
    },
    timer::get_time_us,
};

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
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
pub fn sys_get_time(ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");
    let ts_size = core::mem::size_of::<TimeVal>();
    let bufs = translated_byte_buffer(current_user_token(), ts as *const u8, ts_size);
    assert!(
        bufs.len() >= 1 && bufs.len() <= 2,
        "ts should only take 1 or 2 page(s), but got {}",
        bufs.len()
    );
    let us = get_time_us();
    let timeval = TimeVal {
        sec: us / 1_000_000,
        usec: us % 1_000_000,
    };
    // byte-by-byte copy, sth like copy_to_user in linux
    let mut copied = 0;
    for buf in bufs {
        // move [timeval + copied, timeval + copied + buf.len()) to buf
        for i in 0..min(buf.len(), ts_size - copied) {
            unsafe {
                *buf.get_unchecked_mut(i) = *(&timeval as *const TimeVal as *const u8).add(copied)
            };
            copied += 1;
        }
    }
    assert_eq!(
        copied, ts_size,
        "copied {} bytes, but should copy {} bytes",
        copied, ts_size
    );
    0
}

/// TODO: Finish sys_trace to pass testcases
/// HINT: You might reimplement it with virtual memory management.
pub fn sys_trace(trace_request: usize, id: usize, data: usize) -> isize {
    trace!(
        "kernel: sys_trace, trace_request: {}, id: {}, data: {}",
        trace_request,
        id,
        data
    );
    let helper = |flag| {
        // check if `id` is valid in SV39
        if !va_valid(id) {
            debug!("[sys_trace]: id(addr):{:x} is invalid in SV39", id);
            return -1;
        }
        let token = current_user_token();
        let page_table = PageTable::from_token(token);
        let vpn = VirtAddr::from(id).floor();
        if let Some(pte) = page_table.translate(vpn) {
            if !pte.is_valid() || ((pte.flags() & flag) == PTEFlags::empty()) {
                debug!("[sys_trace]: vpn {:?} is invalid or not allowed", vpn);
                return -1;
            }
        } else {
            debug!("[sys_trace]: vpn {:?} is not in page table", vpn);
            return -1;
        }
        let mut bufs = translated_byte_buffer(token, id as *const u8, 1);
        assert_eq!(bufs.len(), 1);
        assert!(bufs[0].len() >= 1);
        match flag {
            PTEFlags::R => bufs[0][0] as isize,
            PTEFlags::W => {
                bufs[0][0] = data as u8;
                0
            }
            _ => panic!("Invalid flag"),
        }
    };
    match trace_request {
        0 => helper(PTEFlags::R),
        1 => helper(PTEFlags::W),
        2 => get_syscall_times(id) as isize,
        _ => -1,
    }
}

// YOUR JOB: Implement mmap.
pub fn sys_mmap(start: usize, len: usize, prot: usize) -> isize {
    trace!("kernel: sys_mmap");
    // check if `prot` is valid
    if (prot & (!0x7 as usize) != 0) || (prot & 0x7 == 0) {
        debug!("[sys_mmap]: prot:{} is invalid", prot);
        return -1;
    }
    // check if `start` is valid in SV39
    if !va_valid(start) {
        debug!("[sys_mmap]: start:{:x} is invalid in SV39", start);
        return -1;
    }
    // check if `start` is aligned
    let start_va = VirtAddr::from(start);
    if !start_va.aligned() {
        debug!("[sys_mmap]: start:{} is not aligned", start);
        return -1;
    }
    // Because `MapArea::new` does not check,
    // we need to manually verify that there are no mapped pages within the range [start, start + len)
    let end_va = VirtAddr::from(start + len);
    let start_vpn = start_va.floor();
    let end_vpn = end_va.ceil();
    let token = current_user_token();
    let page_table = PageTable::from_token(token);
    let has_mapping = VPNRange::new(start_vpn, end_vpn).into_iter().any(|vpn| {
        page_table
            .translate(vpn)
            .map_or(false, |pte| pte.is_valid())
    });
    if has_mapping {
        debug!("[sys_mmap]: has mapping");
        return -1;
    }
    // Ok, we can insert frame into current task!
    let mut permission = MapPermission::U;
    if prot & 0x1 != 0 {
        permission |= MapPermission::R;
    }
    if prot & 0x2 != 0 {
        permission |= MapPermission::W;
    }
    if prot & 0x4 != 0 {
        permission |= MapPermission::X;
    }
    debug!("[sys_mmap]: start:{:?}, end:{:?}", start_va, end_va);
    insert_current_frame(start_va, end_va, permission);
    0
}

// YOUR JOB: Implement munmap.
pub fn sys_munmap(start: usize, len: usize) -> isize {
    trace!("kernel: sys_munmap");
    // check if `start` is valid in SV39
    if !va_valid(start) {
        debug!("[sys_munmap]: start:{:x} is invalid in SV39", start);
        return -1;
    }
    // check if `start` is aligned
    let start_va = VirtAddr::from(start);
    if !start_va.aligned() {
        debug!("[sys_munmap]: start:{} is not aligned", start);
        return -1;
    }
    // unmap the range [start, start + len)
    let end_va = VirtAddr::from(start + len);
    debug!("[sys_munmap]: start:{:?}, end:{:?}", start_va, end_va);
    match munmap_current_frames(start_va, end_va) {
        true => 0,
        false => -1,
    }
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

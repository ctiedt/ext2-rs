#![feature(lang_items, const_fn, box_into_raw_non_null)]
#![no_std]
#![feature(allocator_api)]
#![feature(alloc_error_handler)]
#![feature(panic_info_message)]
#![allow(improper_ctypes)]
extern crate alloc;

#[macro_use]
mod io;
mod c_fns;
mod c_structs;
mod mem;

#[allow(non_upper_case_globals)]
#[allow(non_camel_case_types)]
#[allow(non_snake_case)]
mod bindings;

use bindings::{address_space, dentry, dev_t, gfp_t, inode, super_block, umode_t};

use c_fns::rs_page_symlink;
use c_structs::InodeOperations;

extern "C" {
    fn _mapping_set_gfp_mask(m: *mut address_space, mask: gfp_t);
    fn _mapping_set_unevictable(m: *mut address_space);
}

#[cfg(not(test))]
#[lang = "eh_personality"]
fn rust_eh_personality() {}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    println!("{}", info);
    loop {}
}

#[global_allocator]
static A: mem::KernelAllocator = mem::KernelAllocator {};

#[no_mangle]
pub extern "C" fn rs_ramfs_mknod(
    dir: *mut inode,
    dentry: *mut dentry,
    mode: umode_t,
    dev: dev_t,
) -> Result<(), cty::c_int> {
    use bindings::ENOSPC;
    use c_fns::{rs_d_instantiate, rs_dget, rs_ramfs_get_inode};

    let sb = unsafe { (*dir).i_sb };

    match rs_ramfs_get_inode(sb, dir, mode, dev) {
        Some(inode) => {
            rs_d_instantiate(dentry, inode);
            rs_dget(dentry);
            unsafe { (*dir).set_mctime_current() };
            Ok(())
        }
        None => Err(-(ENOSPC as i32)),
    }
}

#[no_mangle]
pub extern "C" fn ramfs_mknod(
    dir: *mut inode,
    dentry: *mut dentry,
    mode: umode_t,
    dev: dev_t,
) -> cty::c_int {
    match rs_ramfs_mknod(dir, dentry, mode, dev) {
        Ok(()) => 0,
        Err(e) => e,
    }
}

#[no_mangle]
pub extern "C" fn ramfs_mkdir(dir: *mut inode, dentry: *mut dentry, mode: umode_t) -> cty::c_int {
    use bindings::S_IFDIR;
    use c_fns::rs_inc_nlink;
    match rs_ramfs_mknod(dir, dentry, mode | (S_IFDIR as u16), 0) {
        Ok(_) => {
            rs_inc_nlink(dir);
            0
        }
        Err(e) => e,
    }
}

#[no_mangle]
pub extern "C" fn ramfs_create(
    dir: *mut inode,
    dentry: *mut dentry,
    mode: umode_t,
    _excl: bool,
) -> cty::c_int {
    use bindings::S_IFREG;
    match rs_ramfs_mknod(dir, dentry, mode | (S_IFREG as u16), 0) {
        Ok(_) => 0,
        Err(e) => e,
    }
}

#[no_mangle]
pub extern "C" fn ramfs_symlink(
    dir: *mut inode,
    dentry: *mut dentry,
    symname: *const cty::c_char,
) -> cty::c_int {
    use bindings::{ENOSPC, S_IFLNK, S_IRWXUGO};
    use c_fns::{rs_d_instantiate, rs_dget, rs_iput, rs_ramfs_get_inode};
    let name = unsafe { cstr_core::CStr::from_ptr(symname) };

    match rs_ramfs_get_inode(
        unsafe { (*dir).i_sb },
        dir,
        (S_IFLNK as u16) | (S_IRWXUGO as u16),
        0,
    ) {
        Some(inode) => {
            let len = name.to_str().unwrap().len() + 1;
            match rs_page_symlink(inode, symname, len as cty::c_int) {
                Ok(_) => {
                    rs_d_instantiate(dentry, inode);
                    rs_dget(dentry);
                    unsafe { (*dir).set_mctime_current() };
                    0
                }
                Err(e) => {
                    rs_iput(inode);
                    e
                }
            }
        }
        None => -(ENOSPC as cty::c_int),
    }
}

#[no_mangle]
pub extern "C" fn ramfs_kill_super(sb: *mut super_block) {
    use bindings::{kfree, kill_litter_super};
    unsafe { kfree((*sb).s_fs_info) };
    unsafe { kill_litter_super(sb) };
}

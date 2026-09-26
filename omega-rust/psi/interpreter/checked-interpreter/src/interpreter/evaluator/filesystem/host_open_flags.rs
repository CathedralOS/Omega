//! Per-target open-flag BIT POSITIONS, mirroring the checked target encoders in
//! `std/targets/<target>/filesystem_impl.omg`. The differential oracle compiles
//! for `host()` and runs ON the host, so selecting by `cfg!(target_os)` needs no
//! target threading. The create/open differential canaries guard this mirror.
//! Access mode (O_WRONLY 1 / O_RDWR 2, mask 0x3) is universal.

#[cfg(target_os = "windows")]
pub const O_CREAT_BIT: i32 = 8;
#[cfg(target_os = "windows")]
pub const O_EXCL_BIT: i32 = 10;
#[cfg(target_os = "windows")]
pub const O_TRUNC_BIT: i32 = 9;
#[cfg(target_os = "windows")]
pub const O_APPEND_BIT: i32 = 3;

#[cfg(target_os = "macos")]
pub const O_CREAT_BIT: i32 = 9;
#[cfg(target_os = "macos")]
pub const O_EXCL_BIT: i32 = 11;
#[cfg(target_os = "macos")]
pub const O_TRUNC_BIT: i32 = 10;
#[cfg(target_os = "macos")]
pub const O_APPEND_BIT: i32 = 3;

#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
pub const O_CREAT_BIT: i32 = 6;
#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
pub const O_EXCL_BIT: i32 = 7;
#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
pub const O_TRUNC_BIT: i32 = 9;
#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
pub const O_APPEND_BIT: i32 = 10;

pub const fn o_creat(flags: i32) -> bool {
    (flags >> O_CREAT_BIT) & 1 != 0
}
pub const fn o_excl(flags: i32) -> bool {
    (flags >> O_EXCL_BIT) & 1 != 0
}
pub const fn o_trunc(flags: i32) -> bool {
    (flags >> O_TRUNC_BIT) & 1 != 0
}
pub const fn o_append(flags: i32) -> bool {
    (flags >> O_APPEND_BIT) & 1 != 0
}

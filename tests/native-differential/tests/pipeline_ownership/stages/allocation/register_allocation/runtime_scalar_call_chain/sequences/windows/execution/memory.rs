//! Windows-only W^X allocation for bounded test-machine-code execution.

use std::ffi::c_void;

#[link(name = "kernel32")]
unsafe extern "system" {
    fn VirtualAlloc(
        address: *mut c_void,
        size: usize,
        allocation: u32,
        protection: u32,
    ) -> *mut c_void;
    fn VirtualProtect(address: *mut c_void, size: usize, protection: u32, old: *mut u32) -> i32;
    fn VirtualFree(address: *mut c_void, size: usize, operation: u32) -> i32;
    fn GetCurrentProcess() -> *mut c_void;
    fn FlushInstructionCache(process: *mut c_void, address: *const c_void, size: usize) -> i32;
}

pub(super) struct Code {
    address: *mut c_void,
    length: usize,
}

impl Code {
    pub(super) fn new(bytes: &[u8]) -> Self {
        assert!(!bytes.is_empty());
        // SAFETY: allocate private committed RW pages, copy only within the
        // requested extent, then remove write permission before execution.
        unsafe {
            let address = VirtualAlloc(std::ptr::null_mut(), bytes.len(), 0x3000, 0x04);
            assert!(
                !address.is_null(),
                "VirtualAlloc: {}",
                std::io::Error::last_os_error()
            );
            let allocation = Self {
                address,
                length: bytes.len(),
            };
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), address.cast::<u8>(), bytes.len());
            let mut previous = 0;
            assert_ne!(VirtualProtect(address, bytes.len(), 0x20, &mut previous), 0);
            assert_ne!(
                FlushInstructionCache(GetCurrentProcess(), address, bytes.len()),
                0
            );
            allocation
        }
    }

    pub(super) fn call_unit(&self, offset: usize) {
        assert!(offset < self.length);
        // SAFETY: the caller supplies a validated no-argument Unit entry.
        // This allocation remains executable and live for the entire call.
        unsafe {
            let entry: unsafe extern "system" fn() =
                std::mem::transmute(self.address.cast::<u8>().add(offset));
            entry();
        }
    }

    pub(super) fn call_scalar(&self, offset: usize, arguments: [u64; 4]) -> u64 {
        assert!(offset < self.length);
        // SAFETY: caller supplies a scalar callee or our preservation wrapper.
        // Extra register arguments are ignored by smaller-arity callees.
        unsafe {
            let entry: unsafe extern "system" fn(u64, u64, u64, u64) -> u64 =
                std::mem::transmute(self.address.cast::<u8>().add(offset));
            entry(arguments[0], arguments[1], arguments[2], arguments[3])
        }
    }
}

impl Drop for Code {
    fn drop(&mut self) {
        // SAFETY: this is the allocation base returned by VirtualAlloc, and
        // no generated function can outlive its Code owner.
        unsafe {
            assert_ne!(VirtualFree(self.address, 0, 0x8000), 0);
        }
    }
}

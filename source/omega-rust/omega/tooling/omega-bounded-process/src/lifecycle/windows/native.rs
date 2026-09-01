use std::io;
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};

use windows_sys::Win32::Foundation::{FALSE, HANDLE, INVALID_HANDLE_VALUE};

/// Owned adaptation of a nullable or sentinel-valued Win32 kernel handle.
#[derive(Debug)]
pub(super) struct KernelHandle(OwnedHandle);

impl KernelHandle {
    pub(super) fn from_nullable(raw: HANDLE) -> io::Result<Self> {
        if raw.is_null() {
            Err(io::Error::last_os_error())
        } else {
            Ok(Self(unsafe { OwnedHandle::from_raw_handle(raw) }))
        }
    }

    pub(super) fn from_snapshot(raw: HANDLE) -> io::Result<Self> {
        if raw == INVALID_HANDLE_VALUE {
            Err(io::Error::last_os_error())
        } else {
            Self::from_nullable(raw)
        }
    }

    pub(super) fn raw(&self) -> HANDLE {
        self.0.as_raw_handle()
    }
}

pub(super) fn bool_result(result: i32) -> io::Result<()> {
    if result == FALSE {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

pub(super) fn raw_os_error_is(error: &io::Error, expected: u32) -> bool {
    error
        .raw_os_error()
        .and_then(|code| u32::try_from(code).ok())
        == Some(expected)
}

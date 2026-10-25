//! The only typed invocation into loaded native code is the compiler's fixed
//! two-pointer frame ABI. Foreign C signatures are never cast at runtime.
//! The worker's authenticated parent must have admitted the exact image/export
//! and complete argument/result geometry before reaching this physical leaf.

use std::{
    alloc::{Layout, alloc_zeroed, dealloc},
    ffi::CString,
    io,
    mem::ManuallyDrop,
    path::Path,
    ptr::NonNull,
};

use runtime_abi::NativeForeignFrame;
type FrameEntry = unsafe extern "C" fn(*mut NativeForeignFrame);

pub(crate) struct NativeValue {
    address: NonNull<u8>,
    layout: Layout,
    extent: usize,
}
impl NativeValue {
    pub(crate) fn zeroed(extent: usize, alignment: usize) -> io::Result<Self> {
        let layout = Layout::from_size_align(extent.max(1), alignment).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid native value allocation geometry",
            )
        })?;
        let address = NonNull::new(unsafe { alloc_zeroed(layout) })
            .ok_or_else(|| io::Error::other("native value allocation exhausted"))?;
        Ok(Self {
            address,
            layout,
            extent,
        })
    }
    pub(crate) fn initialize(&mut self, bytes: &[u8]) -> io::Result<()> {
        if bytes.len() != self.extent {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "native argument differs from complete value extent",
            ));
        }
        unsafe {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), self.address.as_ptr(), bytes.len());
        }
        Ok(())
    }
    pub(crate) fn pointer(&self) -> *mut u8 {
        self.address.as_ptr()
    }
    pub(crate) fn bytes(&self) -> Vec<u8> {
        // No native call is in flight when the serialized worker copies this
        // owned direct result; retained referent storage is a different owner.
        unsafe { std::slice::from_raw_parts(self.address.as_ptr(), self.extent).to_vec() }
    }
}
impl Drop for NativeValue {
    fn drop(&mut self) {
        unsafe {
            dealloc(self.address.as_ptr(), self.layout);
        }
    }
}

pub(crate) struct LoadedFrame {
    entry: FrameEntry,
    // Native constructors can register callbacks that outlive a failed symbol
    // lookup or this adapter. The dedicated worker exits with code resident;
    // unloading before process-exit callbacks is not a valid retirement path.
    _library: ManuallyDrop<Library>,
}
impl LoadedFrame {
    /// SAFETY: the isolated worker has authenticated its producing parent;
    /// that parent admitted these exact bytes and this fixed-frame export.
    pub(crate) unsafe fn load(path: &Path, export: &str) -> io::Result<Self> {
        let export = CString::new(export).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "native frame export contains NUL",
            )
        })?;
        let library = ManuallyDrop::new(Library::open(path)?);
        let address = library.symbol(&export)?;
        // The product export has one fixed signature for every foreign plan.
        // This is not a cast from a runtime-discovered foreign C signature.
        let entry = unsafe { std::mem::transmute::<*mut std::ffi::c_void, FrameEntry>(address) };
        Ok(Self {
            entry,
            _library: library,
        })
    }
    /// SAFETY: each argument names an aligned complete value of this product's
    /// admitted extent; result names its complete aligned output storage. Any
    /// native loans name live mapped homes for their full admitted lifetime.
    pub(crate) unsafe fn call(&self, arguments: &[*const u8], result: *mut u8) {
        let mut frame = NativeForeignFrame {
            arguments: arguments.as_ptr(),
            result,
        };
        unsafe {
            (self.entry)(&mut frame);
        }
    }
}

struct Library(NonNull<std::ffi::c_void>);
#[cfg(unix)]
impl Library {
    fn open(path: &Path) -> io::Result<Self> {
        use std::os::unix::ffi::OsStrExt;
        let path = CString::new(path.as_os_str().as_bytes()).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "native image path contains NUL",
            )
        })?;
        NonNull::new(unsafe { libc::dlopen(path.as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL) })
            .map(Self)
            .ok_or_else(loader_error)
    }
    fn symbol(&self, symbol: &CString) -> io::Result<*mut std::ffi::c_void> {
        NonNull::new(unsafe { libc::dlsym(self.0.as_ptr(), symbol.as_ptr()) })
            .map(NonNull::as_ptr)
            .ok_or_else(loader_error)
    }
}
#[cfg(unix)]
fn loader_error() -> io::Error {
    let error = unsafe { libc::dlerror() };
    let text = if error.is_null() {
        "native loader returned no address".to_owned()
    } else {
        unsafe { std::ffi::CStr::from_ptr(error) }
            .to_string_lossy()
            .into_owned()
    };
    io::Error::other(text)
}
#[cfg(unix)]
impl Drop for Library {
    fn drop(&mut self) {
        unsafe {
            libc::dlclose(self.0.as_ptr());
        }
    }
}

#[cfg(windows)]
impl Library {
    fn open(path: &Path) -> io::Result<Self> {
        use std::os::windows::ffi::OsStrExt;
        let path = path
            .as_os_str()
            .encode_wide()
            .chain([0])
            .collect::<Vec<_>>();
        let raw = unsafe {
            windows_sys::Win32::System::LibraryLoader::LoadLibraryExW(
                path.as_ptr(),
                std::ptr::null_mut(),
                windows_sys::Win32::System::LibraryLoader::LOAD_WITH_ALTERED_SEARCH_PATH,
            )
        };
        NonNull::new(raw)
            .map(Self)
            .ok_or_else(io::Error::last_os_error)
    }
    fn symbol(&self, symbol: &CString) -> io::Result<*mut std::ffi::c_void> {
        unsafe {
            windows_sys::Win32::System::LibraryLoader::GetProcAddress(
                self.0.as_ptr(),
                symbol.as_ptr().cast(),
            )
        }
        .map(|address| address as *mut std::ffi::c_void)
        .ok_or_else(io::Error::last_os_error)
    }
}
#[cfg(windows)]
impl Drop for Library {
    fn drop(&mut self) {
        unsafe {
            windows_sys::Win32::Foundation::FreeLibrary(self.0.as_ptr());
        }
    }
}

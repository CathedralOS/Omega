//! The same physical file-object pages in host and worker, never a per-call
//! snapshot. Each process owns its mapping independently. Parent unmapping or
//! unlinking cannot free pages still mapped by a live provider process. Backing
//! files are never truncated or recycled; semantic leases remain a separate
//! interpreter obligation even after a failed process cleanup.

use std::fs::{File, OpenOptions};
use std::io;
use std::path::Path;
#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod windows;
#[cfg(unix)]
use unix::Mapping;
#[cfg(windows)]
use windows::Mapping;

pub(crate) struct SharedBacking {
    mapping: Mapping,
    extent: usize,
    _file: File,
}

impl SharedBacking {
    /// Only the invocation's private owner may name this file. Its extent is
    /// immutable through every mapping's lifetime, including fault quarantine.
    pub(crate) fn create(path: &Path, extent: usize) -> io::Result<Self> {
        if extent == 0 || extent > isize::MAX as usize {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "native backing extent is empty or unaddressable",
            ));
        }
        let mut options = OpenOptions::new();
        options.read(true).write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = options.open(path)?;
        file.set_len(extent as u64)?;
        Self::map(file, extent)
    }

    pub(crate) fn attach(path: &Path, extent: usize) -> io::Result<Self> {
        let file = OpenOptions::new().read(true).write(true).open(path)?;
        if extent == 0 || extent > isize::MAX as usize || file.metadata()?.len() != extent as u64 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "native backing differs from the admitted exact extent",
            ));
        }
        Self::map(file, extent)
    }

    fn map(file: File, extent: usize) -> io::Result<Self> {
        let mapping = Mapping::new(&file, extent)?;
        Ok(Self {
            mapping,
            extent,
            _file: file,
        })
    }

    pub(crate) fn pointer(
        &self,
        offset: usize,
        extent: usize,
        alignment: usize,
    ) -> io::Result<*mut u8> {
        if !alignment.is_power_of_two()
            || offset
                .checked_add(extent)
                .is_none_or(|end| end > self.extent)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "native backing range is outside its exact home",
            ));
        }
        let address = unsafe { self.mapping.address().as_ptr().add(offset) };
        if !(address as usize).is_multiple_of(alignment) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "native backing range has incompatible alignment",
            ));
        }
        Ok(address)
    }

    // Byte copies never expose a Rust slice/reference that could alias C. The
    // serialized host protocol owns access while copying; concurrent native
    // writes require separate External backing authority, not these methods.
    pub(crate) fn read(&self, offset: usize, output: &mut [u8]) -> io::Result<()> {
        let source = self.pointer(offset, output.len(), 1)?;
        unsafe {
            std::ptr::copy_nonoverlapping(source, output.as_mut_ptr(), output.len());
        }
        Ok(())
    }
    pub(crate) fn write(&mut self, offset: usize, input: &[u8]) -> io::Result<()> {
        let destination = self.pointer(offset, input.len(), 1)?;
        unsafe {
            std::ptr::copy_nonoverlapping(input.as_ptr(), destination, input.len());
        }
        Ok(())
    }
}

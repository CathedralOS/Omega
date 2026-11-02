//! Physical byte ownership shared with an admitted host allocator. Cloning a
//! handle pins the same storage; it neither copies bytes nor issues an Extent.
//! Retirement invalidates every handle, including handles already joined to
//! guest descriptors. Guest access still requires the ordinary custody/profile
//! checks before obtaining a byte window.

use crate::TerminalEffectRejection;
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex, MutexGuard};

#[derive(Debug)]
pub(crate) enum ProviderBytes {
    Owned(Vec<u8>),
    Host(TerminalProviderBacking),
}

pub(crate) enum ProviderBytesGuard<'a> {
    Owned(&'a mut [u8]),
    Host(ProviderBackingGuard<'a>),
}

pub(crate) enum ProviderBytesRead<'a> {
    Owned(&'a [u8]),
    Host(ProviderBackingGuard<'a>),
}

impl ProviderBytes {
    /// Hand back the allocation before its descriptor/claim bookkeeping is
    /// removed. A failed host retirement leaves that bookkeeping intact.
    pub(crate) fn take(&mut self) -> Result<Vec<u8>, TerminalEffectRejection> {
        match self {
            Self::Owned(bytes) => Ok(std::mem::take(bytes)),
            Self::Host(backing) => backing.retire(),
        }
    }

    pub(crate) fn read(&self) -> Result<ProviderBytesRead<'_>, TerminalEffectRejection> {
        Ok(match self {
            Self::Owned(bytes) => ProviderBytesRead::Owned(bytes),
            Self::Host(backing) => ProviderBytesRead::Host(backing.lock()?),
        })
    }

    pub(crate) fn write(&mut self) -> Result<ProviderBytesGuard<'_>, TerminalEffectRejection> {
        Ok(match self {
            Self::Owned(bytes) => ProviderBytesGuard::Owned(bytes),
            Self::Host(backing) => ProviderBytesGuard::Host(backing.lock()?),
        })
    }
}

impl Deref for ProviderBytesRead<'_> {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        match self {
            Self::Owned(bytes) => bytes,
            Self::Host(guard) => guard,
        }
    }
}

impl Deref for ProviderBytesGuard<'_> {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        match self {
            Self::Owned(bytes) => bytes,
            Self::Host(guard) => guard,
        }
    }
}

impl DerefMut for ProviderBytesGuard<'_> {
    fn deref_mut(&mut self) -> &mut [u8] {
        match self {
            Self::Owned(bytes) => bytes,
            Self::Host(guard) => guard,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TerminalProviderBacking {
    bytes: Arc<Mutex<ProviderBackingState>>,
}

#[derive(Debug)]
struct ProviderBackingState {
    bytes: Vec<u8>,
    retired: bool,
}

impl TerminalProviderBacking {
    /// Retain the supplied allocation without copying or changing its contents.
    pub fn new(bytes: Vec<u8>) -> Self {
        Self {
            bytes: Arc::new(Mutex::new(ProviderBackingState {
                bytes,
                retired: false,
            })),
        }
    }

    /// Observe the live allocation's size without exposing its bytes. Later
    /// operations still recheck retirement while holding their own guard.
    pub fn byte_count(&self) -> Result<usize, TerminalEffectRejection> {
        Ok(self.lock()?.len())
    }

    /// Retire the original allocation, not just this handle. A second retirement
    /// and every subsequent observation reject. This host operation supplies no
    /// guest retirement receipt or authority to invoke the releasing boundary.
    pub fn retire(&self) -> Result<Vec<u8>, TerminalEffectRejection> {
        let mut state = self
            .bytes
            .lock()
            .map_err(|_| TerminalEffectRejection::new("provider backing lock was poisoned"))?;
        if state.retired {
            return Err(TerminalEffectRejection::new(
                "provider backing was already retired",
            ));
        }
        state.retired = true;
        Ok(std::mem::take(&mut state.bytes))
    }

    pub(crate) fn lock(&self) -> Result<ProviderBackingGuard<'_>, TerminalEffectRejection> {
        let guard = self
            .bytes
            .lock()
            .map_err(|_| TerminalEffectRejection::new("provider backing lock was poisoned"))?;
        if guard.retired {
            return Err(TerminalEffectRejection::new("provider backing was retired"));
        }
        Ok(ProviderBackingGuard { guard })
    }

    pub(crate) fn same_allocation(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.bytes, &other.bytes)
    }
}

/// The private guard is constructed only for a live allocation and never exposes
/// its retirement flag, so the lock keeps it live for the whole access.
pub(crate) struct ProviderBackingGuard<'a> {
    guard: MutexGuard<'a, ProviderBackingState>,
}

impl Deref for ProviderBackingGuard<'_> {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        &self.guard.bytes
    }
}

impl DerefMut for ProviderBackingGuard<'_> {
    fn deref_mut(&mut self) -> &mut [u8] {
        &mut self.guard.bytes
    }
}

#[cfg(test)]
mod tests {
    use super::TerminalProviderBacking;

    #[test]
    fn handles_share_writes_and_retirement() {
        let original = TerminalProviderBacking::new(vec![7, 11, 13]);
        let retained = original.clone();
        assert!(original.same_allocation(&retained));
        retained.lock().unwrap()[1] = 23;
        assert_eq!(&*original.lock().unwrap(), &[7, 23, 13]);
        assert_eq!(original.retire().unwrap(), vec![7, 23, 13]);
        assert!(retained.lock().is_err());
        assert!(retained.retire().is_err());
    }

    #[test]
    fn equal_bytes_are_not_the_same_allocation() {
        let first = TerminalProviderBacking::new(vec![0; 8]);
        let second = TerminalProviderBacking::new(vec![0; 8]);
        assert!(!first.same_allocation(&second));
        first.retire().unwrap();
        assert_eq!(second.lock().unwrap().len(), 8);
    }
}

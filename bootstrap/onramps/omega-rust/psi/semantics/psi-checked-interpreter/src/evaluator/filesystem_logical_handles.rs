use crate::{
    FilesystemLogicalHandleIdentity, FilesystemLogicalHandleInputResolution,
    FilesystemLogicalHandleKind,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug)]
pub(super) enum FilesystemLogicalHandleError {
    IdentityExhausted,
    LiveProviderTokenCollision {
        kind: FilesystemLogicalHandleKind,
        raw: i64,
    },
    BorrowSourceMismatch {
        raw: i64,
    },
}

#[derive(Debug)]
pub(super) struct FilesystemLogicalHandles {
    next_identity: u64,
    descriptors: BTreeMap<i64, FilesystemLogicalHandleIdentity>,
    native_handles: BTreeMap<i64, FilesystemLogicalHandleIdentity>,
    find_handles: BTreeMap<i64, FilesystemLogicalHandleIdentity>,
    borrowed_native_sources:
        BTreeMap<FilesystemLogicalHandleIdentity, FilesystemLogicalHandleIdentity>,
}

impl Default for FilesystemLogicalHandles {
    fn default() -> Self {
        Self {
            next_identity: 1,
            descriptors: BTreeMap::new(),
            native_handles: BTreeMap::new(),
            find_handles: BTreeMap::new(),
            borrowed_native_sources: BTreeMap::new(),
        }
    }
}

impl FilesystemLogicalHandles {
    pub(super) fn resolve(
        &self,
        kind: FilesystemLogicalHandleKind,
        raw: i64,
        null_allowed: bool,
    ) -> FilesystemLogicalHandleInputResolution {
        if null_allowed && raw == 0 {
            return FilesystemLogicalHandleInputResolution::Null;
        }
        self.map(kind)
            .get(&raw)
            .copied()
            .map(FilesystemLogicalHandleInputResolution::Resolved)
            .unwrap_or(FilesystemLogicalHandleInputResolution::Unknown)
    }

    /// Whether a raw token is live, but only in a different logical domain.
    /// Providers may share a physical integer table across ABI handle kinds;
    /// this check prevents that representation detail from authorizing a
    /// wrong-domain operation before the provider is entered.
    pub(super) fn conflicts_with_live_domain(
        &self,
        expected: FilesystemLogicalHandleKind,
        raw: i64,
        null_allowed: bool,
    ) -> bool {
        if null_allowed && raw == 0 || self.map(expected).contains_key(&raw) {
            return false;
        }
        [
            FilesystemLogicalHandleKind::Descriptor,
            FilesystemLogicalHandleKind::Native,
            FilesystemLogicalHandleKind::Find,
        ]
        .into_iter()
        .any(|kind| kind != expected && self.map(kind).contains_key(&raw))
    }

    pub(super) fn create(
        &mut self,
        kind: FilesystemLogicalHandleKind,
        raw: i64,
    ) -> Result<FilesystemLogicalHandleIdentity, FilesystemLogicalHandleError> {
        if self.map(kind).contains_key(&raw) {
            return Err(FilesystemLogicalHandleError::LiveProviderTokenCollision { kind, raw });
        }
        let identity = self.allocate()?;
        self.map_mut(kind).insert(raw, identity);
        Ok(identity)
    }

    pub(super) fn borrow_native(
        &mut self,
        raw: i64,
        source: FilesystemLogicalHandleIdentity,
    ) -> Result<FilesystemLogicalHandleIdentity, FilesystemLogicalHandleError> {
        if let Some(existing) = self.native_handles.get(&raw).copied() {
            if self.borrowed_native_sources.get(&existing) == Some(&source) {
                return Ok(existing);
            }
            return Err(FilesystemLogicalHandleError::BorrowSourceMismatch { raw });
        }
        let identity = self.allocate()?;
        self.native_handles.insert(raw, identity);
        self.borrowed_native_sources.insert(identity, source);
        Ok(identity)
    }

    /// Retire one successful close target plus every alias whose lifetime the
    /// close invalidates. Returned identities are globally sorted so evidence
    /// order never depends on provider-token ordering or map implementation.
    pub(super) fn retire(
        &mut self,
        kind: FilesystemLogicalHandleKind,
        identity: FilesystemLogicalHandleIdentity,
    ) -> Vec<FilesystemLogicalHandleIdentity> {
        let mut retired = BTreeSet::new();
        match kind {
            FilesystemLogicalHandleKind::Descriptor => {
                self.retire_descriptor(identity, &mut retired);
            }
            FilesystemLogicalHandleKind::Native => {
                let borrowed_source = self.borrowed_native_sources.remove(&identity);
                remove_identity(&mut self.native_handles, identity);
                retired.insert(identity);
                if let Some(source) = borrowed_source {
                    // Closing a handle borrowed through `_get_osfhandle` also
                    // invalidates the owning CRT descriptor in the modeled ABI.
                    self.retire_descriptor(source, &mut retired);
                }
            }
            FilesystemLogicalHandleKind::Find => {
                remove_identity(&mut self.find_handles, identity);
                retired.insert(identity);
            }
        }
        retired.into_iter().collect()
    }

    fn retire_descriptor(
        &mut self,
        identity: FilesystemLogicalHandleIdentity,
        retired: &mut BTreeSet<FilesystemLogicalHandleIdentity>,
    ) {
        remove_identity(&mut self.descriptors, identity);
        retired.insert(identity);
        let aliases = self
            .borrowed_native_sources
            .iter()
            .filter_map(|(alias, source)| (*source == identity).then_some(*alias))
            .collect::<Vec<_>>();
        for alias in aliases {
            self.borrowed_native_sources.remove(&alias);
            remove_identity(&mut self.native_handles, alias);
            retired.insert(alias);
        }
    }

    fn allocate(
        &mut self,
    ) -> Result<FilesystemLogicalHandleIdentity, FilesystemLogicalHandleError> {
        let identity = FilesystemLogicalHandleIdentity::new(self.next_identity)
            .ok_or(FilesystemLogicalHandleError::IdentityExhausted)?;
        self.next_identity = self
            .next_identity
            .checked_add(1)
            .ok_or(FilesystemLogicalHandleError::IdentityExhausted)?;
        Ok(identity)
    }

    fn map(
        &self,
        kind: FilesystemLogicalHandleKind,
    ) -> &BTreeMap<i64, FilesystemLogicalHandleIdentity> {
        match kind {
            FilesystemLogicalHandleKind::Descriptor => &self.descriptors,
            FilesystemLogicalHandleKind::Native => &self.native_handles,
            FilesystemLogicalHandleKind::Find => &self.find_handles,
        }
    }

    fn map_mut(
        &mut self,
        kind: FilesystemLogicalHandleKind,
    ) -> &mut BTreeMap<i64, FilesystemLogicalHandleIdentity> {
        match kind {
            FilesystemLogicalHandleKind::Descriptor => &mut self.descriptors,
            FilesystemLogicalHandleKind::Native => &mut self.native_handles,
            FilesystemLogicalHandleKind::Find => &mut self.find_handles,
        }
    }
}

fn remove_identity(
    map: &mut BTreeMap<i64, FilesystemLogicalHandleIdentity>,
    identity: FilesystemLogicalHandleIdentity,
) {
    map.retain(|_, candidate| *candidate != identity);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_token_reuse_gets_a_fresh_logical_identity() {
        let mut handles = FilesystemLogicalHandles::default();
        let first = handles
            .create(FilesystemLogicalHandleKind::Descriptor, 3)
            .unwrap();
        assert_eq!(
            handles.resolve(FilesystemLogicalHandleKind::Descriptor, 3, false),
            FilesystemLogicalHandleInputResolution::Resolved(first)
        );
        assert_eq!(
            handles.retire(FilesystemLogicalHandleKind::Descriptor, first),
            vec![first]
        );
        let second = handles
            .create(FilesystemLogicalHandleKind::Descriptor, 3)
            .unwrap();
        assert_ne!(first, second);
    }

    #[test]
    fn descriptor_close_retires_every_borrowed_native_alias() {
        let mut handles = FilesystemLogicalHandles::default();
        let descriptor = handles
            .create(FilesystemLogicalHandleKind::Descriptor, 3)
            .unwrap();
        let native = handles.borrow_native(103, descriptor).unwrap();
        assert_eq!(
            handles.borrow_native(103, descriptor).unwrap(),
            native,
            "repeated conversion preserves one borrowed lifetime"
        );
        assert_eq!(
            handles.retire(FilesystemLogicalHandleKind::Descriptor, descriptor),
            vec![descriptor, native]
        );
        assert_eq!(
            handles.resolve(FilesystemLogicalHandleKind::Native, 103, false),
            FilesystemLogicalHandleInputResolution::Unknown
        );
    }

    #[test]
    fn closing_a_borrowed_native_handle_invalidates_its_descriptor() {
        let mut handles = FilesystemLogicalHandles::default();
        let descriptor = handles
            .create(FilesystemLogicalHandleKind::Descriptor, 3)
            .unwrap();
        let native = handles.borrow_native(103, descriptor).unwrap();
        assert_eq!(
            handles.retire(FilesystemLogicalHandleKind::Native, native),
            vec![descriptor, native]
        );
        assert_eq!(
            handles.resolve(FilesystemLogicalHandleKind::Descriptor, 3, false),
            FilesystemLogicalHandleInputResolution::Unknown
        );
    }

    #[test]
    fn null_is_distinct_from_an_unknown_handle() {
        let handles = FilesystemLogicalHandles::default();
        assert_eq!(
            handles.resolve(FilesystemLogicalHandleKind::Native, 0, true),
            FilesystemLogicalHandleInputResolution::Null
        );
        assert_eq!(
            handles.resolve(FilesystemLogicalHandleKind::Native, 0, false),
            FilesystemLogicalHandleInputResolution::Unknown
        );
    }

    #[test]
    fn live_tokens_cannot_cross_logical_handle_domains() {
        let mut handles = FilesystemLogicalHandles::default();
        handles
            .create(FilesystemLogicalHandleKind::Descriptor, 3)
            .unwrap();
        assert!(handles.conflicts_with_live_domain(FilesystemLogicalHandleKind::Native, 3, false));
        assert!(!handles.conflicts_with_live_domain(
            FilesystemLogicalHandleKind::Descriptor,
            3,
            false
        ));
        assert!(!handles.conflicts_with_live_domain(
            FilesystemLogicalHandleKind::Native,
            99,
            false
        ));
        assert!(!handles.conflicts_with_live_domain(FilesystemLogicalHandleKind::Native, 0, true));
    }
}

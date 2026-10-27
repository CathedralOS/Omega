//! Invocation-private byte backing with immutable shared views.
//! Storage identity never enters Terminal bytes.
//!
//! `binding.rs` distinguishes a frame's immutable byte values from its field
//! loans and prepares structural call arguments over them; `subslice.rs`
//! replaces one operation-owned immutable descriptor; `write.rs` mutates a
//! fixed extent through an existing mutable field loan; `field_store.rs`
//! replaces a bounded byte field's whole contents; `field_index_store.rs`
//! observes a field's length and stores one byte through it.

pub(crate) mod binding;
pub(crate) mod concatenation;
pub(crate) mod equality;
pub(crate) mod field_index_store;
pub(crate) mod field_store;
pub(crate) mod operands;
pub(crate) mod predicate;
pub(crate) mod read;
pub(crate) mod storage;
pub(crate) mod store;
pub(crate) mod subslice;
pub(crate) mod write;

use std::ops::Range;
use std::sync::Arc;

#[derive(Clone)]
pub(crate) struct ByteSequenceView {
    backing: Arc<ByteSequenceBacking>,
    window: Range<usize>,
    /// Exact metadata of a projected binding; absent for a newly owned descriptor.
    descriptor_home: Option<crate::values::TerminalStructuralValue>,
}

struct ByteSequenceBacking {
    storage: storage::ByteSequenceStorage,
    exposure_namespace: Option<u64>,
}

/// Two views are the same storage when they share backing and window. Equal
/// contents in distinct backings are distinct endpoints.
impl PartialEq for ByteSequenceView {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.backing, &other.backing) && self.window == other.window
    }
}

impl Eq for ByteSequenceView {}

impl ByteSequenceView {
    pub(crate) fn shares_backing(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.backing, &other.backing)
    }

    pub(crate) fn retained_payload_bytes(&self) -> usize {
        self.backing.storage.bytes().len()
    }

    pub(crate) fn new(storage: storage::ByteSequenceStorage) -> Self {
        let length = storage.bytes().len();
        Self {
            backing: Arc::new(ByteSequenceBacking {
                storage,
                exposure_namespace: crate::structural_identities::new_exposure_namespace(),
            }),
            window: 0..length,
            descriptor_home: None,
        }
    }

    pub(crate) fn with_descriptor_home(
        mut self,
        home: crate::values::TerminalStructuralValue,
    ) -> Self {
        self.descriptor_home = Some(home);
        self
    }

    pub(crate) fn descriptor_home(&self) -> Option<&crate::values::TerminalStructuralValue> {
        self.descriptor_home.as_ref()
    }

    pub(crate) fn detached_descriptor(&self) -> Self {
        let mut descriptor = self.clone();
        descriptor.descriptor_home = None;
        descriptor
    }

    pub(crate) fn matches_descriptor_home(
        &self,
        value: &crate::values::TerminalStructuralValue,
    ) -> bool {
        self.descriptor_home
            .as_ref()
            .map_or_else(|| value.path.is_empty(), |home| home == value)
    }

    /// A home-less descriptor inherits only its keyed frame binding. Full
    /// runtime identity, selected path and carrier must agree before reuse.
    pub(crate) fn matches_bound_descriptor_home(
        &self,
        binding: &crate::values::TerminalStructuralValue,
        selected: &crate::values::TerminalStructuralValue,
    ) -> bool {
        let same_home = |home: &crate::values::TerminalStructuralValue| {
            home.opaque_identity == selected.opaque_identity
                && home.path == selected.path
                && home.structural_type == selected.structural_type
        };
        same_home(binding) && self.descriptor_home.as_ref().is_none_or(same_home)
    }

    pub(crate) fn storage_address_coordinate(&self, index: usize) -> Option<(u64, u64)> {
        if index >= self.len() {
            return None;
        }
        Some((
            self.backing.exposure_namespace?,
            u64::try_from(self.window.start.checked_add(index)?).ok()?,
        ))
    }

    pub(crate) fn len(&self) -> usize {
        self.window.end - self.window.start
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        &self.backing.storage.bytes()[self.window.clone()]
    }

    /// A foreign host result cannot charge another invocation or omit the
    /// receiving invocation's storage budget. Preserve its allocation only
    /// when it already belongs to that exact account.
    pub(crate) fn into_account(
        self,
        account: Option<&evaluation_accounting::EvaluationInvocationAccount>,
    ) -> Result<Self, crate::errors::TerminalInterpretError> {
        let same = match (self.backing.storage.account(), account) {
            (None, None) => true,
            (Some(previous), Some(current)) => previous.same_invocation(current),
            _ => false,
        };
        if same {
            return Ok(self);
        }
        Ok(Self::new(storage::ByteSequenceStorage::copy(
            self.bytes(),
            account,
        )?))
    }

    pub(crate) fn get(&self, index: usize) -> Option<&u8> {
        self.bytes().get(index)
    }

    /// Replace one live byte without changing another view's immutable value.
    /// Shared backing detaches only this window, not unused source bytes.
    pub(crate) fn replace_byte(
        &mut self,
        byte_index: usize,
        value: u8,
    ) -> Result<(), crate::errors::TerminalInterpretError> {
        if byte_index >= self.len() {
            return Err(crate::errors::malformed());
        }
        let storage_index = self
            .window
            .start
            .checked_add(byte_index)
            .ok_or_else(crate::errors::malformed)?;
        if let Some(bytes) = Arc::get_mut(&mut self.backing) {
            *bytes
                .storage
                .bytes_mut()
                .get_mut(storage_index)
                .ok_or_else(crate::errors::malformed)? = value;
        } else {
            let mut storage =
                storage::ByteSequenceStorage::copy(self.bytes(), self.backing.storage.account())?;
            storage.bytes_mut()[byte_index] = value;
            *self = Self::new(storage);
        }
        Ok(())
    }

    pub(crate) fn subslice(&self, start: usize, end: usize) -> Option<Self> {
        self.bytes().get(start..end)?;
        Some(Self {
            backing: Arc::clone(&self.backing),
            window: self.window.start.checked_add(start)?..self.window.start.checked_add(end)?,
            descriptor_home: None,
        })
    }
}

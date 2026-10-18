//! Execution-wide structural identities for local storage and host results.
//!
//! Record copies and successor bindings stage allocation through a private cursor
//! and publish it with their payloads. Host reservations prevent later local
//! allocations from colliding with supplied identities.

use std::collections::BTreeSet;

use terminal_psi::StructuralPlaceDeclaration;

use crate::errors::{TerminalInterpretError, malformed};
use crate::values::TerminalStructuralValue;

pub(crate) struct LocalStructuralIdentities {
    reserved: BTreeSet<u64>,
    next: Option<u64>,
    pub(crate) exposure_namespace: Option<u64>,
}

impl LocalStructuralIdentities {
    /// Reserve the identities the admitted program's declared structural
    /// places own plus every host-supplied argument identity. `places` is the
    /// program's declared-place iterator — executable bodies live on the
    /// shared program, not on any one instance.
    pub(crate) fn new<'a>(
        structural_places: impl IntoIterator<Item = &'a StructuralPlaceDeclaration>,
        arguments: &[TerminalStructuralValue],
    ) -> Self {
        Self {
            reserved: structural_places
                .into_iter()
                .map(|place| place.id.get())
                .chain(arguments.iter().map(|value| value.opaque_identity))
                .collect(),
            next: Some(0),
            // A process-wide occurrence namespace lets addresses from separately
            // loaded programs meet without reusing a compiler-local identity.
            exposure_namespace: new_exposure_namespace(),
        }
    }

    pub(crate) fn allocate(&mut self) -> Result<u64, TerminalInterpretError> {
        let mut cursor = self.next;
        let result = self.allocate_staged(&mut cursor);
        self.next = cursor;
        result
    }

    pub(crate) fn cursor(&self) -> Option<u64> {
        self.next
    }

    /// Reserve identities in a transaction-local cursor without changing the
    /// live allocator or cloning its immutable host reservation set.
    pub(crate) fn allocate_staged(
        &self,
        cursor: &mut Option<u64>,
    ) -> Result<u64, TerminalInterpretError> {
        loop {
            let identity = cursor.ok_or(TerminalInterpretError::StructuralIdentityExhausted)?;
            *cursor = identity.checked_add(1);
            if !self.reserved.contains(&identity) {
                return Ok(identity);
            }
        }
    }

    pub(crate) fn commit_cursor(&mut self, cursor: Option<u64>) {
        self.next = cursor;
    }

    /// Boundary inputs can stage local identities before the host returns a
    /// value. Reject collisions with those identities without committing their
    /// cursor or payload if the host result is rejected.
    pub(crate) fn reserve_host_at_cursor(
        &mut self,
        value: &TerminalStructuralValue,
        cursor: Option<u64>,
    ) -> Result<(), TerminalInterpretError> {
        if match (self.next, cursor) {
            (Some(current), Some(staged)) => staged < current,
            (None, Some(_)) => true,
            _ => false,
        } {
            return Err(malformed());
        }
        let identity = value.opaque_identity;
        // A host result cannot claim an identity allocated to an interpreter local.
        if cursor.is_none_or(|next| identity < next) && !self.reserved.contains(&identity) {
            return Err(TerminalInterpretError::StructuralArgumentAliasing(identity));
        }
        self.reserved.insert(identity);
        Ok(())
    }
}

/// Unique occurrence namespaces for executions and independently owned byte
/// backings. This is an allocator, not an address-to-storage lookup registry.
pub(crate) fn new_exposure_namespace() -> Option<u64> {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    NEXT.try_update(
        std::sync::atomic::Ordering::Relaxed,
        std::sync::atomic::Ordering::Relaxed,
        |next| next.checked_add(1),
    )
    .ok()
}

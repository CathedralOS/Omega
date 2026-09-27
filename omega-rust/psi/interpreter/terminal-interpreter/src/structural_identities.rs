//! Execution-wide structural identities for local storage and host results.
//!
//! Record copies and successor bindings stage allocation through a private cursor
//! and publish it with their payloads. Host reservations prevent later local
//! allocations from colliding with supplied identities.

use std::collections::BTreeSet;

use terminal_psi::TerminalModule;

use crate::errors::TerminalInterpretError;
use crate::values::TerminalStructuralValue;

pub(crate) struct LocalStructuralIdentities {
    reserved: BTreeSet<u64>,
    next: Option<u64>,
}

impl LocalStructuralIdentities {
    #[cfg(test)]
    pub(crate) fn with_reserved_identities(reserved: impl IntoIterator<Item = u64>) -> Self {
        Self {
            reserved: reserved.into_iter().collect(),
            next: Some(0),
        }
    }

    pub(crate) fn new(module: &TerminalModule, arguments: &[TerminalStructuralValue]) -> Self {
        Self {
            reserved: module
                .machines
                .iter()
                .flat_map(|machine| &machine.structural_places)
                .map(|place| place.id.get())
                .chain(arguments.iter().map(|value| value.opaque_identity))
                .collect(),
            next: Some(0),
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

    pub(crate) fn reserve_host(
        &mut self,
        value: &TerminalStructuralValue,
    ) -> Result<(), TerminalInterpretError> {
        let identity = value.opaque_identity;
        // A host result cannot claim an identity allocated to an interpreter local.
        if self.next.is_none_or(|next| identity < next) && !self.reserved.contains(&identity) {
            return Err(TerminalInterpretError::StructuralArgumentAliasing(identity));
        }
        self.reserved.insert(identity);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::errors::TerminalInterpretError;
    use crate::structural_identities::LocalStructuralIdentities;
    use crate::values::TerminalStructuralValue;
    use semantic_vocabulary::StructuralTypeId;
    use std::collections::BTreeSet;

    #[test]
    fn host_results_cannot_reintroduce_allocated_local_identity() {
        let mut identities = LocalStructuralIdentities {
            reserved: BTreeSet::from([0]),
            next: Some(0),
        };
        let view = |opaque_identity| TerminalStructuralValue {
            opaque_identity,
            structural_type: StructuralTypeId::new(1).unwrap(),
            qualifications: Vec::new(),
            path: Vec::new(),
        };
        identities.reserve_host(&view(2)).unwrap();
        assert_eq!(identities.allocate(), Ok(1));
        assert_eq!(
            identities.reserve_host(&view(1)),
            Err(TerminalInterpretError::StructuralArgumentAliasing(1))
        );
        identities.reserve_host(&view(0)).unwrap();
        assert_eq!(identities.allocate(), Ok(3));
        assert_eq!(
            identities.reserved,
            BTreeSet::from([0, 2]),
            "allocation does not retain one tombstone per dead local"
        );
    }
}

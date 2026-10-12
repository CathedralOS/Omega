//! Portable declaration identity shared by emitted bodies and their demands.
//!
//! Psi derives nominal bytes from exact package/toolchain ownership and the
//! declaration path. These records identify checked inputs; they do not grant
//! execution authority or replace the artifact's semantic proof.
//! Table coordinates belong to one module, not a global declaration namespace.
//! Structural equality is useful within that module; cross-artifact joins use
//! `same_identity_as` with both owning tables and retain every commitment.

use crate::terminal_module::{TerminalIdentityIndex, TerminalIdentityTable};
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NominalDeclarationIdentity(TerminalIdentityIndex);

impl NominalDeclarationIdentity {
    pub fn new(identity: TerminalIdentityIndex) -> Self {
        Self(identity)
    }

    pub fn identity(&self) -> TerminalIdentityIndex {
        self.0
    }

    /// Compare nominal content across independently interned module tables.
    /// Absent or out-of-range identities never establish a declaration join.
    pub fn same_identity_as(
        &self,
        identities: &TerminalIdentityTable,
        other: &Self,
        other_identities: &TerminalIdentityTable,
    ) -> bool {
        let Some(identity) = identities
            .text(self.0)
            .filter(|identity| !identity.is_empty())
        else {
            return false;
        };
        other_identities.text(other.0) == Some(identity)
    }
}

/// Exact checked callable and initial state from which an executable body is
/// produced. Optimization preserves this identity, not the source arenas.
/// Derived equality compares coordinates within one issuing table. Independent
/// artifacts use [`Self::content_matches`] with both original tables.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CheckedMachineDeclaration {
    pub machine: NominalDeclarationIdentity,
    pub state: NominalDeclarationIdentity,
    pub contract: [u8; 32],
    pub specialization: Option<CheckedMachineSpecialization>,
}

impl CheckedMachineDeclaration {
    /// Join complete checked declaration content across independently produced
    /// modules. Every present nominal identity resolves in its own issuer even
    /// when other declaration components differ. This establishes declaration
    /// correspondence, not semantic proof, ABI admission or execution authority.
    pub fn content_matches(
        &self,
        identities: &TerminalIdentityTable,
        other: &Self,
        other_identities: &TerminalIdentityTable,
    ) -> Result<bool, &'static str> {
        let machine_matches = identities.identity_content_matches(
            self.machine.identity(),
            other_identities,
            other.machine.identity(),
        )?;
        let state_matches = identities.identity_content_matches(
            self.state.identity(),
            other_identities,
            other.state.identity(),
        )?;
        let specialization = self
            .specialization
            .as_ref()
            .map(|specialization| {
                identities
                    .required_text(specialization.template.identity())
                    .map(|template| (template, specialization.commitment))
            })
            .transpose()?;
        let other_specialization = other
            .specialization
            .as_ref()
            .map(|specialization| {
                other_identities
                    .required_text(specialization.template.identity())
                    .map(|template| (template, specialization.commitment))
            })
            .transpose()?;
        Ok(machine_matches
            && state_matches
            && self.contract == other.contract
            && specialization == other_specialization)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CheckedMachineSpecialization {
    pub template: NominalDeclarationIdentity,
    pub commitment: [u8; 32],
}

#[cfg(test)]
mod tests {
    use super::NominalDeclarationIdentity;
    use crate::terminal_module::{TerminalIdentityIndex, TerminalIdentityTableBuilder};

    #[test]
    fn nominal_identity_compares_content_in_each_owning_table() {
        let mut caller = TerminalIdentityTableBuilder::default();
        let first = NominalDeclarationIdentity::new(caller.intern("first"));
        let second = NominalDeclarationIdentity::new(caller.intern("second"));
        let caller = caller.build();

        let mut callee = TerminalIdentityTableBuilder::default();
        let other_second = NominalDeclarationIdentity::new(callee.intern("second"));
        let other_first = NominalDeclarationIdentity::new(callee.intern("first"));
        let callee = callee.build();

        assert_eq!(first.identity(), other_second.identity());
        assert!(!first.same_identity_as(&caller, &other_second, &callee));
        assert!(first.same_identity_as(&caller, &other_first, &callee));
        assert!(second.same_identity_as(&caller, &other_second, &callee));

        let absent = NominalDeclarationIdentity::new(TerminalIdentityIndex::ABSENT);
        assert!(!absent.same_identity_as(&caller, &absent, &callee));
        let missing = NominalDeclarationIdentity::new(TerminalIdentityIndex(u32::MAX));
        assert!(!missing.same_identity_as(&caller, &missing, &callee));
        assert!(!first.same_identity_as(&caller, &missing, &callee));
    }
}

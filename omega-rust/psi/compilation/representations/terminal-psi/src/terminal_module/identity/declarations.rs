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
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CheckedMachineDeclaration {
    pub machine: NominalDeclarationIdentity,
    pub state: NominalDeclarationIdentity,
    /// Source signature identities retained independently of executable storage.
    pub semantic_declaration: TerminalIdentityIndex,
    pub parameter_types: Vec<TerminalIdentityIndex>,
    pub callable: TerminalIdentityIndex,
    pub result_type: TerminalIdentityIndex,
    pub contract: [u8; 32],
    pub specialization: Option<CheckedMachineSpecialization>,
}

impl CheckedMachineDeclaration {
    /// Join checked declarations from distinct artifacts without comparing
    /// their table-local coordinates. Contract and specialization commitments
    /// remain exact; matching a name alone cannot substitute another body.
    pub fn same_identity_as(
        &self,
        identities: &TerminalIdentityTable,
        other: &Self,
        other_identities: &TerminalIdentityTable,
    ) -> bool {
        if !self
            .machine
            .same_identity_as(identities, &other.machine, other_identities)
            || !self
                .state
                .same_identity_as(identities, &other.state, other_identities)
            || identities.text(self.semantic_declaration)
                != other_identities.text(other.semantic_declaration)
            || self.parameter_types.len() != other.parameter_types.len()
            || self
                .parameter_types
                .iter()
                .zip(&other.parameter_types)
                .any(|(left, right)| identities.text(*left) != other_identities.text(*right))
            || identities.text(self.callable) != other_identities.text(other.callable)
            || identities.text(self.result_type) != other_identities.text(other.result_type)
            || self.contract != other.contract
        {
            return false;
        }
        match (&self.specialization, &other.specialization) {
            (None, None) => true,
            (Some(specialization), Some(other_specialization)) => {
                specialization.commitment == other_specialization.commitment
                    && specialization.template.same_identity_as(
                        identities,
                        &other_specialization.template,
                        other_identities,
                    )
            }
            _ => false,
        }
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

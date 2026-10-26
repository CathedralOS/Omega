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

/// Exact checked callable coordinates. Machine bodies use the machine and its
/// initial state; bodyless trait requirements use the declaring trait and its
/// signature. Neither coordinate names a selected implementation. Optimization
/// preserves this identity, not the source arenas.
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
    /// The existing proof-facade callable identity, with its exact executable
    /// specialization commitment when present. Producers and receivers share
    /// this rendering; declaration names never select an application.
    pub fn callable_evidence_identity(callable: &str, specialization: Option<&[u8; 32]>) -> String {
        let Some(commitment) = specialization else {
            return callable.to_owned();
        };
        let mut hexadecimal = String::with_capacity(64);
        for byte in commitment {
            use std::fmt::Write;
            write!(&mut hexadecimal, "{byte:02x}")
                .expect("writing an identity to String cannot fail");
        }
        format!(
            "specialized-machine|callable={}:{}|application={}",
            callable.len(),
            callable,
            hexadecimal
        )
    }

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
                    && match (&specialization.capture, &other_specialization.capture) {
                        (None, None) => true,
                        (Some(left), Some(right)) => left.same_identity_as(identities, right, other_identities),
                        _ => false,
                    }
                    && identities
                        .text(specialization.template_callable)
                        .zip(other_identities.text(other_specialization.template_callable))
                        .is_some_and(|(left, right)| !left.is_empty() && left == right)
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
    /// Independent expected capture receipt, before any reach projection.
    pub capture: Option<CheckedMachineCapture>,
    pub template: NominalDeclarationIdentity,
    /// Exact authored overload, distinct from the instantiated callable and
    /// from the template's nominal declaration path. Issuer routes name this
    /// declaration, not a same-spelled specialization or sibling overload.
    pub template_callable: TerminalIdentityIndex,
    pub commitment: [u8; 32],
}

/// Exact capture composition retained on the checked declaration independently
/// of its optional closed-reach payload. Omitting a wholly unused prefix cannot
/// turn a captured method into an ordinary invocation.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CheckedMachineCapture {
    pub template_identity: TerminalIdentityIndex,
    pub template_commitment: [u8; 32],
    pub specialization_commitment: [u8; 32],
}

impl CheckedMachineCapture {
    pub fn same_identity_as(
        &self,
        identities: &TerminalIdentityTable,
        other: &Self,
        other_identities: &TerminalIdentityTable,
    ) -> bool {
        self.template_commitment == other.template_commitment
            && self.specialization_commitment == other.specialization_commitment
            && identities
                .text(self.template_identity)
                .zip(other_identities.text(other.template_identity))
                .is_some_and(|(left, right)| !left.is_empty() && left == right)
    }
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

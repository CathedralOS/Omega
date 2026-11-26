use crate::{TerminalIdentityIndex, TerminalIdentityTable};
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, record_storage::Record)]
pub struct NominalDeclarationIdentity(TerminalIdentityIndex);

impl NominalDeclarationIdentity {
    pub fn new(identity: TerminalIdentityIndex) -> Self {
        Self(identity)
    }

    pub fn identity(&self) -> TerminalIdentityIndex {
        self.0
    }

    pub fn same_identity_as(
        &self,
        identities: &TerminalIdentityTable,
        other: &Self,
        other_identities: &TerminalIdentityTable,
    ) -> bool {
        if identities.is_invocation_local(self.0)
            || other_identities.is_invocation_local(other.0)
        {
            return identities.same_invocation_declaration(self.0, other_identities, other.0);
        }
        let Some(identity) = identities
            .text(self.0)
            .filter(|identity| !identity.is_empty())
        else {
            return false;
        };
        other_identities.text(other.0) == Some(identity)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CheckedMachineDeclaration {
    pub machine: NominalDeclarationIdentity,
    pub state: NominalDeclarationIdentity,
    pub semantic_declaration: TerminalIdentityIndex,
    pub parameter_types: Vec<TerminalIdentityIndex>,
    pub callable: TerminalIdentityIndex,
    pub result_type: TerminalIdentityIndex,
    pub contract: [u8; 32],
    pub published_may_suspend: bool,
    pub published_may_block: bool,
    pub specialization: Option<CheckedMachineSpecialization>,
}

impl CheckedMachineDeclaration {
    pub fn callable_evidence_identity(callable: &str, specialization: Option<&[u8; 32]>) -> String {
        let Some(commitment) = specialization else {
            return callable.to_owned();
        };
        let hexadecimal = crate::digest_encoder::hex(commitment);
        format!(
            "specialized-machine|callable={}:{}|application={}",
            callable.len(),
            callable,
            hexadecimal
        )
    }

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
            || self.published_may_suspend != other.published_may_suspend
            || self.published_may_block != other.published_may_block
        {
            return false;
        }
        match (&self.specialization, &other.specialization) {
            (None, None) => true,
            (Some(specialization), Some(other_specialization)) => {
                specialization.commitment == other_specialization.commitment
                    && match (&specialization.capture, &other_specialization.capture) {
                        (None, None) => true,
                        (Some(left), Some(right)) => {
                            left.same_identity_as(identities, right, other_identities)
                        }
                        _ => false,
                    }
                    && identities
                        .text(specialization.template_callable)
                        .zip(other_identities.text(other_specialization.template_callable))
                        .is_some_and(|(left, right)| !left.is_empty() && left == right)
                    && identities
                        .text(specialization.template_semantic_declaration)
                        .zip(
                            other_identities
                                .text(other_specialization.template_semantic_declaration),
                        )
                        .is_some_and(|(left, right)| !left.is_empty() && left == right)
                    && specialization.template.same_identity_as(
                        identities,
                        &other_specialization.template,
                        other_identities,
                    )
                    && specialization.template_entry.same_identity_as(
                        identities,
                        &other_specialization.template_entry,
                        other_identities,
                    )
            }
            _ => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CheckedMachineSpecialization {
    pub capture: Option<CheckedMachineCapture>,
    pub template: NominalDeclarationIdentity,
    /// The original entry is distinct from the generated body's state.
    /// Caller descriptions must retain this link rather than infer an
    /// entry from the machine's spelling or its parameter types.
    pub template_entry: NominalDeclarationIdentity,
    pub template_callable: TerminalIdentityIndex,
    pub template_semantic_declaration: TerminalIdentityIndex,
    pub commitment: [u8; 32],
}

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
    use crate::{TerminalIdentityIndex, TerminalIdentityTableBuilder};

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

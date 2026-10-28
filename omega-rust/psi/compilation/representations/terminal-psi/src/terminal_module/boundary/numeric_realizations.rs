//! Checked numeric catalogue realization is portable; choosing and admitting a provider
//! is not. The explicit requirement survives lowering even without a call.

use crate::{
    CheckedMachineDeclaration, NominalDeclarationIdentity, TerminalIdentityIndex,
    TerminalIdentityTable,
};
use semantic_vocabulary::PackageKeyIdentity;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NumericRealizationOwner {
    pub declaration: NominalDeclarationIdentity,
    pub identity: TerminalIdentityIndex,
    pub package: Option<PackageKeyIdentity>,
}

/// A bodyless realization has no Terminal MachineId or body refinement to
/// replay. Psi has checked its satisfaction edge. Retain the exact declaration
/// and mechanism so a consumer can join an independently selected whole plan
/// without reopening source or treating numeric meaning as provider authority.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NumericRealization {
    pub requirement: NominalDeclarationIdentity,
    pub requirement_identity: TerminalIdentityIndex,
    pub requirement_package: Option<PackageKeyIdentity>,
    pub numeric_intrinsic: language_semantics::numeric_intrinsics::NumericIntrinsic,
    pub instruction_set: language_semantics::numeric_intrinsics::NumericInstructionSet,
    pub realization: CheckedMachineDeclaration,
    pub realization_package: Option<PackageKeyIdentity>,
    pub provider: Option<NumericRealizationOwner>,
}

impl NumericRealization {
    pub fn validate(&self, identities: &TerminalIdentityTable) -> Result<(), &'static str> {
        let declaration = &self.realization;
        let present = |identity| {
            identities
                .text(identity)
                .is_some_and(|text| !text.is_empty())
        };
        if self.numeric_intrinsic.signature().is_none() {
            return Err("numeric realization has invalid numeric meaning");
        }
        if ![
            self.requirement.identity(),
            self.requirement_identity,
            declaration.machine.identity(),
            declaration.state.identity(),
            declaration.semantic_declaration,
            declaration.callable,
            declaration.result_type,
        ]
        .into_iter()
        .chain(declaration.parameter_types.iter().copied())
        .all(present)
            || declaration.contract == [0; 32]
        {
            return Err("external provider lost its exact checked declaration");
        }
        if let Some(specialization) = &declaration.specialization {
            if ![
                specialization.template.identity(),
                specialization.template_semantic_declaration,
                specialization.template_callable,
            ]
            .into_iter()
            .all(present)
                || specialization.commitment == [0; 32]
                || specialization.capture.as_ref().is_some_and(|capture| {
                    !present(capture.template_identity)
                        || capture.template_commitment == [0; 32]
                        || capture.specialization_commitment == [0; 32]
                })
            {
                return Err("external provider lost its checked specialization");
            }
        }
        if self.provider.as_ref().is_some_and(|provider| {
            !present(provider.declaration.identity()) || !present(provider.identity)
        }) {
            return Err("external provider lost its selecting declaration");
        }
        Ok(())
    }
}

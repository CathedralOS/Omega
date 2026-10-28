//! Checked external satisfaction is portable; choosing and admitting a provider
//! is not. These edges belong to the enclosing exact boundary requirement.

use crate::{
    CheckedMachineDeclaration, NominalDeclarationIdentity, TerminalIdentityIndex,
    TerminalIdentityTable,
};
use language_semantics::ExternalBindingIdentity;
use semantic_vocabulary::PackageKeyIdentity;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExternalProviderOwner {
    pub declaration: NominalDeclarationIdentity,
    pub identity: TerminalIdentityIndex,
    pub package: Option<PackageKeyIdentity>,
}

/// A bodyless realization has no Terminal MachineId or body refinement to
/// replay. Psi has checked its satisfaction edge. Retain the exact declaration
/// and mechanism so a consumer can join an independently selected whole plan
/// without reopening source or treating numeric meaning as provider authority.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ExternalProviderConformance {
    pub realization: CheckedMachineDeclaration,
    pub realization_package: Option<PackageKeyIdentity>,
    pub provider: Option<ExternalProviderOwner>,
    pub binding: ExternalBindingIdentity,
}

impl ExternalProviderConformance {
    pub fn validate(&self, identities: &TerminalIdentityTable) -> Result<(), &'static str> {
        let declaration = &self.realization;
        let present = |identity| {
            identities
                .text(identity)
                .is_some_and(|text| !text.is_empty())
        };
        if ![
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
        match &self.binding {
            ExternalBindingIdentity::VtableField { field }
            | ExternalBindingIdentity::TableFunction { field }
                if field.is_empty() =>
            {
                Err("external provider has an empty binding field")
            }
            _ => Ok(()),
        }
    }
}

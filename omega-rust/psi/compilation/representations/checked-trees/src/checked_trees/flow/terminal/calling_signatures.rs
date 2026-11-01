//! Declaration ABI storage retained before structural shape pruning.
//!
//! These rows are descriptive checked input, not provider choice or a second
//! public invocation contract. Exact boundary operations keep their own
//! contracts and callback selections; several may reference one ABI row.

use language_semantics::callback_declarations::BoundaryTelescope;
use semantic_vocabulary::{CallingSignatureId, CallingStaticOperand};
use typed_trees::type_identity::TypeIdentityId;

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct CheckedCallingSignaturePlan {
    pub id: CallingSignatureId,
    pub source_template: symbols::SymbolHandle,
    pub requirement_identity: String,
    pub declaring_application: String,
    pub operands: Vec<CallingStaticOperand>,
    pub formals: Vec<semantic_vocabulary::CallingGenericFormal>,
    pub default_eligible: bool,
    pub storage: Result<CheckedCallingSignatureStorage, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct CheckedCallingSignatureStorage {
    pub parameters: Vec<CheckedCallingParameter>,
    pub result: Option<TypeIdentityId>,
    pub telescope: BoundaryTelescope,
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct CheckedCallingParameter {
    pub type_identity: TypeIdentityId,
    /// Complete formal backing when it has a declared representation. An
    /// abstract pointer may transport without inventing pointee storage.
    pub callback_backing: Option<TypeIdentityId>,
    /// Retains an abstract referent's pointer/descriptor judgment through
    /// shape pruning, without demanding the referent's own storage.
    pub reference_custody: Option<TypeIdentityId>,
    pub access: super::CheckedStructuralAccess,
}

impl CheckedCallingSignatureStorage {
    pub fn type_roots(&self) -> impl Iterator<Item = TypeIdentityId> + '_ {
        self.parameters
            .iter()
            .flat_map(|parameter| {
                std::iter::once(
                    parameter
                        .reference_custody
                        .unwrap_or(parameter.type_identity),
                )
                .chain(parameter.callback_backing)
            })
            .chain(self.result)
    }
}

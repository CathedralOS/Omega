//! Checked-source nominal and physical shape of a routed `Binding<R>` shell.
//!
//! Later representations consume this checked-owned summary rather than
//! reopening the typed-tree vocabulary that was retained as checking custody.
//! No authored qualification is accepted on the carrier. This shape summary
//! does not close a receiving application: its supplying scope and checked
//! provider correspondence belong to the occurrence and establishment receipt.

use language_semantics::SemanticDomainId;
use symbols::SymbolHandle;

use crate::{CheckedTrees, types};
use typed_trees::type_identity::TypeIdentityId;

/// Exact checked-source meaning needed to rejoin a routed Service parameter to
/// its compiler-owned erasure receipt. This summary grants no provider,
/// storage, ABI, or erasure authority by itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedServiceParameterCarrier {
    pub service_data: SymbolHandle,
    pub requirement: SymbolHandle,
    pub carrier_type_identity: TypeIdentityId,
    pub base_type_identity: TypeIdentityId,
    pub qualifications: Vec<SemanticDomainId>,
    pub access: crate::CheckedStructuralAccess,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckedServiceParameterError {
    InvalidCarrier(String),
}

impl std::fmt::Display for CheckedServiceParameterError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidCarrier(reason) => formatter.write_str(reason),
        }
    }
}

impl std::error::Error for CheckedServiceParameterError {}

impl CheckedTrees {
    /// Classify and project one already-checked type shell. `Ok(None)` means
    /// that the source parameter is not the exact core routed Binding carrier.
    pub fn service_parameter_shape(
        &self,
        type_reference: types::TypeReferenceHandle,
    ) -> Result<Option<CheckedServiceParameterCarrier>, CheckedServiceParameterError> {
        let Some(carrier) =
            typed_trees::service::inspect_service_parameter_contract(&self.typed, type_reference)
                .map_err(CheckedServiceParameterError::InvalidCarrier)?
        else {
            return Ok(None);
        };

        let carrier_type_identity = self.normalized_type_identity(type_reference);
        let (base_type_identity, qualifications) =
            self.service_base_and_qualifications(carrier.carrier_type);
        Ok(Some(CheckedServiceParameterCarrier {
            service_data: carrier.carrier.service_data,
            requirement: carrier.carrier.requirement,
            carrier_type_identity,
            base_type_identity,
            qualifications,
            access: carrier
                .access
                .map_or(crate::CheckedStructuralAccess::Owned, Into::into),
        }))
    }

    fn service_base_and_qualifications(
        &self,
        mut type_reference: types::TypeReferenceHandle,
    ) -> (TypeIdentityId, Vec<SemanticDomainId>) {
        let mut qualifications = Vec::new();
        while let types::TypeReferenceNode::Constrained {
            base_type,
            constraints,
        } = self.type_reference_table.type_reference(type_reference)
        {
            qualifications.extend(
                self.type_reference_table
                    .constraints(*constraints)
                    .iter()
                    .filter_map(|constraint| match constraint {
                        types::TypeConstraintNode::Domain(domain) => Some(domain.semantic_id),
                        _ => None,
                    }),
            );
            type_reference = *base_type;
        }
        (
            self.normalized_type_identity(type_reference),
            qualifications,
        )
    }
}

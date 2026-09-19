//! Checked-source projection for the exact routed `Service<R>` parameter
//! carrier.
//!
//! Later representations consume this checked-owned summary rather than
//! reopening the typed-tree vocabulary that was retained as checking custody.
//! The carrier is closed: `Service<R>` alone is the service identity and no
//! authored qualification is accepted on it. The `bound_` prefix on the entry
//! point is a retired-era relic kept for callers that have not migrated.

use language_semantics::SemanticDomainId;
use symbols::SymbolHandle;

use crate::{CheckedTrees, types};

/// Exact checked-source meaning needed to rejoin a routed Service parameter to
/// its compiler-owned erasure receipt. This summary grants no provider,
/// storage, ABI, or erasure authority by itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedServiceParameterCarrier {
    pub service_data: SymbolHandle,
    pub requirement: SymbolHandle,
    pub carrier_type_identity: String,
    pub base_type_identity: String,
    pub qualifications: Vec<SemanticDomainId>,
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
    /// that the source parameter is not the exact core routed Service carrier.
    pub fn bound_service_parameter_carrier(
        &self,
        type_reference: types::TypeReferenceHandle,
    ) -> Result<Option<CheckedServiceParameterCarrier>, CheckedServiceParameterError>
    {
        let Some(carrier) =
            typed_trees::service::classify_exact_bound_service_carrier(&self.typed, type_reference)
                .map_err(CheckedServiceParameterError::InvalidCarrier)?
        else {
            return Ok(None);
        };

        let carrier_type_identity = self.normalized_type_identity(type_reference).into_string();
        let (base_type_identity, qualifications) =
            self.service_base_and_qualifications(type_reference);
        Ok(Some(CheckedServiceParameterCarrier {
            service_data: carrier.service_data,
            requirement: carrier.requirement,
            carrier_type_identity,
            base_type_identity,
            qualifications,
        }))
    }

    fn service_base_and_qualifications(
        &self,
        mut type_reference: types::TypeReferenceHandle,
    ) -> (String, Vec<SemanticDomainId>) {
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
            self.normalized_type_identity(type_reference).into_string(),
            qualifications,
        )
    }
}

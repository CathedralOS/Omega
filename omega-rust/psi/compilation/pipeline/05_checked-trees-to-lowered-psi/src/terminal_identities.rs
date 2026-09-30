//! Fixed Terminal-Psi identity allocation and checked-to-lowered identity lookup.
//!
//! Producers allocate dense nonzero identities from these constructors and
//! rejoin checked sources to the lowered tables through the lookup functions.
//! Both lookup failures and namespace exhaustion fail closed.

use language_semantics::{PermissionClaimIdentity, ServiceReachId};
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, ClaimId, ContractId, EdgeId, MachineId, ObligationId, OperationId,
    PlaceId, PropositionId, ServiceId, StructuralDomainId, StructuralFieldId, StructuralTypeId,
    ValueId,
};

use crate::lowering_error::LoweringError;

pub(crate) const TERMINAL_MACHINE_IDENTITY_STRIDE: u64 = 1_u64 << 32;
// Structural Unit call requirements occupy the upper half of the module-wide
// obligation namespace. Existing contract and cleanup producers allocate from
// the lower half, so composing their proof bundles cannot alias a call site.
pub(crate) const TERMINAL_UNIT_CALL_OBLIGATION_BASE: u64 = 1_u64 << 63;

macro_rules! id_constructor {
    ($function:ident, $type:ty) => {
        pub(crate) fn $function(raw: u64) -> $type {
            <$type>::new(raw).expect("fixed terminal-Psi identities are nonzero")
        }
    };
}

id_constructor!(value_id, ValueId);
id_constructor!(structural_type_id, StructuralTypeId);
id_constructor!(structural_field_id, StructuralFieldId);
id_constructor!(structural_domain_id, StructuralDomainId);
id_constructor!(service_id, ServiceId);
id_constructor!(boundary_machine_id, BoundaryMachineId);
id_constructor!(machine_id, MachineId);
id_constructor!(block_id, BlockId);
id_constructor!(place_id, PlaceId);
id_constructor!(claim_id, ClaimId);
id_constructor!(operation_id, OperationId);
id_constructor!(edge_id, EdgeId);
id_constructor!(contract_id, ContractId);
id_constructor!(obligation_id, ObligationId);
id_constructor!(proposition_id, PropositionId);

/// The checked type identity of the `u8` scalar a borrowed `&[u8]` view
/// establishes through the element-view op family. The byte carrier keeps no
/// element field, so the catalog roots this row at that establishment and the
/// emitter resolves the op's element through it.
pub(crate) const BYTE_VIEW_ELEMENT_TYPE_IDENTITY: &str = "named(name(u8))";

pub(crate) fn lookup_type_id(
    ids: &[(String, StructuralTypeId)],
    identity: &str,
) -> Result<StructuralTypeId, LoweringError> {
    ids.iter()
        .find_map(|(candidate, id)| (candidate == identity).then_some(*id))
        .ok_or(LoweringError::Unsupported(
            "Unit closure references an unlowered structural type",
        ))
}

/// The Terminal attachment a checked plan retains. A receiver-free machine,
/// qualified or not, retains none.
pub(crate) fn lookup_attachment_type_id(
    ids: &[(String, StructuralTypeId)],
    identity: Option<&str>,
) -> Result<Option<StructuralTypeId>, LoweringError> {
    identity
        .map(|identity| lookup_type_id(ids, identity))
        .transpose()
}

/// A semantic domain family instantiated on one exact structural carrier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct StructuralDomainBinding {
    pub semantic_domain: language_semantics::SemanticDomainId,
    pub carrier: StructuralTypeId,
    pub domain: StructuralDomainId,
}

/// Diagnostic identity includes the authored carrier, never its allocation ID.
/// Always qualify it so pruning another instantiation cannot rename this one.
pub(crate) fn structural_domain_identity(family: &str, carrier_identity: &str) -> String {
    format!("{family}@{carrier_identity}")
}

pub(crate) fn lookup_domain_id(
    ids: &[StructuralDomainBinding],
    source: language_semantics::SemanticDomainId,
) -> Result<StructuralDomainId, LoweringError> {
    let mut matches = ids.iter().filter(|row| row.semantic_domain == source);
    let first = matches.next().ok_or(LoweringError::Unsupported(
        "Unit closure references an unlowered structural domain",
    ))?;
    if matches.next().is_some() {
        return Err(LoweringError::Unsupported(
            "Unit closure structural domain family requires an exact carrier",
        ));
    }
    Ok(first.domain)
}

pub(crate) fn lookup_domain_id_for_carrier(
    ids: &[StructuralDomainBinding],
    source: language_semantics::SemanticDomainId,
    carrier: StructuralTypeId,
) -> Result<StructuralDomainId, LoweringError> {
    let mut matches = ids
        .iter()
        .filter(|row| row.semantic_domain == source && row.carrier == carrier);
    let first = matches.next().ok_or(LoweringError::Unsupported(
        "Unit closure references an unlowered structural domain carrier",
    ))?;
    if matches.next().is_some() {
        return Err(LoweringError::Unsupported(
            "Unit closure repeats a structural domain carrier binding",
        ));
    }
    Ok(first.domain)
}

pub(crate) fn lookup_service_id(
    ids: &[(ServiceReachId, ServiceId)],
    source: ServiceReachId,
) -> Result<ServiceId, LoweringError> {
    ids.iter()
        .find_map(|(candidate, id)| (*candidate == source).then_some(*id))
        .ok_or(LoweringError::Unsupported(
            "Unit closure references an unlowered checked service",
        ))
}

pub(crate) fn lookup_machine_id(
    ids: &[(symbols::SymbolHandle, MachineId)],
    source: symbols::SymbolHandle,
) -> Result<MachineId, LoweringError> {
    ids.iter()
        .find_map(|(candidate, id)| (*candidate == source).then_some(*id))
        .ok_or(LoweringError::Unsupported(
            "Unit call target is absent from the lowered closure",
        ))
}

pub(crate) fn lookup_claim_id(
    ids: &[(PermissionClaimIdentity, ClaimId)],
    source: PermissionClaimIdentity,
) -> Result<ClaimId, LoweringError> {
    ids.iter()
        .find_map(|(candidate, id)| (*candidate == source).then_some(*id))
        .ok_or(LoweringError::Unsupported(
            "Unit claim transfer references a non-entry caller claim",
        ))
}

pub(crate) fn dense_identity(index: usize) -> Result<u64, LoweringError> {
    u64::try_from(index)
        .map_err(|_| LoweringError::Unsupported("terminal Unit identity count exceeds u64"))?
        .checked_add(1)
        .ok_or(LoweringError::Unsupported(
            "terminal Unit identity count exceeds u64",
        ))
}

pub(crate) fn allocate_dense(next: &mut u64) -> Result<u64, LoweringError> {
    let current = *next;
    *next = next.checked_add(1).ok_or(LoweringError::Unsupported(
        "terminal Unit identity count exceeds u64",
    ))?;
    Ok(current)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domain_family_binding_requires_exact_carrier_when_instantiated_twice() {
        let family = language_semantics::SemanticDomainId(1);
        let bindings = [
            StructuralDomainBinding {
                semantic_domain: family,
                carrier: structural_type_id(10),
                domain: structural_domain_id(1),
            },
            StructuralDomainBinding {
                semantic_domain: family,
                carrier: structural_type_id(27),
                domain: structural_domain_id(2),
            },
        ];
        assert!(lookup_domain_id(&bindings, family).is_err());
        assert_eq!(
            lookup_domain_id(&bindings[..1], family).unwrap(),
            structural_domain_id(1)
        );
        for binding in bindings {
            assert_eq!(
                lookup_domain_id_for_carrier(&bindings, family, binding.carrier).unwrap(),
                binding.domain
            );
        }
        assert!(lookup_domain_id_for_carrier(&bindings, family, structural_type_id(8)).is_err());
        assert!(
            lookup_domain_id_for_carrier(&[bindings[0], bindings[0]], family, bindings[0].carrier)
                .is_err()
        );
    }
}

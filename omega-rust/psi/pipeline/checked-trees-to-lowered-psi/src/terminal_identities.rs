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

pub(crate) fn lookup_domain_id(
    ids: &[(language_semantics::SemanticDomainId, StructuralDomainId)],
    source: language_semantics::SemanticDomainId,
) -> Result<StructuralDomainId, LoweringError> {
    ids.iter()
        .find_map(|(candidate, id)| (*candidate == source).then_some(*id))
        .ok_or(LoweringError::Unsupported(
            "Unit closure references an unlowered structural domain",
        ))
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

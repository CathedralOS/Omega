//! Typed identities shared by scalar-affine cleanup fixtures.

use psi_core::{
    BlockId, ClaimId, ContractId, EdgeId, MachineId, ObligationId, PlaceId, StructuralDomainId,
    StructuralTypeId, ValueId,
};

pub(super) fn machine_id(raw: u64) -> MachineId {
    MachineId::new(raw).unwrap()
}

pub(super) fn block_id(raw: u64) -> BlockId {
    BlockId::new(raw).unwrap()
}

pub(super) fn edge_id(raw: u64) -> EdgeId {
    EdgeId::new(raw).unwrap()
}

pub(super) fn contract_id(raw: u64) -> ContractId {
    ContractId::new(raw).unwrap()
}

pub(super) fn value_id(raw: u64) -> ValueId {
    ValueId::new(raw).unwrap()
}

pub(super) fn place_id(raw: u64) -> PlaceId {
    PlaceId::new(raw).unwrap()
}

pub(super) fn structural_type_id(raw: u64) -> StructuralTypeId {
    StructuralTypeId::new(raw).unwrap()
}

pub(super) fn structural_domain_id(raw: u64) -> StructuralDomainId {
    StructuralDomainId::new(raw).unwrap()
}

pub(super) fn claim_id(raw: u64) -> ClaimId {
    ClaimId::new(raw).unwrap()
}

pub(super) fn obligation_id(raw: u64) -> ObligationId {
    ObligationId::new(raw).unwrap()
}

use semantic_vocabulary::{
    BlockId, BoundaryMachineId, ClaimId, ContractId, EdgeId, MachineId, OperationId, PlaceId,
    ServiceId, StructuralTypeId,
};

pub(super) const REQUIREMENT: &str = "Signal::emit()->Unit";

pub(super) fn machine_id(value: u64) -> MachineId {
    MachineId::new(value).unwrap()
}
pub(super) fn boundary_id(value: u64) -> BoundaryMachineId {
    BoundaryMachineId::new(value).unwrap()
}
pub(super) fn structural_type_id(value: u64) -> StructuralTypeId {
    StructuralTypeId::new(value).unwrap()
}
pub(super) fn service_id(value: u64) -> ServiceId {
    ServiceId::new(value).unwrap()
}
pub(super) fn block_id(value: u64) -> BlockId {
    BlockId::new(value).unwrap()
}
pub(super) fn operation_id(value: u64) -> OperationId {
    OperationId::new(value).unwrap()
}
pub(super) fn place_id(value: u64) -> PlaceId {
    PlaceId::new(value).unwrap()
}
pub(super) fn claim_id(value: u64) -> ClaimId {
    ClaimId::new(value).unwrap()
}
pub(super) fn edge_id(value: u64) -> EdgeId {
    EdgeId::new(value).unwrap()
}
pub(super) fn contract_id(value: u64) -> ContractId {
    ContractId::new(value).unwrap()
}

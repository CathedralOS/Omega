//! Joint catalog and identity ownership for Unit bodies beneath external control.

use super::*;

/// Additional roots already selected from the external caller's authored body.
/// They join ordinary body discovery before any semantic identity is assigned.
pub(crate) struct ExternalUnitRoots<'a> {
    pub(crate) boundary_roots: &'a [symbols::SymbolHandle],
    pub(crate) structural_type_roots: &'a [String],
    pub(crate) service_roots: &'a [ServiceReachId],
    pub(crate) scalar_roots: &'a [symbols::SymbolHandle],
}

/// Ordinary bodies and scalar helpers are complete, but the reserved external
/// entry has not been emitted. Its graph uses these same catalogs and counters;
/// proof finalization follows insertion of that graph into this one module.
pub(crate) struct SharedUnitClosure {
    pub(crate) lowered: LoweredPsi,
    pub(crate) machine_ids: Vec<(symbols::SymbolHandle, MachineId)>,
    pub(crate) type_ids: Vec<(String, StructuralTypeId)>,
    pub(crate) domain_ids: Vec<(SemanticDomainId, StructuralDomainId)>,
    pub(crate) service_ids: Vec<(ServiceReachId, ServiceId)>,
    pub(crate) boundary_parameters: Vec<(
        symbols::SymbolHandle,
        BoundaryMachineId,
        Vec<StructuralParameterDeclaration>,
        Vec<ScalarType>,
    )>,
    pub(crate) scalar_requirement_counts: Vec<(symbols::SymbolHandle, usize)>,
    pub(crate) next_place: u64,
    pub(crate) next_value: u64,
    pub(crate) next_block: u64,
    pub(crate) next_operation: u64,
    pub(crate) next_edge: u64,
    pub(crate) next_call_obligation: u64,
}

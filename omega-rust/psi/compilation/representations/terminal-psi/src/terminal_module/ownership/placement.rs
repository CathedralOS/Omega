use crate::StructuralAccess;
use crate::terminal_module::TerminalIdentityIndex;
use semantic_vocabulary::MachineId;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalPlacedViewInput {
    pub machine: MachineId,
    pub position: u32,
    /// Dense structural ingress coordinate for this exact source state.
    /// Authored positions include scalars and erased formals and cannot index
    /// the execution lane. The machine's checked initial-state identity selects
    /// which roster rows actually arrive at an ordinary invocation.
    pub runtime_position: u32,
    /// Exact structural carrier identity, independently of the placement seal.
    pub carrier_identity: TerminalIdentityIndex,
    pub source_machine_identity: TerminalIdentityIndex,
    pub source_state_identity: TerminalIdentityIndex,
    pub source_parameter_identity: TerminalIdentityIndex,
    pub access: StructuralAccess,
    pub binding_is_const: bool,
    pub binding_is_mutable: bool,
    pub view_identity: TerminalIdentityIndex,
    pub policy_identity: TerminalIdentityIndex,
    pub policy_plan_machine_identity: TerminalIdentityIndex,
    pub schema_identity: TerminalIdentityIndex,
    /// Compatibility/report coordinate only. `placement_commitment` is the
    /// collision-resistant canonical layout/access/reach identity.
    pub placement_report_fingerprint: u64,
    pub placement_commitment: [u8; 32],
}

/// Canonical source-free identity of the synthesized `Placed<P, T>` view.
/// Length framing keeps the policy/schema pair injective even when declaration
/// paths contain punctuation used by the presentation grammar.
pub fn canonical_placed_view_identity(policy: &str, schema: &str) -> String {
    format!(
        "placed-view:{}:{policy}:{}:{schema}",
        policy.len(),
        schema.len()
    )
}

/// Source-free placement geometry and exact accessor contracts. The normalized
/// plan describes demand; actual storage, supply, rights and lifetime are bound
/// independently by establishment. No concrete address crosses this boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalPlacedViewPlan {
    pub carrier: semantic_vocabulary::StructuralTypeId,
    pub schema: semantic_vocabulary::StructuralTypeId,
    pub view_identity: TerminalIdentityIndex,
    pub policy_identity: TerminalIdentityIndex,
    pub policy_plan_machine_identity: TerminalIdentityIndex,
    pub schema_identity: TerminalIdentityIndex,
    pub placement: access_plans::ValidatedPlacementPlan,
    pub fields: Vec<TerminalPlacedFieldPlan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalPlacedFieldPlan {
    /// Exact normalizer-issued slot under the enclosing complete plan.
    pub access_slot: u32,
    /// Member identities on the view and original schema respectively.
    pub view_field: TerminalIdentityIndex,
    pub schema_field: TerminalIdentityIndex,
    pub accessor: semantic_vocabulary::StructuralTypeId,
    /// Only actually retained callable rows appear here; an unused source
    /// accessor cannot create an ambient provider or execution demand.
    pub operations: Vec<TerminalPlacedAccessor>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminalPlacedAccessorKind {
    Read,
    Take,
    Write,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalPlacedAccessor {
    pub boundary: semantic_vocabulary::BoundaryMachineId,
    pub kind: TerminalPlacedAccessorKind,
}

/// Physical compatibility metadata; this data grants no storage or access.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacedRamFieldCorrespondence {
    pub access_slot: u32,
    pub source_field: semantic_vocabulary::StructuralFieldId,
    pub byte_offset: u64,
    pub transfer_width_bits: u16,
    pub logical_scalar: semantic_vocabulary::ScalarType,
    pub stored_scalar: semantic_vocabulary::ScalarType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacedRamCorrespondence {
    pub schema: semantic_vocabulary::StructuralTypeId,
    pub carrier: semantic_vocabulary::StructuralTypeId,
    pub source_byte_length: u64,
    /// Remaining condition on the concrete captured source address. Declared
    /// type alignment alone cannot discharge it for a re-viewed source.
    pub required_base: access_plans::BaseCongruence,
    pub fields: Vec<PlacedRamFieldCorrespondence>,
}

/// Exact source-free scalar boundary demand. Geometry supplies no backing or loan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlacedScalarBoundaryDemand {
    pub boundary: semantic_vocabulary::BoundaryMachineId,
    pub carrier: semantic_vocabulary::StructuralTypeId,
    pub schema: semantic_vocabulary::StructuralTypeId,
    pub accessor: semantic_vocabulary::StructuralTypeId,
    pub view_field: crate::TerminalIdentityIndex,
    pub access_slot: u32,
    pub placement_commitment: [u8; 32],
    pub operation: TerminalPlacedAccessorKind,
    pub access: crate::StructuralAccess,
    pub scalar: semantic_vocabulary::ScalarType,
    pub byte_offset: u64,
    pub transfer_width_bits: u16,
    pub placement_byte_extent: u64,
    pub observation: access_plans::ObservationModel,
}

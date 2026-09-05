use crate::StructuralAccess;
use semantic_vocabulary::MachineId;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TerminalPlacedViewInput {
    pub machine: MachineId,
    pub position: u32,
    pub source_machine_identity: String,
    pub source_state_identity: String,
    pub source_parameter_identity: String,
    pub access: StructuralAccess,
    pub binding_is_const: bool,
    pub binding_is_mutable: bool,
    pub view_identity: String,
    pub policy_identity: String,
    pub policy_plan_machine_identity: String,
    pub schema_identity: String,
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

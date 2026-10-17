//! Exact foreign callable dependencies, before executable demand selects them.
//!
//! A direct call names its leaf, not the whole-trait implementation whose
//! member may forward to it. Retain the satisfied contract and normalized ABI
//! projection together; none of this inventory supplies execution authority.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectForeignLeaf {
    pub declaration_identity: String,
    pub callable_identity: String,
    pub satisfied_schema: effects::provider_plan::ServiceSchema,
    pub plan: effects::provider_plan::ProviderPlan,
    pub satisfied_requirement_identity: String,
    /// Native projection may be unavailable for a checked but unexecuted leaf.
    /// Preserve its exact refusal until executable demand activates this row.
    pub external_binding:
        Result<calling_conventions::ExternalBindingRow, Vec<diagnostics::Diagnostic>>,
}

use omega_calling_conventions::HostOperationKey;
use omega_control_flow::StateKey;
use psi_arena::{Arena, Handle};
use psi_symbols::SymbolHandle;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AbstractSourceBoundaryEdge {
    pub source_key: StateKey,
    pub statement_index: usize,
    pub call_ordinal: usize,
    pub receiver_symbol: SymbolHandle,
    pub target_symbol: SymbolHandle,
    pub boundary_trait_symbol: SymbolHandle,
    pub boundary_signature_symbol: SymbolHandle,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AbstractBoundaryEdge {
    pub source_key: StateKey,
    pub statement_index: usize,
    pub call_ordinal: usize,
    pub operation_ordinal: usize,
    pub operation_key: HostOperationKey,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AbstractBoundaryLink {
    pub source_edge: Handle<AbstractSourceBoundaryEdge>,
    pub lowered_edge: Handle<AbstractBoundaryEdge>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AbstractBoundaryPolicyVerdict {
    #[default]
    Unknown,
    Accepted,
    MissingSourceBoundary,
    MissingHostBinding,
    DisallowedBoundaryPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbstractBoundaryPolicyCheck {
    pub source_edge: Handle<AbstractSourceBoundaryEdge>,
    pub lowered_edge: Handle<AbstractBoundaryEdge>,
    pub operation_key: HostOperationKey,
    pub boundary_policy: Arc<str>,
    pub verdict: AbstractBoundaryPolicyVerdict,
}

impl Default for AbstractBoundaryPolicyCheck {
    fn default() -> Self {
        Self {
            source_edge: Handle::invalid(),
            lowered_edge: Handle::invalid(),
            operation_key: HostOperationKey::default(),
            boundary_policy: Arc::from(""),
            verdict: AbstractBoundaryPolicyVerdict::Unknown,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AbstractBoundarySummary {
    pub source_edges: Arena<AbstractSourceBoundaryEdge>,
    pub edges: Arena<AbstractBoundaryEdge>,
    pub links: Arena<AbstractBoundaryLink>,
    pub policy_checks: Arena<AbstractBoundaryPolicyCheck>,
    /// Retained, contract-bound evidence for the generated boundary-code
    /// footprint. This lives in the semantic boundary root so every later
    /// representation carries the same canonical evidence through emission.
    pub footprints: crate::BoundaryFootprintPlan,
}

impl AbstractBoundarySummary {
    pub fn with_capacity(edge_capacity: usize) -> Self {
        Self::with_source_and_host_capacity(0, edge_capacity)
    }

    pub fn with_source_and_host_capacity(
        source_edge_capacity: usize,
        edge_capacity: usize,
    ) -> Self {
        Self {
            source_edges: Arena::with_capacity(source_edge_capacity),
            edges: Arena::with_capacity(edge_capacity),
            links: Arena::new(),
            policy_checks: Arena::new(),
            footprints: crate::BoundaryFootprintPlan::default(),
        }
    }
}

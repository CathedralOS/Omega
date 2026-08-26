use omega_calling_conventions::{HostOperationKey, NativeParameterId};
use omega_control_flow::StateKey;
use psi_arena::{Arena, Handle};
use psi_symbols::SymbolHandle;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbstractHostCallSourceSite {
    Statement(psi_typed_trees::statement::StatementHandle),
    Expression(psi_typed_trees::expression::ExpressionHandle),
}

impl Default for AbstractHostCallSourceSite {
    fn default() -> Self {
        Self::Expression(psi_typed_trees::expression::ExpressionHandle::invalid())
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AbstractHostCallNativeArgument {
    pub formal_ordinal: u32,
    pub native_parameter: Option<NativeParameterId>,
}

/// Identity-only retention of one exact outbound host-call occurrence.
///
/// This row deliberately contains no physical place, address, byte offset, or
/// relocation authority. Those belong to later target-closed stages.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbstractHostCallOccurrence {
    pub source_call_index: u32,
    pub source_call_generation: u32,
    pub source_site: AbstractHostCallSourceSite,
    pub registration_operation: SymbolHandle,
    pub requirement_identity: Arc<str>,
    pub source_key: StateKey,
    pub statement_index: usize,
    pub call_ordinal: usize,
    pub lowering_index: u32,
    pub lowering_generation: u32,
    pub arguments: psi_arena::HandleSpan<AbstractHostCallNativeArgument>,
}

impl Default for AbstractHostCallOccurrence {
    fn default() -> Self {
        Self {
            source_call_index: 0,
            source_call_generation: 0,
            source_site: AbstractHostCallSourceSite::default(),
            registration_operation: SymbolHandle::invalid(),
            requirement_identity: Arc::from(""),
            source_key: StateKey::default(),
            statement_index: 0,
            call_ordinal: 0,
            lowering_index: 0,
            lowering_generation: 0,
            arguments: psi_arena::HandleSpan::empty(),
        }
    }
}

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
    pub host_call: Handle<AbstractHostCallOccurrence>,
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
    pub host_calls: Arena<AbstractHostCallOccurrence>,
    pub host_call_arguments: Arena<AbstractHostCallNativeArgument>,
    pub edges: Arena<AbstractBoundaryEdge>,
    pub links: Arena<AbstractBoundaryLink>,
    pub policy_checks: Arena<AbstractBoundaryPolicyCheck>,
    /// Retained, contract-bound evidence for the generated boundary-code
    /// footprint. This lives in the semantic boundary root so every later
    /// representation carries the same canonical evidence through emission.
    pub footprints: crate::BoundaryFootprintPlan,
    /// Exact callback-root evidence in canonical placement order. These rows
    /// remain separate because callback contracts may differ from the process
    /// entry and from one another.
    pub callback_footprints: Vec<crate::CallbackBoundaryFootprintPlan>,
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
            host_calls: Arena::with_capacity(edge_capacity),
            host_call_arguments: Arena::new(),
            edges: Arena::with_capacity(edge_capacity),
            links: Arena::new(),
            policy_checks: Arena::new(),
            footprints: crate::BoundaryFootprintPlan::default(),
            callback_footprints: Vec::new(),
        }
    }
}

//! Builds exact boundary-provider approvals and audits boundary calls.
//!
//! Every declared boundary trait that the package does not implement itself is
//! an approved external provider edge. A whole-trait in-package implementation
//! is ordinary code attempting to mint that boundary capability and rejects.
//! Approval is exact to the boundary trait symbol; service rows are unrelated.

use symbols::SymbolHandle;
use typed_trees::TypedTrees;

use crate::capabilities::provider_approval::{
    BoundaryCallApproval, BoundaryProviderApproval, BoundaryProviderApprovalRegistry,
};

/// Exact checked boundary-edge identity supplied by orchestration. Provider
/// approval consumes the retained trait directly; target, signature, and call
/// coordinates remain independent custody evidence. Service reach and the
/// independent suspension/blocking axes never participate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoundaryCallCoordinate {
    pub machine_symbol: SymbolHandle,
    pub state_symbol: SymbolHandle,
    pub target_state_symbol: SymbolHandle,
    pub boundary_trait_symbol: SymbolHandle,
    pub boundary_signature_symbol: SymbolHandle,
    pub statement_index: usize,
    pub call_ordinal: usize,
}

/// A boundary call whose exact capability has no approved provider edge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnapprovedBoundaryCall {
    pub machine_symbol: SymbolHandle,
    pub state_symbol: SymbolHandle,
    pub boundary_trait_symbol: SymbolHandle,
    pub statement_index: usize,
    pub call_ordinal: usize,
}

/// Builds exact boundary provider approvals for `program`.
///
/// A boundary trait is approved unless the application provides an in-package
/// whole-trait implementation. Requirement-level checked adapters and external
/// leaves are supply edges, not authority minting.
pub fn build_boundary_provider_approval_registry(
    program: &TypedTrees,
) -> BoundaryProviderApprovalRegistry {
    let mut registry = BoundaryProviderApprovalRegistry::new();

    for trait_definition in program.traits() {
        if !trait_definition.is_boundary {
            continue;
        }

        let exact_symbol_owners = program
            .traits()
            .iter()
            .filter(|candidate| candidate.symbol == trait_definition.symbol)
            .count();
        let exact_name_owners = program
            .traits()
            .iter()
            .filter(|candidate| {
                candidate.is_boundary && candidate.name.as_str() == trait_definition.name.as_str()
            })
            .count();
        let has_exact_identity = trait_definition.symbol.is_valid()
            && !trait_definition.name.as_str().is_empty()
            && exact_symbol_owners == 1
            && exact_name_owners == 1;
        let implemented_in_package =
            boundary_trait_is_implemented(program, trait_definition.name.as_str());
        registry.register(BoundaryProviderApproval::new(
            trait_definition.symbol,
            has_exact_identity && !implemented_in_package,
        ));
    }

    registry
}

/// Audits every resolved boundary call against exact provider approval.
pub fn audit_boundary_provider_calls(
    _program: &TypedTrees,
    calls: impl IntoIterator<Item = BoundaryCallCoordinate>,
    registry: &BoundaryProviderApprovalRegistry,
) -> Vec<UnapprovedBoundaryCall> {
    let mut unapproved = Vec::new();

    for call in calls {
        match registry.authorize_boundary_call(call.boundary_trait_symbol) {
            BoundaryCallApproval::Unapproved => {
                unapproved.push(UnapprovedBoundaryCall {
                    machine_symbol: call.machine_symbol,
                    state_symbol: call.state_symbol,
                    boundary_trait_symbol: call.boundary_trait_symbol,
                    statement_index: call.statement_index,
                    call_ordinal: call.call_ordinal,
                });
            }
            BoundaryCallApproval::Approved => {}
        }
    }
    unapproved
}

fn boundary_trait_is_implemented(program: &TypedTrees, trait_name: &str) -> bool {
    // PRV4 supply edges: a machine satisfying ONE exact requirement (a
    // checked adapter forwarding already-held authority, or a `via` external
    // leaf) is not an in-package implementation of the trait. Whole-trait
    // implementation is expressed only by a carrier-owned conformance item;
    // that ordinary implementation revokes external provider approval.
    program.conformances().iter().any(|conformance| {
        matches!(
            &conformance.subject,
            typed_trees::trait_definition::ConformanceSubject::Carrier(_)
        ) && conformance.trait_name.as_str() == trait_name
    })
}

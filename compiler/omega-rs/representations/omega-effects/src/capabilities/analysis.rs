//! Builds exact boundary-provider approvals and audits boundary calls.
//!
//! Every declared boundary trait that the package does not implement itself is
//! an approved external provider edge. A whole-trait in-package implementation
//! is ordinary code attempting to mint that boundary capability and rejects.
//! Approval is exact to the boundary trait symbol; service rows are unrelated.

use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;

use crate::capabilities::provider_approval::{
    BoundaryCallApproval, BoundaryProviderApproval, BoundaryProviderApprovalRegistry,
};
use psi_effects::OperationalPlan;

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

        let implemented_in_package =
            boundary_trait_is_implemented(program, trait_definition.symbol);
        registry.register(BoundaryProviderApproval::new(
            trait_definition.symbol,
            !implemented_in_package,
        ));
    }

    registry
}

/// Audits every resolved boundary call against exact provider approval.
pub fn audit_boundary_provider_calls(
    program: &TypedTrees,
    operational: &OperationalPlan,
    registry: &BoundaryProviderApprovalRegistry,
) -> Vec<UnapprovedBoundaryCall> {
    let mut unapproved = Vec::new();

    for machine in operational.machines() {
        for state in operational.states.span_or_empty(machine.states) {
            for call in operational.calls.span_or_empty(state.calls) {
                let Some(boundary_trait_symbol) =
                    boundary_trait_symbol(program, call.target_state_symbol)
                else {
                    continue;
                };

                match registry.authorize_boundary_call(boundary_trait_symbol) {
                    BoundaryCallApproval::Unapproved => {
                        unapproved.push(UnapprovedBoundaryCall {
                            machine_symbol: machine.symbol,
                            state_symbol: state.symbol,
                            boundary_trait_symbol,
                            statement_index: call.statement_index,
                            call_ordinal: call.call_ordinal,
                        });
                    }
                    BoundaryCallApproval::Approved => {}
                }
            }
        }
    }

    unapproved
}

fn boundary_trait_is_implemented(program: &TypedTrees, trait_symbol: SymbolHandle) -> bool {
    let Some(trait_definition) = program
        .traits()
        .iter()
        .find(|definition| definition.symbol == trait_symbol)
    else {
        return false;
    };

    // PRV4 supply edges: a machine satisfying ONE exact requirement (a
    // checked adapter forwarding already-held authority, or a `via` external
    // leaf) is not an in-package implementation of the trait. Whole-trait
    // implementation is expressed only by a carrier-owned conformance item;
    // that ordinary implementation revokes external provider approval.
    program.conformances().iter().any(|conformance| {
        matches!(
            &conformance.subject,
            psi_typed_trees::trait_definition::ConformanceSubject::Carrier(_)
        ) && conformance.trait_name == trait_definition.name
    })
}

/// Resolves a call target to the boundary trait signature it reaches. The
/// target may be a boundary trait signature directly, or an in-package
/// implementation state whose machine conforms to a boundary trait.
fn boundary_trait_symbol(
    program: &TypedTrees,
    target_symbol: SymbolHandle,
) -> Option<SymbolHandle> {
    if !target_symbol.is_valid() {
        return None;
    }

    if let Some(found) = program.traits().iter().find_map(|trait_definition| {
        if !trait_definition.is_boundary {
            return None;
        }
        program
            .trait_machine_signatures(trait_definition)
            .iter()
            .find(|signature| signature.symbol == target_symbol)
            .map(|_| trait_definition.symbol)
    }) {
        return Some(found);
    }

    // The target is an implementation state; map it back to the boundary trait
    // signature its machine conforms to, matched by state name.
    let (machine, state) = program.machines().iter().find_map(|machine| {
        program
            .machine_states(machine)
            .iter()
            .find(|state| state.symbol == target_symbol)
            .map(|state| (machine, state))
    })?;

    for conformance in program.machine_trait_conformances(machine) {
        let Some(trait_definition) = program
            .traits()
            .iter()
            .find(|trait_definition| trait_definition.symbol == conformance.symbol)
        else {
            continue;
        };
        if !trait_definition.is_boundary {
            continue;
        }
        if program
            .trait_machine_signatures(trait_definition)
            .iter()
            .any(|signature| signature.name == state.name)
        {
            return Some(trait_definition.symbol);
        }
    }

    None
}

//! Checked capability-flow fact population.
//!
//! Capability-flow facts describe how authority-bearing values move through a
//! package (chapter 18, "Capabilities And Authority Flow"). Each boundary call
//! is classified into one or more verbs:
//!
//! - `uses`     — the call requires a capability (a boundary-trait edge).
//! - `acquires` — the boundary mints fresh host authority (a host-authority
//!   boundary call that returns a capability value, e.g. `choose_folder`).
//! - `returns`  — a capability value leaves through the enclosing state's
//!   capability-typed return.
//! - `stores`   — a capability is retained in a machine-owned field
//!   (`self.field = <boundary call>`).
//! - `derives`  — a narrower capability is produced from an existing capability
//!   passed into the boundary call (a sub-capability operation).
//!
//! The verbs are inferred from boundary provenance, signature effects, return
//! types, and capability-typed arguments — no new source keywords are required.

use omega_checked_trees::FlowFacts;
use omega_core::arena::Arena;
use omega_core::symbols::SymbolHandle;
use omega_effects::{
    CapabilityFlowFact, CapabilityFlowKind, CapabilityFlowPlan, EffectPlan, EffectSet,
    requires_host_authority,
};
use omega_typed_trees::TypedTrees;
use omega_typed_trees::expression::ExpressionNode;
use omega_typed_trees::signature::StateSignature;
use omega_typed_trees::statement::StatementNode;

pub(crate) fn build_capability_facts(
    program: &TypedTrees,
    effects: &EffectPlan,
    flow: &FlowFacts,
) -> CapabilityFlowPlan {
    let mut flows = Arena::with_capacity(flow.boundaries.edges.len());

    for (_, state) in flow.control.states.iter() {
        let enclosing = enclosing_state(program, state.machine_symbol, state.state_symbol);
        for call in flow.control.calls.span_or_empty(state.calls) {
            // `uses`: every boundary-trait edge reached by this call.
            let mut capability_symbol = SymbolHandle::invalid();
            for edge in flow.boundaries.edges.span_or_empty(call.boundary_edges) {
                push_unique(
                    &mut flows,
                    CapabilityFlowKind::Uses,
                    edge.boundary_trait_symbol,
                    state.machine_symbol,
                    state.state_symbol,
                    edge.statement_index,
                    edge.call_ordinal,
                );
                capability_symbol = edge.boundary_trait_symbol;
            }

            if !capability_symbol.is_valid() {
                if let Some(symbol) = boundary_trait_for_signature(program, call.target_symbol) {
                    capability_symbol = symbol;
                    push_unique(
                        &mut flows,
                        CapabilityFlowKind::Uses,
                        capability_symbol,
                        state.machine_symbol,
                        state.state_symbol,
                        call.statement_index,
                        call.call_ordinal,
                    );
                }
            }

            if !capability_symbol.is_valid() {
                continue;
            }

            let signature = boundary_signature(program, call.target_symbol);
            let call_effects = call_effects(call.direct_effects, signature, program);
            let returns_capability = signature
                .map(|signature| is_capability_type(program, signature.return_type))
                .unwrap_or(false);

            // `acquires`: a host-authority boundary that yields a fresh
            // capability value mints authority for the package.
            if returns_capability && requires_host_authority(call_effects) {
                push_unique(
                    &mut flows,
                    CapabilityFlowKind::Acquires,
                    capability_symbol,
                    state.machine_symbol,
                    state.state_symbol,
                    call.statement_index,
                    call.call_ordinal,
                );
            }

            // `derives`: a narrower capability produced from an existing
            // capability passed into the boundary call.
            if returns_capability
                && signature
                    .map(|signature| passes_capability_argument(program, signature))
                    .unwrap_or(false)
            {
                push_unique(
                    &mut flows,
                    CapabilityFlowKind::Derives,
                    capability_symbol,
                    state.machine_symbol,
                    state.state_symbol,
                    call.statement_index,
                    call.call_ordinal,
                );
            }

            // `stores`: the boundary call result is retained in a machine-owned
            // field (`self.field = <call>`).
            if returns_capability
                && enclosing
                    .map(|state| {
                        call_stored_into_field(program, state, call.statement_index)
                    })
                    .unwrap_or(false)
            {
                push_unique(
                    &mut flows,
                    CapabilityFlowKind::Stores,
                    capability_symbol,
                    state.machine_symbol,
                    state.state_symbol,
                    call.statement_index,
                    call.call_ordinal,
                );
            }

            // `returns`: a capability leaves through the enclosing state's
            // capability-typed return.
            if returns_capability
                && enclosing
                    .map(|state| is_capability_type(program, state.return_type))
                    .unwrap_or(false)
            {
                push_unique(
                    &mut flows,
                    CapabilityFlowKind::Returns,
                    capability_symbol,
                    state.machine_symbol,
                    state.state_symbol,
                    call.statement_index,
                    call.call_ordinal,
                );
            }
        }
    }

    // Mirror boundary-trait `uses` discovered through the effect plan so the
    // counts stay stable even where flow control facts are sparse.
    for machine in effects.machines() {
        for state in effects.states.span_or_empty(machine.states) {
            for call in effects.calls.span_or_empty(state.calls) {
                let Some(capability_symbol) =
                    boundary_trait_for_signature(program, call.target_state_symbol)
                else {
                    continue;
                };

                push_unique(
                    &mut flows,
                    CapabilityFlowKind::Uses,
                    capability_symbol,
                    machine.symbol,
                    state.symbol,
                    call.statement_index,
                    call.call_ordinal,
                );
            }
        }
    }

    CapabilityFlowPlan::with_roots(flows)
}

#[allow(clippy::too_many_arguments)]
fn push_unique(
    flows: &mut Arena<CapabilityFlowFact>,
    kind: CapabilityFlowKind,
    capability_symbol: SymbolHandle,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    call_ordinal: usize,
) {
    let fact = CapabilityFlowFact {
        kind,
        capability_symbol,
        machine_symbol,
        state_symbol,
        statement_index,
        call_ordinal,
    };

    let exists = flows.iter().any(|(_, existing)| *existing == fact);
    if !exists {
        flows.append(fact);
    }
}

fn call_effects(
    direct: EffectSet,
    signature: Option<&StateSignature>,
    program: &TypedTrees,
) -> EffectSet {
    let mut effects = direct;
    if let Some(signature) = signature {
        for effect in program.state_signature_effects(signature) {
            effects.insert_name(effect.as_str());
        }
    }
    effects
}

fn enclosing_state<'program>(
    program: &'program TypedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
) -> Option<&'program omega_typed_trees::state::State> {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)?;
    program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == state_symbol)
}

/// Whether the assignment at `statement_index` stores its value into a
/// machine-owned field (`self.field = ...`).
fn call_stored_into_field(
    program: &TypedTrees,
    state: &omega_typed_trees::state::State,
    statement_index: usize,
) -> bool {
    let Some(statement) = program
        .statement_table
        .statements(state.statement_nodes)
        .get(statement_index)
    else {
        return false;
    };

    let StatementNode::Assignment(assignment) = statement else {
        return false;
    };

    assignment_target_is_field(program, assignment.target)
}

fn assignment_target_is_field(
    program: &TypedTrees,
    target: omega_typed_trees::expression::ExpressionHandle,
) -> bool {
    if !target.is_valid() {
        return false;
    }

    match program.expression_table.expression(target) {
        ExpressionNode::Member(_) => true,
        ExpressionNode::Mutable(inner) => assignment_target_is_field(program, *inner),
        ExpressionNode::Name(name) => {
            // A multi-segment name path rooted at `self` (e.g. `self.field`).
            program.expression_table.name_path_members(name.members).len() > 1
        }
        _ => false,
    }
}

/// Whether a boundary signature takes another capability as an argument, making
/// its capability-typed result a derived sub-capability.
fn passes_capability_argument(program: &TypedTrees, signature: &StateSignature) -> bool {
    program
        .state_signature_parameters(signature)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .any(|parameter| is_capability_type(program, parameter.type_reference))
}

/// Whether a type reference resolves to a capability type: a boundary trait, or
/// a non-primitive named/data type used through boundary surfaces (e.g.
/// `Folder`). Primitive types are never capabilities.
fn is_capability_type(
    program: &TypedTrees,
    type_reference: omega_typed_trees::types::TypeReferenceHandle,
) -> bool {
    if !type_reference.is_valid() {
        return false;
    }
    if program.primitive_type_reference(type_reference).is_some() {
        return false;
    }
    program.type_reference_symbol(type_reference).is_valid()
}

fn boundary_signature<'program>(
    program: &'program TypedTrees,
    signature_symbol: SymbolHandle,
) -> Option<&'program StateSignature> {
    if !signature_symbol.is_valid() {
        return None;
    }

    program.traits().iter().find_map(|trait_definition| {
        if !trait_definition.is_boundary {
            return None;
        }
        program
            .trait_machine_signatures(trait_definition)
            .iter()
            .find(|signature| signature.symbol == signature_symbol)
    })
}

fn boundary_trait_for_signature(
    program: &TypedTrees,
    signature_symbol: SymbolHandle,
) -> Option<SymbolHandle> {
    if !signature_symbol.is_valid() {
        return None;
    }

    program.traits().iter().find_map(|trait_definition| {
        (trait_definition.is_boundary
            && program
                .trait_machine_signatures(trait_definition)
                .iter()
                .any(|signature| signature.symbol == signature_symbol))
        .then_some(trait_definition.symbol)
    })
}

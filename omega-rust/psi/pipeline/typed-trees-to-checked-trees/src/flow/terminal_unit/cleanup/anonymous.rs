//! Anonymous result roots use the existing nested schedule and partial plan.

use super::*;

pub(super) fn binding(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
) -> Option<(CheckedUnitStructuralResultBindingPlan, facts::PlaceRoot)> {
    // The temporary dies at this call's continuation. With no later statements,
    // the existing Unit return edge owns that cleanup; do not extend its life.
    let [StatementNode::Call(_)] = program.statement_table.statements(state.statement_nodes) else {
        return None;
    };
    let flow = state_flow(facts, machine.symbol, state.symbol)?;
    let calls = facts.flow.control.calls.span(flow.calls)?;
    if calls.len() != 2 || calls.iter().any(|call| call.statement_index != 0) {
        return None;
    }
    let outer = calls.iter().find(|call| call.call_ordinal == 0)?;
    let nested = control::structural_operands::for_call(program, facts, machine, state, outer)?;
    let [producer] = nested.as_slice() else {
        return None;
    };
    if producer.call_ordinal != 1 {
        return None;
    }
    let source = crate::find_call_site(program, machine.symbol, state.symbol, 0, 0)?;
    let arguments = crate::call_site_argument_expressions(program, &source);
    let [argument] = arguments else {
        return None;
    };
    let place =
        crate::flow::canonical_place_from_expression_in_state(program, state.symbol, 0, *argument)?;
    if place.root != facts::PlaceRoot::Expression(producer.authored_expression)
        || place.segments.is_empty()
    {
        return None;
    }
    let root = crate::flow::CanonicalPlace {
        root: place.root,
        segments: Vec::new(),
    };
    let reference = crate::flow::canonical_place_type_reference(program, state.symbol, 0, &root)?;
    let type_identity =
        shapes.add_partial_affine_type(reference, &machine_binders(program, machine))?;
    let result = control::structural_operands::result(
        program,
        facts,
        machine.symbol,
        producer.authored_expression,
        shapes,
    )?;
    if result.type_identity != type_identity {
        return None;
    }
    Some((
        CheckedUnitStructuralResultBindingPlan {
            statement_index: 0,
            binding_ordinal: 0,
            type_identity,
            multiplicity: Multiplicity::Affine,
        },
        place.root,
    ))
}

pub(super) fn validate_permissions(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    root: facts::PlaceRoot,
    residuals: &[CheckedUnitPartialAffineDiscardPlan],
) -> Option<()> {
    let flow = state_flow(facts, machine.symbol, state.symbol)?;
    let calls = facts.flow.control.calls.span(flow.calls)?;
    let producer = calls.iter().find(|call| call.call_ordinal == 1)?;
    let consumer = calls.iter().find(|call| call.call_ordinal == 0)?;
    let source = |call: &checked_trees::FlowCallFact| PermissionEventSource::Call {
        statement_index: call.statement_index,
        call_ordinal: call.call_ordinal,
        target_symbol: call.target_symbol,
    };
    let provenance = language_semantics::PermissionProvenance::Established {
        machine_symbol: machine.symbol,
        state_symbol: state.symbol,
        source: source(producer),
    };
    let mut established = false;
    let mut transferred = false;
    let mut dropped = Vec::new();
    for (_, event) in facts
        .flow
        .ownership
        .permissions
        .iter()
        .filter(|(_, event)| {
            event.machine_symbol == machine.symbol
                && event.state_symbol == state.symbol
                && event.root == root
        })
    {
        if event.access != PermissionAccess::Owned
            || event.multiplicity != Multiplicity::Affine
            || event.claim_identity != PermissionClaimIdentity::Unknown
            || event.provenance != provenance
            || event.obligation_live
        {
            return None;
        }
        let segments = facts.flow.ownership.segments.span_or_empty(event.segments);
        match event.kind {
            PermissionEventKind::Establish
                if !established && event.source == source(producer) && segments.is_empty() =>
            {
                established = true;
            }
            PermissionEventKind::Transfer
                if established
                    && !transferred
                    && event.source == source(consumer)
                    && !segments.is_empty() =>
            {
                transferred = true;
            }
            PermissionEventKind::AffineDrop
                if transferred && event.source == source(consumer) && !segments.is_empty() =>
            {
                let place = crate::flow::CanonicalPlace {
                    root,
                    segments: segments.to_vec(),
                };
                let (reference, path) = projected_argument_path(program, state.symbol, 0, &place)?;
                dropped.push((path, base_type_identity(program, reference, &[])?));
            }
            _ => return None,
        }
    }
    (established
        && transferred
        && dropped.len() == residuals.len()
        && dropped
            .iter()
            .zip(residuals)
            .all(|((path, identity), residual)| {
                *path == residual.path
                    && *identity == residual.type_identity
                    && residual.source
                        == CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                            binding_ordinal: 0,
                        }
            }))
    .then_some(())
}

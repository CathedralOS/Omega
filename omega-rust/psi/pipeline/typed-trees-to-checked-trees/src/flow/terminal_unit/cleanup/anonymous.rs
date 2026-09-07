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
    binding_at(program, facts, shapes, machine, state, 0, 0)
}

pub(in crate::flow::terminal_unit) fn binding_at(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    binding_ordinal: u32,
) -> Option<(CheckedUnitStructuralResultBindingPlan, facts::PlaceRoot)> {
    if !matches!(
        program
            .statement_table
            .statements(state.statement_nodes)
            .get(statement_index),
        Some(StatementNode::Call(_))
    ) {
        return None;
    }
    let flow = state_flow(facts, machine.symbol, state.symbol)?;
    let calls = facts.flow.control.calls.span(flow.calls)?;
    let calls = calls
        .iter()
        .filter(|call| call.statement_index == statement_index)
        .collect::<Vec<_>>();
    if calls.len() != 2 {
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
    let source = crate::find_call_site(program, machine.symbol, state.symbol, statement_index, 0)?;
    let arguments = crate::call_site_argument_expressions(program, &source);
    let [argument] = arguments else {
        return None;
    };
    let place = crate::flow::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        statement_index,
        *argument,
    )?;
    if place.root != facts::PlaceRoot::Expression(producer.authored_expression)
        || place.segments.is_empty()
    {
        return None;
    }
    let root = crate::flow::CanonicalPlace {
        root: place.root,
        segments: Vec::new(),
    };
    let reference =
        crate::flow::canonical_place_type_reference(program, state.symbol, statement_index, &root)?;
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
            statement_index: u32::try_from(statement_index).ok()?,
            binding_ordinal,
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
    validate_permissions_at(program, facts, machine, state, root, 0, 0, residuals)
}

#[allow(clippy::too_many_arguments)]
pub(in crate::flow::terminal_unit) fn validate_permissions_at(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    root: facts::PlaceRoot,
    statement_index: usize,
    binding_ordinal: u32,
    residuals: &[CheckedUnitPartialAffineDiscardPlan],
) -> Option<()> {
    let flow = state_flow(facts, machine.symbol, state.symbol)?;
    let calls = facts.flow.control.calls.span(flow.calls)?;
    let calls = calls
        .iter()
        .filter(|call| call.statement_index == statement_index)
        .collect::<Vec<_>>();
    if calls.len() != 2 {
        return None;
    }
    let producer = calls.iter().find(|call| call.call_ordinal == 1)?;
    let consumer = calls.iter().find(|call| call.call_ordinal == 0)?;
    let consumer_site =
        crate::find_call_site(program, machine.symbol, state.symbol, statement_index, 0)?;
    let [argument] = crate::call_site_argument_expressions(program, &consumer_site) else {
        return None;
    };
    let selected = crate::flow::canonical_place_from_expression_in_state(
        program,
        state.symbol,
        statement_index,
        *argument,
    )?;
    if selected.root != root || selected.segments.is_empty() {
        return None;
    }
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
        let segments = facts.flow.ownership.segments.span(event.segments)?;
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
                    && segments == selected.segments.as_slice() =>
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
                let (reference, path) =
                    projected_argument_path(program, state.symbol, statement_index, &place)?;
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
                            binding_ordinal,
                        }
            }))
    .then_some(())
}

/// Dispose only this expression owner's maximal complement at its actual call.
#[allow(clippy::too_many_arguments)]
pub(in crate::flow::terminal_unit) fn append_continuation(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    result: &CheckedUnitStructuralResultBindingPlan,
    root: facts::PlaceRoot,
    operations: &mut Vec<CheckedUnitEffectOperationPlan>,
) -> Option<()> {
    let CheckedUnitEffectOperationPlan::CallUnit {
        coordinate,
        target_machine,
        target_state,
        scalar_arguments,
        structural_arguments,
        claim_transfers,
        ..
    } = operations.last()?
    else {
        return None;
    };
    let [argument] = structural_arguments.as_slice() else {
        return None;
    };
    if coordinate.statement_index != result.statement_index
        || coordinate.call_ordinal != 0
        || argument.source_structural_result_binding_ordinal() != Some(result.binding_ordinal)
        || argument.access != CheckedStructuralAccess::Owned
        || argument.path.is_empty()
        || !scalar_arguments.is_empty()
        || !claim_transfers.is_empty()
        || !matches!(root, facts::PlaceRoot::Expression(_))
        || machine_has_content_evidence(facts, machine.symbol, state.symbol)
        || facts
            .qualifications
            .for_machine(machine.symbol)
            .is_some_and(|fact| !fact.body_committed.is_empty())
    {
        return None;
    }
    let target = program
        .machines()
        .iter()
        .find(|candidate| candidate.symbol == *target_machine)?;
    let [callee] = program.machine_states(target) else {
        return None;
    };
    let callee_flow = state_flow(facts, target.symbol, callee.symbol)?;
    if callee.symbol != *target_state
        || target.supply_mode != MachineSupplyMode::CheckedBody
        || !program
            .statement_table
            .statements(callee.statement_nodes)
            .is_empty()
        || !service_reach_is_empty(facts, callee_flow.service_reach)
        || !service_reach_plan_is_empty(
            facts,
            facts.service_reaches.plan_for_machine(target.symbol)?,
        )
    {
        return None;
    }
    if argument
        .path
        .iter()
        .any(|segment| matches!(segment, CheckedUnitStructuralPathSegment::FixedIndex(_)))
        && (!program.machine_contracts(machine).is_empty()
            || !program.state_contracts(state).is_empty()
            || !program.machine_contracts(target).is_empty()
            || !program.state_contracts(callee).is_empty())
    {
        return None;
    }
    let coordinate = *coordinate;
    let residuals = partial_affine_residuals(
        &shapes.types,
        &argument.source,
        &result.type_identity,
        &[(argument.path.clone(), argument.type_identity.clone())],
    )?;
    validate_permissions_at(
        program,
        facts,
        machine,
        state,
        root,
        usize::try_from(result.statement_index).ok()?,
        result.binding_ordinal,
        &residuals,
    )?;
    let mut producers = operations
        .iter_mut()
        .filter_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::StructuralCall {
                coordinate,
                result: produced,
                discard_result_on_return,
                ..
            }
            | CheckedUnitEffectOperationPlan::BoundaryStructuralCall {
                coordinate,
                result: produced,
                discard_result_on_return,
                ..
            } if produced.binding_ordinal == result.binding_ordinal => {
                Some((coordinate, produced, discard_result_on_return))
            }
            _ => None,
        });
    let (producer_coordinate, produced, discard_on_return) = producers.next()?;
    if producers.next().is_some()
        || producer_coordinate.statement_index != coordinate.statement_index
        || producer_coordinate.call_ordinal != 1
        || produced != result
        || !*discard_on_return
    {
        return None;
    }
    *discard_on_return = false;
    // Keep even an empty complement: it records the exact dying continuation.
    operations.push(CheckedUnitEffectOperationPlan::CallContinuationCleanup {
        coordinate,
        affine_discards: residuals,
    });
    Some(())
}

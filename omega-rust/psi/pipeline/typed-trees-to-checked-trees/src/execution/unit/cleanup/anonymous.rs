//! Anonymous result roots use the existing nested schedule and partial plan.
use super::super::CheckedUnitStructuralResultBindingPlan;
use super::{
    CheckFacts, CheckedStructuralAccess, CheckedUnitEffectOperationPlan,
    CheckedUnitPartialAffineDiscardPlan, CheckedUnitStructuralArgumentSourcePlan,
    CheckedUnitStructuralPathSegment, MachineSupplyMode, Multiplicity, PermissionAccess,
    PermissionClaimIdentity, PermissionEventKind, PermissionEventSource, StatementNode, TypedTrees,
    control,
};
use crate::execution::terminal_unit::calls::projected_argument_path;
use crate::execution::terminal_unit::cleanup::cleanup_evidence::{
    machine_has_content_evidence, service_reach_is_empty, service_reach_plan_is_empty,
};
use crate::execution::terminal_unit::cleanup::partial_affine_cleanup::partial_affine_residuals;
use crate::execution::terminal_unit::{
    ShapeCollector, base_type_identity, machine_binders, state_flow,
};

pub(in crate::execution::terminal_unit) fn binding(
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
    let calls = calls
        .iter()
        .filter(|call| call.statement_index == 0)
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
    // This lane retains exactly one consumer operand. Wider argument lists and
    // multiple producers belong to the shared statement sequencer, where each
    // temporary's exact dying continuation is its own cleanup row.
    let source =
        crate::semantic_calls::find_call_site(program, machine.symbol, state.symbol, 0, 0)?;
    if crate::semantic_calls::call_site_argument_expressions(program, &source).len() != 1 {
        return None;
    }
    binding_at(
        program,
        facts,
        shapes,
        machine,
        state,
        0,
        0,
        facts::PlaceRoot::Expression(producer.authored_expression),
    )
}

/// Identify the anonymous temporary `root` projected by one operand of the
/// `call_ordinal == 0` consumer at `statement_index`. The producer is the
/// nested call that authored the operand's expression root; any number of
/// sibling producers or unrelated arguments may share the same consumer.
pub(in crate::execution::terminal_unit) fn binding_at(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    binding_ordinal: u32,
    root: facts::PlaceRoot,
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
    let facts::PlaceRoot::Expression(expression) = root else {
        return None;
    };
    let flow = state_flow(facts, machine.symbol, state.symbol)?;
    let calls = facts.flow.control.calls.span(flow.calls)?;
    let calls = calls
        .iter()
        .filter(|call| call.statement_index == statement_index)
        .collect::<Vec<_>>();
    if calls.len() < 2 {
        return None;
    }
    if calls.iter().all(|call| call.call_ordinal != 0) {
        return None;
    }
    let mut producers = calls
        .iter()
        .filter(|call| call.call_ordinal != 0 && call.authored_expression == expression);
    let producer = producers.next()?;
    if producers.next().is_some() {
        return None;
    }
    let _ = producer;
    let source = crate::semantic_calls::find_call_site(
        program,
        machine.symbol,
        state.symbol,
        statement_index,
        0,
    )?;
    // Exactly one operand may project this temporary's root; a second use would
    // leave part of the residual without a checked owner.
    let mut projected = crate::semantic_calls::call_site_argument_expressions(program, &source)
        .iter()
        .filter_map(|argument| {
            crate::flow::canonical_place_from_expression_in_state(
                program,
                state.symbol,
                statement_index,
                *argument,
            )
        })
        .filter(|place| place.root == root && !place.segments.is_empty());
    if projected.next().is_none() || projected.next().is_some() {
        return None;
    }
    let root_place = crate::flow::CanonicalPlace {
        root,
        segments: Vec::new(),
    };
    let reference = crate::flow::canonical_place_type_reference(
        program,
        state.symbol,
        statement_index,
        &root_place,
    )?;
    let type_identity =
        shapes.add_partial_affine_type(reference, &machine_binders(program, machine))?;
    let result =
        control::structural_operands::result(program, facts, machine.symbol, expression, shapes)?;
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
        root,
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
pub(in crate::execution::terminal_unit) fn validate_permissions_at(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    root: facts::PlaceRoot,
    statement_index: usize,
    binding_ordinal: u32,
    residuals: &[CheckedUnitPartialAffineDiscardPlan],
) -> Option<()> {
    let facts::PlaceRoot::Expression(expression) = root else {
        return None;
    };
    let flow = state_flow(facts, machine.symbol, state.symbol)?;
    let calls = facts.flow.control.calls.span(flow.calls)?;
    let calls = calls
        .iter()
        .filter(|call| call.statement_index == statement_index)
        .collect::<Vec<_>>();
    if calls.len() < 2 {
        return None;
    }
    let consumer = calls.iter().find(|call| call.call_ordinal == 0)?;
    // The temporary's producer is the nested call that authored its
    // expression root, not a fixed ordinal: wider argument lists admit
    // producers at any later ordinal in the same statement.
    let mut producers = calls
        .iter()
        .filter(|call| call.call_ordinal != 0 && call.authored_expression == expression);
    let producer = producers.next()?;
    if producers.next().is_some() {
        return None;
    }
    let consumer_site = crate::semantic_calls::find_call_site(
        program,
        machine.symbol,
        state.symbol,
        statement_index,
        0,
    )?;
    let mut selected =
        crate::semantic_calls::call_site_argument_expressions(program, &consumer_site)
            .iter()
            .filter_map(|argument| {
                crate::flow::canonical_place_from_expression_in_state(
                    program,
                    state.symbol,
                    statement_index,
                    *argument,
                )
            })
            .filter(|place| place.root == root && !place.segments.is_empty());
    let selected_place = selected.next()?;
    if selected.next().is_some() {
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
                    && segments == selected_place.segments.as_slice() =>
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

/// Dispose each expression owner's maximal complement at its actual call.
/// One consumer may die for several temporaries at once: every projected
/// owned operand names its own producer's result binding, and the residual
/// rows keep that operand order.
#[allow(clippy::too_many_arguments)]
pub(in crate::execution::terminal_unit) fn append_continuation(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    temporaries: &[(CheckedUnitStructuralResultBindingPlan, facts::PlaceRoot)],
    operations: &mut Vec<CheckedUnitEffectOperationPlan>,
) -> Option<()> {
    let CheckedUnitEffectOperationPlan::CallUnit {
        coordinate,
        target_machine,
        target_state,
        structural_arguments,
        ..
    } = operations.last()?
    else {
        return None;
    };
    if coordinate.call_ordinal != 0
        || machine_has_content_evidence(facts, machine.symbol, state.symbol)
        || facts
            .qualifications
            .for_machine(machine.symbol)
            .is_some_and(|fact| !fact.body_committed.is_empty())
    {
        return None;
    }
    // Every projected owned operand must belong to a dying temporary, and
    // every temporary must own exactly one of them. Whole moves and scalar
    // operands keep their own custody outside this cleanup row.
    let mut affine_discards = Vec::new();
    let mut covered = Vec::new();
    for argument in structural_arguments {
        let Some(binding_ordinal) = argument.source_structural_result_binding_ordinal() else {
            continue;
        };
        if argument.access != CheckedStructuralAccess::Owned {
            // A borrowed anonymous operand owns the separate shared-temporary
            // continuation; mixing it into this row is not admitted here.
            return None;
        }
        if argument.path.is_empty() {
            continue;
        }
        let (result, root) = temporaries
            .iter()
            .find(|(result, _)| result.binding_ordinal == binding_ordinal)?;
        if covered.contains(&binding_ordinal)
            || coordinate.statement_index != result.statement_index
            || !matches!(root, facts::PlaceRoot::Expression(_))
        {
            return None;
        }
        covered.push(binding_ordinal);
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
            *root,
            usize::try_from(result.statement_index).ok()?,
            result.binding_ordinal,
            &residuals,
        )?;
        affine_discards.extend(residuals);
    }
    if covered.len() != temporaries.len() {
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
    if structural_arguments.iter().any(|argument| {
        argument
            .source_structural_result_binding_ordinal()
            .is_some()
            && argument
                .path
                .iter()
                .any(|segment| matches!(segment, CheckedUnitStructuralPathSegment::FixedIndex(_)))
    }) && (!program.machine_contracts(machine).is_empty()
        || !program.state_contracts(state).is_empty()
        || !program.machine_contracts(target).is_empty()
        || !program.state_contracts(callee).is_empty())
    {
        return None;
    }
    let coordinate = *coordinate;
    for (result, _) in temporaries {
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
            || producer_coordinate.call_ordinal == 0
            || *produced != *result
            || !*discard_on_return
        {
            return None;
        }
        *discard_on_return = false;
    }
    // Keep even an empty complement: it records the exact dying continuation.
    operations.push(CheckedUnitEffectOperationPlan::CallContinuationCleanup {
        coordinate,
        affine_discards,
    });
    Some(())
}

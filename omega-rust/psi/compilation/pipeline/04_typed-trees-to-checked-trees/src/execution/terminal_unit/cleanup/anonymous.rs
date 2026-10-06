//! Anonymous argument owners die at the consuming call's normal continuation.
//! The nested schedule establishes each owner once; a projected argument names
//! its subtree without establishing another owner. Borrowed owners retire whole,
//! while partial transfers retire the maximal untouched complement, in reverse
//! establishment order. Callee effects and result shape do not alter that debt.
//! Partial transfers still require the captured permission partition; automatic
//! affine disposal does not discharge a live linear obligation or nominal hook.
use super::super::CheckedUnitStructuralResultBindingPlan;
use super::{
    CheckFacts, CheckedStructuralAccess, CheckedUnitEffectOperationPlan,
    CheckedUnitPartialAffineDiscardPlan, CheckedUnitStructuralArgumentSourcePlan, Multiplicity,
    PermissionAccess, PermissionClaimIdentity, PermissionEventKind, PermissionEventSource,
    TypedTrees,
};
use crate::execution::terminal_unit::calls::projected_argument_path;
use crate::execution::terminal_unit::cleanup::partial_affine_cleanup::partial_affine_residuals;
use crate::execution::terminal_unit::types::{ShapeCollector, base_type_identity};

#[allow(clippy::too_many_arguments)]
fn validate_permissions_at(
    program: &TypedTrees,
    flow: &checked_trees::FlowFacts,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    root: facts::PlaceRoot,
    statement_index: usize,
    call_ordinal: usize,
    source_plan: &CheckedUnitStructuralArgumentSourcePlan,
    residuals: &[CheckedUnitPartialAffineDiscardPlan],
) -> Option<()> {
    let facts::PlaceRoot::Expression(expression) = root else {
        return None;
    };
    let (_, state_flow) = flow.control.states.iter().find(|(_, candidate)| {
        candidate.machine_symbol == machine && candidate.state_symbol == state
    })?;
    let calls = flow.control.calls.span(state_flow.calls)?;
    let calls = calls
        .iter()
        .filter(|call| call.statement_index == statement_index)
        .collect::<Vec<_>>();
    if calls.len() < 2 {
        return None;
    }
    let consumer = calls
        .iter()
        .find(|call| call.call_ordinal == call_ordinal)?;
    // The temporary's producer is the nested call that authored its
    // expression root, not a fixed ordinal: wider argument lists admit
    // producers at any later ordinal in the same statement.
    let mut producers = calls
        .iter()
        .filter(|call| call.call_ordinal > call_ordinal && call.authored_expression == expression);
    let producer = producers.next()?;
    if producers.next().is_some() {
        return None;
    }
    let consumer_site = crate::semantic::calls::find_call_site(
        program,
        machine,
        state,
        statement_index,
        call_ordinal,
    )?;
    let mut selected =
        crate::semantic::calls::call_site_argument_expressions(program, &consumer_site)
            .iter()
            .filter_map(|argument| {
                crate::flow::canonical_place_from_expression_in_state(
                    program,
                    state,
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
        machine_symbol: machine,
        state_symbol: state,
        source: source(producer),
    };
    let mut established = false;
    let mut transferred = false;
    let mut dropped = Vec::new();
    for (_, event) in flow.ownership.permissions.iter().filter(|(_, event)| {
        event.machine_symbol == machine && event.state_symbol == state && event.root == root
    }) {
        if event.access != PermissionAccess::Owned
            || event.multiplicity != Multiplicity::Affine
            || event.claim_identity != PermissionClaimIdentity::Unknown
            || event.provenance != provenance
            || event.obligation_live
        {
            return None;
        }
        let segments = flow.ownership.segments.span(event.segments)?;
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
                    projected_argument_path(program, state, statement_index, &place)?;
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
                    && residual.source == *source_plan
            }))
    .then_some(())
}

/// Publish the original anonymous owner's residual partition for any call
/// evaluation schedule. Scalar and operation callers consume these same rows;
/// a selected subtree never becomes a second whole temporary.
pub(crate) fn projected_temporary_residuals(
    program: &TypedTrees,
    flow: &checked_trees::FlowFacts,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    consumer: &checked_trees::FlowCallFact,
    owner_type: typed_trees::types::TypeReferenceHandle,
    argument: &checked_trees::CheckedUnitStructuralArgumentPlan,
) -> Option<Vec<CheckedUnitPartialAffineDiscardPlan>> {
    let CheckedUnitStructuralArgumentSourcePlan::StructuralExpression { expression } =
        argument.source
    else {
        return None;
    };
    if argument.access != CheckedStructuralAccess::Owned
        || argument.path.is_empty()
        || program.type_multiplicity(owner_type) != Multiplicity::Affine
        || !validation::has_plain_owned_contents_with_represented_constraints(program, owner_type)
    {
        return None;
    }
    let mut shapes = ShapeCollector::new(program);
    let identity = shapes.add_type(owner_type, &[], &[])?;
    let residuals = partial_affine_residuals(
        &shapes.types,
        &argument.source,
        &identity,
        &[(argument.path.clone(), argument.type_identity.clone())],
    )?;
    validate_permissions_at(
        program,
        flow,
        machine,
        state,
        facts::PlaceRoot::Expression(expression),
        consumer.statement_index,
        consumer.call_ordinal,
        &argument.source,
        &residuals,
    )?;
    Some(residuals)
}

/// Dispose anonymous argument owners at the call's normal continuation.
/// The argument's access determines whether the whole owner or its maximal
/// complement dies. The callee's effects and return shape do not change that
/// lifetime; its already admitted call owns those independent obligations.
/// Results follow establishment order, so reverse traversal also orders mixed
/// borrowed owners and partially transferred owners without regrouping them.
#[allow(clippy::too_many_arguments)]
pub(in crate::execution::terminal_unit) fn append_continuation(
    program: &TypedTrees,
    facts: &CheckFacts,
    shapes: &ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    coordinate: checked_trees::CheckedUnitCallCoordinate,
    results: &[(CheckedUnitStructuralResultBindingPlan, facts::PlaceRoot)],
    operations: &mut Vec<CheckedUnitEffectOperationPlan>,
) -> Option<()> {
    let call = operations.iter().rev().find_map(|operation| {
        operation
            .call()
            .filter(|call| call.coordinate == coordinate)
    })?;
    if call.is_boundary() {
        return Some(());
    }
    let mut dying = Vec::new();
    let mut affine_discards = Vec::new();
    for (result, root) in results.iter().rev() {
        if result.statement_index != coordinate.statement_index
            || !matches!(root, facts::PlaceRoot::Expression(_))
            || result.multiplicity == Multiplicity::Unrestricted
        {
            continue;
        }
        let arguments = call
            .structural_arguments
            .iter()
            .filter(|argument| {
                argument.source_structural_result_binding_ordinal() == Some(result.binding_ordinal)
            })
            .collect::<Vec<_>>();
        if arguments.is_empty() {
            continue;
        }
        let transfers = arguments
            .iter()
            .filter(|argument| argument.access == CheckedStructuralAccess::Owned)
            .collect::<Vec<_>>();
        if transfers.iter().any(|argument| argument.path.is_empty()) {
            // Whole transfers carry custody into the callee, including claims.
            continue;
        }
        if result.multiplicity != Multiplicity::Affine {
            // Automatic disposal cannot discharge a live linear obligation.
            return None;
        }
        let source = CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
            binding_ordinal: result.binding_ordinal,
        };
        if transfers.is_empty() {
            affine_discards.push(CheckedUnitPartialAffineDiscardPlan {
                source,
                path: Vec::new(),
                type_identity: result.type_identity.clone(),
            });
        } else {
            let [argument] = transfers.as_slice() else {
                return None;
            };
            let residuals = partial_affine_residuals(
                &shapes.types,
                &source,
                &result.type_identity,
                &[(argument.path.clone(), argument.type_identity.clone())],
            )?;
            validate_permissions_at(
                program,
                &facts.flow,
                machine.symbol,
                state.symbol,
                *root,
                usize::try_from(coordinate.statement_index).ok()?,
                usize::try_from(coordinate.call_ordinal).ok()?,
                &source,
                &residuals,
            )?;
            affine_discards.extend(residuals);
        }
        dying.push(result);
    }
    if dying.is_empty() {
        return Some(());
    }
    for result in dying {
        // Establishments and calls publish the same disposal debt. Move it
        // to this continuation exactly once, never leave a second return drop.
        let mut producers = operations.iter_mut().filter_map(|operation| {
            operation
                .discardable_structural_result_mut()
                .filter(|(produced, _)| produced.binding_ordinal == result.binding_ordinal)
        });
        let (produced, discard_on_return) = producers.next()?;
        if producers.next().is_some() || *produced != *result || !*discard_on_return {
            return None;
        }
        *discard_on_return = false;
    }
    operations.push(CheckedUnitEffectOperationPlan::ContinuationCleanup {
        boundary: checked_trees::CheckedUnitCleanupBoundary::Call(coordinate),
        affine_discards,
    });
    Some(())
}

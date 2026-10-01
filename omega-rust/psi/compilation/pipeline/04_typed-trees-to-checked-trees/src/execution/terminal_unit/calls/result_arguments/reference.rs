//! Calls lend the referent captured by a returned permission, not storage
//! owned by the result binding. Reconstruct its producer and lifetime, then
//! retain the explicit crossing and the authored projection independently.
use crate::execution::terminal_unit::calls::argument_paths::projected_borrowed_path;
use crate::execution::terminal_unit::calls::structural_arguments::exact_structural_argument_access;
use crate::execution::terminal_unit::types::{ShapeCollector, base_type_identity, state_flow};
use checked_trees::{
    CheckFacts, CheckedStructuralAccess, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralArgumentSourcePlan, CheckedUnitStructuralParameterPlan,
    CheckedUnitStructuralPathSegment, CheckedUnitStructuralResultBindingPlan,
    CheckedUnitStructuralTypeShape,
};
use language_semantics::{Multiplicity, ReferenceAccess};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::signature::StateParameter;
use typed_trees::state::State;
use typed_trees::statement::TableLocalData;
use typed_trees::types::TypeReferenceHandle;
use validation::reference_result_custody;

/// Slice views have their own descriptor argument route. This route keeps the
/// existing primitive reference and plain-record permission representations.
pub(super) fn is_carrier(program: &TypedTrees, reference: TypeReferenceHandle) -> bool {
    if reference_result_custody::parts(program, reference).is_some() {
        return true;
    }
    let Some((referent, _)) = reference_result_custody::mutable_borrowed_parts(program, reference)
    else {
        return false;
    };
    if !validation::has_plain_owned_contents_with_represented_constraints(program, referent) {
        return false;
    }
    let mut shapes = ShapeCollector::new(program);
    shapes.add_type(referent, &[], &[]).is_some_and(|identity| {
        shapes.types.get(&identity).is_some_and(|declaration| {
            matches!(
                declaration.shape,
                CheckedUnitStructuralTypeShape::Record { .. }
            )
        })
    })
}

pub(super) fn argument(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &State,
    call: &checked_trees::FlowCallFact,
    expression: ExpressionHandle,
    place: &crate::flow::CanonicalPlace,
    local: &TableLocalData,
    result: &CheckedUnitStructuralResultBindingPlan,
    parameter: &StateParameter,
    target_identity: &str,
) -> Option<CheckedUnitStructuralArgumentPlan> {
    let (referent, access) =
        reference_result_custody::mutable_borrowed_parts(program, local.type_reference)?;
    reference_result_custody::mutable_borrowed_parts(program, parameter.type_reference)?;
    if local.is_mutable
        || place.root != facts::PlaceRoot::Symbol(local.symbol)
        || result.multiplicity != Multiplicity::Affine
        || result.type_identity
            != program
                .normalized_type_identity(local.type_reference)
                .as_str()
    {
        return None;
    }
    let flow = state_flow(facts, machine, state.symbol)?;
    let mut producers = facts
        .flow
        .control
        .calls
        .span_or_empty(flow.calls)
        .iter()
        .filter(|producer| producer.authored_expression == local.initial_value);
    let producer = producers.next()?;
    if producers.next().is_some() {
        return None;
    }
    let loan =
        reference_result_custody::result_loan(program, facts, machine, state, producer, result)?;
    let end = reference_result_custody::release_statement(facts, machine, state.symbol, loan)?;
    if call.statement_index <= result.statement_index as usize
        || call.statement_index >= end as usize
    {
        return None;
    }
    let mut path = vec![CheckedUnitStructuralPathSegment::Referent];
    if place.segments.is_empty() {
        if base_type_identity(program, referent, &[])? != target_identity
            || !super::names_whole_local(program, call, expression, local.symbol)
        {
            return None;
        }
    } else {
        let ExpressionNode::Borrow(borrow) = program.expression_table.expression(expression) else {
            return None;
        };
        if borrow.access != ReferenceAccess::Mutable
            || crate::flow::canonical_place_from_expression_in_state(
                program,
                state.symbol,
                call.statement_index,
                borrow.target,
            )? != *place
            || exact_structural_argument_access(
                program,
                facts,
                machine,
                state.symbol,
                call,
                place,
                access,
            )? != access
        {
            return None;
        }
        let (selected, suffix) =
            projected_borrowed_path(program, machine, state.symbol, call.statement_index, place)?;
        if base_type_identity(program, selected, &[])? != target_identity
            || !suffix.iter().all(|segment| {
                matches!(
                    segment,
                    CheckedUnitStructuralPathSegment::Field(_)
                        | CheckedUnitStructuralPathSegment::FixedIndex(_)
                )
            })
        {
            return None;
        }
        path.extend(suffix);
    }
    Some(CheckedUnitStructuralArgumentPlan {
        source: CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
            binding_ordinal: result.binding_ordinal,
        },
        path,
        type_identity: target_identity.to_owned(),
        access: CheckedStructuralAccess::MutableBorrow,
    })
}

/// Reconcile an implicit mutable receiver with the same returned-reference
/// custody used by an explicit whole-local argument. The result binding is a
/// permission carrier: the emitted argument follows its `Referent`, and the
/// producer loan and release boundary remain the authority for that access.
#[allow(clippy::too_many_arguments)]
pub(in crate::execution::terminal_unit) fn receiver_argument(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: SymbolHandle,
    state: &State,
    call: &checked_trees::FlowCallFact,
    place: &crate::flow::CanonicalPlace,
    local: &TableLocalData,
    result: &CheckedUnitStructuralResultBindingPlan,
    target_state: SymbolHandle,
    target: &CheckedUnitStructuralParameterPlan,
) -> Option<CheckedUnitStructuralArgumentPlan> {
    let target_state = crate::semantic::calls::find_state(program, target_state)?;
    // Structural signatures retain the authored source position even when a
    // scalar or erased sibling receives no structural slot. Reconstruct the
    // unique authored receiver and compare that position rather than treating
    // `target.position` as an index into a compact structural roster.
    let mut target_parameters = program
        .state_parameters(target_state)
        .iter()
        .enumerate()
        .filter(|(_, parameter)| parameter.is_self);
    let (target_position, target_parameter) = target_parameters.next()?;
    if target_parameters.next().is_some()
        || u32::try_from(target_position).ok()? != target.position
        || target.access != CheckedStructuralAccess::MutableBorrow
        || target.multiplicity != program.type_multiplicity(target_parameter.type_reference)
    {
        return None;
    }
    let argument = argument(
        program,
        facts,
        machine,
        state,
        call,
        ExpressionHandle::invalid(),
        place,
        local,
        result,
        target_parameter,
        &target.type_identity,
    )?;
    (argument.type_identity == target.type_identity && argument.access == target.access)
        .then_some(argument)
}

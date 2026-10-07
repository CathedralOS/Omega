//! Finish temporary disposition actions after executable hook targets exist.
//! Operand planning captures the exact dying owner and maximal complement.
//! Completing that roster consumes the checker's contextual dispositions; it
//! does not infer cleanup from a callee's name or fabricate another owner.

use super::actions::disposition_actions;
use crate::execution::terminal_unit::types::ShapeCollector;
use checked_trees::{
    CheckFacts, CheckedNormalCleanupAction, CheckedUnitCallCoordinate, CheckedUnitCallableEntry,
    CheckedUnitEffectOperationPlan, CheckedUnitStructuralArgumentSourcePlan,
};
use language_semantics::PermissionEventSource;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;

fn selected_actions(
    program: &TypedTrees,
    facts: &CheckFacts,
    targets: &[CheckedUnitCallableEntry<'_>],
    shapes: &mut ShapeCollector<'_>,
    machine: SymbolHandle,
    state: SymbolHandle,
    coordinate: CheckedUnitCallCoordinate,
    root: facts::PlaceRoot,
    source: CheckedUnitStructuralArgumentSourcePlan,
) -> Option<Vec<CheckedNormalCleanupAction>> {
    let mut actions = Vec::new();
    for (_, disposition) in facts.contextual_cleanup.dispositions.iter().filter(|(_, row)| {
        row.machine == machine && row.state == state && row.root == root
            && matches!(row.source, PermissionEventSource::Call { statement_index, call_ordinal, .. }
                if statement_index == coordinate.statement_index as usize
                    && call_ordinal == coordinate.call_ordinal as usize)
    }) {
        actions.extend(disposition_actions(program, facts, targets, shapes, disposition, source.clone(), None)?);
    }
    Some(actions)
}

/// Operation indices and their completed action streams, prepared while the
/// candidate roster remains immutable. Statement/local retirement retains its
/// independent, already selected disposition; only anonymous operand owners
/// belong to this call-continuation roster.
pub(in crate::execution::terminal_unit) fn operations(
    program: &TypedTrees,
    facts: &CheckFacts,
    targets: &[CheckedUnitCallableEntry<'_>],
    shapes: &mut ShapeCollector<'_>,
    machine: SymbolHandle,
    state: SymbolHandle,
    parameters: &[checked_trees::CheckedUnitStructuralParameterPlan],
    operations: &[CheckedUnitEffectOperationPlan],
) -> Option<Vec<(usize, Vec<CheckedNormalCleanupAction>)>> {
    let mut replacements = Vec::new();
    for (operation_index, operation) in operations.iter().enumerate() {
        if matches!(
            operation,
            CheckedUnitEffectOperationPlan::ContinuationCleanup {
                boundary: checked_trees::CheckedUnitCleanupBoundary::Replacement { .. },
                ..
            }
        ) {
            replacements.push((
                operation_index,
                super::replacements::actions(
                    program,
                    facts,
                    targets,
                    shapes,
                    machine,
                    state,
                    parameters,
                    operations,
                    operation_index,
                )?,
            ));
            continue;
        }
        let CheckedUnitEffectOperationPlan::ContinuationCleanup {
            boundary: checked_trees::CheckedUnitCleanupBoundary::Call(coordinate),
            cleanup_actions,
        } = operation
        else {
            continue;
        };
        let Some(consumer) = operations[..operation_index]
            .iter()
            .rev()
            .filter_map(CheckedUnitEffectOperationPlan::call)
            .find(|call| call.coordinate == *coordinate)
        else {
            continue;
        };
        let mut sources = Vec::new();
        let mut completed = Vec::new();
        for action in cleanup_actions {
            let source = action.source();
            if sources.contains(source) {
                continue;
            }
            sources.push(source.clone());
            let CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal } =
                *source
            else {
                return None;
            };
            if !consumer
                .structural_arguments
                .iter()
                .any(|argument| argument.source == *source)
            {
                completed.extend(
                    cleanup_actions
                        .iter()
                        .filter(|action| action.source() == source)
                        .cloned(),
                );
                continue;
            }
            let producer = operations[..operation_index].iter().find(|operation| {
                operation
                    .structural_result_binding()
                    .is_some_and(|result| result.binding_ordinal == binding_ordinal)
            })?;
            let expression = match producer {
                CheckedUnitEffectOperationPlan::EstablishStructuralValue { value, .. } => {
                    facts.values.structural_values.nodes.get(*value).expression
                }
                CheckedUnitEffectOperationPlan::Call(call) => {
                    let state_flow = facts.flow.control.states.iter().find_map(|(_, row)| {
                        (row.machine_symbol == machine && row.state_symbol == state).then_some(row)
                    })?;
                    facts
                        .flow
                        .control
                        .calls
                        .span(state_flow.calls)?
                        .iter()
                        .find_map(|row| {
                            (row.statement_index == call.coordinate.statement_index as usize
                                && row.call_ordinal == call.coordinate.call_ordinal as usize)
                                .then_some(row.authored_expression)
                        })?
                }
                _ => return None,
            };
            completed.extend(selected_actions(
                program,
                facts,
                targets,
                shapes,
                machine,
                state,
                *coordinate,
                facts::PlaceRoot::Expression(expression),
                source.clone(),
            )?);
        }
        replacements.push((operation_index, completed));
    }
    Some(replacements)
}

pub(in crate::execution::terminal_unit) fn install(
    operations: &mut [CheckedUnitEffectOperationPlan],
    replacements: Vec<(usize, Vec<CheckedNormalCleanupAction>)>,
) {
    for (operation_index, selected) in replacements {
        if let CheckedUnitEffectOperationPlan::ContinuationCleanup {
            cleanup_actions, ..
        } = &mut operations[operation_index]
        {
            *cleanup_actions = selected;
        }
    }
}

/// Complete the computed-call roster in the same contextual action namespace.
/// Action storage is rebuilt once: scratch geometry never remains as a second
/// published cleanup stream beside the selected hooks.
pub(crate) fn finalize_scalar_actions(program: &TypedTrees, facts: &mut CheckFacts) -> Option<()> {
    let targets = facts
        .flow
        .terminal_unit_effects
        .machines
        .iter()
        .map(checked_trees::CheckedUnitEffectMachinePlan::callable_entry)
        .chain(
            facts
                .flow
                .terminal_unit_effects
                .composed_machines
                .iter()
                .filter_map(checked_trees::CheckedComposedUnitControlMachinePlan::callable_entry),
        )
        .collect::<Vec<_>>();
    let mut shapes = ShapeCollector::new(program);
    let plans = &facts.values.scalar_computations;
    let mut replacements = Vec::new();
    for (_, node) in plans.nodes.iter() {
        let (source_call, operands, retained) = match node.kind {
            checked_trees::CheckedScalarComputationKind::Call {
                source_call,
                structural_arguments,
                temporary_cleanups,
                ..
            } => (source_call, structural_arguments, temporary_cleanups),
            _ => continue,
        };
        if retained.is_empty() {
            continue;
        }
        let call = facts.flow.control.calls.get(source_call);
        let state_flow = facts.flow.control.states.iter().find_map(|(_, row)| {
            facts
                .flow
                .control
                .calls
                .span_or_empty(row.calls)
                .iter()
                .any(|candidate| std::ptr::eq(candidate, call))
                .then_some(row)
        })?;
        let operands = plans.structural_arguments.span(operands)?;
        let coordinate = CheckedUnitCallCoordinate {
            statement_index: u32::try_from(call.statement_index).ok()?,
            call_ordinal: u32::try_from(call.call_ordinal).ok()?,
        };
        for (row_position, row) in plans.temporary_cleanups.span(retained)?.iter().enumerate() {
            let identity = arena::Handle::from_parts(
                retained
                    .start()
                    .arena_index()
                    .checked_add(u32::try_from(row_position).ok()?)?,
                retained.start().generation(),
            );
            let argument = operands.get(row.argument_ordinal as usize)?.as_place()?;
            let CheckedUnitStructuralArgumentSourcePlan::StructuralExpression { expression } =
                argument.source
            else {
                return None;
            };
            replacements.push((
                identity,
                selected_actions(
                    program,
                    facts,
                    &targets,
                    &mut shapes,
                    state_flow.machine_symbol,
                    state_flow.state_symbol,
                    coordinate,
                    facts::PlaceRoot::Expression(expression),
                    argument.source.clone(),
                )?,
            ));
        }
    }
    let mut actions = arena::Arena::new();
    for (identity, selected) in replacements {
        facts
            .values
            .scalar_computations
            .temporary_cleanups
            .get_mut(identity)
            .actions = actions.insert_many(selected);
    }
    facts.values.scalar_computations.temporary_cleanup_actions = actions;
    for shape in shapes.types.into_values() {
        if !facts
            .flow
            .terminal_unit_effects
            .structural_types
            .iter()
            .any(|existing| existing.identity == shape.identity)
        {
            facts
                .flow
                .terminal_unit_effects
                .structural_types
                .push(shape);
        }
    }
    Some(())
}

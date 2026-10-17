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

fn selected_actions(
    program: &dyn ::validation::CheckingInput,
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
    let mut retirement = None;
    for (disposition_handle, disposition) in facts.contextual_cleanup.dispositions.iter().filter(|(_, row)| {
        row.machine == machine && row.state == state && row.root == root
            && matches!(row.source, PermissionEventSource::Call { statement_index, call_ordinal, .. }
                if statement_index == coordinate.statement_index as usize
                    && call_ordinal == coordinate.call_ordinal as usize)
    }) {
        if retirement.is_none() {
            retirement = Some(super::actions::home_retirement(program, shapes, disposition, source.clone())?);
        }
        actions.extend(disposition_actions(program, facts, targets, shapes, disposition_handle, source.clone(), None)?);
    }
    if let Some(retirement) = retirement {
        super::actions::finish_home(&mut actions, retirement);
    }
    Some(actions)
}

pub(in crate::execution::terminal_unit) struct SelectedActions {
    pub actions: Vec<CheckedNormalCleanupAction>,
    pub selected_regions: Vec<checked_trees::CheckedUnitStructuralArgumentPlan>,
}

/// Operation indices and their completed action streams, prepared while the
/// candidate roster remains immutable. Statement/local retirement retains its
/// independent, already selected disposition; only anonymous operand owners
/// belong to this call-continuation roster.
pub(in crate::execution::terminal_unit) fn operations(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    targets: &[CheckedUnitCallableEntry<'_>],
    shapes: &mut ShapeCollector<'_>,
    machine: SymbolHandle,
    state: SymbolHandle,
    parameters: &[checked_trees::CheckedUnitStructuralParameterPlan],
    operations: &[CheckedUnitEffectOperationPlan],
) -> Option<Vec<(usize, SelectedActions)>> {
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
            ..
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
            let root = super::normal_edges::producer_root(facts, state, producer)?;
            completed.extend(selected_actions(
                program,
                facts,
                targets,
                shapes,
                machine,
                state,
                *coordinate,
                root,
                source.clone(),
            )?);
        }
        replacements.push((
            operation_index,
            SelectedActions {
                actions: completed,
                selected_regions: Vec::new(),
            },
        ));
    }
    Some(replacements)
}

pub(in crate::execution::terminal_unit) fn install(
    operations: &mut [CheckedUnitEffectOperationPlan],
    replacements: Vec<(usize, SelectedActions)>,
) {
    for (operation_index, selected) in replacements {
        if let CheckedUnitEffectOperationPlan::ContinuationCleanup {
            cleanup_actions,
            selected_regions,
            ..
        } = &mut operations[operation_index]
        {
            *cleanup_actions = selected.actions;
            *selected_regions = selected.selected_regions;
        }
    }
}

/// Complete the computed-call roster in the same contextual action namespace.
/// Action storage is rebuilt once: scratch geometry never remains as a second
/// published cleanup stream beside the selected hooks.
pub(crate) fn finalize_scalar_actions(
    program: &dyn ::validation::CheckingInput,
    facts: &mut CheckFacts,
) -> Option<()> {
    let mut shapes = ShapeCollector::new(program);
    // Computed calls append conditions to the same published catalog as
    // statement continuations. Restore that owner even when plan selection
    // declines: existing action spans must never outlive their arena.
    shapes.cleanup_control = std::mem::take(&mut facts.flow.terminal_unit_effects.cleanup_control);
    let replacements = (|| {
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
                    .filter_map(
                        checked_trees::CheckedComposedUnitControlMachinePlan::callable_entry,
                    ),
            )
            .collect::<Vec<_>>();
        let plans = &facts.values.scalar_computations;
        let mut replacements = Vec::new();
        for (_, node) in plans.nodes.iter() {
            let (source_call, operands, retained, authority) = match node.kind {
                checked_trees::CheckedScalarComputationKind::Call {
                    source_call,
                    structural_arguments,
                    temporary_cleanups,
                    ref callee,
                    ..
                } => (
                    source_call,
                    structural_arguments,
                    temporary_cleanups,
                    callee.authority_value(),
                ),
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
                let source = match row.argument_ordinal {
                    Some(ordinal) => operands.get(ordinal as usize)?.as_place()?.source.clone(),
                    None if facts.values.structural_values.nodes.is_valid(authority) => {
                        CheckedUnitStructuralArgumentSourcePlan::StructuralExpression {
                            expression: facts
                                .values
                                .structural_values
                                .nodes
                                .get(authority)
                                .expression,
                        }
                    }
                    None => return None,
                };
                let CheckedUnitStructuralArgumentSourcePlan::StructuralExpression { expression } =
                    source
                else {
                    return None;
                };
                let retained_value = facts.values.structural_values.root_for_expression(
                    state_flow.state_symbol,
                    coordinate.statement_index,
                    expression,
                )?;
                let reference = retained_value.type_reference;
                let selected =
                    if validation::reference_result_custody::stored_loan_parts(program, reference)
                        .is_some()
                    {
                        // The completed descriptor owns a lifetime even without
                        // a source-local contextual disposition. End that exact
                        // carrier, never the backing beneath Referent. Stored
                        // output copies retain their independently tracked loans;
                        // the ordinary custody check still verifies their backing.
                        // Nominal
                        // temporary owners still require their selected hooks.
                        let type_identity = shapes.add_type(reference, &[], &[])?;
                        vec![CheckedNormalCleanupAction::Discard(
                            checked_trees::CheckedUnitPartialAffineDiscardPlan {
                                source,
                                path: Vec::new(),
                                type_identity,
                                case_conditions: Default::default(),
                            },
                        )]
                    } else {
                        selected_actions(
                            program,
                            facts,
                            &targets,
                            &mut shapes,
                            state_flow.machine_symbol,
                            state_flow.state_symbol,
                            coordinate,
                            facts::PlaceRoot::Expression(expression),
                            source,
                        )?
                    };
                replacements.push((identity, selected));
            }
        }
        Some(replacements)
    })();
    facts.flow.terminal_unit_effects.cleanup_control = std::mem::take(&mut shapes.cleanup_control);
    let replacements = replacements?;
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
    facts
        .flow
        .terminal_unit_effects
        .adopt_structural_types(shapes.types.into_values())
        .ok()?;
    Some(())
}

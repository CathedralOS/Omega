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
        operations[..operation_index]
            .iter()
            .rev()
            .find(|operation| operation.invocation_coordinate() == Some(*coordinate))?;
        let mut sources = Vec::new();
        let mut completed = Vec::new();
        for action in cleanup_actions {
            let source = action.source();
            if sources.contains(source) {
                continue;
            }
            sources.push(source.clone());
            let root = match *source {
                CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal } => {
                    let mut producers = operations[..operation_index].iter().filter(|operation| {
                        operation
                            .structural_result_binding()
                            .is_some_and(|result| result.binding_ordinal == binding_ordinal)
                    });
                    let producer = producers.next()?;
                    if producers.next().is_some() {
                        return None;
                    }
                    super::normal_edges::producer_root(facts, state, producer)?
                }
                CheckedUnitStructuralArgumentSourcePlan::StructuralExpression { expression } => {
                    facts::PlaceRoot::Expression(expression)
                }
                _ => return None,
            };
            let contextual = facts.contextual_cleanup.dispositions.iter().any(|(_, row)| {
                row.machine == machine && row.state == state && row.root == root
                    && matches!(row.source, PermissionEventSource::Call { statement_index, call_ordinal, .. }
                        if statement_index == coordinate.statement_index as usize
                            && call_ordinal == coordinate.call_ordinal as usize)
            });
            if !contextual {
                // Independent result/descriptor disposal retains its existing
                // authority. A contextual owner seed must instead resolve its
                // complete partition, including an admitted empty complement.
                if cleanup_actions.iter().any(|action| {
                    action.source() == source
                        && matches!(action, CheckedNormalCleanupAction::RetireHome { .. })
                }) {
                    return None;
                }
                completed.extend(
                    cleanup_actions
                        .iter()
                        .filter(|action| action.source() == source)
                        .cloned(),
                );
                continue;
            }
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

/// Complete calls and scalar observations in the same action namespace.
/// Action storage is rebuilt once: scratch geometry never remains as a second
/// published cleanup stream beside the selected hooks.
pub(crate) fn finalize_scalar_actions(
    program: &dyn ::validation::CheckingInput,
    facts: &mut CheckFacts,
) -> Result<(), String> {
    let mut shapes = ShapeCollector::new(program);
    // Computed calls append conditions to the same published catalog as
    // statement continuations. Restore that owner even when plan selection
    // declines: existing action spans must never outlive their arena.
    shapes.cleanup_control = std::mem::take(&mut facts.flow.terminal_unit_effects.cleanup_control);
    let mut refusal = String::from("missing executable cleanup catalog");
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
        let mut observation_replacements = Vec::new();
        for (node_handle, node) in plans.nodes.iter() {
            if !node.normal_cleanup_actions.is_empty() {
                let checked_trees::CheckedScalarComputationKind::StructuralField {
                    receiver_value,
                    ..
                } = node.kind
                else {
                    return None;
                };
                let receiver = facts.values.structural_values.nodes.get(receiver_value);
                let type_identity = shapes.add_type(receiver.type_reference, &[], &[])?;
                let selected = plans
                    .temporary_cleanup_actions
                    .span(node.normal_cleanup_actions)?;
                if selected
                    .iter()
                    .any(|action| action.type_identity() != type_identity)
                {
                    return None;
                }
                observation_replacements.push((node_handle, selected.to_vec()));
            }
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
            let site = format!(
                "machine `{}`, state `{}`, statement {}, call {}",
                program.symbols.name(state_flow.machine_symbol),
                program.symbols.name(state_flow.state_symbol),
                call.statement_index,
                call.call_ordinal
            );
            refusal = format!("{site}: missing checked operand/root/completion correspondence");
            let operands = plans.structural_arguments.span(operands)?;
            let coordinate = CheckedUnitCallCoordinate {
                statement_index: u32::try_from(call.statement_index).ok()?,
                call_ordinal: u32::try_from(call.call_ordinal).ok()?,
            };
            // An operand graph is not a death roster. In particular, omitted
            // zeros reuse their constructor's expression as a source location.
            // Bind admitted owner roots once across the whole call, then retain
            // that exact value handle through lowering.
            let values = &facts.values.structural_values;
            let mut reserved = Vec::new();
            for row in plans.temporary_cleanups.span(retained)? {
                let expression = match row.argument_ordinal {
                    Some(ordinal) => {
                        match operands.get(ordinal as usize)?.as_place()?.source.clone() {
                            CheckedUnitStructuralArgumentSourcePlan::StructuralExpression {
                                expression,
                            } => expression,
                            _ => return None,
                        }
                    }
                    None if values.nodes.is_valid(authority) => {
                        values.nodes.get(authority).expression
                    }
                    None => return None,
                };
                let root = values.root_for_expression(
                    state_flow.state_symbol,
                    coordinate.statement_index,
                    expression,
                )?;
                reserved.push((
                    row.argument_ordinal,
                    root,
                    values.value_dependencies(root.root)?,
                ));
            }
            let machine = program.machine_by_symbol(state_flow.machine_symbol)?;
            let state = program.state_in_machine(machine, state_flow.state_symbol)?;
            let (_, statement) = program
                .statement_table
                .indexed_statements(state.statement_nodes)
                .find(|(statement_index, _)| *statement_index == call.statement_index)?;
            let completed = crate::checks::ownership_occurrences::completed_expressions(
                program,
                machine,
                state,
                call.statement_index,
                statement,
                &facts.operators,
            );
            let mut owners = Vec::new();
            // Stored descriptor retirement has independently admitted lifetime
            // authority. Its backing remains a separate contextual owner.
            for (ordinal, root, _) in &reserved {
                if validation::reference_result_custody::stored_loan_parts(
                    program,
                    root.type_reference,
                )
                .is_some()
                {
                    shapes.add_type(root.type_reference, &[], &[])?;
                    owners.push((
                        *ordinal,
                        root.root,
                        super::descriptor_retirement(
                            program,
                            CheckedUnitStructuralArgumentSourcePlan::StructuralExpression {
                                expression: root.expression,
                            },
                            root.type_reference,
                        )?,
                    ));
                }
            }
            // The ownership producer has already selected the roots, paths and
            // exclusions at this exact consumer. Contained graph nodes grant no
            // additional death. Multiple regions of one owner stay one ordered
            // action group; two operands cannot retire that owner twice.
            for (_, disposition) in facts.contextual_cleanup.dispositions.iter().filter(|(_, disposition)| {
                disposition.machine == state_flow.machine_symbol
                    && disposition.state == state_flow.state_symbol
                    && matches!(disposition.source, PermissionEventSource::Call { statement_index, call_ordinal, .. }
                        if statement_index == call.statement_index && call_ordinal == call.call_ordinal)
            }) {
                let facts::PlaceRoot::Expression(expression) = disposition.root else { continue; };
                let root = values.root_for_expression(state_flow.state_symbol, coordinate.statement_index, expression);
                let Some(root) = root else {
                    // A named/local owner belongs to another cleanup stream.
                    // An anonymous backing reached by this operand must publish
                    // its real owner, not borrow a child's diagnostic location.
                    if reserved.iter().any(|(_, _, dependencies)| dependencies.iter().any(|value| values.nodes.get(*value).expression == expression)) {
                        refusal = format!("{site}: anonymous backing {expression:?} has no published completed root");
                        return None;
                    }
                    continue;
                };
                let mut containing = reserved.iter().filter(|(_, _, dependencies)| dependencies.contains(&root.root));
                let Some(first) = containing.next() else { continue; };
                let exact = reserved.iter().filter(|(_, candidate, _)| candidate.root == root.root).collect::<Vec<_>>();
                let ordinal = match exact.as_slice() {
                    [slot] => slot.0,
                    [] if containing.next().is_none() => first.0,
                    _ => {
                        refusal = format!("{site}: completed owner {expression:?} has ambiguous reserved operand storage");
                        return None;
                    }
                };
                if owners.iter().any(|(_, owner, _)| *owner == root.root) {
                    // Descriptor lifetime and a nominal contextual death may
                    // not both dispose the same carrier.
                    if validation::reference_result_custody::stored_loan_parts(program, root.type_reference).is_some() {
                        refusal = format!("{site}: descriptor retirement overlaps a contextual owner death for {expression:?}");
                        return None;
                    }
                    continue;
                }
                let actions = selected_actions(program, facts, &targets, &mut shapes,
                    state_flow.machine_symbol, state_flow.state_symbol, coordinate,
                    disposition.root, CheckedUnitStructuralArgumentSourcePlan::StructuralExpression { expression })?;
                owners.push((ordinal, root.root, actions));
            }
            let mut ordered = Vec::new();
            for (ordinal, owner, actions) in owners {
                let expression = values.nodes.get(owner).expression;
                let mut positions = completed
                    .iter()
                    .enumerate()
                    .filter(|(_, candidate)| **candidate == expression);
                let Some((position, _)) = positions.next() else {
                    refusal = format!("{site}: owner {expression:?} has no authored completion");
                    return None;
                };
                if positions.next().is_some() {
                    refusal =
                        format!("{site}: owner {expression:?} has multiple authored completions");
                    return None;
                }
                ordered.push((position, ordinal, owner, actions));
            }
            ordered.sort_by_key(|(position, ..)| std::cmp::Reverse(*position));
            replacements.push((node_handle, ordered));
        }
        Some((replacements, observation_replacements))
    })();
    facts.flow.terminal_unit_effects.cleanup_control = std::mem::take(&mut shapes.cleanup_control);
    let (replacements, observation_replacements) = replacements.ok_or(refusal)?;
    let mut actions = arena::Arena::new();
    let mut cleanups = arena::Arena::new();
    for (node_handle, owners) in replacements {
        let groups = owners
            .into_iter()
            .map(|(_, argument_ordinal, owner, selected)| {
                checked_trees::CheckedScalarComputationTemporaryCleanup {
                    argument_ordinal,
                    owner,
                    actions: actions.insert_many(selected),
                }
            });
        let checked_trees::CheckedScalarComputationKind::Call {
            temporary_cleanups, ..
        } = &mut facts
            .values
            .scalar_computations
            .nodes
            .get_mut(node_handle)
            .kind
        else {
            return Err("temporary cleanup finalization lost its checked call".into());
        };
        *temporary_cleanups = cleanups.insert_many(groups);
    }
    facts.values.scalar_computations.temporary_cleanups = cleanups;
    // Both consumers publish spans into this one arena. Replacing it without
    // remapping observations would silently alias unrelated call actions.
    for (identity, selected) in observation_replacements {
        facts
            .values
            .scalar_computations
            .nodes
            .get_mut(identity)
            .normal_cleanup_actions = actions.insert_many(selected);
    }
    facts.values.scalar_computations.temporary_cleanup_actions = actions;
    facts
        .flow
        .terminal_unit_effects
        .adopt_structural_types(shapes.types.into_values())
        .map_err(|_| {
            String::from("temporary cleanup has conflicting executable structural types")
        })?;
    Ok(())
}

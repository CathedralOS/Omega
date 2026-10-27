//! Finish temporary disposition actions after executable hook targets exist.
//! Operand planning retains the exact dying owner. Source checking owns its
//! maximal complement, including any transferred linear claim frontier.
//! Completing that roster consumes the checker's contextual dispositions; it
//! does not infer cleanup from a callee's name or fabricate another owner.

use super::actions::disposition_actions;
use super::retained_owners::{owner_group_actions, selected_actions};
use crate::execution::terminal_unit::types::ShapeCollector;
use checked_trees::{
    CheckFacts, CheckedNormalCleanupAction, CheckedUnitCallCoordinate, CheckedUnitCallableEntry,
    CheckedUnitEffectOperationPlan, CheckedUnitStructuralArgumentSourcePlan,
};
use language_semantics::PermissionEventSource;
use symbols::SymbolHandle;

/// Explain only the failed lifetime query using its completed capture graph.
/// Unavailable graph rows remain unavailable; diagnostic traversal decides no
/// custody, formation, lifetime coverage or cleanup disposition.
fn incomplete_lifetime_capture_diagnostic(
    program: &dyn validation::CheckingInput,
    control: &checked_trees::FlowControlFacts,
    operand: arena::Handle<checked_trees::FlowOperandFact>,
) -> String {
    let mut details = String::new();
    let mut pending = vec![operand];
    let mut visited = Vec::new();
    while let Some(handle) = pending.pop() {
        if !control.operands.is_valid(handle) || visited.contains(&handle) {
            continue;
        }
        visited.push(handle);
        let capture = control.operands.get(handle);
        details.push_str(&format!("\n  lifetime operand {handle:?}: {capture:?}"));
        if capture.type_reference.is_valid() {
            details.push_str(&format!(
                "\n  declared lifetime frontier {:?}",
                validation::borrow_contracts::borrow_carrying_owner_paths(
                    program,
                    capture.type_reference,
                ),
            ));
        } else {
            details.push_str("\n  declared lifetime carrier unavailable");
        }
        pending.push(capture.backing);
        pending.extend(
            control
                .operand_children
                .span_or_empty(capture.constructed_children())
                .iter()
                .map(|child| child.value),
        );
        let mut references = control
            .captured_reference_leaves
            .span_or_empty(capture.reference_leaves)
            .to_vec();
        if let checked_trees::FlowOperandCompletion::Copied { copy, path } = capture.completion {
            if control.copied_values.is_valid(copy) {
                let copy = control.copied_values.get(copy);
                details.push_str(&format!(
                    "\n  copied selection {:?} from {:?}",
                    control.operand_child_paths.span_or_empty(path),
                    copy.source,
                ));
                pending.push(copy.source);
                references.extend_from_slice(
                    control
                        .captured_reference_leaves
                        .span_or_empty(copy.references),
                );
            } else {
                details.push_str("\n  copied value capture unavailable");
            }
        }
        let mut retentions = vec![capture.retention];
        for leaf in references {
            details.push_str(&format!(
                "\n  saved reference {:?}: {leaf:?}",
                control.operand_child_paths.span_or_empty(leaf.path),
            ));
            retentions.push(leaf.retention);
        }
        for retention in retentions {
            if !control.operand_retentions.is_valid(retention) {
                details.push_str("\n  lifetime retention unavailable");
                continue;
            }
            let profile = control.operand_retentions.get(retention);
            details.push_str(&format!(
                "\n  lifetime profile {retention:?}: {profile:?}; roots {:?}; projection {:?}",
                control.operand_retained_roots.span_or_empty(profile.roots),
                control
                    .operand_retained_paths
                    .span_or_empty(profile.stored_projection),
            ));
            pending.push(profile.stored_value);
            for leaf in control
                .operand_retained_leaves
                .span_or_empty(profile.leaves)
            {
                details.push_str(&format!(
                    "\n  retained leaf {:?}: {leaf:?}; roots {:?}",
                    control.operand_retained_paths.span_or_empty(leaf.path),
                    control.retained_roots(leaf.retention),
                ));
            }
        }
    }
    details
}

pub(in crate::execution::terminal_unit) struct SelectedActions {
    pub actions: Vec<CheckedNormalCleanupAction>,
    pub selected_regions: Vec<checked_trees::CheckedUnitStructuralArgumentPlan>,
}

/// The two storage owners expose one indexed operation schedule without
/// flattening arena-backed operands or cloning nested call bodies.
#[derive(Clone, Copy)]
enum OperationSequence<'plans> {
    Body(&'plans [CheckedUnitEffectOperationPlan]),
    Operands {
        sequence: &'plans checked_trees::CheckedOperandSequence,
        end: usize,
    },
}

impl<'plans> OperationSequence<'plans> {
    fn operands(sequence: &'plans checked_trees::CheckedOperandSequence) -> Self {
        Self::Operands {
            sequence,
            end: sequence.len(),
        }
    }

    fn len(self) -> usize {
        match self {
            Self::Body(operations) => operations.len(),
            Self::Operands { end, .. } => end,
        }
    }

    fn get(self, ordinal: usize) -> Option<&'plans CheckedUnitEffectOperationPlan> {
        match self {
            Self::Body(operations) => operations.get(ordinal),
            Self::Operands { sequence, end } => {
                (ordinal < end).then(|| sequence.get(ordinal)).flatten()
            }
        }
    }

    fn prefix(self, end: usize) -> Option<Self> {
        if end > self.len() {
            return None;
        }
        Some(match self {
            Self::Body(operations) => Self::Body(&operations[..end]),
            Self::Operands { sequence, .. } => Self::Operands { sequence, end },
        })
    }
}

/// Construction scratch: selections retain only changed action streams and
/// their exact schedule positions, not copied executable bodies.
#[derive(Default)]
pub(in crate::execution::terminal_unit) struct OperationSelections {
    continuations: Vec<(usize, SelectedActions)>,
    constructions: Vec<(usize, Vec<(usize, CallSelections)>)>,
}

impl OperationSelections {
    fn is_empty(&self) -> bool {
        self.continuations.is_empty() && self.constructions.is_empty()
    }
}

pub(in crate::execution::terminal_unit) struct CallSelections {
    operands: OperationSelections,
    completions: OperationSelections,
}

impl CallSelections {
    fn is_empty(&self) -> bool {
        self.operands.is_empty() && self.completions.is_empty()
    }
}

/// Only attached releases publish delayed deaths. The collector's arena can
/// also contain rows from a plan that subsequently failed construction.
fn release_owned_dispositions<'plans>(
    operations: impl IntoIterator<Item = &'plans CheckedUnitEffectOperationPlan>,
    control: &checked_trees::CheckedCleanupControlPlans,
) -> Option<Vec<arena::Handle<checked_trees::ContextualCleanupDisposition>>> {
    let mut dispositions = Vec::new();
    for operation in operations
        .into_iter()
        .flat_map(CheckedUnitEffectOperationPlan::with_value_calls)
    {
        if let CheckedUnitEffectOperationPlan::ReleaseReference { owner_cleanups, .. } = operation {
            for owner in control.reference_owner_cleanups.span(*owner_cleanups)? {
                crate::collections::push_unique(&mut dispositions, owner.disposition);
            }
        }
    }
    Some(dispositions)
}

#[allow(clippy::too_many_arguments)]
pub(in crate::execution::terminal_unit) fn operations(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    targets: &[CheckedUnitCallableEntry<'_>],
    shapes: &mut ShapeCollector<'_>,
    machine: SymbolHandle,
    state: SymbolHandle,
    parameters: &[checked_trees::CheckedUnitStructuralParameterPlan],
    operations: &[CheckedUnitEffectOperationPlan],
) -> Option<OperationSelections> {
    let release_owned = release_owned_dispositions(operations, &shapes.cleanup_control)?;
    sequence_actions(
        program,
        facts,
        targets,
        shapes,
        machine,
        state,
        parameters,
        OperationSequence::Body(operations),
        None,
        &release_owned,
    )
}

#[allow(clippy::too_many_arguments)]
fn sequence_actions(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    targets: &[CheckedUnitCallableEntry<'_>],
    shapes: &mut ShapeCollector<'_>,
    machine: SymbolHandle,
    state: SymbolHandle,
    parameters: &[checked_trees::CheckedUnitStructuralParameterPlan],
    sequence: OperationSequence<'_>,
    completion: Option<(&CheckedUnitEffectOperationPlan, OperationSequence<'_>)>,
    release_owned: &[arena::Handle<checked_trees::ContextualCleanupDisposition>],
) -> Option<OperationSelections> {
    let selected = (|| {
        let mut selected = OperationSelections::default();
        for operation_index in 0..sequence.len() {
            let operation = sequence.get(operation_index)?;
            if let CheckedUnitEffectOperationPlan::ReleaseReference { owner_cleanups, .. } =
                operation
            {
                let owners = shapes
                    .cleanup_control
                    .reference_owner_cleanups
                    .span(*owner_cleanups)?
                    .to_vec();
                for (ordinal, owner) in owners.into_iter().enumerate() {
                    let actions = owner_group_actions(
                        program,
                        facts,
                        targets,
                        shapes,
                        owner.disposition,
                        owner.owner.source,
                    )?;
                    if actions.is_empty() {
                        return None;
                    }
                    let actions = shapes.cleanup_control.actions.insert_many(actions);
                    let handle = owner_cleanups.handle_at(u32::try_from(ordinal).ok()?);
                    shapes
                        .cleanup_control
                        .reference_owner_cleanups
                        .get_mut(handle)
                        .actions = actions;
                }
                continue;
            }

            if let CheckedUnitEffectOperationPlan::EstablishLocal(
                checked_trees::CheckedUnitLocalBindingPlan::StructuralValue { calls, .. },
            )
            | CheckedUnitEffectOperationPlan::EvaluateStructuralValue { calls, .. } = operation
            {
                let mut nested = Vec::new();
                for (call_index, call) in calls.iter().enumerate() {
                    let actions = value_call_actions(
                        program,
                        facts,
                        targets,
                        shapes,
                        machine,
                        state,
                        parameters,
                        call,
                        release_owned,
                    )?;
                    if !actions.is_empty() {
                        nested.push((call_index, actions));
                    }
                }
                if !nested.is_empty() {
                    selected.constructions.push((operation_index, nested));
                }
            }
            match operation {
                CheckedUnitEffectOperationPlan::ContinuationCleanup {
                    boundary: checked_trees::CheckedUnitCleanupBoundary::Replacement { .. },
                    ..
                } => {
                    selected.continuations.push((
                        operation_index,
                        super::replacements::actions(
                            program, facts, targets, shapes, machine, state, parameters, operation,
                        )?,
                    ));
                }
                CheckedUnitEffectOperationPlan::ContinuationCleanup {
                    boundary: checked_trees::CheckedUnitCleanupBoundary::Call(coordinate),
                    cleanup_actions,
                    ..
                } => {
                    let prior = sequence.prefix(operation_index)?;
                    // Retained nested calls keep their own invocation identities.
                    // The dependency inventory locates a source; it does not turn
                    // an arm-local producer into an unconditional live occurrence.
                    let mut local = None;
                    for position in 0..prior.len() {
                        for candidate in prior.get(position)?.with_value_calls() {
                            if candidate.invocation_coordinate() != Some(*coordinate) {
                                continue;
                            }
                            if local.replace(candidate).is_some() {
                                return None;
                            }
                        }
                    }
                    let enclosing = completion.filter(|(consumer, _)| {
                        consumer.invocation_coordinate() == Some(*coordinate)
                    });
                    let (consumer, producers) = match (local, enclosing) {
                        (Some(_), Some(_)) => return None,
                        (Some(consumer), None) => (consumer, prior),
                        (None, Some(completion)) => completion,
                        (None, None) => return None,
                    };
                    let actions = call_actions(
                        program,
                        facts,
                        targets,
                        shapes,
                        machine,
                        state,
                        consumer,
                        producers,
                        cleanup_actions,
                        release_owned,
                    )?;
                    selected.continuations.push((operation_index, actions));
                }
                _ => {}
            }
        }
        Some(selected)
    })();
    selected
}

#[allow(clippy::too_many_arguments)]
fn value_call_actions(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    targets: &[CheckedUnitCallableEntry<'_>],
    shapes: &mut ShapeCollector<'_>,
    machine: SymbolHandle,
    state: SymbolHandle,
    parameters: &[checked_trees::CheckedUnitStructuralParameterPlan],
    call: &checked_trees::CheckedStructuralValueCall,
    release_owned: &[arena::Handle<checked_trees::ContextualCleanupDisposition>],
) -> Option<CallSelections> {
    let preparation = OperationSequence::operands(call.operands());
    Some(CallSelections {
        operands: sequence_actions(
            program,
            facts,
            targets,
            shapes,
            machine,
            state,
            parameters,
            preparation,
            None,
            release_owned,
        )?,
        completions: if let Some(completions) = call.completions() {
            sequence_actions(
                program,
                facts,
                targets,
                shapes,
                machine,
                state,
                parameters,
                OperationSequence::operands(completions),
                call.operation().map(|operation| (operation, preparation)),
                release_owned,
            )?
        } else {
            OperationSelections::default()
        },
    })
}

#[allow(clippy::too_many_arguments)]
fn call_actions(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    targets: &[CheckedUnitCallableEntry<'_>],
    shapes: &mut ShapeCollector<'_>,
    machine: SymbolHandle,
    state: SymbolHandle,
    consumer: &CheckedUnitEffectOperationPlan,
    producers: OperationSequence<'_>,
    cleanup_actions: &[CheckedNormalCleanupAction],
    release_owned: &[arena::Handle<checked_trees::ContextualCleanupDisposition>],
) -> Option<SelectedActions> {
    let selected = (|| {
        let coordinate = consumer.invocation_coordinate()?;
        let mut sources = Vec::new();
        let mut completed = Vec::new();
        for action in cleanup_actions {
            let source = action.source();
            if sources.contains(source) {
                continue;
            }
            sources.push(source.clone());
            if let CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal } =
                *source
                && consumer
                    .structural_result_binding()
                    .is_some_and(|result| result.binding_ordinal == binding_ordinal)
            {
                // This call's discarded output has its own result disposition;
                // it is not an operand owner and may have no expression root.
                if cleanup_actions
                    .iter()
                    .filter(|action| action.source() == source)
                    .any(|action| !matches!(action, CheckedNormalCleanupAction::Discard(_)))
                {
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
            let root = match *source {
                CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal } => {
                    // Rejoin the same exact binding selected at normal completion,
                    // including calls retained inside an earlier value operation.
                    // Missing schedule entries and duplicate producers both refuse.
                    let mut producer = None;
                    for position in 0..producers.len() {
                        for candidate in producers.get(position)?.with_value_calls() {
                            if !candidate
                                .structural_result_binding()
                                .is_some_and(|result| result.binding_ordinal == binding_ordinal)
                            {
                                continue;
                            }
                            if producer.replace(candidate).is_some() {
                                return None;
                            }
                        }
                    }
                    let producer = producer?;
                    // Independent statement results may have no expression home.
                    // Contextual retirement below still requires an exact root.
                    super::normal_edges::producer_root(facts, state, producer)
                }
                CheckedUnitStructuralArgumentSourcePlan::StructuralExpression { expression } => {
                    Some(facts::PlaceRoot::Expression(expression))
                }
                _ => return None,
            };
            let contextual = root.is_some_and(|root| {
                facts
                    .contextual_cleanup
                    .any_state_disposition(machine, state, |_, row| {
                        row.root == root
                            && matches!(
                                row.source,
                                PermissionEventSource::Call {
                                    statement_index,
                                    call_ordinal,
                                    ..
                                } if statement_index == coordinate.statement_index as usize
                                    && call_ordinal == coordinate.call_ordinal as usize
                            )
                    })
            });
            if !contextual {
                // An existing independent disposal is not a contextual death.
                // An unresolved home-retirement request cannot borrow its authority.
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
            // A real retained release owns this complete source group, even when
            // it is later or nested in another value call's completion sequence.
            // Independent disposal above requires no contextual call association.
            let consumer = facts.flow.control.call_at(
                state,
                coordinate.statement_index as usize,
                coordinate.call_ordinal as usize,
            )?;
            if release_owned.iter().any(|handle| {
                let disposition = facts.contextual_cleanup.dispositions.get(*handle);
                disposition.machine == machine
                    && disposition.state == state
                    && Some(disposition.root) == root
                    && matches!(disposition.source,
                        PermissionEventSource::Call { statement_index, call_ordinal, target_symbol }
                        if target_symbol == consumer.target_symbol
                            && statement_index == coordinate.statement_index as usize
                            && call_ordinal == coordinate.call_ordinal as usize)
            }) {
                continue;
            }
            // Row presence authorizes selection. An admitted empty complement is
            // not the absence of a disposition, and its shape is not re-derived.
            completed.extend(selected_actions(
                program,
                facts,
                targets,
                shapes,
                machine,
                state,
                coordinate,
                consumer.target_symbol,
                root?,
                source.clone(),
            )?);
        }
        Some(SelectedActions {
            actions: completed,
            selected_regions: Vec::new(),
        })
    })();
    selected
}

enum OperationSequenceMut<'plans> {
    Body(&'plans mut [CheckedUnitEffectOperationPlan]),
    Operands(&'plans mut checked_trees::CheckedOperandSequence),
}

impl OperationSequenceMut<'_> {
    fn get_mut(&mut self, ordinal: usize) -> Option<&mut CheckedUnitEffectOperationPlan> {
        match self {
            Self::Body(operations) => operations.get_mut(ordinal),
            Self::Operands(sequence) => sequence.get_mut(ordinal),
        }
    }
}

pub(in crate::execution::terminal_unit) fn install(
    operations: &mut [CheckedUnitEffectOperationPlan],
    selected: OperationSelections,
) -> Option<()> {
    install_sequence(OperationSequenceMut::Body(operations), selected)
}

fn install_sequence(
    mut operations: OperationSequenceMut<'_>,
    selected: OperationSelections,
) -> Option<()> {
    for (position, actions) in selected.continuations {
        let CheckedUnitEffectOperationPlan::ContinuationCleanup {
            cleanup_actions,
            selected_regions,
            ..
        } = operations.get_mut(position)?
        else {
            return None;
        };
        *cleanup_actions = actions.actions;
        *selected_regions = actions.selected_regions;
    }
    for (position, selections) in selected.constructions {
        let calls = match operations.get_mut(position)? {
            CheckedUnitEffectOperationPlan::EstablishLocal(
                checked_trees::CheckedUnitLocalBindingPlan::StructuralValue { calls, .. },
            )
            | CheckedUnitEffectOperationPlan::EvaluateStructuralValue { calls, .. } => calls,
            _ => return None,
        };
        for (call_index, selected) in selections {
            install_value_call(calls.get_mut(call_index)?, selected)?;
        }
    }
    Some(())
}

fn install_value_call(
    call: &mut checked_trees::CheckedStructuralValueCall,
    selected: CallSelections,
) -> Option<()> {
    install_sequence(
        OperationSequenceMut::Operands(call.operands_mut()),
        selected.operands,
    )?;
    if let Some(completions) = call.completions_mut() {
        install_sequence(
            OperationSequenceMut::Operands(completions),
            selected.completions,
        )
    } else if selected.completions.is_empty() {
        Some(())
    } else {
        None
    }
}

/// Complete reads, computed calls and retained value calls from their checked
/// contextual dispositions.
/// Action storage is rebuilt once: scratch geometry never remains as a second
/// published cleanup stream beside the selected hooks.
pub(crate) fn finalize_completion_actions(
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
            .chain(
                facts
                    .flow
                    .terminal_partial_affine_unit_cleanups
                    .machines
                    .iter()
                    .map(|plan| &plan.machine),
            )
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
        finalize_value_completions(program, facts, &targets, &mut shapes)?;
        let mut value_calls = Vec::new();
        for (handle, call) in facts.values.structural_values.scheduled_calls() {
            let value = facts.values.structural_values.nodes.get(call.value);
            let parameters = facts
                .flow
                .terminal_unit_effects
                .machines
                .iter()
                .chain(
                    facts
                        .flow
                        .terminal_partial_affine_unit_cleanups
                        .machines
                        .iter()
                        .map(|plan| &plan.machine),
                )
                .find(|plan| plan.machine == value.machine && plan.state == value.state)
                .map(|plan| plan.structural_parameters.as_slice())
                .or_else(|| {
                    facts
                        .flow
                        .terminal_unit_effects
                        .composed_machines
                        .iter()
                        .filter(|plan| plan.machine == value.machine)
                        .flat_map(|plan| &plan.states)
                        .find(|state| state.state == value.state)
                        .map(|state| state.structural_parameters.as_slice())
                })?;
            let unit = &facts.flow.terminal_unit_effects;
            let release_owned = release_owned_dispositions(
                unit.machines
                    .iter()
                    .filter(|plan| plan.machine == value.machine && plan.state == value.state)
                    .flat_map(|plan| &plan.operations)
                    .chain(
                        unit.composed_machines
                            .iter()
                            .filter(|plan| plan.machine == value.machine)
                            .flat_map(|plan| &plan.states)
                            .filter(|state| state.state == value.state)
                            .flat_map(|state| &state.operations),
                    ),
                &shapes.cleanup_control,
            )?;
            let selected = value_call_actions(
                program,
                facts,
                &targets,
                &mut shapes,
                value.machine,
                value.state,
                parameters,
                call,
                &release_owned,
            )?;
            if !selected.is_empty() {
                value_calls.push((handle, selected));
            }
        }
        let plans = &facts.values.scalar_computations;
        let mut replacements = Vec::new();
        let mut observation_replacements = Vec::new();
        for (node_handle, node) in plans.nodes.iter() {
            if !node.normal_cleanup_actions.is_empty() {
                let receiver_value = node.kind.observed_receiver();
                if !facts
                    .values
                    .structural_values
                    .nodes
                    .is_valid(receiver_value)
                {
                    refusal = String::from("scalar observation cleanup lost its retained receiver");
                    return None;
                }
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
            struct ReservedOperand<'values> {
                argument_ordinal: Option<u32>,
                operand: arena::Handle<checked_trees::FlowOperandFact>,
                root: &'values checked_trees::CheckedStructuralValueRoot,
                dependencies: Vec<checked_trees::CheckedStructuralValueHandle>,
                retained_owners: Vec<facts::PlaceRoot>,
            }
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
                if !facts.flow.control.operands.is_valid(row.operand)
                    || !call
                        .operands
                        .handles()
                        .any(|operand| operand == row.operand)
                {
                    refusal = format!("{site}: temporary owner has no captured call operand");
                    return None;
                }
                let retained_owners = crate::flow::captured_lifetime_roots(
                    program,
                    &facts.flow.control,
                    facts.flow.control.operands.get(row.operand),
                );
                let Some(retained_owners) = retained_owners else {
                    refusal = format!(
                        "{site}: temporary operand has incomplete backing lifetime coverage"
                    );
                    if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
                        refusal.push_str(&incomplete_lifetime_capture_diagnostic(
                            program,
                            &facts.flow.control,
                            row.operand,
                        ));
                    }
                    return None;
                };
                reserved.push(ReservedOperand {
                    argument_ordinal: row.argument_ordinal,
                    operand: row.operand,
                    root,
                    dependencies: values.value_dependencies(root.root)?,
                    retained_owners,
                });
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
            // A materialized reference carrier has intrinsic lifetime cleanup,
            // whether it is a stored pointer or a formed slice descriptor.
            // Its actual completed root selects the carrier declaration; a
            // borrowed formal cannot turn an owned backing into a descriptor.
            for operand in &reserved {
                let root = operand.root;
                if validation::reference_shell(program, root.type_reference).is_some() {
                    shapes.add_type(root.type_reference, &[], &[])?;
                    let selected = super::descriptor_retirement(
                        program,
                        CheckedUnitStructuralArgumentSourcePlan::StructuralExpression {
                            expression: root.expression,
                        },
                        root.type_reference,
                    )?;
                    if !selected.is_empty() {
                        owners.push((
                            operand.argument_ordinal,
                            operand.operand,
                            root.root,
                            selected,
                        ));
                    }
                }
            }
            // The ownership producer has already selected the roots, paths and
            // exclusions at this exact consumer. Contained graph nodes grant no
            // additional death. Multiple regions of one owner stay one ordered
            // action group; two operands cannot retire that owner twice.
            for disposition_handle in facts
                .contextual_cleanup
                .state_disposition_handles(state_flow.machine_symbol, state_flow.state_symbol)
            {
                let disposition = facts
                    .contextual_cleanup
                    .dispositions
                    .get(disposition_handle);
                if !matches!(
                    disposition.source,
                    PermissionEventSource::Call {
                        statement_index,
                        call_ordinal,
                        ..
                    } if statement_index == call.statement_index
                        && call_ordinal == call.call_ordinal
                ) {
                    continue;
                }
                let facts::PlaceRoot::Expression(expression) = disposition.root else {
                    continue;
                };
                let root = values.root_for_expression(
                    state_flow.state_symbol,
                    coordinate.statement_index,
                    expression,
                );
                let Some(root) = root else {
                    // A named/local owner belongs to another cleanup stream.
                    // An anonymous backing reached by this operand must publish
                    // its real owner, not borrow a child's diagnostic location.
                    if reserved.iter().any(|operand| {
                        operand.retained_owners.contains(&disposition.root)
                            || operand
                                .dependencies
                                .iter()
                                .any(|value| values.nodes.get(*value).expression == expression)
                    }) {
                        refusal = format!(
                            "{site}: anonymous backing {expression:?} has no published completed root"
                        );
                        return None;
                    }
                    continue;
                };
                // Returned views keep their backing through captured lifetime
                // relations, not call-argument value edges. Both relations only
                // associate an already selected death with its actual capture.
                let mut containing = reserved.iter().filter(|operand| {
                    operand.dependencies.contains(&root.root)
                        || operand.retained_owners.contains(&disposition.root)
                });
                let Some(first) = containing.next() else {
                    continue;
                };
                let exact = reserved
                    .iter()
                    .filter(|operand| operand.root.root == root.root)
                    .collect::<Vec<_>>();
                let operand = match exact.as_slice() {
                    [slot] => *slot,
                    // Several actuals may retain one backing. The call retires
                    // that exact owner once, independent of this association.
                    [] => first,
                    _ => {
                        refusal = format!(
                            "{site}: completed owner {expression:?} has ambiguous reserved operand storage"
                        );
                        return None;
                    }
                };
                if owners.iter().any(|(_, _, owner, _)| *owner == root.root) {
                    // Descriptor lifetime and a nominal contextual death may
                    // not both dispose the same carrier.
                    if validation::reference_shell(program, root.type_reference).is_some() {
                        refusal = format!(
                            "{site}: descriptor retirement overlaps a contextual owner death for {expression:?}"
                        );
                        return None;
                    }
                    continue;
                }
                let actions = selected_actions(
                    program,
                    facts,
                    &targets,
                    &mut shapes,
                    state_flow.machine_symbol,
                    state_flow.state_symbol,
                    coordinate,
                    call.target_symbol,
                    disposition.root,
                    CheckedUnitStructuralArgumentSourcePlan::StructuralExpression { expression },
                )?;
                owners.push((
                    operand.argument_ordinal,
                    operand.operand,
                    root.root,
                    actions,
                ));
            }
            for operand in &reserved {
                if !owners
                    .iter()
                    .any(|(_, _, owner, _)| *owner == operand.root.root)
                {
                    refusal = format!(
                        "{site}: completed operand `{}` ({:?}) has no selected normal death",
                        program
                            .expression_table
                            .display_name(operand.root.expression),
                        operand.root.expression,
                    );
                    return None;
                }
            }
            let mut ordered = Vec::new();
            for (ordinal, operand, owner, actions) in owners {
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
                ordered.push((position, ordinal, operand, owner, actions));
            }
            ordered.sort_by_key(|(position, ..)| std::cmp::Reverse(*position));
            replacements.push((node_handle, ordered));
        }
        Some((replacements, value_calls, observation_replacements))
    })();
    facts.flow.terminal_unit_effects.cleanup_control = std::mem::take(&mut shapes.cleanup_control);
    let (replacements, value_calls, observation_replacements) = replacements.ok_or(refusal)?;
    for (handle, selected) in value_calls {
        install_value_call(
            facts
                .values
                .structural_values
                .scheduled_call_mut(handle)
                .ok_or("normal cleanup lost its scheduled value call")?,
            selected,
        )
        .ok_or("normal cleanup lost its value-call completion")?;
    }
    let mut actions = arena::Arena::new();
    let mut cleanups = arena::Arena::new();
    for (node_handle, owners) in replacements {
        let groups = owners
            .into_iter()
            .map(|(_, argument_ordinal, operand, owner, selected)| {
                checked_trees::CheckedScalarComputationTemporaryCleanup {
                    argument_ordinal,
                    operand,
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

/// Preserve disposition order across the descriptor and its backing owners.
/// Home retirement follows that home's last selected action, not the observer's
/// result kind or an inferred call argument.
fn finalize_value_completions(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    targets: &[CheckedUnitCallableEntry<'_>],
    shapes: &mut ShapeCollector<'_>,
) -> Option<()> {
    struct Completion {
        plan: checked_trees::CheckedValueCompletionPlan,
        actions: Vec<CheckedNormalCleanupAction>,
        retirements: Vec<CheckedNormalCleanupAction>,
    }
    let mut completions: Vec<Completion> = Vec::new();
    for (handle, disposition) in facts.contextual_cleanup.dispositions.iter() {
        if !disposition.completion.is_valid() {
            continue;
        }
        let PermissionEventSource::Statement { statement_index } = disposition.source else {
            return None;
        };
        let statement_index = u32::try_from(statement_index).ok()?;
        let expression = disposition.root.as_expression()?;
        let source = CheckedUnitStructuralArgumentSourcePlan::StructuralExpression { expression };
        let selected = disposition_actions(
            program,
            facts,
            targets,
            shapes,
            handle,
            source.clone(),
            None,
        )
        .ok()?;
        let retirement = super::actions::home_retirement(program, shapes, disposition, source)?;
        let position = completions
            .iter()
            .position(|completion| {
                let plan = &completion.plan;
                plan.machine == disposition.machine
                    && plan.state == disposition.state
                    && plan.statement_index == statement_index
                    && plan.expression == disposition.completion
            })
            .unwrap_or_else(|| {
                completions.push(Completion {
                    plan: checked_trees::CheckedValueCompletionPlan {
                        machine: disposition.machine,
                        state: disposition.state,
                        statement_index,
                        expression: disposition.completion,
                        actions: Default::default(),
                    },
                    actions: Vec::new(),
                    retirements: Vec::new(),
                });
                completions.len() - 1
            });
        let completion = &mut completions[position];
        completion.actions.extend(selected);
        if !completion
            .retirements
            .iter()
            .any(|existing| existing.source() == retirement.source())
        {
            completion.retirements.push(retirement);
        }
    }
    for mut completion in completions {
        for retirement in completion.retirements {
            super::actions::finish_home(&mut completion.actions, retirement);
        }
        if completion.actions.is_empty() {
            return None;
        }
        completion.plan.actions = shapes
            .cleanup_control
            .actions
            .insert_many(completion.actions);
        shapes
            .cleanup_control
            .value_completions
            .append(completion.plan);
    }
    Some(())
}

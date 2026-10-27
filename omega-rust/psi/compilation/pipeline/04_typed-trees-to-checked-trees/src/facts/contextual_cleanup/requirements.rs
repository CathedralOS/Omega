//! Check each selected death against the hook's complete callable predicates.
//! Formal observations rebase onto the exact live receiver at the selected
//! edge, then use the ordinary current-value predicate algebra. Unsupported
//! clauses fail closed; a missing Boolean recognizer cannot erase a requirement.
//! Generic entry transport uses that same complete predicate vocabulary.
//!
//! A fixed concrete owner in a generic signature is not a symbolic death. Only
//! storage under the source type parameter can introduce a bound obligation.
//! Its missing proposition must still denote the invocation's original value;
//! writes and non-entry locals cannot be repaired by assuming a fact at exit.
use validation::CanonicalPlace;

use checked_trees::{
    CheckFacts, CheckedBooleanExpression, CheckedStructuralPredicatePathSegment,
    ContextualCleanupEntryRequirement, ContextualCleanupEntryRequirements, FlowStateFact,
};
use diagnostics::Diagnostic;
use facts::{FactContextHandle, PlaceRoot, PlaceSegment};
use language_semantics::PermissionEventSource;
use symbols::SymbolHandle;
use typed_trees::{expression::ExpressionHandle, types::TypeReferenceNode};

use crate::checks::contracts::{boolean_value_at_place, contextual_cleanup as calls};
use crate::execution::terminal_unit::cleanup::{
    checked_requires_expressions, direct_boolean_field_requirement,
};
use crate::flow::{CleanupLeaf, selected_cleanup_leaves};

use super::super::boolean_predicates::{self, BoundPredicate, conjunction, entry_path_condition};
use super::sequence::{CurrentCleanupFacts, bound_clause, ordered_hooks};

struct Pending {
    machine: SymbolHandle,
    consequence: CheckedBooleanExpression,
    antecedent: CheckedBooleanExpression,
    hook_machine: SymbolHandle,
    hook_clause: ExpressionHandle,
    template_machine: SymbolHandle,
}

pub(crate) fn infer_entry_requirements(
    program: &dyn ::validation::CheckingInput,
    facts: &mut CheckFacts,
    frames: Option<&validation::CallFrameResolver<'_>>,
) -> Result<bool, Vec<Diagnostic>> {
    let mut pending = Vec::new();
    for obligation in selected_requirements(program, facts, frames)? {
        let place = &obligation.place;
        let instance = facts
            .contextual_cleanup
            .dispositions
            .get(obligation.disposition);
        let introduced =
            facts
                .contextual_cleanup
                .applications
                .iter()
                .find_map(|(_, application)| {
                    if application.instance != obligation.disposition {
                        return None;
                    }
                    let template = facts
                        .contextual_cleanup
                        .dispositions
                        .get(application.template);
                    let leaves = selected_cleanup_leaves(program, facts, template)?;
                    let path = super::source_path(program, &place.segments, instance.machine);
                    leaves
                        .iter()
                        .any(|leaf| {
                            matches!(leaf,
                CleanupLeaf::Symbolic { path: symbolic, .. } if covers(symbolic, &path))
                        })
                        .then_some(template.machine)
                });
        let Some(template) = introduced else { continue };
        if obligation.proven(program, facts) {
            continue;
        }
        if !obligation.entry_stable {
            continue;
        }
        let Some(antecedent) = entry_path_condition(
            &facts.semantic,
            &obligation.current.contexts,
            instance.machine,
        ) else {
            continue;
        };
        if let Some(row) = transport(
            program,
            instance.machine,
            instance.state,
            obligation.statement,
            &obligation.predicate,
            antecedent,
            obligation.hook,
            obligation.clause,
            template,
        ) {
            pending.push(row);
        }
    }
    // Generic forwarding carries the same obligation through the exact actual
    // place. It cannot turn a fixed-type local or unrelated parameter into a
    // new public demand merely because another parameter happens to be generic.
    if !facts.contextual_cleanup.entry_requirements.rows.is_empty() {
        for (_, state) in facts.flow.control.states.iter() {
            let Some(specialization) = program.machine_specialization(state.machine_symbol) else {
                continue;
            };
            for call in facts.flow.control.calls.span_or_empty(state.calls) {
                let Some(target) = calls::target_machine(program, call) else {
                    continue;
                };
                if !facts
                    .contextual_cleanup
                    .entry_requirements
                    .rows
                    .iter()
                    .any(|(_, row)| row.machine == target)
                {
                    continue;
                }
                let contexts: Vec<_> = facts
                    .flow
                    .state_call_entry_semantic_contexts(
                        state,
                        call.statement_index,
                        call.call_ordinal,
                        call.target_symbol,
                        call.receiver_symbol,
                    )
                    .collect();
                for (_, requirement) in facts
                    .contextual_cleanup
                    .entry_requirements
                    .rows
                    .iter()
                    .filter(|(_, row)| row.machine == target)
                {
                    if calls::proves(program, facts, state, call, &contexts, requirement, frames) {
                        continue;
                    }
                    let Some(place) = calls::actual_place(program, state, call, requirement) else {
                        continue;
                    };
                    if !has_symbolic_origin(program, state, call.statement_index, &place) {
                        continue;
                    }
                    let Some(path_condition) =
                        entry_path_condition(&facts.semantic, &contexts, state.machine_symbol)
                    else {
                        continue;
                    };
                    let Some(callee_condition) =
                        calls::entry_antecedent(program, facts, state, call, requirement)
                    else {
                        continue;
                    };
                    let Some(consequence) =
                        calls::entry_consequence(program, facts, state, call, requirement)
                    else {
                        continue;
                    };
                    pending.push(Pending {
                        machine: state.machine_symbol,
                        consequence,
                        antecedent: conjunction(path_condition, callee_condition),
                        hook_machine: requirement.hook_machine,
                        hook_clause: requirement.hook_clause,
                        template_machine: specialization.template,
                    });
                }
            }
        }
    }
    let mut changed = false;
    for row in pending {
        changed |= append(&mut facts.contextual_cleanup.entry_requirements, row)?;
    }
    Ok(changed)
}

struct SelectedRequirement {
    disposition: arena::Handle<checked_trees::ContextualCleanupDisposition>,
    statement: usize,
    current: CurrentCleanupFacts,
    entry_stable: bool,
    place: CanonicalPlace,
    hook: SymbolHandle,
    clause: ExpressionHandle,
    predicate: BoundPredicate,
}

impl SelectedRequirement {
    fn proven(&self, program: &dyn ::validation::CheckingInput, facts: &CheckFacts) -> bool {
        let disposition = facts.contextual_cleanup.dispositions.get(self.disposition);
        let point = facts::ProgramPoint::Statement {
            machine_symbol: disposition.machine,
            state_symbol: disposition.state,
            statement_index: self.statement,
        };
        let mut retained = boolean_predicates::retained(
            program,
            &facts.operators,
            &facts.semantic,
            &self.current.contexts,
            point,
        );
        retained.extend(self.current.guarantees.iter().cloned());
        let mut required = self.predicate.clone();
        if let Some(state) = facts
            .flow
            .control
            .state(disposition.machine, disposition.state)
        {
            let transported = crate::checks::contracts::cleanup_predicates(
                program,
                facts,
                state,
                &self.current.contexts,
                &mut required,
                &retained,
                |binding| {
                    self.current.entry_stable(
                        program,
                        &BoundPredicate {
                            bindings: vec![binding.clone()],
                            ..self.predicate.clone()
                        },
                    )
                },
            );
            if transported.proves_required {
                return true;
            }
            retained.extend(transported.predicates);
        }
        crate::checks::contracts::proves_captured_predicates(
            program,
            &facts.semantic,
            &self.current.contexts,
            &retained,
            &required,
            |place| {
                self.current
                    .value_at_place(program, facts, point, place)
                    .or_else(|| {
                        boolean_value_at_place(
                            program,
                            &facts.operators,
                            &facts.semantic,
                            &self.current.contexts,
                            point,
                            place,
                        )
                        .map(facts::ScalarValue::Boolean)
                        .or_else(|| {
                            crate::values::scalar_value_at_place(
                                program,
                                &facts.semantic,
                                self.current
                                    .contexts
                                    .iter()
                                    .map(|context| facts.semantic.contexts.get(*context)),
                                place,
                            )
                        })
                    })
            },
        )
    }

    fn transportable_subject(
        &self,
        program: &dyn ::validation::CheckingInput,
    ) -> Option<(CanonicalPlace, bool)> {
        let hook = program.machine_by_symbol(self.hook)?;
        let state = program.machine_states(hook).first()?;
        let receiver = program.state_parameters(state).first()?;
        let requirement =
            direct_boolean_field_requirement(program, state.symbol, receiver, self.clause)?;
        let (_, path, _, _) = crate::values::resolve_structural_parameter_path(
            program,
            program.state_parameters(state),
            0,
            &[CheckedStructuralPredicatePathSegment::Field(
                requirement.field_identity,
            )],
        )?;
        let mut place = self.place.clone();
        place.segments.extend(path);
        Some((place, requirement.expected))
    }
}

// Selection and actual-edge substitution are shared by inference and final
// source checking. A Terminal planner declining a shape must not silently
// authorize cleanup whose source premise was never established.
fn selected_requirements(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    shared_call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Result<Vec<SelectedRequirement>, Vec<Diagnostic>> {
    struct Hook {
        disposition: arena::Handle<checked_trees::ContextualCleanupDisposition>,
        selected: bool,
        receiver: super::sequence::SelectedHook,
    }
    struct Group<'a> {
        state: &'a FlowStateFact,
        source: PermissionEventSource,
        exit: checked_trees::FlowClaimJoinExit,
        root: PlaceRoot,
        statement: usize,
        contexts: Vec<FactContextHandle>,
        hooks: Vec<Hook>,
    }
    struct Occurrence {
        machine: SymbolHandle,
        state: SymbolHandle,
        source: PermissionEventSource,
        exit: checked_trees::FlowClaimJoinExit,
        statement: usize,
        entry: Vec<FactContextHandle>,
        current: CurrentCleanupFacts,
    }
    if facts.contextual_cleanup.dispositions.is_empty() {
        return Ok(Vec::new());
    }
    let mut result = Vec::new();
    let proof_only = validation::proof_only_classification(program);
    let summaries = crate::flow::StateMutationSummaryCache::default();
    let mut owned_frames = None;
    let call_frames =
        crate::flow::shared_call_frames_or(shared_call_frames, program, &mut owned_frames);
    let mut groups = Vec::<Group<'_>>::new();
    for (handle, disposition) in facts.contextual_cleanup.dispositions.iter() {
        let Some(machine) = program.machine_by_symbol(disposition.machine) else {
            continue;
        };
        if proof_only.is_proof_machine(program, machine) {
            continue;
        }
        let Some(state) = facts
            .flow
            .control
            .state(disposition.machine, disposition.state)
        else {
            continue;
        };
        let Some(leaves) = selected_cleanup_leaves(program, facts, disposition) else {
            continue;
        };
        if leaves.is_empty() {
            continue;
        }
        let path = facts
            .flow
            .ownership
            .segments
            .span_or_empty(disposition.path);
        let hooks = ordered_hooks(program, path, &leaves).ok_or_else(|| {
            vec![Diagnostic::error(
                "cannot prove automatic cleanup requires: selected receiver order is unresolved",
            )]
        })?;
        if hooks.is_empty() {
            continue;
        }
        let selected = facts.contextual_cleanup.is_selected_disposition(handle);
        let points = occurrence_contexts(program, facts, state, disposition).ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "cleanup prerequisite has no exact outgoing edge context in {}",
                crate::labels::machine_name(program, disposition.machine)
            ))]
        })?;
        for (statement, contexts) in points {
            let position = groups
                .iter()
                .position(|group| {
                    group.state.machine_symbol == disposition.machine
                        && group.state.state_symbol == disposition.state
                        && group.source == disposition.source
                        && group.exit == disposition.exit
                        && group.root == disposition.root
                        && group.statement == statement
                        && group.contexts == contexts
                })
                .unwrap_or_else(|| {
                    let position = groups.len();
                    groups.push(Group {
                        state,
                        source: disposition.source,
                        exit: disposition.exit,
                        root: disposition.root,
                        statement,
                        contexts,
                        hooks: Vec::new(),
                    });
                    position
                });
            groups[position]
                .hooks
                .extend(hooks.iter().cloned().map(|receiver| Hook {
                    disposition: handle,
                    selected,
                    receiver,
                }));
        }
    }
    let mut occurrences = Vec::<Occurrence>::new();
    for mut group in groups {
        // An ordinary complement may lie on both sides of a selected region.
        // Merge their actual hooks, retaining each optional subtree as one
        // contiguous group. Untouched construction schedules keep source order.
        if group.hooks.iter().any(|hook| hook.selected) {
            let mut ordered = group.hooks.into_iter().map(|hook| {
                crate::flow::cleanup_path_order(program, &hook.receiver.path)
                    .map(|order| (order, hook))
            }).collect::<Option<Vec<_>>>().ok_or_else(|| vec![Diagnostic::error(
                "cannot prove automatic cleanup requires: selected partition order is unresolved",
            )])?;
            ordered.sort_by(|left, right| left.0.cmp(&right.0));
            group.hooks = ordered.into_iter().map(|(_, hook)| hook).collect();
        }
        let state = group.state;
        let statement = group.statement;
        let position = occurrences
            .iter()
            .position(|row| {
                row.machine == state.machine_symbol
                    && row.state == state.state_symbol
                    && row.source == group.source
                    && row.exit == group.exit
                    && row.statement == statement
                    && row.entry == group.contexts
            })
            .unwrap_or_else(|| {
                let position = occurrences.len();
                occurrences.push(Occurrence {
                    machine: state.machine_symbol,
                    state: state.state_symbol,
                    source: group.source,
                    exit: group.exit,
                    statement,
                    entry: group.contexts.clone(),
                    current: CurrentCleanupFacts::new(
                        group.contexts.clone(),
                        call_frames
                            .as_ref()
                            .map(|frames| {
                                crate::checks::contracts::current_call_guarantee_predicates(
                                    program,
                                    facts,
                                    state,
                                    // State entry has evaluated no operands.
                                    // The other selected contexts are exact
                                    // pre-store, normal-call or outgoing edges.
                                    match group.source {
                                        PermissionEventSource::StateEntry => crate::checks::contracts::GuaranteeUse::StatementEntry(statement),
                                        _ => crate::checks::contracts::GuaranteeUse::EvaluatedContext(statement),
                                    },
                                    &group.contexts,
                                    frames,
                                )
                            })
                            .unwrap_or_default(),
                    ),
                });
                position
            });
        let current = &mut occurrences[position].current;
        let mut offset = 0;
        while offset < group.hooks.len() {
            let first = &group.hooks[offset];
            let mut end = offset + 1;
            if first.selected {
                while end < group.hooks.len() && group.hooks[end].disposition == first.disposition {
                    end += 1;
                }
            }
            let skipped = first.selected.then(|| current.clone());
            for item in &group.hooks[offset..end] {
                let handle = item.disposition;
                let disposition = facts.contextual_cleanup.dispositions.get(handle);
                let selected = &item.receiver;
                let hook = selected.machine;
                let mut place = CanonicalPlace {
                    root: disposition.root,
                    segments: selected.path.clone(),
                };
                // This is the captured subject, not a may-effect range set.
                // Immutable selector copies retain their exact value at this
                // edge even when a later operand changes their former source.
                crate::flow::narrow_pinned_index_segments(
                    program,
                    &facts.semantic,
                    state.state_symbol,
                    statement,
                    &current.contexts,
                    &mut place.segments,
                );
                crate::flow::compose_captured_view_segments(&mut place.segments).ok_or_else(
                    || {
                        vec![Diagnostic::error(
                            "selected cleanup receiver has invalid captured view coordinates",
                        )]
                    },
                )?;
                let hook_machine = program
                    .machine_by_symbol(hook)
                    .ok_or_else(|| vec![Diagnostic::error("selected cleanup hook is absent")])?;
                let hook_state = program
                    .machine_states(hook_machine)
                    .first()
                    .ok_or_else(|| {
                        vec![Diagnostic::error("selected cleanup hook entry is absent")]
                    })?;
                let clauses = checked_requires_expressions(program, facts, hook, hook_state.symbol).ok_or_else(|| vec![Diagnostic::error(format!(
                    "cannot prove automatic cleanup requires: {} has no complete callable clause roster",
                    crate::labels::machine_name(program, hook),
                ))])?;
                for clause in clauses {
                    let predicate = bound_clause(program, facts, hook, clause, checked_trees::ContractProofFactKind::Requires, &place).ok_or_else(|| vec![Diagnostic::error(format!(
                        "cannot prove automatic cleanup requires: {} clause `{}` has no complete callable predicate",
                        crate::labels::machine_name(program, hook), program.expression_table.display_name(clause),
                    ))])?;
                    result.push(SelectedRequirement {
                        disposition: handle,
                        statement,
                        entry_stable: current.entry_stable(program, &predicate),
                        current: current.clone(),
                        place: place.clone(),
                        hook,
                        clause,
                        predicate,
                    });
                }
                current.advance(program, facts, &summaries, call_frames, hook, &place).ok_or_else(|| vec![Diagnostic::error(
                    "cannot prove automatic cleanup requires: selected hook completion is unresolved",
                )])?;
            }
            if let Some(skipped) = skipped {
                current.join_skipped(&skipped);
            }
            offset = end;
        }
    }
    Ok(result)
}

pub(crate) fn check_cleanup_requirements(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
) -> Vec<Diagnostic> {
    let selected = match selected_requirements(program, facts, None) {
        Ok(selected) => selected,
        Err(diagnostics) => return diagnostics,
    };
    selected.into_iter().filter_map(|obligation| {
        if obligation.proven(program, facts) { return None; }
        let disposition = facts.contextual_cleanup.dispositions.get(obligation.disposition);
        if let Some((place, expected)) = obligation.transportable_subject(program) {
            return Some(Diagnostic::error(format!(
                "cannot prove automatic cleanup requires at selected edge from {} state {} after statement {}: missing {} == {} required by {} clause `{}`",
                crate::labels::machine_name(program, disposition.machine),
                program.symbols.name(disposition.state), obligation.statement,
                place.label(program), expected,
                crate::labels::machine_name(program, obligation.hook),
                program.expression_table.display_name(obligation.clause),
            )));
        }
        Some(Diagnostic::error(format!(
            "cannot prove automatic cleanup requires at selected edge from {} state {} after statement {}: {} does not establish {} clause `{}`",
            crate::labels::machine_name(program, disposition.machine),
            program.symbols.name(disposition.state), obligation.statement,
            obligation.place.label(program),
            crate::labels::machine_name(program, obligation.hook),
            program.expression_table.display_name(obligation.clause),
        )))
    }).collect()
}

fn transport(
    program: &dyn ::validation::CheckingInput,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement: usize,
    predicate: &BoundPredicate,
    antecedent: CheckedBooleanExpression,
    hook_machine: SymbolHandle,
    hook_clause: ExpressionHandle,
    template_machine: SymbolHandle,
) -> Option<Pending> {
    let consequence =
        boolean_predicates::entry_expression(program, machine, state, statement, predicate)?;
    Some(Pending {
        machine,
        consequence,
        antecedent,
        hook_machine,
        hook_clause,
        template_machine,
    })
}

fn append(
    requirements: &mut ContextualCleanupEntryRequirements,
    row: Pending,
) -> Result<bool, Vec<Diagnostic>> {
    for (_, prior) in requirements.rows.iter() {
        if prior.machine == row.machine && prior.antecedent == row.antecedent {
            let opposed = matches!(&prior.consequence,
                CheckedBooleanExpression::Not(subject) if subject.as_ref() == &row.consequence)
                || matches!(&row.consequence,
                    CheckedBooleanExpression::Not(subject) if subject.as_ref() == &prior.consequence);
            if opposed && row.antecedent == CheckedBooleanExpression::Constant(true) {
                return Err(vec![Diagnostic::error(
                    "contextual cleanup has contradictory prerequisites for the same invocation-entry place",
                )]);
            }
            if prior.consequence == row.consequence {
                return Ok(false);
            }
        }
    }
    requirements.rows.append(ContextualCleanupEntryRequirement {
        machine: row.machine,
        consequence: row.consequence,
        antecedent: row.antecedent,
        hook_machine: row.hook_machine,
        hook_clause: row.hook_clause,
        template_machine: row.template_machine,
    });
    Ok(true)
}

fn covers(symbolic: &[PlaceSegment], concrete: &[PlaceSegment]) -> bool {
    symbolic.len() <= concrete.len()
        && symbolic
            .iter()
            .zip(concrete)
            .all(|(left, right)| match (*left, *right) {
                (PlaceSegment::FixedRange { start, end }, PlaceSegment::FixedIndex { index }) => {
                    (start..end).contains(&index)
                }
                (
                    PlaceSegment::FixedRange { start, end },
                    PlaceSegment::FixedRange {
                        start: right_start,
                        end: right_end,
                    },
                ) => start <= right_start && right_end <= end,
                _ => left == right,
            })
}

fn has_symbolic_origin(
    program: &dyn ::validation::CheckingInput,
    state: &FlowStateFact,
    statement: usize,
    place: &CanonicalPlace,
) -> bool {
    let Some(root) = place.root.as_symbol() else {
        return false;
    };
    let root = super::source_symbol(program, root, state.machine_symbol);
    let source_state = super::source_symbol(program, state.state_symbol, state.machine_symbol);
    let path = super::source_path(program, &place.segments, state.machine_symbol);
    for length in 0..=path.len() {
        let source = CanonicalPlace {
            root: PlaceRoot::Symbol(root),
            segments: path[..length].to_vec(),
        };
        if let Some(reference) =
            validation::canonical_place_type_reference(program, source_state, statement, &source)
        {
            let reference = program
                .type_reference_table
                .unconstrained_type_reference(reference);
            if let TypeReferenceNode::Named { symbol, .. } =
                program.type_reference_table.type_reference(reference)
                && program.symbols.get(*symbol).kind == symbols::SymbolKind::TypeParameter
            {
                return true;
            }
        }
    }
    false
}

pub(crate) fn occurrence_contexts(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state: &FlowStateFact,
    disposition: &checked_trees::ContextualCleanupDisposition,
) -> Option<Vec<(usize, Vec<FactContextHandle>)>> {
    if disposition.completion.is_valid() {
        let PermissionEventSource::Statement { statement_index } = disposition.source else {
            return None;
        };
        facts.flow.state_statement(state, statement_index)?;
        let constraints = disposition.completion_constraints?;
        facts.flow.contexts.constraint_refs.span(constraints)?;
        return Some(vec![(
            statement_index,
            facts
                .flow
                .semantic_constraint_contexts(constraints)
                .collect(),
        )]);
    }
    if disposition.selection.is_valid() {
        // The receiving complement owns the captured pre-transfer occurrence.
        // A new value later installed under its source symbol cannot prove
        // prerequisites for that anonymous old owner.
        let receipt = facts
            .flow
            .ownership
            .owned_selections
            .get(disposition.selection);
        if receipt.machine != disposition.machine
            || receipt.state != disposition.state
            || !facts
                .flow
                .ownership
                .selection_sources
                .span_or_empty(receipt.sources)
                .iter()
                .any(|source| Some(source.symbol) == disposition.root.as_symbol())
        {
            return None;
        }
        let statement = receipt.statement_ordinal as usize;
        if !facts.flow.control.operands.is_valid(receipt.value) {
            return None;
        }
        let value = facts.flow.control.operands.get(receipt.value);
        if value.state_symbol != receipt.state
            || value.statement_index != statement
            || value.expression != receipt.expression
            || value.type_reference != receipt.type_reference
        {
            return None;
        }
        // Completion already filtered facts invalidated by operand effects.
        // Reconstructing this cut from the statement would use a transition's
        // missed-guard facts or a newer occupant of the same source home.
        return Some(vec![(
            statement,
            facts
                .flow
                .semantic_constraint_contexts(value.constraints)
                .collect(),
        )]);
    }
    let points = match disposition.source {
        PermissionEventSource::InstructionDisposition { .. }
        | PermissionEventSource::InstructionExitDisposition { .. } => return None,
        PermissionEventSource::StateEntry => vec![(
            0,
            facts
                .flow
                .semantic_constraint_contexts(state.entry_constraints)
                .collect(),
        )],
        PermissionEventSource::Statement { statement_index } => facts
            .flow
            .state_statement(state, statement_index)
            .map(|statement| {
                vec![(
                    statement_index,
                    facts
                        .flow
                        .semantic_constraint_contexts(statement.pre_store_constraints)
                        .collect(),
                )]
            })
            .unwrap_or_default(),
        PermissionEventSource::Call {
            statement_index,
            call_ordinal,
            target_symbol,
        } => facts
            .flow
            .control
            .calls
            .span_or_empty(state.calls)
            .iter()
            .filter(|call| {
                call.statement_index == statement_index
                    && call.call_ordinal == call_ordinal
                    && call.target_symbol == target_symbol
            })
            .map(|call| {
                (
                    statement_index,
                    facts
                        .flow
                        .semantic_constraint_contexts(call.exit_constraints)
                        .collect(),
                )
            })
            .collect(),
        PermissionEventSource::StateExit => {
            let coordinate = disposition.exit;
            if coordinate.state_symbol != state.state_symbol || !coordinate.state_symbol.is_valid()
            {
                return None;
            }
            let target = match coordinate.kind {
                checked_trees::FlowClaimJoinExitKind::FinalExpression => Default::default(),
                checked_trees::FlowClaimJoinExitKind::TransitionTarget
                | checked_trees::FlowClaimJoinExitKind::TransitionContinuation => {
                    let Some(typed_state) = program.state_by_symbol(state.state_symbol) else {
                        return None;
                    };
                    let Some(typed_trees::statement::StatementNode::Transition(transition)) =
                        program
                            .statement_table
                            .statement_at(typed_state, coordinate.statement_index)
                    else {
                        return None;
                    };
                    let target = if coordinate.kind
                        == checked_trees::FlowClaimJoinExitKind::TransitionTarget
                    {
                        transition.target
                    } else {
                        transition.continuation
                    };
                    if !target.is_valid() {
                        return None;
                    }
                    target
                }
                checked_trees::FlowClaimJoinExitKind::CallResult { .. }
                | checked_trees::FlowClaimJoinExitKind::SelectionArm { .. }
                | checked_trees::FlowClaimJoinExitKind::SelectionDecision { .. } => return None,
            };
            // A successor loan retains its owned backing until that
            // invocation returns. Check its cleanup after transitive writes
            // and checked normal guarantees, not at argument capture. Exact
            // authored arguments distinguish sibling calls to the same state.
            if target.is_valid()
                && let typed_trees::statement::TransitionTargetNode::Named {
                    path, arguments, ..
                } = program.statement_table.transition_target(target)
            {
                let arguments = program.statement_table.expression_handles(*arguments);
                let mut calls = state.calls.handles().filter(|handle| {
                    let call = facts.flow.control.calls.get(*handle);
                    call.statement_index == coordinate.statement_index
                        && call.authored_site == checked_trees::FlowCallSite::NamedTransition
                        && call.authored_target == path.symbol
                        && facts
                            .flow
                            .control
                            .call_arguments
                            .span_or_empty(call.authored_arguments)
                            == arguments
                        && (facts
                                .borrow
                                .argument_accesses
                                .span_or_empty(call.accesses)
                                .iter()
                                .any(|access| {
                                    access.root == disposition.root
                                        && access.end_before_access.is_none()
                                })
                                // A stored carrier can name another symbol.
                                // Its captured invocation resource retains the
                                // original backing, including reference leaves
                                // inside completed aggregate operands. Never
                                // recover that identity from initializer text.
                                || facts.borrow.call_loan_resources.iter().any(|(_, resource)| {
                                    resource.call == *handle
                                        && resource.machine_symbol == state.machine_symbol
                                        && resource.state_symbol == state.state_symbol
                                        && resource.parent_lifetime.root == disposition.root
                                        && call.argument_operands().handles().any(|operand| {
                                            operand == resource.owner_operand
                                        })
                                }))
                });
                if let Some(call) = calls.next() {
                    if calls.next().is_some() {
                        return None;
                    }
                    let call = facts.flow.control.calls.get(call);
                    return Some(vec![(
                        coordinate.statement_index,
                        facts
                            .flow
                            .semantic_constraint_contexts(call.exit_constraints)
                            .collect(),
                    )]);
                }
            }
            facts
                .flow
                .control
                .outgoing
                .span_or_empty(state.outgoing)
                .iter()
                .filter(|exit| {
                    exit.statement_index == coordinate.statement_index
                        && exit.transition_target == target
                })
                .map(|exit| {
                    (
                        exit.statement_index,
                        facts
                            .flow
                            .semantic_constraint_contexts(exit.constraints)
                            .collect(),
                    )
                })
                .collect()
        }
    };
    (!points.is_empty()).then_some(points)
}

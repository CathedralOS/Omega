//! Bind a symbolic generic death to the prerequisites of its substituted hook.
//!
//! A fixed concrete owner in a generic signature is not a symbolic death. Only
//! storage under the source type parameter can introduce a bound obligation.
//! Its missing proposition must still denote the invocation's original value;
//! writes and non-entry locals cannot be repaired by assuming a fact at exit.
use checked_trees::{
    CheckFacts, CheckedStructuralPredicatePathSegment, ContextualCleanupEntryRequirement,
    ContextualCleanupEntryRequirements, FlowStateFact,
};
use diagnostics::Diagnostic;
use facts::{FactContextHandle, PlaceRoot, PlaceSegment};
use language_semantics::PermissionEventSource;
use symbols::SymbolHandle;
use typed_trees::{TypedTrees, expression::ExpressionHandle, types::TypeReferenceNode};

use crate::checks::contracts::{boolean_value_at_place, contextual_cleanup as calls};
use crate::execution::terminal_unit::cleanup::{
    checked_requires_expressions, direct_boolean_field_requirement,
};
use crate::flow::{CanonicalPlace, CleanupLeaf, selected_cleanup_leaves};

struct Pending {
    machine: SymbolHandle,
    parameter_position: u32,
    path: Vec<CheckedStructuralPredicatePathSegment>,
    expected: bool,
    hook_machine: SymbolHandle,
    hook_clause: ExpressionHandle,
    template_machine: SymbolHandle,
}

pub(crate) fn infer_entry_requirements(
    program: &TypedTrees,
    facts: &mut CheckFacts,
) -> Result<bool, Vec<Diagnostic>> {
    let mut pending = Vec::new();
    for obligation in selected_requirements(program, facts)? {
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
                    let path =
                        super::source_path(program, &obligation.place.segments, instance.machine);
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
        if let Some(row) = transport(
            program,
            instance.machine,
            instance.state,
            obligation.statement,
            &obligation.place,
            obligation.expected,
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
    for (_, state) in facts.flow.control.states.iter() {
        let Some(specialization) = program
            .machine_specializations
            .iter()
            .find(|specialization| specialization.instance == state.machine_symbol)
        else {
            continue;
        };
        for call in facts.flow.control.calls.span_or_empty(state.calls) {
            let Some(target) = calls::target_machine(program, call) else {
                continue;
            };
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
                if calls::proves(program, facts, state, call, &contexts, requirement) {
                    continue;
                }
                let Some(place) = calls::actual_place(
                    program,
                    state,
                    call,
                    requirement,
                    &facts.contextual_cleanup.entry_requirements.paths,
                ) else {
                    continue;
                };
                if !has_symbolic_origin(program, state, call.statement_index, &place) {
                    continue;
                }
                if let Some(row) = transport(
                    program,
                    state.machine_symbol,
                    state.state_symbol,
                    call.statement_index,
                    &place,
                    requirement.expected,
                    requirement.hook_machine,
                    requirement.hook_clause,
                    specialization.template,
                ) {
                    pending.push(row);
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
    contexts: Vec<FactContextHandle>,
    place: CanonicalPlace,
    hook: SymbolHandle,
    clause: ExpressionHandle,
    expected: bool,
}

impl SelectedRequirement {
    fn proven(&self, program: &TypedTrees, facts: &CheckFacts) -> bool {
        boolean_value_at_place(program, &facts.semantic, &self.contexts, &self.place)
            == Some(self.expected)
    }
}

// Selection and actual-edge substitution are shared by inference and final
// source checking. A Terminal planner declining a shape must not silently
// authorize cleanup whose source premise was never established.
fn selected_requirements(
    program: &TypedTrees,
    facts: &CheckFacts,
) -> Result<Vec<SelectedRequirement>, Vec<Diagnostic>> {
    let mut result = Vec::new();
    let proof_only = validation::proof_only_classification(program);
    for (handle, disposition) in facts.contextual_cleanup.dispositions.iter() {
        let Some(machine) = program.machine_by_symbol(disposition.machine) else {
            continue;
        };
        if proof_only.is_proof_machine(program, machine) {
            continue;
        }
        let Some((_, state)) = facts.flow.control.states.iter().find(|(_, state)| {
            state.machine_symbol == disposition.machine && state.state_symbol == disposition.state
        }) else {
            continue;
        };
        let points = occurrence_contexts(program, facts, state, disposition);
        let Some(leaves) = selected_cleanup_leaves(program, facts, disposition) else {
            continue;
        };
        for leaf in leaves {
            let CleanupLeaf::Nominal {
                machine: hook,
                path,
            } = leaf
            else {
                continue;
            };
            let Some(hook_machine) = program.machine_by_symbol(hook) else {
                continue;
            };
            let Some(hook_state) = program.machine_states(hook_machine).first() else {
                continue;
            };
            let parameters = program.state_parameters(hook_state);
            let Some(receiver) = parameters.first() else {
                continue;
            };
            let Some(clauses) =
                checked_requires_expressions(program, facts, hook, hook_state.symbol)
            else {
                continue;
            };
            for clause in clauses {
                let Some(requirement) =
                    direct_boolean_field_requirement(program, hook_state.symbol, receiver, clause)
                else {
                    continue;
                };
                let Some((_, field_path, _, _)) = crate::values::resolve_structural_parameter_path(
                    program,
                    parameters,
                    0,
                    &[CheckedStructuralPredicatePathSegment::Field(
                        requirement.field_identity,
                    )],
                ) else {
                    continue;
                };
                let points = points.as_ref().ok_or_else(|| {
                    vec![Diagnostic::error(format!(
                        "cleanup prerequisite has no exact outgoing edge context in {}",
                        crate::labels::machine_name(program, disposition.machine),
                    ))]
                })?;
                let mut selected = path.clone();
                selected.extend(field_path);
                for_each_place(&selected, &mut Vec::new(), &mut |segments| {
                    for (statement, contexts) in points {
                        result.push(SelectedRequirement {
                            disposition: handle,
                            statement: *statement,
                            contexts: contexts.clone(),
                            place: CanonicalPlace {
                                root: disposition.root,
                                segments: segments.to_vec(),
                            },
                            hook,
                            clause,
                            expected: requirement.expected,
                        });
                    }
                });
            }
        }
    }
    Ok(result)
}

pub(crate) fn check_cleanup_requirements(
    program: &TypedTrees,
    facts: &CheckFacts,
) -> Vec<Diagnostic> {
    let selected = match selected_requirements(program, facts) {
        Ok(selected) => selected,
        Err(diagnostics) => return diagnostics,
    };
    selected.into_iter().filter_map(|obligation| {
        if obligation.proven(program, facts) { return None; }
        let disposition = facts.contextual_cleanup.dispositions.get(obligation.disposition);
        Some(Diagnostic::error(format!(
            "cannot prove automatic cleanup requires at selected edge from {} state {} after statement {}: missing {} == {} required by {} clause `{}`",
            crate::labels::machine_name(program, disposition.machine),
            program.symbols.name(disposition.state), obligation.statement,
            obligation.place.label(program), obligation.expected,
            crate::labels::machine_name(program, obligation.hook),
            program.expression_table.display_name(obligation.clause),
        )))
    }).collect()
}

fn transport(
    program: &TypedTrees,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement: usize,
    place: &CanonicalPlace,
    expected: bool,
    hook_machine: SymbolHandle,
    hook_clause: ExpressionHandle,
    template_machine: SymbolHandle,
) -> Option<Pending> {
    let (parameter_position, path) = super::super::crash_entry_values::entry_parameter_place(
        program, machine, state, statement, place,
    )?;
    Some(Pending {
        machine,
        parameter_position,
        path,
        expected,
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
        if prior.machine == row.machine
            && prior.parameter_position == row.parameter_position
            && requirements.paths.span_or_empty(prior.path) == row.path
        {
            if prior.expected != row.expected {
                return Err(vec![Diagnostic::error(
                    "contextual cleanup has contradictory prerequisites for the same invocation-entry place",
                )]);
            }
            return Ok(false);
        }
    }
    let mut path = arena::HandleSpan::default();
    for segment in row.path {
        requirements.paths.append_to_span(&mut path, segment);
    }
    requirements.rows.append(ContextualCleanupEntryRequirement {
        machine: row.machine,
        parameter_position: row.parameter_position,
        path,
        expected: row.expected,
        antecedent: checked_trees::CheckedBooleanExpression::Constant(true),
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

fn for_each_place(
    path: &[PlaceSegment],
    prefix: &mut Vec<PlaceSegment>,
    visit: &mut impl FnMut(&[PlaceSegment]),
) {
    let Some((first, remaining)) = path.split_first() else {
        visit(prefix);
        return;
    };
    if let PlaceSegment::FixedRange { start, end } = *first {
        for index in start..end {
            prefix.push(PlaceSegment::FixedIndex { index });
            for_each_place(remaining, prefix, visit);
            prefix.pop();
        }
    } else {
        prefix.push(*first);
        for_each_place(remaining, prefix, visit);
        prefix.pop();
    }
}

fn has_symbolic_origin(
    program: &TypedTrees,
    state: &FlowStateFact,
    statement: usize,
    place: &CanonicalPlace,
) -> bool {
    let PlaceRoot::Symbol(root) = place.root else {
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
        if let Some(mut reference) =
            crate::flow::canonical_place_type_reference(program, source_state, statement, &source)
        {
            while let TypeReferenceNode::Constrained { base_type, .. } =
                program.type_reference_table.type_reference(reference)
            {
                reference = *base_type;
            }
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

pub(super) fn occurrence_contexts(
    program: &TypedTrees,
    facts: &CheckFacts,
    state: &FlowStateFact,
    disposition: &checked_trees::ContextualCleanupDisposition,
) -> Option<Vec<(usize, Vec<FactContextHandle>)>> {
    let points = match disposition.source {
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
                    let Some(typed_state) =
                        crate::semantic::calls::find_state(program, state.state_symbol)
                    else {
                        return None;
                    };
                    let Some(typed_trees::statement::StatementNode::Transition(transition)) =
                        program
                            .statement_table
                            .statements(typed_state.statement_nodes)
                            .get(coordinate.statement_index)
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
                checked_trees::FlowClaimJoinExitKind::CallResult { .. } => return None,
            };
            facts
                .flow
                .control
                .exits
                .span_or_empty(state.exits)
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
                            .semantic_constraint_contexts(exit.entry_constraints)
                            .collect(),
                    )
                })
                .collect()
        }
    };
    (!points.is_empty()).then_some(points)
}

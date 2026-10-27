//! Guarded structural case arms and structural state leaves.
//!
//! Ordinary expression tails and explicit value transitions finish the same
//! normal-return path. Earlier expressions only advance that path's facts;
//! named state transfers retain their captured arguments and entry subjects.
//! A selected crash closes a path without a normal-return leaf. It cannot
//! prevent an independently surviving path from proving its call premises,
//! and excluding it supplies no crash permission or denotational admission.

use super::call_requirements::establish_citation;
use super::structural_prefix::StructuralPrefixVisitor;
use crate::proof_contracts::contract_entailment::citations::{
    CitationFacts, CitationTarget, citation_argument_types, citation_call_in_statement,
    instantiate_citation, is_citation_statement,
};
use crate::proof_contracts::contract_entailment::is_arm_pattern_marker;
use crate::proof_contracts::contract_entailment::self_induction::intake_available_self_induction_hypotheses;
use crate::proof_contracts::contract_entailment::structural_judgment::{
    StructuralJudge, StructuralJudgment, StructuralTerm,
};
use crate::proof_contracts::contract_entailment::structural_terms::structural_term;
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::statement::{
    StatementNode, TransitionExit, TransitionGuardNode, TransitionTargetNode,
};

/// One normal-return leaf of the structural proof path walker.
/// Case refinements and selected public guarantees stay in the leaf's own
/// frame. Calls retain exact selected owner handles and captured result
/// occurrences; diagnostic names do not select the imported contract. Cyclic
/// state paths and unsupported effect statements decline recognition rather
/// than replaying runtime observations as mathematical calls.
pub(crate) struct StructuralCaseArm<'program> {
    /// The completed path keeps its writes and observations. Reconstructing
    /// a runtime leaf from entry hypotheses would revive invalidated storage.
    pub(super) normal_judge: StructuralJudge<'program>,
    /// Entry parameter names, positionally matching self-call arguments.
    pub(crate) parameter_names: Vec<String>,
    /// Case refinements accumulated along the path to this leaf. A variable
    /// subject becomes a substitution to its constructor over FRESH payload
    /// variables. Nested case states contribute one refinement apiece.
    pub(crate) case_hypotheses: Vec<(String, StructuralTerm)>,
    /// COMPUTED-SUBJECT refinements accumulated along the path. A subject
    /// such as `saturating_sub(b, a)` cannot be a substitution, so it is retained as an
    /// equation to intake instead.
    pub(crate) case_equations: Vec<(StructuralTerm, StructuralTerm)>,
    /// PER-ARM CITATIONS (N3 rung 2): equations injected by citation
    /// statements in the arm's SUB-STATE, instantiated under the arm
    /// environment -- the only route to cite a lemma AT A CASE PAYLOAD
    /// (comm's step case cites add_succ_law at `prev`, which machine-level
    /// statements cannot see). Empty for direct value arms.
    pub(crate) citations: Vec<(StructuralTerm, StructuralTerm)>,
    /// Selected callable guarantees captured on this normal path only.
    pub(crate) normal_guarantees: Vec<(StructuralTerm, StructuralTerm)>,
    /// The arm's value term, converted under the case environment (payload
    /// member reads resolve against the fresh-variable constructor).
    pub(crate) value: StructuralTerm,
}

/// Recognize the integer-measured structural-induction bridge: a total
/// two-way guarded value transition may build proof data in each branch even
/// though the guard itself is an ordinary integer proposition (`n > 0`).
/// Structural judging needs no interpretation of that proposition: it proves
/// the contract for BOTH exhaustive values.  Self-applications in either
/// value still become induction hypotheses only when the separate recursion
/// validator proves their declared measure decreases, so recognizing this
/// body shape cannot license an ungrounded induction.
pub(crate) fn recognize_guarded_structural_value_arms<'program>(
    program: &'program dyn crate::CheckingInput,
    machine: &Machine,
    judge: &StructuralJudge<'program>,
) -> Option<Vec<StructuralCaseArm<'program>>> {
    let [root] = program.machine_states(machine) else {
        return None;
    };
    let statements: Vec<&StatementNode> = program
        .statement_table
        .statements(root.statement_nodes)
        .iter()
        .filter(|statement| !is_arm_pattern_marker(statement))
        .collect();
    let [first, second] = statements.as_slice() else {
        return None;
    };
    let (StatementNode::Transition(first), StatementNode::Transition(second)) = (first, second)
    else {
        return None;
    };
    let branch = |transition: &typed_trees::statement::TableTransition| {
        if transition.continuation.is_valid() || !transition.target.is_valid() {
            return None;
        }
        let TransitionGuardNode::When(guard) = transition.guard else {
            return None;
        };
        let Some(equality) = program.expression_table.binary_node(guard) else {
            return None;
        };
        if equality.operator != BinaryOperator::Equal {
            return None;
        }
        let polarity = program.expression_table.boolean_node(equality.right)?;
        Some((equality.left, *polarity, transition.target))
    };
    let (first_condition, first_polarity, first_target) = branch(first)?;
    let second_target = match second.guard {
        TransitionGuardNode::Always => {
            // Boolean dispatch evaluates its subject once: the parser makes
            // the opposite final arm an unconditional fallback. Both value
            // obligations are still checked, without assuming either guard.
            // Keep the existing pure scalar grammar; effects could invalidate
            // the entry facts shared by the two structural judgments.
            if second.continuation.is_valid()
                || !second.target.is_valid()
                || !guard_expressions_equal(program, first_condition, first_condition)
            {
                return None;
            }
            second.target
        }
        TransitionGuardNode::When(_) => {
            let (second_condition, second_polarity, second_target) = branch(second)?;
            if first_polarity == second_polarity
                || !guard_expressions_equal(program, first_condition, second_condition)
            {
                return None;
            }
            second_target
        }
    };

    let parameter_names: Vec<String> = program
        .state_parameters(root)
        .iter()
        .map(|parameter| parameter.name.as_str().to_owned())
        .collect();
    let environment: Vec<(String, StructuralTerm)> = parameter_names
        .iter()
        .map(|name| (name.clone(), StructuralTerm::Variable(name.clone())))
        .collect();

    let mut leaves = Vec::new();
    for target in [first_target, second_target] {
        let TransitionTargetNode::Value(value) = program.statement_table.transition_target(target)
        else {
            return None;
        };
        for outcome in
            judge.normal_return_paths(*value, &environment, (machine, root, root.return_type))?
        {
            leaves.push(StructuralCaseArm {
                normal_judge: outcome.judge.clone(),
                parameter_names: parameter_names.clone(),
                case_hypotheses: Vec::new(),
                case_equations: Vec::new(),
                citations: Vec::new(),
                normal_guarantees: outcome.judge.normal_guarantees.to_vec(),
                value: outcome.value,
            });
        }
    }
    Some(leaves)
}

/// Equality for the duplicated condition trees produced by boolean-arm
/// lowering.  This deliberately recognizes only the pure scalar grammar
/// needed to prove that `condition == true` and `condition == false` are the
/// two faces of ONE condition; unsupported trees simply refuse the
/// structural-induction shortcut.
fn guard_expressions_equal(
    program: &dyn crate::CheckingInput,
    left: ExpressionHandle,
    right: ExpressionHandle,
) -> bool {
    match (
        program.expression_table.expression(left),
        program.expression_table.expression(right),
    ) {
        (ExpressionNode::Boolean(left), ExpressionNode::Boolean(right)) => left == right,
        (ExpressionNode::Integer(left), ExpressionNode::Integer(right)) => {
            left.value_i64() == right.value_i64()
        }
        (ExpressionNode::Name(left), ExpressionNode::Name(right)) => {
            left.symbol == right.symbol
                && program.expression_table.name_path_members(left.members)
                    == program.expression_table.name_path_members(right.members)
        }
        (ExpressionNode::Binary(left), ExpressionNode::Binary(right)) => {
            left.operator == right.operator
                && guard_expressions_equal(program, left.left, right.left)
                && guard_expressions_equal(program, left.right, right.right)
        }
        _ => false,
    }
}

/// Stage a citation call's argument terms — each judged under its expected
/// declared type — for establishment after the arm's own walk.
fn stage_citation_argument_terms(
    program: &dyn crate::CheckingInput,
    judge: &StructuralJudge<'_>,
    machine: &Machine,
    state: &typed_trees::state::State,
    environment: &[(String, StructuralTerm)],
    target: &CitationTarget,
    argument_handles: &[ExpressionHandle],
    pending_citations: &mut Vec<PendingStructuralCitation>,
) -> Option<()> {
    let expected = citation_argument_types(program, target);
    let argument_terms = argument_handles
        .iter()
        .enumerate()
        .map(|(position, argument)| {
            judge.callee_term_in_scope(
                expected
                    .as_ref()
                    .and_then(|types| types.get(position))
                    .map(|expected| (machine, state, *expected)),
                *argument,
                environment,
                0,
            )
        })
        .collect::<Option<Vec<_>>>()?;
    pending_citations.push((target.clone(), argument_terms));
    Some(())
}

type PendingStructuralCitation = (CitationTarget, Vec<StructuralTerm>);

pub(crate) fn recognize_structural_case_arms<'a>(
    program: &'a dyn crate::CheckingInput,
    machine: &Machine,
    judge: &StructuralJudge<'a>,
    classification: &typed_trees::proof_only::ProofOnlyClassification,
    diagnostics: &mut Vec<Diagnostic>,
    mut requirement: Option<&mut dyn StructuralPrefixVisitor>,
) -> Option<Vec<StructuralCaseArm<'a>>> {
    let states = program.machine_states(machine);
    let root = states.first()?;
    let parameter_names: Vec<String> = program
        .state_parameters(root)
        .iter()
        .map(|parameter| parameter.name.as_str().to_owned())
        .collect();
    let environment: Vec<(String, StructuralTerm)> = parameter_names
        .iter()
        .map(|name| (name.clone(), StructuralTerm::Variable(name.clone())))
        .collect();
    let mut path = Vec::new();
    let mut fresh = 0usize;
    let arms = recognize_structural_state_leaves(
        program,
        machine,
        judge,
        classification,
        diagnostics,
        states,
        root,
        &parameter_names,
        environment,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        false,
        &mut path,
        &mut fresh,
        &mut requirement,
        CitationFacts::default(),
    )?;
    (!arms.is_empty() || requirement.is_some()).then_some(arms)
}

/// A runtime call obligation is local to one state invocation. All arrivals
/// independently establish that state's Requires; only preceding statements
/// and completed operands can contribute facts at this exact call occurrence.
/// The same path walker stops there, so no recurrent successor is unrolled.
pub(super) fn recognize_structural_state_prefix<'a>(
    program: &'a dyn crate::CheckingInput,
    machine: &Machine,
    state: &typed_trees::state::State,
    judge: &StructuralJudge<'a>,
    classification: &typed_trees::proof_only::ProofOnlyClassification,
    diagnostics: &mut Vec<Diagnostic>,
    requirement: &mut dyn StructuralPrefixVisitor,
) -> Option<Vec<StructuralCaseArm<'a>>> {
    let parameters = program.state_parameters(state);
    let parameter_names = parameters
        .iter()
        .map(|parameter| parameter.name.as_str().to_owned())
        .collect::<Vec<_>>();
    let environment = parameter_names
        .iter()
        .map(|name| (name.clone(), StructuralTerm::Variable(name.clone())))
        .collect();
    recognize_structural_state_leaves(
        program,
        machine,
        judge,
        classification,
        diagnostics,
        program.machine_states(machine),
        state,
        &parameter_names,
        environment,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        false,
        &mut Vec::new(),
        &mut 0,
        &mut Some(requirement),
        CitationFacts::default(),
    )
}

/// True when `local_symbol` is never referenced by any statement at or after
/// `next_execution_position` in `execution` (excluding transition targets when
/// `in_state_prefix_only` is true, since state-local prefix observers stop
/// before evaluating transition targets).
fn local_symbol_is_dead_in_remaining_prefix(
    program: &dyn crate::CheckingInput,
    statements: &[StatementNode],
    execution: &[usize],
    next_execution_position: usize,
    local_symbol: SymbolHandle,
    in_state_prefix_only: bool,
) -> bool {
    if !local_symbol.is_valid() {
        return false;
    }
    let mentions_local = |root: ExpressionHandle| {
        if !root.is_valid() {
            return false;
        }
        let mut expressions = Vec::new();
        crate::expression_traversal::collect_expression_tree(program, root, &mut expressions);
        expressions.into_iter().any(|expression| {
            matches!(
                program.expression_table.expression(expression),
                ExpressionNode::Name(path) if path.symbol == local_symbol
            )
        })
    };
    for &stmt_idx in execution.iter().skip(next_execution_position) {
        let stmt = &statements[stmt_idx];
        if in_state_prefix_only {
            if let StatementNode::Transition(transition) = stmt {
                if let TransitionGuardNode::When(guard) = transition.guard
                    && mentions_local(guard)
                {
                    return false;
                }
                continue;
            }
        }
        for (_, root) in program.tables.expression_roots(stmt) {
            if mentions_local(root) {
                return false;
            }
        }
    }
    true
}

/// True when `local_symbol` is never referenced in any `TransitionGuardNode::When`
/// or `Match` expression at or after `next_execution_position` in `execution`.
fn local_symbol_is_never_branched_on_in_remaining_prefix(
    program: &dyn crate::CheckingInput,
    statements: &[StatementNode],
    execution: &[usize],
    next_execution_position: usize,
    local_symbol: SymbolHandle,
    in_state_prefix_only: bool,
) -> bool {
    if !local_symbol.is_valid() {
        return false;
    }
    let mentions_local = |root: ExpressionHandle| {
        if !root.is_valid() {
            return false;
        }
        let mut expressions = Vec::new();
        crate::expression_traversal::collect_expression_tree(program, root, &mut expressions);
        expressions.into_iter().any(|expression| {
            matches!(
                program.expression_table.expression(expression),
                ExpressionNode::Name(path) if path.symbol == local_symbol
            )
        })
    };
    let mentions_local_in_match = |root: ExpressionHandle| {
        if !root.is_valid() {
            return false;
        }
        let mut expressions = Vec::new();
        crate::expression_traversal::collect_expression_tree(program, root, &mut expressions);
        expressions.into_iter().any(|expression| {
            matches!(
                program.expression_table.expression(expression),
                ExpressionNode::Match(_) if mentions_local(expression)
            )
        })
    };
    for (pos, &stmt_idx) in execution.iter().enumerate().skip(next_execution_position) {
        let stmt = &statements[stmt_idx];
        if in_state_prefix_only {
            if let StatementNode::Transition(transition) = stmt {
                if let TransitionGuardNode::When(guard) = transition.guard
                    && mentions_local(guard)
                {
                    return false;
                }
                continue;
            }
        } else {
            if pos + 1 == execution.len()
                && let StatementNode::Expression(expr) = stmt
                && mentions_local(*expr)
            {
                return false;
            }
            if let StatementNode::Transition(transition) = stmt {
                if let TransitionGuardNode::When(guard) = transition.guard
                    && mentions_local(guard)
                {
                    return false;
                }
                if transition.target.is_valid()
                    && let typed_trees::statement::TransitionTargetNode::Value(target) = program
                        .tables
                        .statement_table
                        .transition_target(transition.target)
                    && mentions_local(*target)
                {
                    return false;
                }
            }
        }
        for (_, root) in program.tables.expression_roots(stmt) {
            if mentions_local_in_match(root) {
                return false;
            }
        }
    }
    true
}

#[derive(Clone)]
struct ActiveStatePrefixPath<'a> {
    judge: StructuralJudge<'a>,
    environment: Vec<(String, StructuralTerm)>,
    established_guarantees: CitationFacts,
}

/// Evaluate all requested statement-entry prefix visitors (`in_transition_target == false`)
/// of `state` in a single forward pass over `state`'s execution order instead of
/// re-evaluating prefix statements `0..statement_index` for each statement.
pub(super) fn recognize_structural_state_prefixes<'a>(
    program: &'a dyn crate::CheckingInput,
    machine: &Machine,
    state: &typed_trees::state::State,
    judge: &StructuralJudge<'a>,
    classification: &typed_trees::proof_only::ProofOnlyClassification,
    visitors: &mut [&mut dyn StructuralPrefixVisitor],
) -> Vec<bool> {
    let mut valid = vec![false; visitors.len()];
    if !state.symbol.is_valid() || visitors.is_empty() {
        return valid;
    }
    let statements = program.statement_table.statements(state.statement_nodes);
    let Some(execution) = program
        .statement_table
        .finite_execution_order(state.statement_nodes)
        .ok()
        .flatten()
    else {
        return valid;
    };
    let parameters = program.state_parameters(state);
    let environment = parameters
        .iter()
        .map(|parameter| {
            let name = parameter.name.as_str().to_owned();
            (name.clone(), StructuralTerm::Variable(name))
        })
        .collect::<Vec<_>>();
    let initial_judge = structural_arm_judge(program, judge, &[], &[]);
    let mut active_paths = vec![ActiveStatePrefixPath {
        judge: initial_judge,
        environment,
        established_guarantees: CitationFacts::default(),
    }];
    let mut transitions = Vec::new();
    let mut saw_transition = false;
    let mut fresh = 0usize;

    let any_visitor_at_transition = visitors.iter().any(|visitor| {
        visitor.site().state == state.symbol
            && !visitor.site().in_transition_target
            && visitor.site().statement_index < statements.len()
            && matches!(
                statements[visitor.site().statement_index],
                StatementNode::Transition(_)
            )
    });

    for (execution_position, statement_index) in execution.iter().copied().enumerate() {
        let statement = &statements[statement_index];
        let dead_local = match statement {
            StatementNode::LocalData(local) => local_symbol_is_dead_in_remaining_prefix(
                program,
                statements,
                &execution,
                execution_position + 1,
                local.symbol,
                !any_visitor_at_transition,
            ),
            _ => false,
        };
        if is_arm_pattern_marker(statement) {
            continue;
        }
        if !matches!(statement, StatementNode::Transition(_)) {
            for (visitor_idx, visitor) in visitors.iter_mut().enumerate() {
                if visitor.site().state == state.symbol
                    && visitor.site().statement_index == statement_index
                    && !visitor.site().in_transition_target
                {
                    for path in &active_paths {
                        if visitor.is_complete() {
                            break;
                        }
                        visitor.visit(
                            program,
                            machine,
                            &path.judge,
                            &path.environment,
                            &[],
                            &[],
                            &path.established_guarantees,
                        );
                    }
                    valid[visitor_idx] = true;
                }
            }
        }
        if visitors
            .iter()
            .enumerate()
            .all(|(idx, visitor)| valid[idx] || visitor.is_complete())
        {
            return valid;
        }
        if let StatementNode::Transition(transition) = statement {
            saw_transition = true;
            if transition.continuation.is_valid()
                || (transition.exit == TransitionExit::Ordinary && !transition.target.is_valid())
            {
                return valid;
            }
            transitions.push((statement_index, transition));
            continue;
        }
        if saw_transition {
            return valid;
        }
        let call_write_frame = if let StatementNode::Call(call) = statement
            && active_paths
                .first()
                .is_some_and(|path| path.judge.requires_value_preservation())
        {
            let Some(frames) = active_paths[0].judge.frame_resolver() else {
                return valid;
            };
            let mut writes = Vec::new();
            let mut complete = true;
            for frame in [
                frames.may_write_frame(machine, call),
                frames.statement_value_write_frame(machine, statement),
            ] {
                match frame.into_complete_paths() {
                    Some(paths) => writes.extend(paths),
                    None => complete = false,
                }
            }
            Some((complete, writes))
        } else {
            None
        };
        let mut next_paths = Vec::new();
        for mut path in active_paths {
            let mut citation_preserves_values = true;
            if let Some((complete, ref writes)) = call_write_frame {
                citation_preserves_values = complete && writes.is_empty();
                if path
                    .judge
                    .observe_storage_effects(state, complete.then_some(writes.as_slice()))
                    .is_none()
                {
                    return valid;
                }
            }
            if citation_preserves_values
                && (!path.judge.has_runtime_body_values()
                    || matches!(statement, StatementNode::Call(_)))
                && let Some((target, arguments)) = citation_call_in_statement(program, statement)
                && super::citations::can_establish_citation(
                    program,
                    classification,
                    machine,
                    &target,
                    arguments.len(),
                    path.judge.has_runtime_body_values(),
                )
            {
                let expected = citation_argument_types(program, &target);
                let arguments = arguments
                    .iter()
                    .enumerate()
                    .map(|(position, argument)| {
                        path.judge.callee_term_in_scope(
                            expected
                                .as_ref()
                                .and_then(|types| types.get(position))
                                .map(|expected| (machine, state, *expected)),
                            *argument,
                            &path.environment,
                            0,
                        )
                    })
                    .collect::<Option<Vec<_>>>();
                if let Some(arguments) = arguments {
                    establish_citation(
                        program,
                        machine,
                        state,
                        statement_index,
                        &path.environment,
                        &mut path.judge,
                        classification,
                        &[],
                        &[],
                        &mut path.established_guarantees,
                        &target,
                        &arguments,
                    );
                } else if !path.judge.has_runtime_body_values() {
                    return valid;
                }
            }
            match statement {
                StatementNode::LocalData(local) => {
                    let Some(mut outcomes) = (if local.initial_value.is_valid() {
                        path.judge
                            .normal_return_paths(
                                local.initial_value,
                                &path.environment,
                                (machine, state, local.type_reference),
                            )
                            .map(|outcomes| {
                                outcomes
                                    .into_iter()
                                    .map(|outcome| (Some(outcome.value), outcome.judge))
                                    .collect::<Vec<_>>()
                            })
                    } else {
                        Some(vec![(
                            super::structural_terms::zero_value_structural_term(
                                program,
                                local.type_reference,
                            ),
                            path.judge.clone(),
                        )])
                    }) else {
                        return valid;
                    };
                    if outcomes.len() > 1
                        && outcomes.iter().all(|(_, outcome_judge)| {
                            outcome_judge.has_no_new_effects_or_guarantees_since(&path.judge)
                        })
                    {
                        if dead_local {
                            let judge_refs: Vec<_> =
                                outcomes.iter().map(|(_, judge)| judge).collect();
                            let rejoined = path
                                .judge
                                .rejoin_pure_outcomes(&judge_refs, Some(local.initial_value));
                            outcomes = vec![(None, rejoined)];
                        } else if program.primitive_type_reference(local.type_reference)
                            == Some(typed_trees::types::PrimitiveType::Bool)
                            && local_symbol_is_never_branched_on_in_remaining_prefix(
                                program,
                                statements,
                                &execution,
                                execution_position + 1,
                                local.symbol,
                                !any_visitor_at_transition,
                            )
                        {
                            let judge_refs: Vec<_> =
                                outcomes.iter().map(|(_, judge)| judge).collect();
                            let mut rejoined = path
                                .judge
                                .rejoin_pure_outcomes(&judge_refs, Some(local.initial_value));
                            rejoined.capture_sequence += 1;
                            let captured = StructuralTerm::CapturedValue {
                                expression: local.initial_value,
                                callable: SymbolHandle::invalid(),
                                occurrence: rejoined.capture_sequence,
                                result_type: local.type_reference,
                            };
                            outcomes = vec![(Some(captured), rejoined)];
                        }
                    }
                    for (value, mut outcome_judge) in outcomes {
                        let mut environment = path.environment.clone();
                        environment.retain(|(name, _)| name != local.name.as_str());
                        if let Some(value) = value {
                            outcome_judge.observe_completed_binding(
                                local.symbol,
                                local.type_reference,
                                value.clone(),
                            );
                            environment.push((local.name.as_str().to_owned(), value));
                        }
                        next_paths.push(ActiveStatePrefixPath {
                            judge: outcome_judge,
                            environment,
                            established_guarantees: path.established_guarantees.clone(),
                        });
                    }
                }
                StatementNode::Assignment(_) if path.judge.has_runtime_body_values() => {
                    let Some(outcomes) = path.judge.normal_assignment_paths(
                        statement,
                        &path.environment,
                        (machine, state, state.return_type),
                    ) else {
                        return valid;
                    };
                    for outcome in outcomes {
                        next_paths.push(ActiveStatePrefixPath {
                            judge: outcome.judge,
                            environment: path.environment.clone(),
                            established_guarantees: path.established_guarantees.clone(),
                        });
                    }
                }
                StatementNode::Expression(expression) => {
                    let is_tail = execution_position + 1 == execution.len();
                    let result_type = if is_tail {
                        state.return_type
                    } else {
                        crate::expression_result_type_reference(
                            program,
                            machine,
                            state,
                            *expression,
                        )
                        .unwrap_or_default()
                    };
                    let Some(outcomes) = path.judge.normal_return_paths(
                        *expression,
                        &path.environment,
                        (machine, state, result_type),
                    ) else {
                        return valid;
                    };
                    if !is_tail {
                        for outcome in outcomes {
                            next_paths.push(ActiveStatePrefixPath {
                                judge: outcome.judge,
                                environment: path.environment.clone(),
                                established_guarantees: path.established_guarantees.clone(),
                            });
                        }
                    }
                }
                StatementNode::Call(_) if path.judge.requires_value_preservation() => {
                    next_paths.push(path);
                }
                StatementNode::Call(call)
                    if is_citation_statement(program, classification, machine, call) =>
                {
                    next_paths.push(path);
                }
                StatementNode::Assembly(_) | StatementNode::AssemblyFact(_) => {
                    next_paths.push(path);
                }
                _ => return valid,
            }
        }
        active_paths = next_paths;
    }

    if transitions.is_empty() {
        let synthetic_end = state.statement_nodes.count() as usize;
        for (visitor_idx, visitor) in visitors.iter_mut().enumerate() {
            if visitor.site().state == state.symbol
                && visitor.site().statement_index == synthetic_end
                && !visitor.site().in_transition_target
            {
                for path in &active_paths {
                    if visitor.is_complete() {
                        break;
                    }
                    visitor.visit(
                        program,
                        machine,
                        &path.judge,
                        &path.environment,
                        &[],
                        &[],
                        &path.established_guarantees,
                    );
                }
                valid[visitor_idx] = true;
            }
        }
        return valid;
    }

    // Step active_paths through transitions in order.
    let mut remaining_paths = active_paths
        .into_iter()
        .map(|path| (path, Vec::<(StructuralTerm, StructuralTerm)>::new()))
        .collect::<Vec<_>>();
    for (statement_index, transition) in transitions {
        for (visitor_idx, visitor) in visitors.iter_mut().enumerate() {
            if visitor.site().state == state.symbol
                && visitor.site().statement_index == statement_index
                && !visitor.site().in_transition_target
            {
                for (path, remaining_equations) in &remaining_paths {
                    if visitor.is_complete() {
                        break;
                    }
                    visitor.visit(
                        program,
                        machine,
                        &path.judge,
                        &path.environment,
                        &[],
                        remaining_equations,
                        &path.established_guarantees,
                    );
                }
                valid[visitor_idx] = true;
            }
        }
        if visitors
            .iter()
            .enumerate()
            .all(|(idx, visitor)| valid[idx] || visitor.is_complete())
        {
            return valid;
        }
        let mut next_remaining = Vec::new();
        for (path, remaining_equations) in remaining_paths {
            let TransitionGuardNode::When(guard) = transition.guard else {
                continue;
            };
            let guard_preserves_values = path
                .judge
                .frame_resolver()
                .and_then(|frames| frames.expression_may_write_paths(machine, guard))
                .is_some_and(|paths| paths.is_empty());
            if path.judge.requires_value_preservation() && !guard_preserves_values {
                return valid;
            }
            let Some((subject_expression, constructor)) =
                guard_observation(program, machine, state, guard, &mut fresh)
            else {
                return valid;
            };
            let observed = structural_arm_judge(program, &path.judge, &[], &remaining_equations);
            let subject_type = crate::expression_result_type_reference(
                program,
                machine,
                state,
                subject_expression,
            )
            .unwrap_or_default();
            let Some(outcomes) = observed.normal_return_paths(
                subject_expression,
                &path.environment,
                (machine, state, subject_type),
            ) else {
                return valid;
            };
            for outcome in outcomes {
                let subject = outcome.value;
                let mut skipped_equations = remaining_equations.clone();
                let truth = guard_preserves_values
                    .then(|| {
                        let StructuralTerm::Constructor { data, case, fields } = &constructor
                        else {
                            return None;
                        };
                        (data == "bool" && fields.is_empty())
                            .then(|| {
                                language_semantics::const_value::boolean_literal_spelling(case)
                            })
                            .flatten()
                    })
                    .flatten();
                let subject_judgment = truth.map(|_| {
                    outcome.judge.judge_equation(
                        outcome.judge.resolve(subject.clone()),
                        constructor.clone(),
                        0,
                    )
                });
                if !matches!(subject_judgment, Some(StructuralJudgment::Proven)) {
                    if let Some(truth) = truth {
                        skipped_equations.push((
                            subject.clone(),
                            StructuralTerm::Constructor {
                                data: "bool".to_owned(),
                                case: (!truth).to_string(),
                                fields: Vec::new(),
                            },
                        ));
                    }
                    next_remaining.push((
                        ActiveStatePrefixPath {
                            judge: outcome.judge,
                            environment: path.environment.clone(),
                            established_guarantees: path.established_guarantees.clone(),
                        },
                        skipped_equations,
                    ));
                }
            }
        }
        remaining_paths = next_remaining;
    }
    valid
}

#[allow(clippy::too_many_arguments)]
fn recognize_structural_state_leaves<'a>(
    program: &'a dyn crate::CheckingInput,
    machine: &Machine,
    judge: &StructuralJudge<'a>,
    classification: &typed_trees::proof_only::ProofOnlyClassification,
    diagnostics: &mut Vec<Diagnostic>,
    states: &[typed_trees::state::State],
    state: &typed_trees::state::State,
    parameter_names: &[String],
    environment: Vec<(String, StructuralTerm)>,
    case_hypotheses: Vec<(String, StructuralTerm)>,
    case_equations: Vec<(StructuralTerm, StructuralTerm)>,
    pending_citations: Vec<PendingStructuralCitation>,
    collect_citations: bool,
    path: &mut Vec<SymbolHandle>,
    fresh: &mut usize,
    requirement: &mut Option<&mut dyn StructuralPrefixVisitor>,
    established_guarantees: CitationFacts,
) -> Option<Vec<StructuralCaseArm<'a>>> {
    recognize_structural_state_leaves_from(
        program,
        machine,
        judge,
        classification,
        diagnostics,
        states,
        state,
        parameter_names,
        environment,
        case_hypotheses,
        case_equations,
        pending_citations,
        collect_citations,
        path,
        fresh,
        requirement,
        established_guarantees,
        0,
    )
}

#[allow(clippy::too_many_arguments)]
fn recognize_structural_state_leaves_from<'a>(
    program: &'a dyn crate::CheckingInput,
    machine: &Machine,
    judge: &StructuralJudge<'a>,
    classification: &typed_trees::proof_only::ProofOnlyClassification,
    diagnostics: &mut Vec<Diagnostic>,
    states: &[typed_trees::state::State],
    state: &typed_trees::state::State,
    parameter_names: &[String],
    environment: Vec<(String, StructuralTerm)>,
    case_hypotheses: Vec<(String, StructuralTerm)>,
    case_equations: Vec<(StructuralTerm, StructuralTerm)>,
    mut pending_citations: Vec<PendingStructuralCitation>,
    collect_citations: bool,
    path: &mut Vec<SymbolHandle>,
    fresh: &mut usize,
    requirement: &mut Option<&mut dyn StructuralPrefixVisitor>,
    mut established_guarantees: CitationFacts,
    start_index: usize,
) -> Option<Vec<StructuralCaseArm<'a>>> {
    if !state.symbol.is_valid() || (start_index == 0 && path.contains(&state.symbol)) {
        return None;
    }
    // The theorem being established remains `machine` while transparent
    // proof calls enter other bodies. Expression and effect lookup belongs
    // to the exact body owner, not to the theorem's lexical namespace.
    let (invocation, _) =
        crate::machine_calls::calls::machine_state_by_symbol(program, state.symbol)?;
    if requirement
        .as_deref()
        .is_some_and(StructuralPrefixVisitor::is_complete)
    {
        return Some(Vec::new());
    }
    if start_index == 0 {
        path.push(state.symbol);
    }
    // Resuming after a completed statement retains the same branch facts.
    // The outcome judge already owns them and its newly captured guarantees;
    // re-intaking the entire branch here duplicates rewrites once per local.
    // A state arrival still establishes its incoming refinements, and the
    // requested premise visitor checks contradiction in its final context.
    let mut path_judge = if start_index == 0 {
        structural_arm_judge(program, judge, &case_hypotheses, &case_equations)
    } else {
        judge.clone()
    };
    let judge = &mut path_judge;

    let result = (|| {
        // Assembly headers and labels carry control metadata; their ordinary
        // lowered instructions own the effects. Follow the shared finite
        // execution order, not lexical windows that may be skipped or reordered.
        // Resume and observation coordinates remain original source ordinals.
        let statements = program.statement_table.statements(state.statement_nodes);
        let execution = program
            .statement_table
            .finite_execution_order(state.statement_nodes)
            .ok()??;
        let position = |source| {
            if source == statements.len() {
                Some(execution.len())
            } else {
                execution.iter().position(|candidate| *candidate == source)
            }
        };
        let start_position = position(start_index)?;
        let observation_position = match requirement.as_deref() {
            Some(requirement) if requirement.site().state == state.symbol => {
                Some(position(requirement.site().statement_index)?)
            }
            _ => None,
        };
        let mut transitions = Vec::new();
        let mut saw_transition = false;
        for (execution_position, statement_index) in
            execution.iter().copied().enumerate().skip(start_position)
        {
            let statement = &statements[statement_index];
            let next_statement = execution
                .get(execution_position + 1)
                .copied()
                .unwrap_or(statements.len());
            if observation_position.is_some_and(|observation| execution_position > observation) {
                break;
            }
            if is_arm_pattern_marker(statement) {
                continue;
            }
            if let Some(requirement) = requirement.as_deref_mut()
                && requirement.site().state == state.symbol
                && requirement.site().statement_index == statement_index
                && !matches!(statement, StatementNode::Transition(_))
            {
                requirement.visit(
                    program,
                    invocation,
                    judge,
                    &environment,
                    &case_hypotheses,
                    &case_equations,
                    &established_guarantees,
                );
                // Nothing after this call can establish its entry premises.
                // Other incoming paths are still visited by the parent.
                return Some(Vec::new());
            }
            // Statement calls and their operands own separate write frames.
            // Retire affected storage without discarding unrelated captures.
            // Legacy citation substitution has no post-state argument model,
            // so only an empty joined frame permits its value guarantees.
            // The requested call above has not executed and owes only operand
            // evaluation, which its visitor checks through normal-return paths.
            let mut citation_preserves_values = true;
            if judge.requires_value_preservation()
                && let StatementNode::Call(call) = statement
            {
                let _s = phase_profile::guard("05.d.7.3b.1.cap.2d.call_writes");
                let frames = judge.frame_resolver()?;
                let mut writes = Vec::new();
                let mut complete = true;
                for frame in [
                    frames.may_write_frame(invocation, call),
                    frames.statement_value_write_frame(invocation, statement),
                ] {
                    match frame.into_complete_paths() {
                        Some(paths) => writes.extend(paths),
                        None => complete = false,
                    }
                }
                citation_preserves_values = complete && writes.is_empty();
                judge.observe_storage_effects(state, complete.then_some(writes.as_slice()))?;
            }
            if requirement.is_some()
                && citation_preserves_values
                && !saw_transition
                && (!judge.has_runtime_body_values() || matches!(statement, StatementNode::Call(_)))
                && let Some((target, arguments)) = citation_call_in_statement(program, statement)
                && super::citations::can_establish_citation(
                    program,
                    classification,
                    machine,
                    &target,
                    arguments.len(),
                    judge.has_runtime_body_values(),
                )
            {
                let _s = phase_profile::guard("05.d.7.3b.1.cap.2d.citation");
                // Value-position runtime calls, including their tag facts,
                // belong to the normal-return traversal below. Termifying
                // their operands here would replay an execution and rejects
                // nested calls before that traversal can capture them.
                // Resultless statement citations have no value expression;
                // their proof and tag-only contracts retain this route.
                let expected = citation_argument_types(program, &target);
                let arguments = arguments
                    .iter()
                    .enumerate()
                    .map(|(position, argument)| {
                        judge.callee_term_in_scope(
                            expected
                                .as_ref()
                                .and_then(|types| types.get(position))
                                .map(|expected| (invocation, state, *expected)),
                            *argument,
                            &environment,
                            0,
                        )
                    })
                    .collect::<Option<Vec<_>>>();
                if let Some(arguments) = arguments {
                    establish_citation(
                        program,
                        machine,
                        state,
                        statement_index,
                        &environment,
                        judge,
                        classification,
                        &case_hypotheses,
                        &case_equations,
                        &mut established_guarantees,
                        &target,
                        &arguments,
                    );
                } else if !judge.has_runtime_body_values() {
                    return None;
                }
                drop(_s);
            }
            match statement {
                StatementNode::LocalData(local) if !saw_transition => {
                    let dead_local = requirement
                        .as_deref()
                        .is_some_and(|observer| observer.site().state == state.symbol)
                        && local_symbol_is_dead_in_remaining_prefix(
                            program,
                            statements,
                            &execution,
                            execution_position + 1,
                            local.symbol,
                            requirement.as_deref().is_some_and(|observer| {
                                !observer.site().in_transition_target
                                    && observer.site().statement_index < statements.len()
                                    && !matches!(
                                        statements[observer.site().statement_index],
                                        StatementNode::Transition(_)
                                    )
                            }),
                        );
                    let mut outcomes =
                        phase_profile::scope("05.d.7.3b.1.cap.2d.local_init", || {
                            if local.initial_value.is_valid() {
                                judge
                                    .normal_return_paths(
                                        local.initial_value,
                                        &environment,
                                        (invocation, state, local.type_reference),
                                    )
                                    .map(|outcomes| {
                                        outcomes
                                            .into_iter()
                                            .map(|outcome| (Some(outcome.value), outcome.judge))
                                            .collect::<Vec<_>>()
                                    })
                            } else {
                                // Zero storage has no expression to execute. Retain
                                // the preceding path even when its contents have no
                                // established mathematical value. Only the ordinary
                                // zero-value owner may contribute equations; omission
                                // must not establish a gated domain or invent a value.
                                Some(vec![(
                                    super::structural_terms::zero_value_structural_term(
                                        program,
                                        local.type_reference,
                                    ),
                                    judge.clone(),
                                )])
                            }
                        })?;
                    if outcomes.len() > 1
                        && outcomes.iter().all(|(_, outcome_judge)| {
                            outcome_judge.has_no_new_effects_or_guarantees_since(&judge)
                        })
                    {
                        if dead_local {
                            let judge_refs: Vec<_> = outcomes
                                .iter()
                                .map(|(_, outcome_judge)| outcome_judge)
                                .collect();
                            let rejoined =
                                judge.rejoin_pure_outcomes(&judge_refs, Some(local.initial_value));
                            outcomes = vec![(None, rejoined)];
                        } else if requirement
                            .as_deref()
                            .is_some_and(|observer| observer.site().state == state.symbol)
                            && program.primitive_type_reference(local.type_reference)
                                == Some(typed_trees::types::PrimitiveType::Bool)
                            && local_symbol_is_never_branched_on_in_remaining_prefix(
                                program,
                                statements,
                                &execution,
                                execution_position + 1,
                                local.symbol,
                                requirement.as_deref().is_some_and(|observer| {
                                    !observer.site().in_transition_target
                                        && observer.site().statement_index < statements.len()
                                        && !matches!(
                                            statements[observer.site().statement_index],
                                            StatementNode::Transition(_)
                                        )
                                }),
                            )
                        {
                            let judge_refs: Vec<_> = outcomes
                                .iter()
                                .map(|(_, outcome_judge)| outcome_judge)
                                .collect();
                            let mut rejoined =
                                judge.rejoin_pure_outcomes(&judge_refs, Some(local.initial_value));
                            rejoined.capture_sequence += 1;
                            let captured = StructuralTerm::CapturedValue {
                                expression: local.initial_value,
                                callable: SymbolHandle::invalid(),
                                occurrence: rejoined.capture_sequence,
                                result_type: local.type_reference,
                            };
                            outcomes = vec![(Some(captured), rejoined)];
                        }
                    }
                    let mut leaves = Vec::new();
                    for (value, mut outcome_judge) in outcomes {
                        let mut pending_citations = pending_citations.clone();
                        if collect_citations
                            && !judge.has_runtime_body_values()
                            && let Some((target, argument_handles)) =
                                citation_call_in_statement(program, statement)
                        {
                            stage_citation_argument_terms(
                                program,
                                &outcome_judge,
                                invocation,
                                state,
                                &environment,
                                &target,
                                &argument_handles,
                                &mut pending_citations,
                            )?;
                        }
                        let mut environment = environment.clone();
                        environment.retain(|(name, _)| name != local.name.as_str());
                        if let Some(value) = value {
                            outcome_judge.observe_completed_binding(
                                local.symbol,
                                local.type_reference,
                                value.clone(),
                            );
                            environment.push((local.name.as_str().to_owned(), value));
                        }
                        leaves.extend(recognize_structural_state_leaves_from(
                            program,
                            machine,
                            &outcome_judge,
                            classification,
                            diagnostics,
                            states,
                            state,
                            parameter_names,
                            environment,
                            case_hypotheses.clone(),
                            case_equations.clone(),
                            pending_citations,
                            collect_citations,
                            path,
                            fresh,
                            requirement,
                            established_guarantees.clone(),
                            next_statement,
                        )?);
                    }
                    return Some(leaves);
                }
                StatementNode::Assignment(_)
                    if !saw_transition && judge.has_runtime_body_values() =>
                {
                    let outcomes = phase_profile::scope("05.d.7.3b.1.cap.2d.assign", || {
                        judge.normal_assignment_paths(
                            statement,
                            &environment,
                            (invocation, state, state.return_type),
                        )
                    })?;
                    let mut leaves = Vec::new();
                    for outcome in outcomes {
                        leaves.extend(recognize_structural_state_leaves_from(
                            program,
                            machine,
                            &outcome.judge,
                            classification,
                            diagnostics,
                            states,
                            state,
                            parameter_names,
                            environment.clone(),
                            case_hypotheses.clone(),
                            case_equations.clone(),
                            pending_citations.clone(),
                            collect_citations,
                            path,
                            fresh,
                            requirement,
                            established_guarantees.clone(),
                            next_statement,
                        )?);
                    }
                    return Some(leaves);
                }
                StatementNode::Expression(expression) if !saw_transition => {
                    let is_tail = execution_position + 1 == execution.len();
                    let result_type = if is_tail {
                        state.return_type
                    } else {
                        crate::expression_result_type_reference(
                            program,
                            invocation,
                            state,
                            *expression,
                        )
                        .unwrap_or_default()
                    };
                    if !is_tail
                        || matches!(
                            program
                                .type_reference_table
                                .type_reference(state.return_type),
                            typed_trees::types::TypeReferenceNode::Unit
                        )
                    {
                        let outcomes = judge.normal_completion_paths(
                            *expression,
                            &environment,
                            (invocation, state, result_type),
                        )?;
                        let mut leaves = Vec::new();
                        for completed in outcomes {
                            leaves.extend(recognize_structural_state_leaves_from(
                                program,
                                machine,
                                &completed,
                                classification,
                                diagnostics,
                                states,
                                state,
                                parameter_names,
                                environment.clone(),
                                case_hypotheses.clone(),
                                case_equations.clone(),
                                pending_citations.clone(),
                                collect_citations,
                                path,
                                fresh,
                                requirement,
                                established_guarantees.clone(),
                                next_statement,
                            )?);
                        }
                        return Some(leaves);
                    }
                    let outcomes = phase_profile::scope("05.d.7.3b.1.cap.2d.expr", || {
                        judge.normal_return_paths(
                            *expression,
                            &environment,
                            (invocation, state, result_type),
                        )
                    })?;
                    let mut leaves = Vec::new();
                    for outcome in outcomes {
                        if requirement.is_none() {
                            leaves.push(finalize_structural_case_arm(
                                program,
                                machine,
                                &outcome.judge,
                                classification,
                                diagnostics,
                                parameter_names,
                                case_hypotheses.clone(),
                                case_equations.clone(),
                                pending_citations.clone(),
                                outcome.value,
                            ));
                        }
                    }
                    return Some(leaves);
                }
                StatementNode::Call(_)
                    if !saw_transition && collect_citations && citation_preserves_values =>
                {
                    // The legacy citation route has no post-state actuals.
                    // A writing call already retired its live observations;
                    // staging them now would either revive old referents or
                    // discard this prefix when those referents are opaque.
                    let (target, argument_handles) =
                        citation_call_in_statement(program, statement)?;
                    stage_citation_argument_terms(
                        program,
                        judge,
                        invocation,
                        state,
                        &environment,
                        &target,
                        &argument_handles,
                        &mut pending_citations,
                    )?;
                }
                StatementNode::Call(_)
                    if !saw_transition && judge.requires_value_preservation() =>
                {
                    // The shared effect step above retired affected storage.
                }
                StatementNode::Call(call)
                    if !saw_transition
                        && is_citation_statement(program, classification, machine, call) =>
                {
                    // Entry citations were already intaken machine-wide.
                }
                StatementNode::Transition(transition) => {
                    saw_transition = true;
                    if transition.continuation.is_valid()
                        || (transition.exit == TransitionExit::Ordinary
                            && !transition.target.is_valid())
                    {
                        return None;
                    }
                    transitions.push((statement_index, transition));
                }
                // Assertions have their own checked obligations; ignoring an
                // assertion here cannot introduce its conclusion as a premise.
                StatementNode::Assembly(_) | StatementNode::AssemblyFact(_) if !saw_transition => {}
                _ => return None,
            }
        }
        if transitions.is_empty() {
            // The completed prefix also exists after the final statement of
            // a fallthrough body. Store observers must see that same state
            // whether another statement follows or the body returns Unit.
            // Explicit transitions own their paths below and never use this
            // synthetic end coordinate.
            if let Some(requirement) = requirement.as_deref_mut()
                && requirement.site().state == state.symbol
                && requirement.site().statement_index == state.statement_nodes.count() as usize
                && !requirement.site().in_transition_target
            {
                requirement.visit(
                    program,
                    invocation,
                    judge,
                    &environment,
                    &case_hypotheses,
                    &case_equations,
                    &established_guarantees,
                );
            }
            if requirement.is_some() {
                return Some(Vec::new());
            }
            if !matches!(
                program
                    .type_reference_table
                    .type_reference(state.return_type),
                typed_trees::types::TypeReferenceNode::Unit
            ) {
                return None;
            }
            return Some(vec![finalize_structural_case_arm(
                program,
                machine,
                judge,
                classification,
                diagnostics,
                parameter_names,
                case_hypotheses,
                case_equations,
                pending_citations,
                StructuralTerm::Opaque("()".to_owned()),
            )]);
        }
        // Arms test their guards in order. A later arm observes each earlier
        // Boolean guard as false, while the chosen arm alone receives its
        // positive refinement. Keep those completed observations on the
        // remaining path, never in a sibling's selected branch environment.
        let _s_trans = phase_profile::guard("05.d.7.3b.1.cap.2d.transitions");
        let mut leaves = Vec::new();
        let mut remaining_paths = vec![(judge.clone(), case_equations.clone())];
        for (statement_index, transition) in transitions {
            let mut next_remaining = Vec::new();
            let mut selected_paths = Vec::new();
            for (incoming, remaining_equations) in remaining_paths {
                if let Some(requirement) = requirement.as_deref_mut()
                    && requirement.site().state == state.symbol
                    && requirement.site().statement_index == statement_index
                    && !requirement.site().in_transition_target
                {
                    // The requested guard call has not returned. Only earlier
                    // guards and preceding operands can establish its premises.
                    requirement.visit(
                        program,
                        invocation,
                        &incoming,
                        &environment,
                        &case_hypotheses,
                        &remaining_equations,
                        &established_guarantees,
                    );
                    continue;
                }
                let TransitionGuardNode::When(guard) = transition.guard else {
                    selected_paths.push((
                        incoming,
                        environment.clone(),
                        case_hypotheses.clone(),
                        remaining_equations,
                    ));
                    continue;
                };
                let guard_preserves_values = judge
                    .frame_resolver()
                    .and_then(|frames| frames.expression_may_write_paths(invocation, guard))
                    .is_some_and(|paths| paths.is_empty());
                if incoming.requires_value_preservation() && !guard_preserves_values {
                    return None;
                }
                let (subject_expression, constructor) =
                    guard_observation(program, invocation, state, guard, fresh)?;
                let raw_subject = structural_term(
                    program,
                    Some((invocation, state, Default::default())),
                    None,
                    subject_expression,
                    None,
                );
                let observed = structural_arm_judge(
                    program,
                    &incoming,
                    &case_hypotheses,
                    &remaining_equations,
                );
                let subject_type = crate::expression_result_type_reference(
                    program,
                    invocation,
                    state,
                    subject_expression,
                )
                .unwrap_or_default();
                for mut outcome in observed.normal_return_paths(
                    subject_expression,
                    &environment,
                    (invocation, state, subject_type),
                )? {
                    let subject = outcome.value;
                    let mut branch_environment = environment.clone();
                    let mut branch_hypotheses = case_hypotheses.clone();
                    let mut branch_equations = remaining_equations.clone();
                    let mut skipped_equations = remaining_equations.clone();
                    let truth = guard_preserves_values
                        .then(|| {
                            let StructuralTerm::Constructor { data, case, fields } = &constructor
                            else {
                                return None;
                            };
                            (data == "bool" && fields.is_empty())
                                .then(|| {
                                    language_semantics::const_value::boolean_literal_spelling(case)
                                })
                                .flatten()
                        })
                        .flatten();
                    let subject_judgment = truth.map(|_| {
                        outcome.judge.judge_equation(
                            outcome.judge.resolve(subject.clone()),
                            constructor.clone(),
                            0,
                        )
                    });
                    // A Boolean has exactly two values. Nominal classifiers
                    // retain their selected tag but do not invent a single
                    // constructor for the complement of a multi-case type.
                    if !matches!(subject_judgment, Some(StructuralJudgment::Proven)) {
                        if let Some(truth) = truth {
                            skipped_equations.push((
                                subject.clone(),
                                StructuralTerm::Constructor {
                                    data: "bool".to_owned(),
                                    case: (!truth).to_string(),
                                    fields: Vec::new(),
                                },
                            ));
                        }
                        next_remaining.push((outcome.judge.clone(), skipped_equations));
                    }
                    if matches!(subject_judgment, Some(StructuralJudgment::Refuted)) {
                        continue;
                    }
                    if let Some((definition, variant)) =
                        super::structural_terms::case_guard_classifier(
                            program,
                            invocation,
                            Some(state),
                            guard,
                        )
                        && let Some(subject_type) =
                            crate::value_custody::field_domains::readable_type_reference(
                                program,
                                subject_type,
                            )
                    {
                        // Only this successful arm owns the case predicate.
                        // Failed and sibling arms retain their prior context.
                        outcome.judge.intake_selected_case_predicates(
                            definition,
                            variant,
                            &constructor,
                            subject_type,
                            invocation,
                            state,
                            &environment,
                        );
                    }
                    match subject {
                        StructuralTerm::Variable(subject) => {
                            branch_hypotheses.push((subject, constructor.clone()));
                        }
                        subject => branch_equations.push((subject, constructor.clone())),
                    }
                    if let Some(StructuralTerm::Variable(raw_name)) = &raw_subject {
                        let (_, binding) = branch_environment
                            .iter_mut()
                            .find(|(name, _)| name == raw_name)?;
                        *binding = constructor.clone();
                    }
                    selected_paths.push((
                        outcome.judge,
                        branch_environment,
                        branch_hypotheses,
                        branch_equations,
                    ));
                }
            }
            if requirement.as_deref().is_some_and(|requirement| {
                requirement.site().state == state.symbol
                    && requirement.site().statement_index == statement_index
                    && !requirement.site().in_transition_target
            }) {
                return Some(Vec::new());
            }
            remaining_paths = next_remaining;
            for (captured, branch_environment, branch_hypotheses, branch_equations) in
                selected_paths
            {
                let judge = &captured;

                if let Some(requirement) = requirement.as_deref_mut()
                    && requirement.site().state == state.symbol
                    && requirement.site().statement_index == statement_index
                    && requirement.site().in_transition_target
                {
                    requirement.visit(
                        program,
                        invocation,
                        judge,
                        &branch_environment,
                        &branch_hypotheses,
                        &branch_equations,
                        &established_guarantees,
                    );
                    continue;
                }
                let mut branch_judge =
                    structural_arm_judge(program, judge, &branch_hypotheses, &branch_equations);
                let judge = &mut branch_judge;
                // A crash closes this selected execution path without a normal
                // result or later call occurrence. Its ceiling and authority
                // remain separately checked; excluding the path imports no
                // fact and does not license a denotational use of this machine.
                if matches!(transition.exit, TransitionExit::Crash(_)) {
                    continue;
                }
                match program.statement_table.transition_target(transition.target) {
                    TransitionTargetNode::Value(value) => {
                        if requirement.is_some() {
                            continue;
                        }
                        for outcome in judge.normal_return_paths(
                            *value,
                            &branch_environment,
                            (invocation, state, state.return_type),
                        )? {
                            leaves.push(finalize_structural_case_arm(
                                program,
                                machine,
                                &outcome.judge,
                                classification,
                                diagnostics,
                                parameter_names,
                                branch_hypotheses.clone(),
                                branch_equations.clone(),
                                pending_citations.clone(),
                                outcome.value,
                            ));
                        }
                    }
                    TransitionTargetNode::Named {
                        path: target,
                        arguments,
                        ..
                    } => {
                        let target_state =
                            crate::machine_calls::calls::named_transition_target_state(
                                program,
                                invocation,
                                state,
                                transition.target,
                            )
                            .or_else(|| {
                                // Unfold checked, observation-free bodies,
                                // regardless of nesting or result relevance.
                                // Boundary declarations supply no body evidence.
                                let callee = program.machine_by_symbol(target.symbol)?;
                                (callee.body_is_present
                                    && callee.supply_mode.is_checked_body()
                                    && !callee.target_family_interface
                                    && transparent_body_is_observation_free(
                                        program,
                                        callee,
                                        classification,
                                    ))
                                .then(|| program.machine_states(callee).first())?
                            })?;
                        if requirement.as_deref().is_some_and(|observer| {
                            observer.site().state == state.symbol
                                || !prefix_site_is_reachable(program, target_state, observer)
                        }) {
                            // A jump leaves this state invocation; a cycle back
                            // is a new arrival, not execution of its later arm.
                            // Other states are traversed only when they can
                            // reach the queried occurrence. Each recurrent
                            // arrival separately owes its state contract.
                            continue;
                        }
                        let target_parameters = program.state_parameters(target_state);
                        let argument_handles =
                            program.statement_table.expression_handles(*arguments);
                        if target_parameters.len() != argument_handles.len() {
                            return None;
                        }
                        let mut target_frames = vec![(Vec::new(), judge.clone())];
                        for (parameter, argument) in target_parameters.iter().zip(argument_handles)
                        {
                            let mut next = Vec::new();
                            for (target_environment, captured) in target_frames {
                                for outcome in captured.normal_return_paths(
                                    *argument,
                                    &branch_environment,
                                    (invocation, state, parameter.type_reference),
                                )? {
                                    let mut target_environment = target_environment.clone();
                                    target_environment
                                        .push((parameter.name.as_str().to_owned(), outcome.value));
                                    next.push((target_environment, outcome.judge));
                                }
                            }
                            target_frames = next;
                        }
                        for (target_environment, captured) in target_frames {
                            leaves.extend(recognize_structural_state_leaves(
                                program,
                                machine,
                                &captured,
                                classification,
                                diagnostics,
                                states,
                                target_state,
                                parameter_names,
                                target_environment,
                                branch_hypotheses.clone(),
                                branch_equations.clone(),
                                pending_citations.clone(),
                                true,
                                path,
                                fresh,
                                requirement,
                                established_guarantees.clone(),
                            )?);
                        }
                    }
                    _ => return None,
                }
            }
            if requirement.as_deref().is_some_and(|observer| {
                observer.site().state == state.symbol
                    && observer.site().statement_index == statement_index
            }) {
                // All normal guard outcomes at the requested occurrence were
                // visited above. Later arms cannot establish or invalidate it;
                // in particular, do not unroll their recurrent successors.
                return Some(leaves);
            }
        }
        Some(leaves)
    })();

    if start_index == 0 {
        path.pop();
    }
    result
}

/// Reachability is over exact call identities, not membership in the root
/// machine's former state list. This is only a pruning query: the path walker
/// still checks each entered body and substitutes every actual argument.
fn prefix_site_is_reachable(
    program: &dyn crate::CheckingInput,
    source: &typed_trees::state::State,
    observer: &dyn StructuralPrefixVisitor,
) -> bool {
    let mut pending = vec![source.symbol];
    let mut visited = Vec::new();
    while let Some(symbol) = pending.pop() {
        if visited.contains(&symbol) {
            continue;
        }
        visited.push(symbol);
        let Some((owner, state)) =
            crate::machine_calls::calls::machine_state_by_symbol(program, symbol)
        else {
            return true;
        };
        if observer.stops_at_machine(owner.symbol) {
            continue;
        }
        if symbol == observer.site().state {
            return true;
        }
        for statement in program.statement_table.statements(state.statement_nodes) {
            let StatementNode::Transition(transition) = statement else {
                continue;
            };
            for target in [transition.target, transition.continuation] {
                let TransitionTargetNode::Named { path, .. } =
                    program.statement_table.transition_target(target)
                else {
                    continue;
                };
                let Some((_, destination)) =
                    crate::machine_calls::calls::machine_state_by_symbol(program, path.symbol)
                        .or_else(|| {
                            let machine = program.machine_by_symbol(path.symbol)?;
                            Some((machine, program.machine_states(machine).first()?))
                        })
                else {
                    return true;
                };
                pending.push(destination.symbol);
            }
        }
    }
    false
}

/// A proof may finish through an ordinary pure helper, including an empty
/// Unit body with no proof-data parameters. Runtime classification alone does
/// not make such a body opaque. Reuse denotational effect admission rather
/// than recognizing a special empty-body or nested-declaration shape.
fn transparent_body_is_observation_free(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    classification: &typed_trees::proof_only::ProofOnlyClassification,
) -> bool {
    if classification.is_proof_machine(program, machine) {
        return true;
    }
    let Some(entry) = program.machine_states(machine).first() else {
        return false;
    };
    let eligible = |operational: &flow_effects::OperationalPlan,
                    reaches: &flow_effects::ServiceReachInferencePlan| {
        crate::machine_calls::denotational_calls::checked_value_call_candidate(
            program,
            machine,
            entry,
            operational,
            reaches,
        )
        .is_ok()
    };
    if let Some(plans) = program.prepared_effect_plans() {
        return eligible(&plans.operational, &plans.service_reaches);
    }
    let operational = crate::machine_calls::effect_inference::infer_operational_may(program);
    let reaches =
        crate::machine_calls::effect_inference::infer_service_reaches(program, &operational);
    eligible(&operational, &reaches)
}

#[allow(clippy::too_many_arguments)]
fn finalize_structural_case_arm<'program>(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    judge: &StructuralJudge<'program>,
    classification: &typed_trees::proof_only::ProofOnlyClassification,
    diagnostics: &mut Vec<Diagnostic>,
    parameter_names: &[String],
    case_hypotheses: Vec<(String, StructuralTerm)>,
    case_equations: Vec<(StructuralTerm, StructuralTerm)>,
    pending_citations: Vec<PendingStructuralCitation>,
    value: StructuralTerm,
) -> StructuralCaseArm<'program> {
    let mut arm_judge = structural_arm_judge(program, judge, &case_hypotheses, &case_equations);
    let vacuous = arm_judge.hypotheses_contradictory;

    // The induction hypothesis is available before an authored citation only
    // when the recursive application's own preconditions are already proven
    // at that point.  This is the conditional theorem rule: a requires-bearing
    // self-call never contributes its ensures merely because it descends.
    // Membership premises remain outside the structural language and suppress
    // the IH entirely.
    intake_available_self_induction_hypotheses(
        program,
        machine,
        parameter_names,
        &value,
        &mut arm_judge,
    );

    let mut citations = Vec::new();
    if !vacuous {
        let machine_has_requires = program.machine_contracts(machine).iter().any(|contract| {
            contract.kind == typed_trees::signature::SignatureContractKind::Requires
                && !program.proof_facts.span_or_empty(contract.facts).is_empty()
        });
        for (target, arguments) in pending_citations {
            let established = instantiate_citation(
                program,
                classification,
                machine,
                &target,
                &arguments,
                diagnostics,
                Some(&arm_judge),
                true,
            );
            established.intake(&mut arm_judge);
            citations.extend(established.equations);
            // An earlier citation may establish a recursive application's
            // requires, making its conditional IH available to later
            // citations in the same authored statement order.
            if machine_has_requires {
                intake_available_self_induction_hypotheses(
                    program,
                    machine,
                    parameter_names,
                    &value,
                    &mut arm_judge,
                );
            }
        }
    }

    StructuralCaseArm {
        normal_judge: arm_judge,
        parameter_names: parameter_names.to_vec(),
        case_hypotheses,
        case_equations,
        citations,
        normal_guarantees: judge.normal_guarantees.to_vec(),
        value,
    }
}

pub(super) fn structural_arm_judge<'program>(
    program: &dyn crate::CheckingInput,
    judge: &StructuralJudge<'program>,
    case_hypotheses: &[(String, StructuralTerm)],
    case_equations: &[(StructuralTerm, StructuralTerm)],
) -> StructuralJudge<'program> {
    let mut arm_judge = judge.clone();
    if case_hypotheses.is_empty() && case_equations.is_empty() {
        return arm_judge;
    }
    for (subject, constructor) in case_equations {
        arm_judge.intake_case_equation(subject.clone(), constructor.clone(), 0);
        arm_judge.intake_equation(subject.clone(), constructor.clone(), 0);
    }
    for (subject, constructor) in case_hypotheses {
        arm_judge.intake_case_equation(
            StructuralTerm::Variable(subject.clone()),
            constructor.clone(),
            0,
        );
        std::sync::Arc::make_mut(&mut arm_judge.substitutions)
            .insert(0, (subject.clone(), constructor.clone()));
    }
    // The supplied judge owns the exact invocation premises. Re-importing the
    // enclosing machine's entry contract would donate stale facts to a named
    // recurrent state whose same-spelled formals are distinct declarations.
    let requires = judge.premise_facts().to_vec();
    let vacuous = requires
        .iter()
        .any(|fact| matches!(arm_judge.judge(program, *fact), StructuralJudgment::Refuted));
    for fact in &requires {
        arm_judge.intake(program, *fact);
        arm_judge.intake_preserved_predicate(*fact);
    }
    arm_judge.hypotheses_contradictory |= vacuous;
    arm_judge
}

/// Observe the selected value, rather than recognizing a spelling of a
/// Boolean guard. Classifiers retain their nominal payload refinement; all
/// other Boolean expressions use the ordinary completed-expression path.
fn guard_observation(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &typed_trees::state::State,
    guard: ExpressionHandle,
    fresh: &mut usize,
) -> Option<(ExpressionHandle, StructuralTerm)> {
    if let Some(comparison) = program.expression_table.binary_node(guard) {
        if comparison.operator == BinaryOperator::Equal
            && crate::has_builtin_binary_expression_meaning(program, machine, Some(state), guard)
            && let ExpressionNode::Boolean(truth) =
                program.expression_table.expression(comparison.right)
        {
            return Some((
                comparison.left,
                StructuralTerm::Constructor {
                    data: "bool".to_owned(),
                    case: truth.to_string(),
                    fields: Vec::new(),
                },
            ));
        }
        if super::structural_terms::case_guard_classifier(program, machine, Some(state), guard)
            .is_some()
        {
            return Some((
                comparison.left,
                guard_case_value(program, machine, state, guard, fresh)?,
            ));
        }
    }
    let reference = crate::expression_result_type_reference(program, machine, state, guard)?;
    if program.primitive_type_reference(reference) != Some(typed_trees::types::PrimitiveType::Bool)
    {
        return None;
    }
    Some((
        guard,
        StructuralTerm::Constructor {
            data: "bool".to_owned(),
            case: "true".to_owned(),
            fields: Vec::new(),
        },
    ))
}

/// Nominal cases expose only their tag; payload and common fields stay fresh.
fn guard_case_value(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &typed_trees::state::State,
    guard: ExpressionHandle,
    fresh: &mut usize,
) -> Option<StructuralTerm> {
    let (definition, variant) =
        super::structural_terms::case_guard_classifier(program, machine, Some(state), guard)?;
    let data = definition.name.as_str().to_owned();
    let case = variant.name.as_str().to_owned();
    let variant_fields: Vec<String> = program
        .data_payload_fields(variant)
        .iter()
        .map(|field| field.name.as_str().to_owned())
        .collect();
    let branch_id = *fresh;
    *fresh += 1;
    // Matching determines the case, never the values of its common
    // fields. Keep those fields symbolic beside the active payload.
    let common_fields = program
        .data_members(definition)
        .iter()
        .filter_map(|member| match member {
            typed_trees::data::DataMember::Field(field) => Some(field.name.as_str().to_owned()),
            _ => None,
        });
    let mut fields: Vec<(String, StructuralTerm)> = variant_fields
        .into_iter()
        .chain(common_fields)
        .map(|field| {
            // These binders must never alias an authored identifier.
            // The NUL prefix is outside the source name alphabet.
            let variable = format!("\0__ih_{}_{}_{}", state.name.as_str(), branch_id, field);
            (field, StructuralTerm::Variable(variable))
        })
        .collect();
    fields.sort_by(|(left, _), (right, _)| left.cmp(right));
    Some(StructuralTerm::Constructor { data, case, fields })
}

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
        let mut transitions = Vec::new();
        let mut saw_transition = false;
        for (statement_index, statement) in program
            .statement_table
            .indexed_statements(state.statement_nodes)
            .filter(|(statement_index, _)| *statement_index >= start_index)
        {
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
                    machine,
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
                let frames = judge.frame_resolver()?;
                let mut writes = Vec::new();
                for frame in [
                    frames.may_write_frame(machine, call),
                    frames.statement_value_write_frame(machine, statement),
                ] {
                    writes.extend(frame.into_complete_paths()?);
                }
                citation_preserves_values = writes.is_empty();
                judge.observe_storage_writes(state, &writes)?;
            }
            if requirement.is_some()
                && citation_preserves_values
                && !saw_transition
                && (!judge.has_runtime_body_values() || matches!(statement, StatementNode::Call(_)))
                && let Some((target, arguments)) = citation_call_in_statement(program, statement)
            {
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
                                .map(|expected| (machine, state, *expected)),
                            *argument,
                            &environment,
                            0,
                        )
                    })
                    .collect::<Option<Vec<_>>>()?;
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
            }
            match statement {
                StatementNode::LocalData(local) if !saw_transition => {
                    let outcomes = judge.normal_return_paths(
                        local.initial_value,
                        &environment,
                        (machine, state, local.type_reference),
                    )?;
                    let mut leaves = Vec::new();
                    for outcome in outcomes {
                        let mut pending_citations = pending_citations.clone();
                        if collect_citations
                            && let Some((target, argument_handles)) =
                                citation_call_in_statement(program, statement)
                        {
                            stage_citation_argument_terms(
                                program,
                                &outcome.judge,
                                machine,
                                state,
                                &environment,
                                &target,
                                &argument_handles,
                                &mut pending_citations,
                            )?;
                        }
                        let mut environment = environment.clone();
                        environment.push((local.name.as_str().to_owned(), outcome.value));
                        leaves.extend(recognize_structural_state_leaves_from(
                            program,
                            machine,
                            &outcome.judge,
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
                            statement_index + 1,
                        )?);
                    }
                    return Some(leaves);
                }
                StatementNode::Assignment(_)
                    if !saw_transition && judge.has_runtime_body_values() =>
                {
                    let outcomes = judge.normal_assignment_paths(
                        statement,
                        &environment,
                        (machine, state, state.return_type),
                    )?;
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
                            statement_index + 1,
                        )?);
                    }
                    return Some(leaves);
                }
                StatementNode::Expression(expression) if !saw_transition => {
                    let is_tail = statement_index + 1
                        == program
                            .statement_table
                            .statements(state.statement_nodes)
                            .len();
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
                    let outcomes = judge.normal_return_paths(
                        *expression,
                        &environment,
                        (machine, state, result_type),
                    )?;
                    let mut leaves = Vec::new();
                    for outcome in outcomes {
                        if is_tail {
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
                        } else {
                            // An evaluated expression is not an early return.
                            // Resume with its own facts and discard only its
                            // value; later statements still own the result.
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
                                statement_index + 1,
                            )?);
                        }
                    }
                    return Some(leaves);
                }
                StatementNode::Call(_) if !saw_transition && collect_citations => {
                    let (target, argument_handles) =
                        citation_call_in_statement(program, statement)?;
                    stage_citation_argument_terms(
                        program,
                        judge,
                        machine,
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
                _ => return None,
            }
        }
        if transitions.is_empty() {
            if requirement.is_some() {
                return Some(Vec::new());
            }
            if state.return_type.is_valid() {
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
                        machine,
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
                    .and_then(|frames| frames.expression_may_write_paths(machine, guard))
                    .is_some_and(|paths| paths.is_empty());
                if incoming.requires_value_preservation() && !guard_preserves_values {
                    return None;
                }
                let (subject_expression, constructor) =
                    guard_observation(program, machine, state, guard, fresh)?;
                let raw_subject = structural_term(
                    program,
                    Some((machine, state, Default::default())),
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
                    machine,
                    state,
                    subject_expression,
                )
                .unwrap_or_default();
                for outcome in observed.normal_return_paths(
                    subject_expression,
                    &environment,
                    (machine, state, subject_type),
                )? {
                    let subject = outcome.value;
                    let mut branch_environment = environment.clone();
                    let mut branch_hypotheses = case_hypotheses.clone();
                    let mut branch_equations = remaining_equations.clone();
                    let mut skipped_equations = remaining_equations.clone();
                    // A Boolean has exactly two values. Nominal classifiers
                    // retain their selected tag but do not invent a single
                    // constructor for the complement of a multi-case type.
                    if guard_preserves_values
                        && let StructuralTerm::Constructor { data, case, fields } = &constructor
                        && data == "bool"
                        && fields.is_empty()
                        && let Some(truth) =
                            language_semantics::const_value::boolean_literal_spelling(case)
                    {
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
                        machine,
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
                            (machine, state, state.return_type),
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
                        let [state_name] =
                            program.statement_table.name_path_members(target.members)
                        else {
                            return None;
                        };
                        let target_state = states[1..]
                            .iter()
                            .find(|candidate| candidate.name.as_str() == state_name.as_str())?;
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
                                    (machine, state, parameter.type_reference),
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
        }
        Some(leaves)
    })();

    if start_index == 0 {
        path.pop();
    }
    result
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
            intake_available_self_induction_hypotheses(
                program,
                machine,
                parameter_names,
                &value,
                &mut arm_judge,
            );
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

//! Guarded structural case arms and structural state leaves.

use super::call_requirements::{CallRequirement, establish_citation};
use crate::proof_contracts::contract_entailment::citations::{
    citation_call_in_statement, instantiate_citation, is_citation_statement, machine_requires_facts,
};
use crate::proof_contracts::contract_entailment::is_arm_pattern_marker;
use crate::proof_contracts::contract_entailment::self_induction::intake_available_self_induction_hypotheses;
use crate::proof_contracts::contract_entailment::structural_judgment::{
    StructuralJudge, StructuralJudgment, StructuralTerm,
};
use crate::proof_contracts::contract_entailment::structural_terms::structural_term;
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};

#[cfg(test)]
mod tests;

/// N3 rung 1: the structural mini-judge for contract conjuncts over
/// proof-only data. Term language (bounded by today's contract grammar --
/// struct literals do not parse in fact position, so payload-carrying
/// constructor terms and their injectivity decomposition are the recorded
/// next rung): variables (single-segment names), nullary case classifiers
/// (`Nat::Zero`), and opaque applications compared only by display name.
/// `requires` equalities with a variable side become directed substitutions
/// (first binding wins; symmetry and transitivity fall out of resolution);
/// two distinct nullary cases equated make the hypotheses CONTRADICTORY and
/// every goal holds vacuously, mirroring the polynomial engine's rule.
/// One recognized case arm of a structurally-inductive proof machine.
pub(crate) struct StructuralCaseArm {
    /// The machine's short (call-target) name, for self-application search.
    pub(crate) machine_name: String,
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
pub(crate) fn recognize_guarded_structural_value_arms(
    program: &TypedTrees,
    machine: &Machine,
    judge: &StructuralJudge<'_>,
) -> Option<Vec<StructuralCaseArm>> {
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
        let ExpressionNode::Binary(equality) = program.expression_table.expression(guard) else {
            return None;
        };
        if equality.operator != BinaryOperator::Equal {
            return None;
        }
        let ExpressionNode::Boolean(polarity) = program.expression_table.expression(equality.right)
        else {
            return None;
        };
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
    let machine_name = machine
        .name
        .as_str()
        .rsplit("::")
        .next()
        .unwrap_or(machine.name.as_str())
        .to_owned();

    [first_target, second_target]
        .into_iter()
        .map(|target| {
            let TransitionTargetNode::Value(value) =
                program.statement_table.transition_target(target)
            else {
                return None;
            };
            Some(StructuralCaseArm {
                machine_name: machine_name.clone(),
                parameter_names: parameter_names.clone(),
                case_hypotheses: Vec::new(),
                case_equations: Vec::new(),
                citations: Vec::new(),
                value: judge.callee_term(*value, &environment, 0)?,
            })
        })
        .collect()
}

/// Equality for the duplicated condition trees produced by boolean-arm
/// lowering.  This deliberately recognizes only the pure scalar grammar
/// needed to prove that `condition == true` and `condition == false` are the
/// two faces of ONE condition; unsupported trees simply refuse the
/// structural-induction shortcut.
fn guard_expressions_equal(
    program: &TypedTrees,
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

type PendingStructuralCitation = (typed_trees::name::Identifier, Vec<StructuralTerm>);

/// Recognize a proof machine as a tree of structural case states. Each named
/// state can either terminate in a value or refine another subject and hand
/// the branch to a further named state. Unsupported statement order, an
/// unresolved target, or a state cycle fails the whole recognition closed.
pub(crate) fn recognize_structural_case_arms(
    program: &TypedTrees,
    machine: &Machine,
    judge: &StructuralJudge<'_>,
    classification: &typed_trees::proof_only::ProofOnlyClassification,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Vec<StructuralCaseArm>> {
    recognize_structural_case_arms_with_requirement(
        program,
        machine,
        judge,
        classification,
        diagnostics,
        None,
    )
}

pub(super) fn recognize_structural_case_arms_with_requirement(
    program: &TypedTrees,
    machine: &Machine,
    judge: &StructuralJudge<'_>,
    classification: &typed_trees::proof_only::ProofOnlyClassification,
    diagnostics: &mut Vec<Diagnostic>,
    mut requirement: Option<&mut CallRequirement<'_>>,
) -> Option<Vec<StructuralCaseArm>> {
    let states = program.machine_states(machine);
    let root = states.first()?;
    let machine_name = machine
        .name
        .as_str()
        .rsplit("::")
        .next()
        .unwrap_or(machine.name.as_str())
        .to_owned();
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
        &machine_name,
        &parameter_names,
        environment,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        false,
        &mut path,
        &mut fresh,
        &mut requirement,
        Vec::new(),
    )?;
    (!arms.is_empty() || requirement.is_some()).then_some(arms)
}

#[allow(clippy::too_many_arguments)]
fn recognize_structural_state_leaves(
    program: &TypedTrees,
    machine: &Machine,
    judge: &StructuralJudge<'_>,
    classification: &typed_trees::proof_only::ProofOnlyClassification,
    diagnostics: &mut Vec<Diagnostic>,
    states: &[typed_trees::state::State],
    state: &typed_trees::state::State,
    machine_name: &str,
    parameter_names: &[String],
    mut environment: Vec<(String, StructuralTerm)>,
    case_hypotheses: Vec<(String, StructuralTerm)>,
    case_equations: Vec<(StructuralTerm, StructuralTerm)>,
    mut pending_citations: Vec<PendingStructuralCitation>,
    collect_citations: bool,
    path: &mut Vec<SymbolHandle>,
    fresh: &mut usize,
    requirement: &mut Option<&mut CallRequirement<'_>>,
    mut established_guarantees: Vec<(StructuralTerm, StructuralTerm)>,
) -> Option<Vec<StructuralCaseArm>> {
    if !state.symbol.is_valid() || path.contains(&state.symbol) {
        return None;
    }
    path.push(state.symbol);

    let result = (|| {
        let mut transitions = Vec::new();
        let mut saw_transition = false;
        for (statement_index, statement) in program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .enumerate()
        {
            if is_arm_pattern_marker(statement) {
                continue;
            }
            if let Some(requirement) = requirement.as_deref_mut()
                && requirement.state == state.symbol
                && requirement.statement_index == statement_index
                && (!requirement.in_transition_target
                    || !matches!(statement, StatementNode::Transition(_)))
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
            if requirement.is_some()
                && !saw_transition
                && let Some((target, arguments)) = citation_call_in_statement(program, statement)
            {
                let arguments = arguments
                    .iter()
                    .map(|argument| judge.callee_term(*argument, &environment, 0))
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
                    target,
                    &arguments,
                );
            }
            match statement {
                StatementNode::LocalData(local) if !saw_transition => {
                    let term = judge.callee_term(local.initial_value, &environment, 0)?;
                    if collect_citations
                        && let Some((target, argument_handles)) =
                            citation_call_in_statement(program, statement)
                    {
                        let argument_terms = argument_handles
                            .iter()
                            .map(|argument| judge.callee_term(*argument, &environment, 0))
                            .collect::<Option<Vec<_>>>()?;
                        pending_citations.push((target.clone(), argument_terms));
                    }
                    environment.push((local.name.as_str().to_owned(), term));
                }
                StatementNode::Call(_) if !saw_transition && collect_citations => {
                    let (target, argument_handles) =
                        citation_call_in_statement(program, statement)?;
                    let argument_terms = argument_handles
                        .iter()
                        .map(|argument| judge.callee_term(*argument, &environment, 0))
                        .collect::<Option<Vec<_>>>()?;
                    pending_citations.push((target.clone(), argument_terms));
                }
                StatementNode::Call(call)
                    if !saw_transition
                        && is_citation_statement(program, classification, machine, call) =>
                {
                    // Entry citations were already intaken machine-wide.
                }
                StatementNode::Transition(transition) => {
                    saw_transition = true;
                    if transition.continuation.is_valid() || !transition.target.is_valid() {
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
                machine_name,
                parameter_names,
                case_hypotheses,
                case_equations,
                pending_citations,
                StructuralTerm::Opaque("()".to_owned()),
            )]);
        }
        if transitions.len() > 1
            && transitions
                .iter()
                .any(|(_, transition)| matches!(transition.guard, TransitionGuardNode::Always))
        {
            return None;
        }

        let mut leaves = Vec::new();
        for (statement_index, transition) in transitions {
            let mut branch_environment = environment.clone();
            let mut branch_hypotheses = case_hypotheses.clone();
            let mut branch_equations = case_equations.clone();
            if let TransitionGuardNode::When(guard) = transition.guard {
                let ExpressionNode::Binary(comparison) = program.expression_table.expression(guard)
                else {
                    return None;
                };
                if !matches!(
                    comparison.operator,
                    BinaryOperator::Equal | BinaryOperator::CaseMembership
                ) {
                    return None;
                }
                let raw_subject = structural_term(program, comparison.left)?;
                let subject = judge.callee_term(comparison.left, &branch_environment, 0)?;
                let (definition, variant) = super::structural_terms::case_guard_classifier(
                    program,
                    machine,
                    Some(state),
                    guard,
                )?;
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
                        typed_trees::data::DataMember::Field(field) => {
                            Some(field.name.as_str().to_owned())
                        }
                        _ => None,
                    });
                let mut fields: Vec<(String, StructuralTerm)> = variant_fields
                    .into_iter()
                    .chain(common_fields)
                    .map(|field| {
                        let variable =
                            format!("__ih_{}_{}_{}", state.name.as_str(), branch_id, field);
                        (field, StructuralTerm::Variable(variable))
                    })
                    .collect();
                fields.sort_by(|(left, _), (right, _)| left.cmp(right));
                let constructor = StructuralTerm::Constructor { data, case, fields };

                match subject {
                    StructuralTerm::Variable(subject) => {
                        branch_hypotheses.push((subject, constructor.clone()));
                    }
                    StructuralTerm::Application { .. } => {
                        branch_equations.push((subject, constructor.clone()));
                    }
                    _ => return None,
                }
                if let StructuralTerm::Variable(raw_name) = raw_subject {
                    let (_, binding) = branch_environment
                        .iter_mut()
                        .find(|(name, _)| name == &raw_name)?;
                    *binding = constructor;
                } else if matches!(raw_subject, StructuralTerm::Application { .. }) {
                    // A computed subject has no environment binding to refine.
                } else {
                    return None;
                }
            }

            if let Some(requirement) = requirement.as_deref_mut()
                && requirement.state == state.symbol
                && requirement.statement_index == statement_index
                && requirement.in_transition_target
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
            match program.statement_table.transition_target(transition.target) {
                TransitionTargetNode::Value(value) => {
                    if requirement.is_some() {
                        continue;
                    }
                    let value = judge.callee_term(*value, &branch_environment, 0)?;
                    leaves.push(finalize_structural_case_arm(
                        program,
                        machine,
                        judge,
                        classification,
                        diagnostics,
                        machine_name,
                        parameter_names,
                        branch_hypotheses,
                        branch_equations,
                        pending_citations.clone(),
                        value,
                    ));
                }
                TransitionTargetNode::Named {
                    path: target,
                    arguments,
                    ..
                } => {
                    let [state_name] = program.statement_table.name_path_members(target.members)
                    else {
                        return None;
                    };
                    let target_state = states[1..]
                        .iter()
                        .find(|candidate| candidate.name.as_str() == state_name.as_str())?;
                    let target_parameters = program.state_parameters(target_state);
                    let argument_handles = program.statement_table.expression_handles(*arguments);
                    if target_parameters.len() != argument_handles.len() {
                        return None;
                    }
                    let target_environment = target_parameters
                        .iter()
                        .zip(argument_handles)
                        .map(|(parameter, argument)| {
                            Some((
                                parameter.name.as_str().to_owned(),
                                judge.callee_term(*argument, &branch_environment, 0)?,
                            ))
                        })
                        .collect::<Option<Vec<_>>>()?;
                    leaves.extend(recognize_structural_state_leaves(
                        program,
                        machine,
                        judge,
                        classification,
                        diagnostics,
                        states,
                        target_state,
                        machine_name,
                        parameter_names,
                        target_environment,
                        branch_hypotheses,
                        branch_equations,
                        pending_citations.clone(),
                        true,
                        path,
                        fresh,
                        requirement,
                        established_guarantees.clone(),
                    )?);
                }
                _ => return None,
            }
        }
        Some(leaves)
    })();

    path.pop();
    result
}

#[allow(clippy::too_many_arguments)]
fn finalize_structural_case_arm(
    program: &TypedTrees,
    machine: &Machine,
    judge: &StructuralJudge<'_>,
    classification: &typed_trees::proof_only::ProofOnlyClassification,
    diagnostics: &mut Vec<Diagnostic>,
    machine_name: &str,
    parameter_names: &[String],
    case_hypotheses: Vec<(String, StructuralTerm)>,
    case_equations: Vec<(StructuralTerm, StructuralTerm)>,
    pending_citations: Vec<PendingStructuralCitation>,
    value: StructuralTerm,
) -> StructuralCaseArm {
    let mut arm_judge =
        structural_arm_judge(program, machine, judge, &case_hypotheses, &case_equations);
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
        machine_name,
        parameter_names,
        &value,
        &mut arm_judge,
    );

    let mut citations = Vec::new();
    if !vacuous {
        for (target, arguments) in pending_citations {
            let before = citations.len();
            instantiate_citation(
                program,
                classification,
                machine,
                &target,
                &arguments,
                diagnostics,
                &mut citations,
                Some(&arm_judge),
                true,
            );
            for (left, right) in &citations[before..] {
                arm_judge.intake_equation(left.clone(), right.clone(), 0);
            }
            // An earlier citation may establish a recursive application's
            // requires, making its conditional IH available to later
            // citations in the same authored statement order.
            intake_available_self_induction_hypotheses(
                program,
                machine,
                machine_name,
                parameter_names,
                &value,
                &mut arm_judge,
            );
        }
    }

    StructuralCaseArm {
        machine_name: machine_name.to_owned(),
        parameter_names: parameter_names.to_vec(),
        case_hypotheses,
        case_equations,
        citations,
        value,
    }
}

pub(super) fn structural_arm_judge<'program>(
    program: &TypedTrees,
    machine: &Machine,
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
        arm_judge
            .substitutions
            .insert(0, (subject.clone(), constructor.clone()));
    }
    let requires = machine_requires_facts(program, machine);
    let vacuous = requires
        .iter()
        .any(|fact| matches!(arm_judge.judge(program, *fact), StructuralJudgment::Refuted));
    for fact in &requires {
        arm_judge.intake(program, *fact);
    }
    arm_judge.hypotheses_contradictory |= vacuous;
    arm_judge
}

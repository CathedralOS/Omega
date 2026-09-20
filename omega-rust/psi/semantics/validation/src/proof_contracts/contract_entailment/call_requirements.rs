//! Positive call-premise judgments at a structural statement boundary.
//!
//! Reuse the case traversal's argument environment and prior citations. A
//! recursive call's eventual result and later citations are not entry evidence.
//! A licensed induction hypothesis is different: it may establish an earlier
//! premise, but only once its own premises hold in that earlier context.
//! The checked caller separately establishes that entry values cannot be
//! mutated; this judgment does not replace the separate recursion validator.
//! Runtime calls reuse the traversal with opaque invocation results and exact
//! builtin equality domains. Empty write frames alone do not make observations
//! deterministic, and IEEE equality is not structural reflexivity.

use super::citations::{
    CitationFacts, CitationTarget, instantiate_citation, machine_requires_facts,
};
use super::refuted_requires::instantiated_fact_judgment;
use super::self_induction::intake_available_self_induction_hypotheses;
use super::structural_case_arms::{
    recognize_structural_case_arms_with_requirement, structural_arm_judge,
};
use super::structural_judgment::{StructuralJudge, StructuralJudgment, StructuralTerm};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};

/// Check one exact substituted requirement on every recognized path to a call.
/// Unsupported paths and unknown judgments do not establish the requirement.
/// Callers must preserve the machine's entry premises and provide the selected
/// callee state and actual arguments, not a display-name reconstruction.
#[allow(clippy::too_many_arguments)]
pub fn structural_call_requirement_entailed(
    program: &TypedTrees,
    machine: &Machine,
    state: SymbolHandle,
    statement_index: usize,
    in_transition_target: bool,
    callee: &State,
    arguments: &[ExpressionHandle],
    expression: ExpressionHandle,
) -> bool {
    let classification = typed_trees::proof_only::classify(program);
    let Some(entry) = program.machine_states(machine).first() else {
        return false;
    };
    if !crate::machine_calls::calls::named_state_transition_subgraph_is_acyclic(
        program, machine, entry,
    ) || program
        .state_parameters(callee)
        .iter()
        .any(|parameter| parameter.is_self)
    {
        return false;
    }
    let Some(callee_machine) = program.machines().iter().find(|candidate| {
        program
            .machine_states(candidate)
            .iter()
            .any(|state| state.symbol == callee.symbol)
    }) else {
        return false;
    };
    let runtime_values = !classification.is_proof_machine(program, machine)
        || !classification.is_proof_machine(program, callee_machine);
    let requires = machine_requires_facts(program, machine);
    if runtime_values
        && (!runtime_equality_fact(program, callee_machine, callee, expression)
            || !requires
                .iter()
                .all(|fact| runtime_equality_fact(program, machine, entry, *fact)))
    {
        return false;
    }
    let mut requirement = CallRequirement {
        state,
        statement_index,
        in_transition_target,
        callee,
        arguments,
        expression,
        seen: false,
        proven: true,
    };
    let judge = if runtime_values {
        StructuralJudge::from_runtime_requires(program, machine, &requires)
    } else if contains_case_premise(program, expression) {
        StructuralJudge::from_case_requires(program, machine, &requires)
    } else {
        StructuralJudge::from_requires(program, machine, &requires)
    };
    let mut diagnostics = Vec::new();
    let recognized = recognize_structural_case_arms_with_requirement(
        program,
        machine,
        &judge,
        &classification,
        &mut diagnostics,
        Some(&mut requirement),
    );
    recognized.is_some() && diagnostics.is_empty() && requirement.seen && requirement.proven
}

pub(super) struct CallRequirement<'program> {
    pub(super) state: SymbolHandle,
    pub(super) statement_index: usize,
    pub(super) in_transition_target: bool,
    callee: &'program State,
    arguments: &'program [ExpressionHandle],
    expression: ExpressionHandle,
    seen: bool,
    proven: bool,
}

impl CallRequirement<'_> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn visit(
        &mut self,
        program: &TypedTrees,
        machine: &Machine,
        judge: &StructuralJudge<'_>,
        environment: &[(String, StructuralTerm)],
        hypotheses: &[(String, StructuralTerm)],
        equations: &[(StructuralTerm, StructuralTerm)],
        guarantees: &CitationFacts,
    ) {
        self.seen = true;
        let mut site_judge = structural_arm_judge(program, machine, judge, hypotheses, equations);
        if site_judge.hypotheses_contradictory {
            return;
        }
        guarantees.intake(&mut site_judge);
        if !judge.has_runtime_body_values()
            && let Some(state) = program
                .machine_states(machine)
                .iter()
                .find(|state| state.symbol == self.state)
        {
            intake_state_induction(
                program,
                machine,
                state,
                self.statement_index,
                environment,
                &mut site_judge,
            );
        }
        let parameters = program.state_parameters(self.callee);
        let substitution = parameters
            .iter()
            .zip(self.arguments)
            .map(|(parameter, argument)| {
                let term = judge.callee_term(*argument, environment, 0)?;
                Some((parameter.name.as_str().to_owned(), term))
            })
            .collect::<Option<Vec<_>>>();
        self.proven &= parameters.len() == self.arguments.len()
            && substitution.is_some_and(|substitution| {
                matches!(
                    instantiated_fact_judgment(
                        program,
                        &site_judge,
                        parameters,
                        self.expression,
                        &substitution
                    ),
                    StructuralJudgment::Proven
                )
            });
    }
}

/// Structural equations interpret builtin Boolean/integer equality, not an
/// arbitrary selected operator or IEEE floating equality. Check assumptions as
/// well as goals: misreading an assumption could manufacture vacuity.
fn runtime_equality_fact(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> bool {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(expression) else {
        return matches!(
            program.expression_table.expression(expression),
            ExpressionNode::Boolean(_)
        );
    };
    if !crate::has_builtin_binary_expression_meaning(program, machine, Some(state), expression) {
        return false;
    }
    match binary.operator {
        BinaryOperator::And => {
            runtime_equality_fact(program, machine, state, binary.left)
                && runtime_equality_fact(program, machine, state, binary.right)
        }
        BinaryOperator::Equal | BinaryOperator::NotEqual => {
            [binary.left, binary.right].iter().all(|operand| {
                crate::expression_result_type_reference(program, machine, state, *operand)
                    .and_then(|reference| program.primitive_type_reference(reference))
                    .is_some_and(|primitive| {
                        primitive == typed_trees::types::PrimitiveType::Bool
                            || primitive.accepts_integer_literal()
                    })
            })
        }
        _ => false,
    }
}

fn contains_case_premise(program: &TypedTrees, expression: ExpressionHandle) -> bool {
    if super::structural_terms::is_case_observation(program, expression) {
        return true;
    }
    matches!(program.expression_table.expression(expression), ExpressionNode::Binary(binary)
        if binary.operator == typed_trees::expression::BinaryOperator::And
        && (contains_case_premise(program, binary.left) || contains_case_premise(program, binary.right)))
}

/// Establish a prior citation before a later branch can refine its arguments.
#[allow(clippy::too_many_arguments)]
pub(super) fn establish_citation(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    environment: &[(String, StructuralTerm)],
    judge: &StructuralJudge<'_>,
    classification: &typed_trees::proof_only::ProofOnlyClassification,
    hypotheses: &[(String, StructuralTerm)],
    equations: &[(StructuralTerm, StructuralTerm)],
    guarantees: &mut CitationFacts,
    target: &CitationTarget,
    arguments: &[StructuralTerm],
) {
    // Proof citations carry their own structural equality vocabulary. Runtime
    // entry transport currently uses checked scalar assumptions and saved value
    // identities; it must not import those wider equations or induction rules.
    if judge.has_runtime_body_values() {
        return;
    }
    let mut site_judge = structural_arm_judge(program, machine, judge, hypotheses, equations);
    guarantees.intake(&mut site_judge);
    intake_state_induction(
        program,
        machine,
        state,
        statement_index,
        environment,
        &mut site_judge,
    );
    let mut diagnostics = Vec::new();
    let established = instantiate_citation(
        program,
        classification,
        machine,
        target,
        arguments,
        &mut diagnostics,
        Some(&site_judge),
        true,
    );
    if diagnostics.is_empty() {
        guarantees.extend(established);
    }
}

/// The same unconditional value arm used by structural entailment supplies an
/// exact induction application, not a future execution result. Its operands
/// must already resolve in this statement's environment; later locals, case
/// refinements and citations cannot license it. A branch-dependent call needs
/// its own path judgment and cannot provide an earlier unconditional IH.
fn intake_state_induction(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    environment: &[(String, StructuralTerm)],
    judge: &mut StructuralJudge<'_>,
) {
    let mut transitions = program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .enumerate()
        .filter_map(|(position, statement)| {
            let StatementNode::Transition(transition) = statement else {
                return None;
            };
            Some((position, transition))
        });
    let Some((position, transition)) = transitions.next() else {
        return;
    };
    if transitions.next().is_some()
        || position < statement_index
        || !matches!(transition.guard, TransitionGuardNode::Always)
        || transition.continuation.is_valid()
        || !transition.target.is_valid()
    {
        return;
    }
    let TransitionTargetNode::Value(value) =
        program.statement_table.transition_target(transition.target)
    else {
        return;
    };
    let Some(entry) = program.machine_states(machine).first() else {
        return;
    };
    let Some(value) = immutable_value_origin(program, state, position, *value) else {
        return;
    };
    let ExpressionNode::Call(call) = program.expression_table.expression(value) else {
        return;
    };
    if !crate::machine_calls::calls::proof_call_has_structural_descent(program, machine, call) {
        return;
    }
    let Some(value) = judge.callee_term(value, environment, 0) else {
        return;
    };
    let parameters = program
        .state_parameters(entry)
        .iter()
        .map(|parameter| parameter.name.as_str().to_owned())
        .collect::<Vec<_>>();
    let mut applications = Vec::new();
    StructuralJudge::self_applications(&value, entry.symbol, &mut applications);
    if applications.len() != 1 {
        return;
    }
    intake_available_self_induction_hypotheses(program, machine, &parameters, &value, judge);
}

/// Follow exact immutable local definitions, including ordinary call hoisting.
/// Earlier-only definition lookup supplies a strictly decreasing walk bound.
fn immutable_value_origin(
    program: &TypedTrees,
    state: &State,
    mut before: usize,
    mut expression: ExpressionHandle,
) -> Option<ExpressionHandle> {
    let statements = program.statement_table.statements(state.statement_nodes);
    loop {
        let ExpressionNode::Name(name) = program.expression_table.expression(expression) else {
            return Some(expression);
        };
        if !name.symbol.is_valid() {
            return None;
        }
        let (position, local) = statements
            .get(..before)?
            .iter()
            .enumerate()
            .rev()
            .find_map(|(position, statement)| {
                let StatementNode::LocalData(local) = statement else {
                    return None;
                };
                (local.symbol == name.symbol && !local.is_mutable).then_some((position, local))
            })?;
        before = position;
        expression = local.initial_value;
    }
}

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
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::statement::{TransitionGuardNode, TransitionTargetNode};

/// Check one exact substituted requirement on every recognized path to a call.
/// Unsupported paths and unknown judgments do not establish the requirement.
/// Callers must preserve the machine's entry premises and provide the selected
/// callee state and actual arguments, not a display-name reconstruction.
/// With `case_facts_only`, callers instead preserve every retained subject's
/// tag across the traversed prefix. Scalar premises and equations are unavailable
/// in that mode; incomplete mutation evidence never licenses either judgment.
#[allow(clippy::too_many_arguments)]
pub fn structural_call_requirement_entailed(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: SymbolHandle,
    statement_index: usize,
    in_transition_target: bool,
    callee: &State,
    arguments: &[ExpressionHandle],
    expression: ExpressionHandle,
    case_facts_only: bool,
) -> bool {
    let classification = crate::proof_only_classification(program);
    let Some(entry) = program.machine_states(machine).first() else {
        return false;
    };
    if !crate::machine_calls::calls::named_state_transition_subgraph_is_acyclic(
        program, machine, entry,
    ) {
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
    if case_facts_only && (!runtime_values || !case_fact_only(program, expression)) {
        return false;
    }
    if runtime_values
        && (!runtime_equality_fact(program, callee_machine, callee, expression)
            || (!case_facts_only
                && !requires
                    .iter()
                    .all(|fact| runtime_equality_fact(program, machine, entry, *fact))))
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
    let mut judge = if runtime_values {
        StructuralJudge::from_runtime_requires(program, machine, &requires)
    } else if contains_case_premise(program, expression) {
        StructuralJudge::from_case_requires(program, machine, &requires)
    } else {
        StructuralJudge::from_requires(program, machine, &requires)
    };
    if case_facts_only {
        judge.restrict_to_case_facts();
    }
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
        program: &dyn crate::CheckingInput,
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
            && let Some(state) = program.state_in_machine(machine, self.state)
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
        let caller_state = program.state_in_machine(machine, self.state);
        let parameters = program.state_parameters(self.callee);
        let substitution = parameters
            .iter()
            .zip(self.arguments)
            .map(|(parameter, argument)| {
                let scope = caller_state.map(|state| (machine, state, parameter.type_reference));
                let term = judge.callee_term_in_scope(scope, *argument, environment, 0)?;
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
                        &substitution,
                        caller_state.map(|state| (machine, state)),
                    ),
                    StructuralJudgment::Proven
                )
            });
    }
}

/// Runtime facts retain exact case membership or builtin Boolean/integer
/// equality, not arbitrary selected operators or IEEE floating equality. Check
/// assumptions as well as goals: misreading one could manufacture vacuity.
fn runtime_equality_fact(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
) -> bool {
    let Some(binary) = program.expression_table.binary_node(expression) else {
        return matches!(
            program.expression_table.expression(expression),
            ExpressionNode::Boolean(_)
        );
    };
    // Authored membership retains its selected meaning on an equality node;
    // synthesized membership uses an explicit tag operation. Neither is a
    // structural equation about the payload.
    if super::structural_terms::is_case_observation(program, expression) {
        return crate::proof_contracts::bound_expression_meaning::has_exact_case_membership_meaning(
            program,
            machine,
            Some(state),
            expression,
            binary,
        );
    }
    match binary.operator {
        BinaryOperator::And => {
            runtime_equality_fact(program, machine, state, binary.left)
                && runtime_equality_fact(program, machine, state, binary.right)
        }
        BinaryOperator::Equal | BinaryOperator::NotEqual => {
            let builtin_meaning = crate::has_builtin_binary_expression_meaning(
                program,
                machine,
                Some(state),
                expression,
            );
            [binary.left, binary.right].iter().all(|operand| {
                (builtin_meaning
                    && crate::expression_result_type_reference(
                        program, machine, state, *operand,
                    )
                    .and_then(|reference| program.primitive_type_reference(reference))
                    .is_some_and(|primitive| {
                        primitive == typed_trees::types::PrimitiveType::Bool
                            || primitive.accepts_integer_literal()
                    }))
                    // Compound equality operands are decidable through the
                    // structural term walk (record constructors and constant
                    // machine applications); the primitive channel only ever
                    // saw scalar equalities before that walk existed.
                    || super::structural_terms::structural_term(program, *operand)
                        .is_some()
            })
        }
        _ => false,
    }
}

fn contains_case_premise(program: &dyn crate::CheckingInput, expression: ExpressionHandle) -> bool {
    if super::structural_terms::is_case_observation(program, expression) {
        return true;
    }
    matches!(program.expression_table.expression(expression), ExpressionNode::Binary(binary)
        if binary.operator == typed_trees::expression::BinaryOperator::And
        && (contains_case_premise(program, binary.left) || contains_case_premise(program, binary.right)))
}

/// Classifier predicates and conjunctions only: tag-preserving writes do not
/// license scalar equations, even apparently contradictory old assumptions.
pub(super) fn case_fact_only(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> bool {
    if super::structural_terms::is_case_observation(program, expression) {
        return true;
    }
    matches!(program.expression_table.expression(expression), ExpressionNode::Binary(binary)
        if binary.operator == BinaryOperator::And
        && case_fact_only(program, binary.left) && case_fact_only(program, binary.right))
}

/// Establish a prior citation before a later branch can refine its arguments.
#[allow(clippy::too_many_arguments)]
pub(super) fn establish_citation(
    program: &dyn crate::CheckingInput,
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
    let mut site_judge = structural_arm_judge(program, machine, judge, hypotheses, equations);
    guarantees.intake(&mut site_judge);
    // Runtime invocations transport only checked tag guarantees. Their saved
    // results are individual values, not mathematical applications or an IH.
    if !judge.has_runtime_body_values() {
        intake_state_induction(
            program,
            machine,
            state,
            statement_index,
            environment,
            &mut site_judge,
        );
    }
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
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    environment: &[(String, StructuralTerm)],
    judge: &mut StructuralJudge<'_>,
) {
    let mut transitions = program
        .statement_table
        .indexed_statements(state.statement_nodes)
        .filter_map(|(position, statement)| {
            let transition = statement.as_transition()?;
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
    let Some(call) = program.expression_table.call_node(value) else {
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
    program: &dyn crate::CheckingInput,
    state: &State,
    mut before: usize,
    mut expression: ExpressionHandle,
) -> Option<ExpressionHandle> {
    loop {
        let Some(name) = program.expression_table.name_path_node(expression) else {
            return Some(expression);
        };
        if !name.symbol.is_valid() {
            return None;
        }
        let (position, local) = program
            .state_local_declaration(state, name.symbol)
            .filter(|&(position, local)| position < before && !local.is_mutable)?;
        before = position;
        expression = local.initial_value;
    }
}

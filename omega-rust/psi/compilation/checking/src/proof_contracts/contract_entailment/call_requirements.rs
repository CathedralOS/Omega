//! Positive call-premise judgments at a structural statement boundary.
//!
//! Reuse the case traversal's argument environment and prior citations. A
//! recursive call's eventual result and later citations are not entry evidence.
//! A licensed induction hypothesis is different: it may establish an earlier
//! premise, but only once its own premises hold in that earlier context.
//! The runtime prefix checks complete write frames before retaining entry
//! observations; proof callers establish preservation separately. This judgment
//! does not replace the separate recursion validator.
//! Runtime calls reuse the traversal with opaque invocation results and exact
//! builtin equality domains. Empty write frames alone do not make observations
//! deterministic, and IEEE equality is not structural reflexivity.
//!
//! Acyclic state paths retain their incoming substitutions. Recurrent runtime
//! states instead start at their own independently checked entry contract and
//! stop at the requested occurrence, before any successor or future citation.
//! Machine-entry assumptions are not state invariants merely because their
//! declarations use the same names.

use super::citations::{
    CitationFacts, CitationTarget, instantiate_citation, machine_requires_facts,
};
use super::refuted_requires::instantiated_fact_judgment;
use super::self_induction::intake_available_self_induction_hypotheses;
use super::structural_case_arms::{
    recognize_structural_case_arms, recognize_structural_state_prefix, structural_arm_judge,
};
use super::structural_judgment::{StructuralJudge, StructuralJudgment, StructuralTerm};
use symbols::SymbolHandle;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::statement::{TransitionGuardNode, TransitionTargetNode};

/// Check one exact substituted requirement on every recognized path to a call.
/// Unsupported paths and unknown judgments do not establish the requirement.
/// Proof callers preserve the machine's entry premises; runtime traversal checks
/// preservation over the executed prefix. Both provide the selected callee state
/// and actual arguments, not a display-name reconstruction.
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
    let Some(callee_machine) = program.machine_holding_state(callee.symbol) else {
        return false;
    };
    structural_requirement_at_prefix(
        program,
        machine,
        state,
        statement_index,
        in_transition_target,
        callee.symbol,
        program.state_parameters(callee),
        Some(callee_machine),
        ExpressionHandle::invalid(),
        0,
        arguments,
        expression,
        case_facts_only,
    )
}

/// Selected expression calls share the structural prefix and execution-path
/// owner. The exact public declaration supplies the telescope; the realized
/// implementation cannot export a stronger private contract.
pub fn selected_value_call_requirement_entailed(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    site: crate::machine_calls::calls::CallSite<'_>,
    expression: ExpressionHandle,
) -> bool {
    let crate::machine_calls::calls::CallSite::Expression {
        expression: call_expression,
        call,
    } = site
    else {
        return false;
    };
    let Some(owner) = super::specification_calls::selected_call_contract(program, call) else {
        return false;
    };
    if !super::specification_calls::owner_contracts(program, &owner)
        .filter(|contract| contract.kind == typed_trees::signature::SignatureContractKind::Requires)
        .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
        .any(|fact| matches!(fact, typed_trees::domain::ProofFact::Expression(required) if *required == expression))
    {
        return false;
    }
    let callee_machine = match owner {
        super::specification_calls::SelectedCallContract::Concrete(machine, _) => Some(machine),
        _ => None,
    };
    let Some(parameter_scope) = super::specification_calls::owner_parameter_scope(program, &owner)
    else {
        return false;
    };
    let leading_operands = super::specification_calls::owner_leading_operands(&owner);
    let Some(arguments) = program
        .expression_table
        .expression_handles(call.arguments)
        .get(leading_operands..)
    else {
        return false;
    };
    structural_requirement_at_prefix(
        program,
        machine,
        state.symbol,
        statement_index,
        super::structural_prefix::expression_is_transition_target(
            program,
            state,
            statement_index,
            call_expression,
        ),
        super::specification_calls::owner_symbol(&owner),
        parameter_scope.as_ref(),
        callee_machine,
        call_expression,
        leading_operands,
        arguments,
        expression,
        false,
    )
}

#[allow(clippy::too_many_arguments)]
fn structural_requirement_at_prefix(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: SymbolHandle,
    statement_index: usize,
    in_transition_target: bool,
    callee_owner: SymbolHandle,
    parameters: &[typed_trees::signature::StateParameter],
    callee_machine: Option<&Machine>,
    call_expression: ExpressionHandle,
    leading_operands: usize,
    arguments: &[ExpressionHandle],
    expression: ExpressionHandle,
    case_facts_only: bool,
) -> bool {
    let classification = crate::proof_only_classification(program);
    let Some(entry) = program.machine_states(machine).first() else {
        return false;
    };
    let runtime_values = !classification.is_proof_machine(program, machine)
        || callee_machine.is_none_or(|callee| !classification.is_proof_machine(program, callee));
    let acyclic = crate::machine_calls::calls::named_state_transition_subgraph_is_acyclic(
        program, machine, entry,
    );
    if !runtime_values && !acyclic {
        return false;
    }
    // An acyclic invocation can retain its complete incoming substitutions and
    // branch facts. A recurrent state instead starts from its independently
    // established contract: first-arrival facts cannot describe every iteration.
    let state_prefix = runtime_values && !acyclic;
    let Some(caller_state) = program.state_in_machine(machine, state) else {
        return false;
    };
    // Named-state contracts are induction obligations of each arrival. They
    // do not acquire the machine entry's facts merely by sharing binder names.
    let mut requires = if state_prefix && caller_state.symbol != entry.symbol {
        Vec::new()
    } else {
        machine_requires_facts(program, machine)
    };
    if state_prefix {
        requires.extend(
            crate::proof_contracts::proof_facts::contract_expression_facts(
                program,
                program.state_contracts(caller_state).iter(),
                typed_trees::signature::SignatureContractKind::Requires,
            ),
        );
    }
    if case_facts_only && (!runtime_values || !case_fact_only(program, expression)) {
        return false;
    }
    if runtime_values
        && (!(runtime_fact_for_parameters(program, callee_owner, parameters, expression)
            || callee_machine.is_some_and(|machine| {
                program
                    .state_in_machine(machine, callee_owner)
                    .is_some_and(|state| runtime_equality_fact(program, machine, state, expression))
            }))
            || (!case_facts_only
                && !requires.iter().all(|fact| {
                    runtime_equality_fact(
                        program,
                        machine,
                        if state_prefix { caller_state } else { entry },
                        *fact,
                    )
                })))
    {
        if super::struct_trace() {
            eprintln!(
                "STRUCT runtime-prefix declined caller={} state={:?} goal={} premises={:?}",
                machine.name,
                state,
                program.expression_table.display_name(expression),
                requires
                    .iter()
                    .map(|fact| (
                        program.expression_table.display_name(*fact),
                        runtime_equality_fact(
                            program,
                            machine,
                            if state_prefix { caller_state } else { entry },
                            *fact
                        )
                    ))
                    .collect::<Vec<_>>()
            );
        }
        return false;
    }
    if super::struct_trace() {
        eprintln!(
            "STRUCT runtime-prefix entered caller={} state={:?} statement={statement_index} goal={}",
            machine.name,
            state,
            program.expression_table.display_name(expression)
        );
    }
    let mut requirement = CallRequirement {
        state,
        statement_index,
        in_transition_target,
        callee_owner,
        parameters,
        call_expression,
        leading_operands,
        arguments,
        expression,
        seen: false,
        proven: true,
    };
    // A state-local proof needs no incoming-path replay. Its independent
    // Requires and completed prefix suffice even when another acyclic branch
    // contains unsupported effects. Keep the full incoming-path analysis as
    // fallback for obligations that genuinely need an arrival substitution.
    if runtime_values && !state_prefix {
        let local = super::structural_prefix::observe_runtime_state_prefix(
            program,
            machine,
            caller_state,
            &mut requirement,
        );
        if local.is_some() && requirement.seen && requirement.proven {
            return true;
        }
        requirement.seen = false;
        requirement.proven = true;
    }
    let mut judge = if state_prefix {
        StructuralJudge::from_runtime_state_requires(program, machine, caller_state, &requires)
    } else if runtime_values {
        StructuralJudge::from_runtime_requires(program, machine, &requires)
    } else if contains_case_premise(program, expression) {
        StructuralJudge::from_case_requires(program, machine, &requires)
    } else {
        StructuralJudge::from_requires(program, machine, &requires, true)
    };
    if case_facts_only {
        judge.restrict_to_case_facts();
    }
    let mut diagnostics = Vec::new();
    let recognized = if state_prefix {
        recognize_structural_state_prefix(
            program,
            machine,
            caller_state,
            &judge,
            &classification,
            &mut diagnostics,
            &mut requirement,
        )
    } else {
        recognize_structural_case_arms(
            program,
            machine,
            &judge,
            &classification,
            &mut diagnostics,
            Some(&mut requirement),
        )
    };
    if super::struct_trace() {
        eprintln!(
            "STRUCT runtime-prefix completed caller={} recognized={} seen={} proven={} diagnostics={}",
            machine.name,
            recognized.is_some(),
            requirement.seen,
            requirement.proven,
            diagnostics.len()
        );
    }
    recognized.is_some() && diagnostics.is_empty() && requirement.seen && requirement.proven
}

pub(super) struct CallRequirement<'program> {
    pub(super) state: SymbolHandle,
    pub(super) statement_index: usize,
    pub(super) in_transition_target: bool,
    callee_owner: SymbolHandle,
    parameters: &'program [typed_trees::signature::StateParameter],
    call_expression: ExpressionHandle,
    leading_operands: usize,
    arguments: &'program [ExpressionHandle],
    expression: ExpressionHandle,
    seen: bool,
    proven: bool,
}

impl super::structural_prefix::StructuralPrefixVisitor for CallRequirement<'_> {
    fn site(&self) -> super::structural_prefix::StructuralPrefixSite {
        super::structural_prefix::StructuralPrefixSite {
            state: self.state,
            statement_index: self.statement_index,
            in_transition_target: self.in_transition_target,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn visit<'program>(
        &mut self,
        program: &'program dyn crate::CheckingInput,
        machine: &Machine,
        judge: &StructuralJudge<'program>,
        environment: &[(String, StructuralTerm)],
        hypotheses: &[(String, StructuralTerm)],
        equations: &[(StructuralTerm, StructuralTerm)],
        guarantees: &CitationFacts,
    ) {
        self.seen = true;
        let mut site_judge = structural_arm_judge(program, judge, hypotheses, equations);
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
        let Some(caller_state) = program.state_in_machine(machine, self.state) else {
            self.proven = false;
            return;
        };
        let entries = if self.call_expression.is_valid() {
            let Some(entries) = site_judge.normal_operation_entries(
                self.statement_index,
                self.call_expression,
                environment,
                (machine, caller_state, caller_state.return_type),
            ) else {
                self.proven = false;
                return;
            };
            entries
        } else {
            let Some(entries) = site_judge.normal_argument_entries(
                self.arguments,
                self.parameters,
                environment,
                (machine, caller_state, caller_state.return_type),
            ) else {
                self.proven = false;
                return;
            };
            entries
        };
        self.proven &= !entries.is_empty();
        for entry in entries {
            let mut substitution = Vec::new();
            let subjects = if self.call_expression.is_valid() {
                let Some(owner_machine) = program.machine(machine.symbol) else {
                    self.proven = false;
                    return;
                };
                site_judge.frame_resolver().and_then(|frames| {
                    frames.expression_call_reference_subjects(
                        owner_machine,
                        self.call_expression,
                        self.parameters,
                    )
                })
            } else {
                Some(Vec::new())
            };
            let Some(subjects) = subjects else {
                self.proven = false;
                return;
            };
            let Some(arguments) = entry.arguments.get(self.leading_operands..) else {
                self.proven = false;
                return;
            };
            if self.parameters.len() != arguments.len()
                || self.parameters.len() != self.arguments.len()
            {
                self.proven = false;
                return;
            }
            for (parameter, value) in self.parameters.iter().zip(arguments) {
                let value = if self.call_expression.is_valid()
                    && crate::reference_shell(program, parameter.type_reference).is_some()
                {
                    super::structural_terms::reference_argument_observation(
                        program,
                        parameter.type_reference,
                        value,
                        subjects
                            .iter()
                            .find(|subject| subject.parameter == parameter.symbol)
                            .and_then(|subject| {
                                entry.judge.reference_subject_term(
                                    subject,
                                    environment,
                                    (machine, caller_state, caller_state.return_type),
                                )
                            }),
                    )
                } else if self.call_expression.is_valid()
                    && !entry.judge.frame_resolver().is_some_and(|frames| {
                        frames.proof_value_is_caller_isolated(parameter.type_reference)
                    })
                {
                    // Partial reference-subject capture does not turn an
                    // owned carrier's live reference leaves into detached
                    // values. This consumer owes isolation for each such
                    // formal, independently of its reference neighbors.
                    None
                } else {
                    Some(value.clone())
                };
                let Some(value) = value else {
                    self.proven = false;
                    return;
                };
                substitution.push((parameter.name.as_str().to_owned(), value));
            }
            self.proven &= matches!(
                instantiated_fact_judgment(
                    program,
                    &entry.judge,
                    self.callee_owner,
                    self.parameters,
                    self.expression,
                    &substitution,
                    Some((machine, caller_state)),
                ),
                StructuralJudgment::Proven,
            );
        }
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
            super::structural_terms::has_identity_equality_meaning(
                program,
                machine.symbol,
                program.state_parameters(state),
                expression,
            ) || runtime_fact_for_parameters(
                program,
                machine.symbol,
                program.state_parameters(state),
                expression,
            )
        }
        _ => runtime_fact_for_parameters(
            program,
            machine.symbol,
            program.state_parameters(state),
            expression,
        ),
    }
}

fn runtime_fact_for_parameters(
    program: &dyn crate::CheckingInput,
    owner: SymbolHandle,
    parameters: &[typed_trees::signature::StateParameter],
    expression: ExpressionHandle,
) -> bool {
    if let Some(binary) = program.expression_table.binary_node(expression) {
        if binary.operator == BinaryOperator::And {
            return runtime_fact_for_parameters(program, owner, parameters, binary.left)
                && runtime_fact_for_parameters(program, owner, parameters, binary.right);
        }
        if super::structural_terms::is_case_observation(program, expression) {
            return crate::proof_contracts::bound_expression_meaning::has_exact_parameter_case_membership_meaning(
                program, parameters, expression, binary,
            );
        }
    }
    let term = super::structural_terms::structural_term(
        program,
        None,
        Some((owner, parameters)),
        expression,
        None,
    );
    if term
        .as_ref()
        .is_some_and(StructuralTerm::is_mathematical_predicate)
    {
        return true;
    }
    if crate::parameter_expression_result_type_reference(program, owner, parameters, expression)
        .and_then(|reference| program.primitive_type_reference(reference))
        != Some(typed_trees::types::PrimitiveType::Bool)
    {
        return false;
    }
    matches!(
        term,
        Some(StructuralTerm::ScalarBinary { .. } | StructuralTerm::Constructor { .. }),
    )
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
    let mut site_judge = structural_arm_judge(program, judge, hypotheses, equations);
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
    let Some(value) = judge.callee_term_in_scope(
        Some((machine, state, state.return_type)),
        value,
        environment,
        0,
    ) else {
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

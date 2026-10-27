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
//! Private entries may use independently checked actual arrivals. Public and
//! top-level bodies keep arbitrary inputs under their declared contracts;
//! convenient callers cannot specialize an otherwise unproved requirement.

use super::citations::{
    CitationFacts, CitationTarget, instantiate_citation, machine_requires_facts,
};
use super::refuted_requires::instantiated_selected_fact_judgment;
use super::self_induction::intake_available_self_induction_hypotheses;
use super::structural_case_arms::{
    recognize_structural_case_arms, recognize_structural_state_prefix, structural_arm_judge,
};
use super::structural_judgment::normal_returns::OperationOperandDemand;
use super::structural_judgment::{StructuralJudge, StructuralJudgment, StructuralTerm};
use super::structural_prefix::PrefixObservation;
use symbols::SymbolHandle;
use typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::statement::{TransitionGuardNode, TransitionTargetNode};

mod inherited_predicates;
use inherited_predicates::{InheritedPredicate, inherited_predicate};

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
        None,
        ExpressionHandle::invalid(),
        0,
        arguments,
        expression,
        case_facts_only,
        None,
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
    case_facts_only: bool,
    inherited: Option<(
        &checked_trees::ProofFacts,
        arena::Handle<checked_trees::ContractProofFact>,
    )>,
) -> bool {
    let call_expression = match site {
        crate::CallSite::Expression { expression, .. } => expression,
        crate::CallSite::Statement(_) => ExpressionHandle::invalid(),
        crate::CallSite::TransitionNamed { .. } => return false,
    };
    let Some(owner) =
        super::specification_calls::selected_call_site_contract(program, machine.symbol, site)
    else {
        return false;
    };
    if inherited.is_none()
        && !super::specification_calls::owner_contracts(program, &owner)
            .filter(|contract| {
                contract.kind == typed_trees::signature::SignatureContractKind::Requires
            })
            .flat_map(|contract| program.proof_facts.span_or_empty(contract.facts))
            .any(|fact| {
                let typed_trees::domain::ProofFact::Expression(required) = fact else {
                    return false;
                };
                let mut pending = vec![*required];
                while let Some(required) = pending.pop() {
                    if required == expression {
                        return true;
                    }
                    if let Some(binary) = program.expression_table.binary_node(required)
                        && binary.operator == BinaryOperator::And
                    {
                        pending.extend([binary.left, binary.right]);
                    }
                }
                false
            })
    {
        return false;
    }
    let callee_machine = match owner {
        super::specification_calls::SelectedCallContract::Concrete(machine, _) => Some(machine),
        _ => None,
    };
    let Some(parameter_scope) =
        super::specification_calls::call_parameter_scope(program, machine.symbol, &owner, site)
    else {
        return false;
    };
    // The physical invocation telescope remains with capture/reference
    // authentication. A migrated clause independently binds its public
    // declaration through the exact call-owned contract reference.
    let predicate = match inherited {
        Some((proof, contract)) => {
            let predicate = match inherited_predicate(
                program,
                machine,
                state,
                statement_index,
                site,
                &owner,
                parameter_scope.as_ref(),
                expression,
                proof,
                contract,
            ) {
                Ok(predicate) => predicate,
                Err(reason) => {
                    if super::struct_trace() {
                        eprintln!(
                            "STRUCT inherited-prefix declined caller={} reason={reason}",
                            machine.name
                        );
                    }
                    return false;
                }
            };
            Some(predicate)
        }
        None => None,
    };
    let leading_operands = super::specification_calls::owner_leading_operands(&owner);
    let Some(arguments) =
        crate::call_site_argument_expressions(program, &site).get(leading_operands..)
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
        Some(site),
        call_expression,
        leading_operands,
        arguments,
        expression,
        case_facts_only,
        predicate.as_ref(),
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
    call_site: Option<crate::CallSite<'_>>,
    call_expression: ExpressionHandle,
    leading_operands: usize,
    arguments: &[ExpressionHandle],
    expression: ExpressionHandle,
    case_facts_only: bool,
    inherited: Option<&InheritedPredicate>,
) -> bool {
    let callee_owner = inherited.map_or(callee_owner, |predicate| predicate.owner);
    let expression = inherited.map_or(expression, |predicate| predicate.expression);
    let predicate_parameters =
        inherited.map_or(parameters, |predicate| predicate.parameters.as_slice());
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
        && (!(runtime_fact_for_parameters(program, callee_owner, predicate_parameters, expression)
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
        call_site,
        call_expression,
        leading_operands,
        arguments,
        expression,
        inherited: inherited.cloned(),
        seen: false,
        proven: true,
    };
    // A state-local proof needs no incoming-path replay. Its independent
    // Requires and completed prefix suffice even when another acyclic branch
    // contains unsupported effects. Keep the full incoming-path analysis as
    // fallback for obligations that genuinely need an arrival substitution.
    if runtime_values && !state_prefix {
        let local = phase_profile::scope("05.d.7.3b.1.cap.2b_local_prefix", || {
            super::structural_prefix::observe_runtime_state_prefix(
                program,
                machine,
                caller_state,
                &mut requirement,
            )
        });
        if local.is_some() && requirement.seen && requirement.proven {
            return true;
        }
        requirement.seen = false;
        requirement.proven = true;
    }
    let mut judge = phase_profile::scope("05.d.7.3b.1.cap.2c_judge_init", || {
        if state_prefix {
            StructuralJudge::from_runtime_state_requires(program, machine, caller_state, &requires)
        } else if runtime_values {
            StructuralJudge::from_runtime_requires(program, machine, &requires)
        } else if contains_case_premise(program, expression) {
            StructuralJudge::from_case_requires(program, machine, &requires)
        } else {
            StructuralJudge::from_requires(program, machine, &requires, true)
        }
    });
    if case_facts_only {
        judge.restrict_to_case_facts();
    }
    let mut diagnostics = Vec::new();
    let recognized = phase_profile::scope("05.d.7.3b.1.cap.2d_recognize", || {
        if state_prefix {
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
        }
    });
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
    if recognized.is_some() && diagnostics.is_empty() && requirement.seen && requirement.proven {
        return true;
    }
    requirement.seen = false;
    requirement.proven = true;
    if let Some(covered) = super::structural_prefix::observe_private_arrivals(
        program,
        machine,
        &mut requirement,
        case_facts_only,
    ) {
        return covered && requirement.seen && requirement.proven;
    }
    false
}

pub(super) struct CallRequirement<'program> {
    pub(super) state: SymbolHandle,
    pub(super) statement_index: usize,
    pub(super) in_transition_target: bool,
    callee_owner: SymbolHandle,
    parameters: &'program [typed_trees::signature::StateParameter],
    call_site: Option<crate::CallSite<'program>>,
    call_expression: ExpressionHandle,
    leading_operands: usize,
    arguments: &'program [ExpressionHandle],
    expression: ExpressionHandle,
    inherited: Option<InheritedPredicate>,
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

    fn is_complete(&self) -> bool {
        !self.proven
    }

    #[allow(clippy::too_many_arguments)]
    fn entered(&mut self) {
        self.seen = true;
    }

    fn unresolved(&mut self, _machine: &Machine) {
        self.proven = false;
    }

    fn observe_private_arrival<'program>(
        &mut self,
        root: &Machine,
        observation: PrefixObservation<'_, 'program>,
    ) {
        let (program, machine, state, mut judge, environment) = observation;
        if !judge.has_runtime_body_values() {
            intake_state_induction(
                program,
                root,
                state,
                self.statement_index,
                environment,
                &mut judge,
            );
        }
        self.observe((program, machine, state, judge, environment));
    }

    fn observe<'program>(&mut self, observation: PrefixObservation<'_, 'program>) {
        if !self.proven {
            return;
        }
        let (program, machine, state, mut judge, environment) = observation;
        let _s = phase_profile::guard("05.d.7.3b.1.cap.2e_visit");
        // Runtime invocations transport only checked tag guarantees. Their saved
        // results are individual values, not mathematical applications or an IH.
        if !judge.has_runtime_body_values() {
            intake_state_induction(
                program,
                machine,
                state,
                self.statement_index,
                environment,
                &mut judge,
            );
        }
        let entries = if self.call_expression.is_valid() {
            let Some(entries) = judge.normal_operation_entries(
                self.statement_index,
                self.call_expression,
                OperationOperandDemand::All,
                environment,
                (machine, state, state.return_type),
            ) else {
                self.proven = false;
                return;
            };
            entries
        } else {
            let Some(entries) = judge.normal_argument_entries(
                self.arguments,
                self.parameters,
                environment,
                (machine, state, state.return_type),
                self.call_site,
            ) else {
                self.proven = false;
                return;
            };
            entries
        };
        self.proven &= !entries.is_empty();
        let predicate_parameters = self
            .inherited
            .as_ref()
            .map_or(self.parameters, |predicate| predicate.parameters.as_slice());
        for entry in entries {
            if !self.proven {
                return;
            }
            let Some(observed_arguments) = entry.observed_arguments() else {
                self.proven = false;
                return;
            };
            let mut requirement_parameters = predicate_parameters.to_vec();
            let mut static_substitution = Vec::new();
            if let Some(site) = self.call_site {
                let Some(owner) = super::specification_calls::selected_call_site_contract(
                    program,
                    machine.symbol,
                    site,
                ) else {
                    self.proven = false;
                    return;
                };
                let Some(bindings) = super::specification_calls::call_contract_scalar_substitution(
                    program,
                    machine.symbol,
                    &owner,
                    site,
                    |subject| {
                        entry.judge.completed_scalar_subject(
                            subject,
                            site,
                            &observed_arguments,
                            machine,
                            state,
                        )
                    },
                ) else {
                    self.proven = false;
                    return;
                };
                for (parameter, value) in bindings {
                    static_substitution.push((parameter.name.to_string(), value));
                    requirement_parameters.push(parameter);
                }
            }
            if let Some(predicate) = &self.inherited {
                for (parameter, value) in &predicate.values {
                    static_substitution.push((parameter.name.to_string(), value.clone()));
                    requirement_parameters.push(parameter.clone());
                }
            }
            let mut substitution = static_substitution;
            let subjects = if self.call_expression.is_valid() {
                let Some(owner_machine) = program.machine(machine.symbol) else {
                    self.proven = false;
                    return;
                };
                judge.frame_resolver().and_then(|frames| {
                    frames.expression_entry_reference_subjects(
                        owner_machine,
                        self.call_expression,
                        self.parameters,
                    )
                })
            } else {
                Some(Vec::new())
            };
            let subjects = subjects.unwrap_or_default();
            let Some(arguments) = observed_arguments.get(self.leading_operands..) else {
                self.proven = false;
                return;
            };
            if self.parameters.len() != arguments.len()
                || self.parameters.len() != self.arguments.len()
            {
                self.proven = false;
                return;
            }
            for ((parameter, predicate_parameter), value) in self
                .parameters
                .iter()
                .zip(predicate_parameters)
                .zip(arguments)
            {
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
                                    value,
                                    environment,
                                    (machine, state, state.return_type),
                                )
                            }),
                    )
                } else if self.call_expression.is_valid() {
                    entry
                        .judge
                        .owned_argument_observation(parameter.type_reference, value)
                } else {
                    Some(value.clone())
                };
                let Some(value) = value else {
                    let mut expressions = Vec::new();
                    crate::expression_traversal::collect_expression_tree(
                        program,
                        self.expression,
                        &mut expressions,
                    );
                    if expressions.iter().any(|expression| {
                        program
                            .expression_table
                            .name_path_node(*expression)
                            .is_some_and(|path| {
                                path.symbol == predicate_parameter.symbol
                                    || path.head_symbol == predicate_parameter.symbol
                            })
                    }) {
                        self.proven = false;
                        return;
                    }
                    continue;
                };
                substitution.push((predicate_parameter.name.as_str().to_owned(), value));
            }
            self.proven &= matches!(
                instantiated_selected_fact_judgment(
                    program,
                    &entry.judge,
                    self.callee_owner,
                    &requirement_parameters,
                    self.expression,
                    &substitution,
                    self.inherited
                        .as_ref()
                        .map_or(&[], |predicate| predicate.selections.as_slice()),
                    Some((machine, state)),
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
    match term {
        Some(StructuralTerm::ScalarBinary { .. } | StructuralTerm::Constructor { .. }) => true,
        Some(StructuralTerm::Application {
            target, selections, ..
        }) => {
            // Selected operators retain their public meaning; ordinary
            // predicates instead owe the shared observation-free value-call
            // closure and independent declaration identity. Eligibility only
            // opens the existing prefix judgment: it proves neither the
            // predicate nor its formation and termination obligations.
            if typed_trees::operator::declaration_by_symbol(program, target).is_some_and(
                |operator| {
                    typed_trees::operator::is_sealed_toolchain_float_operator(program, operator)
                        && typed_trees::operator::primitive_float_binary_semantics(
                            program, operator,
                        )
                        .is_some_and(|(operation, _)| operation.is_comparison())
                },
            ) {
                return true;
            }
            super::structural_judgment::specification_application_candidate(
                program,
                owner,
                target,
                &selections,
                None,
            )
            .is_ok()
        }
        _ => false,
    }
}

pub(super) fn contains_case_premise(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> bool {
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
    if !super::citations::can_establish_citation(
        program,
        classification,
        machine,
        target,
        arguments.len(),
        judge.has_runtime_body_values(),
    ) {
        return;
    }
    let mut diagnostics = Vec::new();
    let established = if super::citations::citation_needs_site_judge(program, target) {
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
        instantiate_citation(
            program,
            classification,
            machine,
            target,
            arguments,
            &mut diagnostics,
            Some(&site_judge),
            true,
        )
    } else {
        instantiate_citation(
            program,
            classification,
            machine,
            target,
            arguments,
            &mut diagnostics,
            Some(judge),
            true,
        )
    };
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
    let Some((call_position, value)) =
        crate::value_custody::places::binding_observations::immutable_value_origin(
            program, state, position, *value,
        )
    else {
        return;
    };
    if !crate::machine_calls::calls::proof_call_has_structural_descent(
        program,
        machine,
        state,
        call_position,
        value,
    ) {
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

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
//! Selected-referent scenarios supply completed actuals to the ordinary
//! declaration-bound predicate judgment. Operand identity is not a substitute
//! for the selected comparison, including IEEE and authored equality.

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
        return borrowed_match_join_records_selected_place(
            program,
            machine,
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
            inherited,
        );
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
    // The callee prefix only has the formal. A borrowed match records the
    // selected referent, and a later projection reads that record. Every
    // direct caller supplies the referent its actual completed.
    borrowed_match_join_records_selected_place(
        program,
        machine,
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
        inherited,
    )
}

/// A borrowed match join records the selected referent. Projection reads of
/// that join use the record, including when the arm is a reference parameter
/// whose caller completed the value. Each direct caller is one scenario.
/// Target-family bodies share the interface call. A caller the scan cannot
/// see leaves the obligation unproved.
#[allow(clippy::too_many_arguments)]
fn borrowed_match_join_records_selected_place(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state_symbol: SymbolHandle,
    statement_index: usize,
    in_transition_target: bool,
    callee_owner: SymbolHandle,
    parameters: &[typed_trees::signature::StateParameter],
    call_site: Option<crate::CallSite<'_>>,
    call_expression: ExpressionHandle,
    leading_operands: usize,
    arguments: &[ExpressionHandle],
    expression: ExpressionHandle,
    inherited: Option<&InheritedPredicate>,
) -> bool {
    if parameters.len() != arguments.len() {
        return false;
    }
    let Some(binary) = program.expression_table.binary_node(expression) else {
        return false;
    };
    if !matches!(
        binary.operator,
        BinaryOperator::Equal | BinaryOperator::NotEqual
    ) {
        return false;
    }
    if !expression_contains_match(program, expression)
        && !arguments
            .iter()
            .any(|argument| expression_contains_match(program, *argument))
        && !machine_contains_match(program, machine)
    {
        return false;
    }
    let Some(state) = program.state_in_machine(machine, state_symbol) else {
        return false;
    };
    let Some(entry) = program.machine_states(machine).first() else {
        return false;
    };
    let Some(scenarios) = state_scenarios(program, machine, state, &mut Vec::new()) else {
        if super::struct_trace() {
            eprintln!(
                "JOIN no scenarios machine={} goal={}",
                machine.name,
                program.expression_table.display_name(expression)
            );
        }
        return false;
    };
    if scenarios.is_empty() {
        return false;
    };
    scenarios.iter().all(|scenario| {
        let mut requires = if state.symbol == entry.symbol {
            machine_requires_facts(program, machine)
        } else {
            Vec::new()
        };
        requires.extend(
            crate::proof_contracts::proof_facts::contract_expression_facts(
                program,
                program.state_contracts(state).iter(),
                typed_trees::signature::SignatureContractKind::Requires,
            ),
        );
        let mut judge =
            StructuralJudge::from_runtime_state_requires(program, machine, state, &requires);
        for (name, term) in scenario {
            judge.intake_equation(StructuralTerm::Variable(name.clone()), term.clone(), 0);
        }
        let classification = crate::proof_only_classification(program);
        let mut diagnostics = Vec::new();
        let mut goal = SelectedReferentGoal {
            requirement: CallRequirement {
                state: state.symbol,
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
            },
            scenario: scenario.clone(),
        };
        let recognized = recognize_structural_state_prefix(
            program,
            machine,
            state,
            &judge,
            &classification,
            &mut diagnostics,
            &mut goal,
        );
        if super::struct_trace() {
            eprintln!(
                "JOIN machine={} goal={} recognized={} diagnostics={} proven={} scenario={}",
                machine.name,
                program.expression_table.display_name(expression),
                recognized.is_some(),
                diagnostics.len(),
                goal.requirement.proven,
                scenario
                    .iter()
                    .map(|(name, term)| format!("{name}:{}", term_kind(term)))
                    .collect::<Vec<_>>()
                    .join(",")
            );
        }
        recognized.is_some()
            && diagnostics.is_empty()
            && goal.requirement.seen
            && goal.requirement.proven
    })
}

fn term_kind(term: &StructuralTerm) -> &'static str {
    match term {
        StructuralTerm::Constructor { .. } => "constructor",
        StructuralTerm::Variable(_) => "variable",
        StructuralTerm::ObservedValue { .. } => "observed",
        StructuralTerm::Projection { .. } => "projection",
        StructuralTerm::Integer(_) => "integer",
        StructuralTerm::CapturedValue { .. } => "captured",
        StructuralTerm::Opaque(_) => "opaque",
        _ => "other",
    }
}

fn machine_contains_match(program: &dyn crate::CheckingInput, machine: &Machine) -> bool {
    program.machine_states(machine).iter().any(|state| {
        program
            .statement_table
            .indexed_statements(state.statement_nodes)
            .any(|(_, statement)| {
                let mut handles = Vec::new();
                crate::expression_traversal::collect_statement_expression_trees(
                    program,
                    statement,
                    &mut handles,
                );
                handles.into_iter().any(|handle| {
                    matches!(
                        program.expression_table.expression(handle),
                        ExpressionNode::Match(_)
                    )
                })
            })
    })
}

struct SelectedReferentGoal<'program> {
    requirement: CallRequirement<'program>,
    scenario: Vec<(String, StructuralTerm)>,
}

impl super::structural_prefix::StructuralPrefixVisitor for SelectedReferentGoal<'_> {
    fn site(&self) -> super::structural_prefix::StructuralPrefixSite {
        self.requirement.site()
    }

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
        let environment = overlay_environment(environment, &self.scenario);
        // Scenarios supply entry values, not a second call evaluator. The
        // ordinary observer retains the exact nested occurrence and preceding
        // sibling effects before judging its declaration-bound requirement.
        self.requirement.visit(
            program,
            machine,
            judge,
            &environment,
            hypotheses,
            equations,
            guarantees,
        );
    }

    /// This visitor forwards to the inner requirement, which owns the premises
    /// intake and the observation of the site's arm.
    fn observe<'program>(&mut self, _observation: PrefixObservation<'_, 'program>) {}
}

fn expression_contains_match(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> bool {
    let mut handles = Vec::new();
    crate::expression_traversal::collect_expression_tree(program, expression, &mut handles);
    handles.into_iter().any(|handle| {
        matches!(
            program.expression_table.expression(handle),
            ExpressionNode::Match(_)
        )
    })
}

struct SelectedReferentCall {
    caller: SymbolHandle,
    state: SymbolHandle,
    statement_index: usize,
    in_transition_target: bool,
    call_expression: ExpressionHandle,
    arguments: Vec<ExpressionHandle>,
}

fn state_scenarios(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    stack: &mut Vec<SymbolHandle>,
) -> Option<Vec<Vec<(String, StructuralTerm)>>> {
    let entry = program.machine_states(machine).first()?;
    if state.symbol == entry.symbol {
        return parameter_scenarios(program, machine, stack);
    }
    let parameters = program.state_parameters(state);
    if parameters.is_empty() {
        return Some(vec![Vec::new()]);
    }
    if stack.contains(&state.symbol) {
        return None;
    }
    stack.push(state.symbol);
    let arrivals = transition_arrivals(program, machine, state);
    if arrivals.is_empty() {
        stack.pop();
        return None;
    }
    let mut scenarios = Vec::new();
    for arrival in &arrivals {
        let source = program.state_in_machine(machine, arrival.state)?;
        let source_scenarios = state_scenarios(program, machine, source, stack)?;
        if arrival.arguments.len() != parameters.len() {
            stack.pop();
            return None;
        }
        for overlay in &source_scenarios {
            for terms in capture_argument_terms(program, machine, arrival, parameters, overlay)? {
                scenarios.push(
                    parameters
                        .iter()
                        .zip(terms)
                        .map(|(parameter, term)| (parameter.name.as_str().to_owned(), term))
                        .collect(),
                );
            }
        }
    }
    stack.pop();
    Some(scenarios)
}

fn transition_arrivals(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
) -> Vec<SelectedReferentCall> {
    let mut arrivals = Vec::new();
    for source in program.machine_states(machine) {
        for (index, statement) in program
            .statement_table
            .indexed_statements(source.statement_nodes)
        {
            let typed_trees::statement::StatementNode::Transition(transition) = statement else {
                continue;
            };
            let TransitionTargetNode::Named {
                path, arguments, ..
            } = program.statement_table.transition_target(transition.target)
            else {
                continue;
            };
            if path.symbol != state.symbol {
                continue;
            }
            arrivals.push(SelectedReferentCall {
                caller: machine.symbol,
                state: source.symbol,
                statement_index: index,
                in_transition_target: true,
                call_expression: ExpressionHandle::invalid(),
                arguments: program
                    .statement_table
                    .expression_handles(*arguments)
                    .to_vec(),
            });
        }
    }
    arrivals
}

fn parameter_scenarios(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    stack: &mut Vec<SymbolHandle>,
) -> Option<Vec<Vec<(String, StructuralTerm)>>> {
    let entry = program.machine_states(machine).first()?;
    let parameters = program.state_parameters(entry);
    if parameters.is_empty() {
        return Some(vec![Vec::new()]);
    }
    if stack.contains(&machine.symbol) {
        return None;
    }
    stack.push(machine.symbol);
    let calls = selected_referent_calls(program, machine);
    if calls.is_empty() {
        stack.pop();
        return None;
    }
    let mut scenarios = Vec::new();
    for call in &calls {
        let caller = program.machine(call.caller)?;
        let caller_scenarios = parameter_scenarios(program, caller, stack)?;
        if call.arguments.len() != parameters.len() {
            stack.pop();
            return None;
        }
        for overlay in &caller_scenarios {
            for terms in capture_argument_terms(program, caller, call, parameters, overlay)? {
                scenarios.push(
                    parameters
                        .iter()
                        .zip(terms)
                        .map(|(parameter, term)| (parameter.name.as_str().to_owned(), term))
                        .collect(),
                );
            }
        }
    }
    stack.pop();
    Some(scenarios)
}

fn selected_referent_calls(
    program: &dyn crate::CheckingInput,
    callee: &Machine,
) -> Vec<SelectedReferentCall> {
    let mut calls = Vec::new();
    for caller in program.machines() {
        for state in program.machine_states(caller) {
            for (index, _) in program
                .statement_table
                .indexed_statements(state.statement_nodes)
            {
                for site in crate::machine_calls::calls::collect_call_sites_in_statement(
                    program, caller, state, index,
                ) {
                    if site.is_internal_state_transfer(program, caller.symbol)
                        || !call_targets_machine(program, &site, callee)
                    {
                        continue;
                    }
                    let call_expression = match site {
                        crate::CallSite::Expression { expression, .. } => expression,
                        _ => ExpressionHandle::invalid(),
                    };
                    calls.push(SelectedReferentCall {
                        caller: caller.symbol,
                        state: state.symbol,
                        statement_index: index,
                        in_transition_target:
                            super::structural_prefix::expression_is_transition_target(
                                program,
                                state,
                                index,
                                call_expression,
                            ),
                        call_expression,
                        arguments: crate::machine_calls::calls::call_site_argument_expressions(
                            program, &site,
                        )
                        .to_vec(),
                    });
                }
            }
        }
    }
    calls
}

fn call_targets_machine(
    program: &dyn crate::CheckingInput,
    site: &crate::CallSite<'_>,
    callee: &Machine,
) -> bool {
    let target = site.target_symbol(program);
    if target == callee.symbol
        || program
            .machine_states(callee)
            .first()
            .is_some_and(|state| state.symbol == target)
    {
        return true;
    }
    let owner = match program.symbols.get(target).kind {
        symbols::SymbolKind::State => program.symbols.get(target).parent,
        _ => target,
    };
    program
        .machine(owner)
        .and_then(|interface| {
            crate::machine_calls::calls::target_family_members(program, interface)
        })
        .is_some_and(|members| members.iter().any(|member| member.machine == callee.symbol))
}

fn capture_argument_terms(
    program: &dyn crate::CheckingInput,
    caller: &Machine,
    call: &SelectedReferentCall,
    parameters: &[typed_trees::signature::StateParameter],
    overlay: &[(String, StructuralTerm)],
) -> Option<Vec<Vec<StructuralTerm>>> {
    let state = program.state_in_machine(caller, call.state)?;
    let mut captured = CapturedArguments {
        state: call.state,
        statement_index: call.statement_index,
        in_transition_target: call.in_transition_target,
        call_expression: call.call_expression,
        parameters,
        arguments: call.arguments.clone(),
        overlay: overlay.to_vec(),
        terms: Vec::new(),
        complete: true,
    };
    super::structural_prefix::observe_runtime_state_prefix(program, caller, state, &mut captured)?;
    (captured.complete && !captured.terms.is_empty()).then_some(captured.terms)
}

struct CapturedArguments<'program> {
    state: SymbolHandle,
    statement_index: usize,
    in_transition_target: bool,
    call_expression: ExpressionHandle,
    parameters: &'program [typed_trees::signature::StateParameter],
    arguments: Vec<ExpressionHandle>,
    overlay: Vec<(String, StructuralTerm)>,
    terms: Vec<Vec<StructuralTerm>>,
    complete: bool,
}

impl super::structural_prefix::StructuralPrefixVisitor for CapturedArguments<'_> {
    fn site(&self) -> super::structural_prefix::StructuralPrefixSite {
        super::structural_prefix::StructuralPrefixSite {
            state: self.state,
            statement_index: self.statement_index,
            in_transition_target: self.in_transition_target,
        }
    }

    fn visit<'program>(
        &mut self,
        program: &'program dyn crate::CheckingInput,
        machine: &Machine,
        judge: &StructuralJudge<'program>,
        environment: &[(String, StructuralTerm)],
        _hypotheses: &[(String, StructuralTerm)],
        _equations: &[(StructuralTerm, StructuralTerm)],
        _guarantees: &super::citations::CitationFacts,
    ) {
        let Some(state) = program.state_in_machine(machine, self.state) else {
            self.complete = false;
            return;
        };
        let environment = overlay_environment(environment, &self.overlay);
        let scope = (machine, state, state.return_type);
        // Incoming scenarios observe the same exact invocation cut as call
        // requirements. Independent actual replay loses preceding siblings'
        // writes and can donate an obsolete value to another machine.
        let entries = if self.call_expression.is_valid() {
            judge.normal_operation_entries(
                self.statement_index,
                self.call_expression,
                OperationOperandDemand::All,
                &environment,
                scope,
            )
        } else {
            judge.normal_argument_entries(
                &self.arguments,
                self.parameters,
                &environment,
                scope,
                program
                    .statement_table
                    .statement_at(state, self.statement_index)
                    .and_then(|statement| statement.as_call())
                    .map(crate::CallSite::Statement),
            )
        };
        let Some(entries) = entries else {
            self.complete = false;
            return;
        };
        if entries.is_empty()
            || entries
                .iter()
                .any(|entry| entry.arguments.len() != self.parameters.len())
        {
            self.complete = false;
            return;
        }
        let Some(arguments) = entries
            .iter()
            .map(|entry| entry.observed_arguments())
            .collect::<Option<Vec<_>>>()
        else {
            self.complete = false;
            return;
        };
        self.terms.extend(arguments);
    }

    /// Captured arguments owe no premises: this visitor reads operand rosters
    /// and deliberately skips the arm's hypotheses, equations and guarantees.
    fn observe<'program>(&mut self, _observation: PrefixObservation<'_, 'program>) {}
}

fn overlay_environment(
    base: &[(String, StructuralTerm)],
    overlay: &[(String, StructuralTerm)],
) -> Vec<(String, StructuralTerm)> {
    let mut environment = base.to_vec();
    for (name, term) in overlay {
        if let Some(slot) = environment
            .iter_mut()
            .find(|(existing, _)| existing == name)
        {
            slot.1 = term.clone();
        } else {
            environment.push((name.clone(), term.clone()));
        }
    }
    environment
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

    #[allow(clippy::too_many_arguments)]
    fn entered(&mut self) {
        self.seen = true;
    }

    fn unresolved(&mut self, _machine: &Machine) {
        self.proven = false;
    }

    fn observe<'program>(&mut self, observation: PrefixObservation<'_, 'program>) {
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
                    frames.expression_call_reference_subjects(
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
    let Some(call) = program.expression_table.call_node(value) else {
        return;
    };
    if !crate::machine_calls::calls::proof_call_has_structural_descent(
        program,
        machine,
        state,
        call_position,
        call,
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

//! Operational eligibility of source machines whose contract has checked
//! indexed induction evidence. Kernel proof and body eligibility are separate:
//! an effectful implementation never becomes a logical citation merely because
//! its mathematical postcondition happens to be a theorem. This early profile
//! is provisional proof placement. Checked source admission independently
//! requires a positive unconditional whole-machine termination summary before
//! execution may erase logical citations or omit erased control.

use symbols::SymbolHandle;
use typed_trees::domain::ProofFact;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::signature::SignatureContractKind;
use typed_trees::statement::{StatementNode, TransitionTargetNode};
use typed_trees::types::{PrimitiveType, TypeReferenceHandle};

pub fn is_indexed_logical_machine(program: &dyn crate::CheckingInput, machine: &Machine) -> bool {
    crate::frozen_program::frozen_memo(
        program,
        |memos| &memos.indexed_logical_machines,
        || {
            let mut candidates = program
                .machines()
                .iter()
                .filter(|machine| has_exact_theorem(program, machine))
                .map(|machine| machine.symbol)
                .collect::<Vec<_>>();
            loop {
                let retained = candidates
                    .iter()
                    .copied()
                    .filter(|symbol| {
                        program
                            .machine_by_symbol(*symbol)
                            .is_some_and(|machine| logical_body(program, machine, &candidates))
                    })
                    .collect::<Vec<_>>();
                if retained.len() == candidates.len() {
                    return retained;
                }
                candidates = retained;
            }
        },
    )
    .contains(&machine.symbol)
}

/// Exact authored postcondition independently established by checked induction.
/// Other conclusions and the source body's ordinary safety/termination checks
/// retain their own checking obligations.
pub fn indexed_contract_is_proved(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    expression: ExpressionHandle,
) -> bool {
    is_indexed_logical_machine(program, machine)
        && program.prepared_mathematics().is_some_and(|view| {
            view.applications
                .indexed_theorems
                .iter()
                .any(|(_, theorem)| {
                    theorem.machine == machine.symbol
                        && theorem.expression == expression
                        && theorem.theorem.is_valid()
                        && theorem.ty.is_valid()
                })
        })
}

fn has_exact_theorem(program: &dyn crate::CheckingInput, machine: &Machine) -> bool {
    let Some(view) = program.prepared_mathematics() else {
        return false;
    };
    let Some(entry) = program.machine_states(machine).first() else {
        return false;
    };
    view.applications.indexed_theorems.iter().any(|(_, theorem)| {
        theorem.machine == machine.symbol
            && theorem.theorem.is_valid() && theorem.ty.is_valid()
            && program.state_parameters(entry).iter().any(|formal| {
                formal.symbol == theorem.evidence_formal && formal.relevance.is_erased()
                    && formal.type_reference == theorem.source_type
                    && super::is_source_evidence_carrier(program, formal.type_reference)
            })
            && program.machine_contracts(machine).iter().chain(program.state_contracts(entry)).any(|contract| {
                matches!(contract.kind, SignatureContractKind::Ensures)
                    && program.proof_facts.span_or_empty(contract.facts).iter().any(|fact| {
                        matches!(fact, ProofFact::Expression(expression) if *expression == theorem.expression)
                    })
            })
    })
}

fn logical_carrier(program: &dyn crate::CheckingInput, reference: TypeReferenceHandle) -> bool {
    super::is_source_evidence_carrier(program, reference)
        || program
            .primitive_type_reference(reference)
            .is_some_and(|primitive| {
                primitive == PrimitiveType::Bool || primitive.integer_bounds().is_some()
            })
}

fn target_machine(program: &dyn crate::CheckingInput, target: SymbolHandle) -> Option<&Machine> {
    program
        .machine_by_symbol(target)
        .or_else(|| program.machine_holding_state(target))
}

fn logical_body(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    candidates: &[SymbolHandle],
) -> bool {
    if !machine.supply_mode.is_checked_body()
        || !machine.body_is_present
        || !crate::machine_calls::calls::machine_has_unit_result(program, machine)
        || machine.attached_data_symbol.is_valid()
        || machine.attached_data.is_some()
        || !program.machine_owned_data(machine).is_empty()
        || !machine.invokes.is_empty()
        || machine.suspends
        || machine.blocks
        || !program
            .service_reach_rows
            .services(machine.service_reach_row)
            .is_empty()
        || program.machine_contracts(machine).iter().any(|contract| {
            !matches!(
                contract.kind,
                SignatureContractKind::Requires | SignatureContractKind::Ensures
            )
        })
    {
        return false;
    }
    program.machine_states(machine).iter().all(|state| {
        let admitted = program.state_contracts(state).iter().all(|contract| {
                matches!(
                    contract.kind,
                    SignatureContractKind::Requires | SignatureContractKind::Ensures
                )
            })
            && program.state_parameters(state).iter().all(|parameter| {
                !parameter.is_mutable
                    && !parameter.is_const
                    && logical_carrier(program, parameter.type_reference)
            })
            && program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .all(|statement| { let admitted = match statement {
                    StatementNode::Assignment(_)
                    | StatementNode::Assembly(_) => false,
                    StatementNode::AssemblyFact(_) => true,
                    StatementNode::Call(call) => {
                        call.receiver.is_empty()
                            && !call.receiver_symbol.is_valid()
                            && !call.receiver_root_symbol.is_valid()
                            && call.static_requirement_dispatch.is_none()
                            && call.evidence_arguments.is_empty()
                            && !call.operational_acknowledgement.acknowledges_suspend
                            && !call.operational_acknowledgement.acknowledges_block
                            && target_machine(program, call.target_symbol)
                                .is_some_and(|callee| candidates.contains(&callee.symbol))
                            && program
                                .statement_table
                                .expression_handles(call.arguments)
                                .iter()
                                .all(|argument| {
                                    logical_expression(
                                        program, machine, state, *argument, candidates,
                                    )
                                })
                    }
                    StatementNode::LocalData(local) => {
                        !local.is_mutable
                            // Exhaustiveness markers carry validation metadata,
                            // not a produced value or storage carrier. Their
                            // subject still obeys logical expression custody.
                            && (crate::proof_contracts::contract_entailment::is_arm_pattern_marker(statement)
                                || logical_carrier(program, local.type_reference))
                            && logical_expression(
                                program,
                                machine,
                                state,
                                local.initial_value,
                                candidates,
                            )
                    }
                    StatementNode::Expression(expression) => {
                        logical_expression(program, machine, state, *expression, candidates)
                    }
                    StatementNode::Transition(transition) => {
                        transition.exit == typed_trees::statement::TransitionExit::Ordinary
                            && program.tables.expression_roots(statement).iter().all(
                                |(_, expression)| {
                                    logical_expression(
                                        program,
                                        machine,
                                        state,
                                        *expression,
                                        candidates,
                                    )
                                },
                            )
                            && transition.targets().all(|target| {
                                match program.statement_table.transition_target(target) {
                                    TransitionTargetNode::Named {
                                        path,
                                        operational_acknowledgement,
                                        static_machine_parameter,
                                        evidence_arguments,
                                        ..
                                    } => {
                                        !operational_acknowledgement.acknowledges_suspend
                                            && !operational_acknowledgement.acknowledges_block
                                            && !static_machine_parameter.is_valid()
                                            && evidence_arguments.is_empty()
                                            && program
                                                .machine_states(machine)
                                                .iter()
                                                .any(|state| state.symbol == path.symbol)
                                    }
                                    TransitionTargetNode::Terminal => true,
                                    TransitionTargetNode::Value(_) => false,
                                }
                            })
                    }
                };
                admitted
                });
        admitted
    })
}

fn logical_expression(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &typed_trees::state::State,
    expression: ExpressionHandle,
    candidates: &[SymbolHandle],
) -> bool {
    if !expression.is_valid() {
        return true;
    }
    let node = program.expression_table.expression(expression);
    let admitted = match node {
        ExpressionNode::Atomic(_)
        | ExpressionNode::Borrow(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::UndeterminedNaN(_)
        | ExpressionNode::String(_)
        | ExpressionNode::ZeroValue(_) => false,
        ExpressionNode::Call(call) => {
            !call.receiver.is_valid()
                && call.selects_only_nominal_route()
                && call.carries_only_positional_arguments()
                && !call.operational_acknowledgement.acknowledges_suspend
                && !call.operational_acknowledgement.acknowledges_block
                && call.static_requirement_dispatch.is_none()
                && (crate::proof_contracts::proof_embeddings::is_exact_embed_call(program, call)
                    || (super::selected_definition(program, call).is_some()
                        && program
                            .prepared_mathematics()
                            .is_some_and(|view| view.application(expression).is_some()))
                    || target_machine(program, call.target_symbol)
                        .is_some_and(|callee| candidates.contains(&callee.symbol)))
        }
        ExpressionNode::Cast(cast) => {
            !cast.form.is_recast()
                && logical_carrier(program, cast.target_type)
                && program.arithmetic_domain_for_type_reference(cast.target_type)
                    != numerics::arithmetic::ArithmeticDomain::Trapping
        }
        ExpressionNode::Binary(_) | ExpressionNode::Unary(_) => {
            (match node {
                ExpressionNode::Binary(_) => crate::proof_contracts::bound_expression_meaning::has_builtin_binary_expression_meaning(program, machine, Some(state), expression),
                _ => crate::proof_contracts::bound_expression_meaning::has_builtin_bound_expression_meaning(program, machine, Some(state), expression),
            }) && crate::expression_result_type_reference(program, machine, state, expression).is_some_and(|reference| {
                    program.arithmetic_domain_for_type_reference(reference)
                        != numerics::arithmetic::ArithmeticDomain::Trapping
                })
        }
        ExpressionNode::Indexed(_) => false,
        ExpressionNode::Member(member) => super::operand_type_reference(program, member.receiver)
            .is_some_and(|reference| super::is_source_evidence_carrier(program, reference)),
        _ => true,
    };
    if !admitted {
        return false;
    }
    let mut children_admitted = true;
    crate::expression_traversal::children(program, node, |child| {
        children_admitted &= logical_expression(program, machine, state, child, candidates);
    });
    children_admitted
}

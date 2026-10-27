//! Fact-only citations: statement calls that cite a theorem-only machine.
//!
//! An ordinary statement call cites a checked theorem: its `requires` are
//! established under the exact operand substitution before its `ensures`
//! enter the caller's facts, and the call is erased when it is fact-only
//! ([citation](../../../../../../wiki/spec/proofs/contracts.md#citation-and-induction)).
//! A theorem-only machine has no Type result, and a fact-only invocation
//! emits no runtime call or work
//! ([machines and bundles](../../../../../../wiki/spec/proofs/contracts.md#machines-and-bundles)).
//!
//! Checking keeps every citation: requirement discharge and conclusion import
//! are ordinary call judgments over the flow call row. Execution planning
//! erases it: the citation has no Terminal operation, no callee in the unit
//! closure and no runtime cost. Terminal must carry whatever the caller's
//! executable operations need through its own requirement and guarantee
//! rows; a dropped citation grants Terminal nothing.
//!
//! Eligibility is settled from checked termination, effect, and custody
//! judgments rather than an empty-body source shape: the cited machine must be
//! a free checked machine with a `Unit` result, a proof-only classification or
//! an `ensures` conclusion, no `crashes` or bound-output contracts, an
//! observation-free,
//! reach-free, suspension-free, blocking-free, crash-free operational closure,
//! unconditional checked termination across its closure and any static machine
//! arguments, read-only non-linear parameters, and work-free arguments.

use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, StaticMachineArgument};
use typed_trees::machine::Machine;
use typed_trees::signature::SignatureContractKind;
use typed_trees::statement::{StatementNode, TableCall};
use typed_trees::types::TypeReferenceNode;

fn machine_by_target<'program>(
    program: &'program dyn crate::CheckingInput,
    target: SymbolHandle,
) -> Option<&'program Machine> {
    if !target.is_valid() {
        return None;
    }
    program.machines().iter().find(|machine| {
        machine.symbol == target
            || program
                .machine_states(machine)
                .first()
                .is_some_and(|entry| entry.symbol == target)
    })
}

fn has_checked_effect_and_termination_closure(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    entry: &typed_trees::state::State,
    termination: &impl crate::CheckedTerminationOracle,
) -> bool {
    if !termination.unconditionally_terminates(machine.symbol) {
        return false;
    }
    if let Some(plans) = program.prepared_effect_plans() {
        return super::denotational_calls::checked_value_call_candidate(
            program,
            machine,
            entry,
            &plans.operational,
            &plans.service_reaches,
        )
        .is_ok();
    }
    let operational = crate::infer_operational_may(program);
    let service_reaches = crate::infer_service_reaches(program, &operational);
    super::denotational_calls::checked_value_call_candidate(
        program,
        machine,
        entry,
        &operational,
        &service_reaches,
    )
    .is_ok()
}

/// The theorem-only machine a call target names, by machine symbol or by its
/// entry state's symbol, when its checked termination, effect, and custody
/// obligations hold.
pub fn fact_only_theorem<'program>(
    program: &'program dyn crate::CheckingInput,
    target: SymbolHandle,
    termination: &impl crate::CheckedTerminationOracle,
) -> Option<&'program Machine> {
    let machine = machine_by_target(program, target)?;
    if crate::mathematical_applications::is_indexed_logical_machine(program, machine) {
        return Some(machine);
    }
    let entry = program.machine_states(machine).first()?;
    let theorem_only = machine.attached_data.is_none()
        && machine.supply_mode == language_semantics::MachineSupplyMode::CheckedBody
        && machine.body_is_present
        && !machine.suspends
        && !machine.blocks
        && !program.machine_declares_crash(machine)
        && program.machine_type_parameters(machine).len()
            == program.machine_kind_type_parameters(machine).count()
        && program.machine_owned_data(machine).is_empty()
        && program
            .service_reach_rows
            .services(machine.service_reach_row)
            .is_empty()
        && matches!(
            program.type_reference_table.type_reference(entry.return_type),
            TypeReferenceNode::Unit
        )
        && program.machine_contracts(machine).iter().all(|contract| {
            matches!(
                contract.kind,
                SignatureContractKind::Requires | SignatureContractKind::Ensures
            ) && contract.binding.is_none()
        })
        && program.machine_states(machine).iter().all(|state| {
            program.state_contracts(state).iter().all(|contract| {
                matches!(
                    contract.kind,
                    SignatureContractKind::Requires | SignatureContractKind::Ensures
                ) && contract.binding.is_none()
            })
        })
        // A proof-only Unit operation remains erased even when it exports no
        // conclusion: its body and all invocation premises still check. An
        // ordinary runtime Unit call instead needs a theorem contract before
        // entering this lane. Empty conclusions grant no facts to its caller.
        && (crate::proof_only_classification(program).is_proof_machine(program, machine)
            || program
            .machine_contracts(machine)
            .iter()
            .any(|contract| contract.kind == SignatureContractKind::Ensures)
            || program
                .state_contracts(entry)
                .iter()
                .any(|contract| contract.kind == SignatureContractKind::Ensures))
        && program
            .state_parameters(entry)
            .iter()
            .all(|parameter| read_only_parameter(program, parameter))
        && has_checked_effect_and_termination_closure(program, machine, entry, termination);
    theorem_only.then_some(machine)
}

/// A parameter a citation can bind without transferring linear resources or
/// writing storage: a bare scalar, a non-linear proof-only carrier, or a
/// shared borrow. Proof-only classification is computed from the carrier,
/// never from a theorem's name.
fn read_only_parameter(
    program: &dyn crate::CheckingInput,
    parameter: &typed_trees::signature::StateParameter,
) -> bool {
    if parameter.is_mutable {
        return false;
    }
    if matches!(
        program
            .type_reference_table
            .type_reference(parameter.type_reference),
        TypeReferenceNode::Reference {
            access: language_core::ReferenceAccess::Shared,
            ..
        }
    ) {
        return true;
    }
    !crate::type_carries_linear_obligation(program, parameter.type_reference)
        && (program
            .primitive_type_reference(parameter.type_reference)
            .is_some()
            || proof_value_parameter(program, parameter.type_reference)
            || crate::CallFrameResolver::new(program).is_some_and(|frames| {
                frames.proof_value_is_caller_isolated(parameter.type_reference)
            }))
}

/// No-layout classification alone does not erase a reference or collection's
/// storage obligations. Only a directly selected mathematical value carrier
/// enters this lane; ordinary custody checking still governs its invocation.
fn proof_value_parameter(
    program: &dyn crate::CheckingInput,
    reference: typed_trees::types::TypeReferenceHandle,
) -> bool {
    match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Named { symbol, .. } => {
            crate::proof_only_classification(program).is_proof_only(*symbol)
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            proof_value_parameter(program, *base_type)
        }
        _ => false,
    }
}

fn static_machine_argument_is_eligible(
    program: &dyn crate::CheckingInput,
    argument: &StaticMachineArgument,
    termination: &impl crate::CheckedTerminationOracle,
) -> bool {
    if let Some(application) = &argument.application
        && !application
            .arguments
            .iter()
            .all(|nested| static_machine_argument_is_eligible(program, nested, termination))
    {
        return false;
    }
    if argument.type_reference.is_valid() || argument.const_literal.is_some() {
        return true;
    }
    if argument.evidence_projection.is_some() {
        return false;
    }
    if !argument.symbol.is_valid() {
        return false;
    }
    if argument.names_implementation(&program.symbols) {
        return program.symbols.get(argument.symbol).kind == symbols::SymbolKind::Conformance;
    }
    let Some(selected) = machine_by_target(program, argument.symbol) else {
        return false;
    };
    let Some(entry) = program.machine_states(selected).first() else {
        return false;
    };
    has_checked_effect_and_termination_closure(program, selected, entry, termination)
}

fn static_machine_argument_terminates(
    program: &dyn crate::CheckingInput,
    argument: &StaticMachineArgument,
    termination: &impl crate::CheckedTerminationOracle,
) -> bool {
    if let Some(application) = &argument.application
        && !application
            .arguments
            .iter()
            .all(|nested| static_machine_argument_terminates(program, nested, termination))
    {
        return false;
    }
    if argument.type_reference.is_valid()
        || argument.const_literal.is_some()
        || argument.evidence_projection.is_some()
        || !argument.symbol.is_valid()
        || argument.names_implementation(&program.symbols)
    {
        return true;
    }
    let Some(selected) = machine_by_target(program, argument.symbol) else {
        return true;
    };
    termination.unconditionally_terminates(selected.symbol)
}

/// Every statement call from a runtime machine to a proof machine must be
/// eligible for fact-only citation erasure before execution planning; proof
/// machines emit no runtime code.
pub fn validate_runtime_proof_citations(
    program: &dyn crate::CheckingInput,
    termination: &impl crate::CheckedTerminationOracle,
) -> Result<(), Vec<diagnostics::Diagnostic>> {
    let proof_only = crate::proof_only_classification(program);
    let mut diagnostics = Vec::new();
    for machine in program.machines() {
        if proof_only.is_proof_machine(program, machine)
            || crate::value_custody::content_projections::is_content_projection_machine(
                program, machine,
            )
        {
            continue;
        }
        for state in program.machine_states(machine) {
            for statement in program.statement_table.statements(state.statement_nodes) {
                let Some(call) = statement.as_call() else {
                    continue;
                };
                if !call.receiver.is_empty() {
                    continue;
                }
                let Some(callee) = machine_by_target(program, call.target_symbol) else {
                    continue;
                };
                if !proof_only.is_proof_machine(program, callee) {
                    continue;
                }
                if is_fact_only_citation_call(program, call, termination) {
                    continue;
                }
                let Some(entry) = program.machine_states(callee).first() else {
                    continue;
                };
                if callee.suspends
                    || callee.blocks
                    || program.machine_declares_crash(callee)
                    || !program.machine_owned_data(callee).is_empty()
                    || !program
                        .service_reach_rows
                        .services(callee.service_reach_row)
                        .is_empty()
                {
                    diagnostics.push(diagnostics::Diagnostic::error(format!(
                        "theorem citation `{}` must be effect-free; proof citations cannot declare or infer service reach, suspension, blocking, crashes, or mutable state",
                        call.target
                    )));
                } else if !termination.unconditionally_terminates(callee.symbol)
                    || !call.machine_arguments.iter().all(|argument| {
                        static_machine_argument_terminates(program, argument, termination)
                    })
                {
                    diagnostics.push(diagnostics::Diagnostic::error(format!(
                        "theorem citation `{}` requires unconditional termination across its call and static machine arguments",
                        call.target
                    )));
                } else if !has_checked_effect_and_termination_closure(
                    program,
                    callee,
                    entry,
                    termination,
                ) || !call.machine_arguments.iter().all(|argument| {
                    static_machine_argument_is_eligible(program, argument, termination)
                }) {
                    diagnostics.push(diagnostics::Diagnostic::error(format!(
                        "theorem citation `{}` must be effect-free; proof citations cannot declare or infer service reach, suspension, blocking, crashes, or mutable state",
                        call.target
                    )));
                } else if !program
                    .state_parameters(entry)
                    .iter()
                    .all(|parameter| read_only_parameter(program, parameter))
                {
                    diagnostics.push(diagnostics::Diagnostic::error(format!(
                        "theorem citation `{}` cannot take mutable or linear parameters; proof citations erase before execution and cannot transfer linear custody or write storage",
                        call.target
                    )));
                } else {
                    diagnostics.push(diagnostics::Diagnostic::error(format!(
                        "theorem citation `{}` is not eligible for fact-only erasure; a runtime statement call to a proof machine must be a read-only, work-free theorem citation",
                        call.target
                    )));
                }
            }
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

/// Whether one statement is a fact-only citation that execution erases.
pub fn is_fact_only_citation(
    program: &dyn crate::CheckingInput,
    statement: &StatementNode,
    termination: &impl crate::CheckedTerminationOracle,
) -> bool {
    let Some(call) = statement.as_call() else {
        return false;
    };
    is_fact_only_citation_call(program, call, termination)
}

/// Whether one statement call cites a theorem-only machine with arguments
/// whose evaluation performs no work.
pub fn is_fact_only_citation_call(
    program: &dyn crate::CheckingInput,
    call: &TableCall,
    termination: &impl crate::CheckedTerminationOracle,
) -> bool {
    call.receiver.is_empty()
        && !call.receiver_symbol.is_valid()
        && !call.static_machine_parameter.is_valid()
        && call.static_requirement_dispatch.is_none()
        && (call
            .machine_arguments
            .iter()
            .all(|argument| static_machine_argument_is_eligible(program, argument, termination))
            || fact_only_theorem(program, call.target_symbol, termination).is_some_and(|theorem| {
                crate::mathematical_applications::is_indexed_logical_machine(program, theorem)
                    && program
                        .machine_type_parameters(theorem)
                        .iter()
                        .all(|parameter| {
                            matches!(
                                parameter.kind,
                                typed_trees::data::TypeParameterKind::Value { .. }
                            )
                        })
            }))
        && call.evidence_arguments.is_empty()
        && !call.operational_acknowledgement.acknowledges_suspend
        && !call.operational_acknowledgement.acknowledges_block
        && !call.discards_result
        && fact_only_theorem(program, call.target_symbol, termination).is_some_and(|theorem| {
            program
                .machine_states(theorem)
                .first()
                .is_some_and(|entry| {
                    program.state_parameters(entry).len()
                        == program
                            .statement_table
                            .expression_handles(call.arguments)
                            .len()
                })
        })
        && program
            .statement_table
            .expression_handles(call.arguments)
            .iter()
            .all(|argument| work_free_argument(program, *argument))
}

/// A literal, a place spelling, or a shared borrow of a place spelling.
fn work_free_argument(program: &dyn crate::CheckingInput, argument: ExpressionHandle) -> bool {
    if !program.expression_table.expression_is_valid(argument) {
        return false;
    }
    match program.expression_table.expression(argument) {
        ExpressionNode::Integer(_) | ExpressionNode::Boolean(_) | ExpressionNode::Float(_) => true,
        ExpressionNode::Borrow(borrow) => {
            borrow.access == language_core::ReferenceAccess::Shared
                && place_spelling(program, borrow.target)
        }
        ExpressionNode::StructLiteral(literal) => {
            (crate::proof_only_classification(program).is_proof_only(literal.type_symbol)
                || program.prepared_mathematics().is_some_and(|view| {
                    view.source_evidence(argument).is_some_and(|evidence| {
                        evidence.type_identity.is_valid()
                            && evidence.type_identity
                                == program.normalized_type_identity(evidence.source_type)
                    })
                }))
                && program
                    .expression_table
                    .struct_fields(literal.fields)
                    .iter()
                    .all(|field| work_free_argument(program, field.value))
        }
        _ => place_spelling(program, argument),
    }
}

/// A name path or a chain of plain member selections over one.
fn place_spelling(program: &dyn crate::CheckingInput, expression: ExpressionHandle) -> bool {
    if !program.expression_table.expression_is_valid(expression) {
        return false;
    }
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(_) => true,
        ExpressionNode::Member(member) => {
            member.case_variant.is_none() && place_spelling(program, member.receiver)
        }
        _ => false,
    }
}

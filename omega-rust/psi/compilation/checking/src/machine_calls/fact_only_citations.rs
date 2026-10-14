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
//! The recognized theorem is deliberately narrow: a free checked machine with
//! a present, empty single-state body, a Unit result, at least one `ensures`
//! conclusion and otherwise only `requires` clauses, no type parameters,
//! owned data, service reach, suspension, blocking or crash route, and only
//! read-only scalar, proof-only, or shared-borrow parameters. Such a body performs no
//! work, so its invocation has no observable behavior to preserve. The
//! citation's arguments must be literals, proof constructors, or place spellings, whose
//! evaluation performs no call, arithmetic or runtime index selection. A
//! theorem with states or statements, or a citation computing an argument,
//! stays an ordinary call.

use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::signature::SignatureContractKind;
use typed_trees::statement::{StatementNode, TableCall};
use typed_trees::types::TypeReferenceNode;

/// The theorem-only machine a call target names, by machine symbol or by its
/// entry state's symbol.
pub fn fact_only_theorem(
    program: &dyn crate::CheckingInput,
    target: SymbolHandle,
) -> Option<&Machine> {
    if !target.is_valid() {
        return None;
    }
    let machine = program.machines().iter().find(|machine| {
        machine.symbol == target
            || program
                .machine_states(machine)
                .first()
                .is_some_and(|entry| entry.symbol == target)
    })?;
    let [entry] = program.machine_states(machine) else {
        return None;
    };
    let theorem_only = machine.attached_data.is_none()
        && machine.supply_mode == language_semantics::MachineSupplyMode::CheckedBody
        && machine.body_is_present
        && !machine.suspends
        && !machine.blocks
        && program.machine_type_parameters(machine).is_empty()
        && program.machine_owned_data(machine).is_empty()
        && program
            .service_reach_rows
            .services(machine.service_reach_row)
            .is_empty()
        && program
            .statement_table
            .statements(entry.statement_nodes)
            .is_empty()
        && program.state_contracts(entry).is_empty()
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
        // A citation imports conclusions: a machine that states none is an
        // ordinary empty call, not a theorem.
        && program
            .machine_contracts(machine)
            .iter()
            .any(|contract| contract.kind == SignatureContractKind::Ensures)
        && program
            .state_parameters(entry)
            .iter()
            .all(|parameter| read_only_parameter(program, parameter));
    theorem_only.then_some(machine)
}

/// A parameter a citation can bind without transferring or writing storage:
/// a bare scalar, a proof-only carrier, or a shared borrow. Proof-only
/// classification is computed from the carrier, never from a theorem's name.
fn read_only_parameter(
    program: &dyn crate::CheckingInput,
    parameter: &typed_trees::signature::StateParameter,
) -> bool {
    if parameter.is_mutable {
        return false;
    }
    program
        .primitive_type_reference(parameter.type_reference)
        .is_some()
        || proof_value_parameter(program, parameter.type_reference)
        || matches!(
            program
                .type_reference_table
                .type_reference(parameter.type_reference),
            TypeReferenceNode::Reference {
                access: language_core::ReferenceAccess::Shared,
                ..
            }
        )
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

/// Whether one statement is a fact-only citation that execution erases.
pub fn is_fact_only_citation(
    program: &dyn crate::CheckingInput,
    statement: &StatementNode,
) -> bool {
    let Some(call) = statement.as_call() else {
        return false;
    };
    is_fact_only_citation_call(program, call)
}

/// Whether one statement call cites a theorem-only machine with arguments
/// whose evaluation performs no work.
pub fn is_fact_only_citation_call(program: &dyn crate::CheckingInput, call: &TableCall) -> bool {
    call.receiver.is_empty()
        && !call.receiver_symbol.is_valid()
        && !call.static_machine_parameter.is_valid()
        && call.static_requirement_dispatch.is_none()
        && call.machine_arguments.is_empty()
        && call.evidence_arguments.is_empty()
        && !call.discards_result
        && fact_only_theorem(program, call.target_symbol).is_some_and(|theorem| {
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
            crate::proof_only_classification(program).is_proof_only(literal.type_symbol)
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

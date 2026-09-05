//! Source inline-assembly contract and operand validation.
//!
//! This owner reads the shared asm catalog, validates literal/register/place
//! operand classes, and checks value-producing intrinsic destinations. Call
//! resolution and ordinary argument validation remain in the parent module.

use crate::expression_types::expression_type_name_handle;
use diagnostics::Diagnostic;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::types::PrimitiveType;

pub(super) fn user_asm_contract(
    mnemonic: &str,
) -> language_core::inline_assembly::AsmInstructionContract {
    let Some(language_core::inline_assembly::AsmCatalogEntry::Contract(contract)) =
        language_core::inline_assembly::asm_catalog_entry(mnemonic)
    else {
        panic!("accepted asm intrinsic `{mnemonic}` is absent from the shared catalog");
    };
    assert_eq!(
        contract.availability,
        language_core::inline_assembly::AsmInstructionAvailability::UserChecked,
        "source asm intrinsic `{mnemonic}` must be user-checked"
    );
    contract
}

pub(super) fn validate_asm_operand_constraint(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    instruction: &str,
    operand: ExpressionHandle,
    constraint: language_core::inline_assembly::AsmOperandConstraint,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if let ExpressionNode::Integer(literal) = program.expression_table.expression(operand) {
        if let Some(maximum) = constraint.maximum_literal()
            && literal.value_u64().is_some_and(|value| value <= maximum)
        {
            return;
        }
        diagnostics.push(Diagnostic::error(format!(
            "asm instruction `{instruction}` operand `{}` requires target register `{}` \
             constraint `{}`{}; integer literal `{}` is outside that operand class",
            constraint.role,
            constraint.target_register,
            constraint.expected_type_name(),
            constraint
                .maximum_literal()
                .map(|maximum| format!(" or a literal in 0..={maximum}"))
                .unwrap_or_default(),
            literal.text(),
        )));
        return;
    }

    let actual = if constraint.requires_place() {
        crate::places::declared_place_type(program, machine, state, operand)
            .and_then(|type_reference| program.primitive_type_reference(type_reference))
    } else {
        asm_operand_primitive_type(program, machine, state, operand)
    };
    let expected = PrimitiveType::from_name(constraint.expected_type_name())
        .expect("asm operand constraint must name a primitive type");
    if actual == Some(expected) {
        return;
    }

    let actual = actual
        .map(|primitive| format!("`{}`", primitive.name()))
        .unwrap_or_else(|| expression_type_name_handle(program, operand).to_owned());
    let place_requirement = if constraint.requires_writable_place() {
        " writable place"
    } else {
        ""
    };
    diagnostics.push(Diagnostic::error(format!(
        "asm instruction `{instruction}` operand `{}` requires an exact `{}`{place_requirement} \
         for target register `{}`, found {actual}",
        constraint.role,
        constraint.expected_type_name(),
        constraint.target_register,
    )));
}

fn asm_operand_primitive_type(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    operand: ExpressionHandle,
) -> Option<PrimitiveType> {
    match program.expression_table.expression(operand) {
        ExpressionNode::Borrow(inner) => {
            asm_operand_primitive_type(program, machine, state, inner.target)
        }
        ExpressionNode::Cast(cast) => program.primitive_type_reference(cast.target_type),
        _ => crate::places::declared_place_type(program, machine, state, operand)
            .and_then(|type_reference| program.primitive_type_reference(type_reference)),
    }
}

pub(crate) fn validate_asm_value_destination(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    assignment: &typed_trees::statement::TableAssignment,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let ExpressionNode::Call(call) = program.expression_table.expression(assignment.value) else {
        return;
    };
    let instruction =
        match language_core::inline_assembly::AsmControlRegister::from_read_intrinsic_name(
            call.target.as_str(),
        ) {
            Some(register) => register.read_mnemonic(),
            None => match call.target.as_str() {
                "asm#port_in" => "in",
                "asm#pushfq" => "pushfq",
                "asm#rdmsr" => "rdmsr",
                _ => return,
            },
        };
    let contract = user_asm_contract(instruction);
    validate_asm_operand_constraint(
        program,
        machine,
        state,
        instruction,
        assignment.target,
        contract.operands[0],
        diagnostics,
    );
}

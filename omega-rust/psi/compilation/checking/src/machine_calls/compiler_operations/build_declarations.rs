//! Typed authority for Build operations selecting product declarations.
//!
//! `Build::` identifies the toolchain namespace; it supplies no receiver value.
//! The first ordinary argument must be a mutable Build borrow. Execution checks
//! that borrow against the current activation's original cell, so a fabricated
//! Build cannot acquire authority merely by passing this type-level check.

use super::exact_build_prelude_data;
use language_semantics::declaration_selection::BuildOperation;
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, StaticMachineArgument};

pub fn is_build_declaration_selection(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> bool {
    use language_semantics::declaration_selection::AuthoredDeclarationSelectionIntrinsic as Intrinsic;
    let Some(call) = program.expression_table.call_node(expression) else {
        return false;
    };
    matches!(
        super::classify_expression_call_intrinsic(program, call),
        Some(
            Intrinsic::BuildProviderSelection
                | Intrinsic::BuildServiceExclusion
                | Intrinsic::BuildAssumedBindingContract
        )
    )
}

/// Rejoin a designated product operand only after exact operation admission.
/// Provider slots admit boundary requirements and operator families; service
/// exclusions admit only a boundary trait, never its implementation.
pub fn resolve_build_declaration_operand(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    argument_index: usize,
    occurrence: source::SourceSpan,
) -> Option<SymbolHandle> {
    if !is_build_declaration_selection(program, expression) {
        return None;
    }
    let call = program.expression_table.call_node(expression)?;
    if BuildOperation::from_call_target(call.target.as_str())
        == Some(BuildOperation::AssumedBindingContract)
    {
        return typed_trees::product_operand::resolve_requirement_operand(
            program.typed_program(),
            call.machine_arguments.get(argument_index)?,
            occurrence,
        );
    }
    let service = BuildOperation::from_call_target(call.target.as_str())
        == Some(BuildOperation::ServiceExclusion);
    let selected = resolve_product_operand(
        program,
        call.machine_arguments.get(argument_index)?,
        occurrence,
        service || argument_index == 0,
    )?;
    if service
        && !program
            .product_declarations()
            .traits()
            .iter()
            .any(|definition| definition.symbol == selected && definition.is_boundary)
    {
        return None;
    }
    Some(selected)
}

pub fn provider_selection_expressions(
    program: &dyn crate::CheckingInput,
    machine: &typed_trees::machine::Machine,
) -> Vec<ExpressionHandle> {
    let mut expressions = Vec::new();
    for state in program.machine_states(machine) {
        for statement in program.statement_table.statements(state.statement_nodes) {
            crate::expression_traversal::collect_statement_expression_trees(
                program,
                statement,
                &mut expressions,
            );
        }
    }
    expressions.retain(|expression| {
        matches!(program.expression_table.expression(*expression), ExpressionNode::Call(call)
            if BuildOperation::from_call_target(call.target.as_str())
                == Some(BuildOperation::ProviderSelection)
                && !call.target_symbol.is_valid())
    });
    expressions
}

pub fn is_build_provider_selection(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> bool {
    let Some(call) = program.expression_table.call_node(expression) else {
        return false;
    };
    is_provider_selection_call(program, call)
}

pub fn is_provider_selection_call(
    program: &dyn crate::CheckingInput,
    call: &typed_trees::expression::TableCallExpression,
) -> bool {
    BuildOperation::from_call_target(call.target.as_str())
        == Some(BuildOperation::ProviderSelection)
        && has_explicit_build_argument(program, call)
        && (1..=2).contains(
            &program
                .expression_table
                .expression_handles(call.arguments)
                .len(),
        )
        && call.machine_arguments.len() == 2
}

/// Namespace identity selects the operation; its first value operand carries
/// the activation. Never infer that operand from a receiver or a local name.
pub(super) fn has_explicit_build_argument(
    program: &dyn crate::CheckingInput,
    call: &typed_trees::expression::TableCallExpression,
) -> bool {
    !call.target_symbol.is_valid()
        && call.receiver.is_valid()
        && matches!(program.expression_table.expression(call.receiver), ExpressionNode::Name(path)
            if exact_build_prelude_data(program, path.symbol, "Build"))
        && program
            .expression_table
            .expression_handles(call.arguments)
            .first()
            .is_some_and(|builder| exact_mutable_build_argument(program, *builder))
        && call.evidence_arguments.is_empty()
}

fn exact_mutable_build_argument(
    program: &dyn crate::CheckingInput,
    receiver: ExpressionHandle,
) -> bool {
    if !receiver.is_valid() {
        return false;
    }
    if let Some(borrow) = program.expression_table.borrow_expression(receiver) {
        return borrow.access == language_semantics::ReferenceAccess::Mutable
            && crate::expression_type_symbol(program, borrow.target)
                .is_some_and(|symbol| exact_build_prelude_data(program, symbol, "Build"));
    }
    crate::expression_place_type_reference(program, receiver, &[]).is_some_and(|reference| {
        matches!(program.type_reference_table.type_reference(reference),
            typed_trees::types::TypeReferenceNode::Reference { access, referee, .. }
                if *access == language_semantics::ReferenceAccess::Mutable
                    && exact_build_prelude_data(program, program.type_reference_table.type_symbol(*referee), "Build"))
    })
}

/// Resolve an already classified product operand, never an ordinary static use.
/// The collector of an executed selection independently rejoins the same path.
pub fn resolve_product_operand(
    program: &dyn crate::CheckingInput,
    argument: &StaticMachineArgument,
    occurrence: source::SourceSpan,
    subject: bool,
) -> Option<SymbolHandle> {
    typed_trees::product_operand::resolve_product_operand(
        program.typed_program(),
        argument,
        occurrence,
        subject,
    )
}

/// Classify the exact compiler Build operation, not an ordinary same-named call.
pub fn build_operation(
    program: &dyn crate::CheckingInput,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<language_semantics::declaration_selection::BuildOperation> {
    let typed_trees::expression::ExpressionNode::Call(call) =
        program.expression_table.expression(expression)
    else {
        return None;
    };
    let operation = language_semantics::declaration_selection::BuildOperation::from_call_target(
        call.target.as_str(),
    )?;
    (super::classify_expression_call_intrinsic(program, call) == Some(operation.intrinsic()))
        .then_some(operation)
}

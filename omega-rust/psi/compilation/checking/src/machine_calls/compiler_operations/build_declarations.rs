//! Typed authority for Build operations selecting product declarations.
//!
//! `Build::` identifies the toolchain namespace; it supplies no receiver value.
//! The first ordinary argument must be a mutable Build borrow. Execution checks
//! that borrow against the current activation's original cell, so a fabricated
//! Build cannot acquire authority merely by passing this type-level check.

use super::exact_build_prelude_data;
use crate::selection_resolution::AuthoredSelectionResolutionTarget;
use language_semantics::declaration_selection::AuthoredDeclarationSelectionIntrinsic;
use language_semantics::declaration_selection::BuildOperation;
use symbols::JoinedPath;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::product_operand::ProductOperandKind;

pub fn is_build_declaration_selection(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
) -> bool {
    let Some(call) = program.expression_table.call_node(expression) else {
        return false;
    };
    BuildOperation::from_call_target(call.target.as_str()).is_some_and(|operation| {
        operation.selects_product_declarations()
            && super::classify_expression_call_intrinsic(program, call)
                == Some(operation.intrinsic())
    })
}

/// Rejoin a designated product operand only after exact operation admission.
/// Provider slots admit boundary requirements and operator families; service
/// exclusions admit only a boundary trait, never its implementation.
pub fn resolve_build_static_operand(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    argument_index: usize,
    occurrence: source::SourceSpan,
) -> Result<AuthoredSelectionResolutionTarget, String> {
    if !is_build_declaration_selection(program, expression) {
        return Err("static operand does not belong to an authenticated Build operation".into());
    }
    let call = program
        .expression_table
        .call_node(expression)
        .ok_or("Build static operand has no retained call")?;
    let operation = BuildOperation::from_call_target(call.target.as_str())
        .ok_or("Build static operand has no retained operation")?;
    let argument = call
        .machine_arguments
        .get(argument_index)
        .ok_or("Build static operand is absent")?;
    if operation == BuildOperation::WireCompatibilityRequest && argument_index >= 4 {
        let fact = argument
            .path
            .first()
            .filter(|_| argument.path.len() == 1)
            .filter(|_| typed_trees::product_operand::is_declaration_operand(argument))
            .and_then(|name| {
                language_semantics::wire::WireCompatibilityFact::from_name(name.as_str())
            })
            .ok_or_else(|| {
                format!(
                    "unknown wire compatibility fact `{}`",
                    argument.path.joined_path()
                )
            })?;
        return Ok(AuthoredSelectionResolutionTarget::Intrinsic(
            AuthoredDeclarationSelectionIntrinsic::WireCompatibilityFact(fact),
        ));
    }
    let kind = match (operation, argument_index) {
        (BuildOperation::ProviderSelection, 0)
        | (
            BuildOperation::ServiceExclusion
            | BuildOperation::ServiceAudit
            | BuildOperation::BoundaryAcceptance,
            0,
        ) => ProductOperandKind::BoundarySubject,
        (BuildOperation::ProviderSelection, 1) => ProductOperandKind::Provider,
        (BuildOperation::AssumedBindingContract, 0 | 1) => ProductOperandKind::Requirement,
        (BuildOperation::WireCompatibilityRequest, 0..=3)
        | (BuildOperation::RepresentationSelection, 0) => ProductOperandKind::Data,
        (BuildOperation::RepresentationSelection, 1) => ProductOperandKind::Representation,
        _ => return Err("Build static operand has no declaration role at this position".into()),
    };
    let selected = typed_trees::product_operand::resolve_product_operand(
        program.typed_program(),
        argument,
        occurrence,
        kind,
    ).ok_or("Build declaration operand does not resolve to one visible product declaration of the required kind")?;
    if matches!(
        operation,
        BuildOperation::ServiceExclusion | BuildOperation::ServiceAudit
    ) && !program
        .product_declarations()
        .traits()
        .iter()
        .any(|definition| definition.symbol == selected && definition.is_boundary)
    {
        return Err(format!(
            "Build service {} operand must name a boundary trait",
            if operation == BuildOperation::ServiceAudit {
                "audit"
            } else {
                "exclusion"
            }
        ));
    }
    Ok(AuthoredSelectionResolutionTarget::Declaration(selected))
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

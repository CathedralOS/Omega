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

/// Retain a designated Product description under its exact Build operation.
/// The initial Build context deliberately has no Product declarations: source
/// acquisition and successful Build completion precede that join. This checked
/// intrinsic classifies the operand's request role, never selects a declaration
/// or grants execution authority. The admitted executed request retains its path
/// and source custody; Product selection resolves role, package and visibility
/// after the deferred Product forest exists.
pub fn describe_build_static_operand(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    argument_index: usize,
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
    let role = build_operand_kind(operation, argument_index)?;
    if !typed_trees::product_operand::is_declaration_operand(argument) {
        return Err(format!(
            "Build {role:?} operand must describe one declaration path"
        ));
    }
    Ok(AuthoredSelectionResolutionTarget::Intrinsic(
        operation.intrinsic(),
    ))
}

/// Resolve the inert description in the completed Product frontier. This is
/// declaration authority; the earlier operation marker cannot replace it.
pub fn resolve_build_static_operand(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    argument_index: usize,
    occurrence: source::SourceSpan,
) -> Result<AuthoredSelectionResolutionTarget, String> {
    let description = describe_build_static_operand(program, expression, argument_index)?;
    if matches!(
        description,
        AuthoredSelectionResolutionTarget::Intrinsic(
            AuthoredDeclarationSelectionIntrinsic::WireCompatibilityFact(_)
        )
    ) {
        return Ok(description);
    }
    let call = program
        .expression_table
        .call_node(expression)
        .ok_or("Build static operand has no retained call")?;
    let operation = BuildOperation::from_call_target(call.target.as_str())
        .ok_or("Build static operand has no retained operation")?;
    let kind = build_operand_kind(operation, argument_index)?;
    let selected = typed_trees::product_operand::resolve_product_operand(
        program.typed_program(), &call.machine_arguments[argument_index], occurrence, kind,
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
        return Err("Build service operand must name a boundary trait".into());
    }
    Ok(AuthoredSelectionResolutionTarget::Declaration(selected))
}

fn build_operand_kind(
    operation: BuildOperation,
    argument_index: usize,
) -> Result<ProductOperandKind, String> {
    let role = match (operation, argument_index) {
        (BuildOperation::ProviderSelection, 0)
        | (BuildOperation::ServiceExclusion | BuildOperation::ServiceAudit, 0) => {
            ProductOperandKind::BoundarySubject
        }
        (BuildOperation::BoundaryAcceptance, 0) => ProductOperandKind::TrustSubject,
        (BuildOperation::ProviderSelection, 1) => ProductOperandKind::Provider,
        (BuildOperation::AssumedBindingContract, 0 | 1) => ProductOperandKind::Requirement,
        (BuildOperation::WireCompatibilityRequest, 0..=3)
        | (BuildOperation::RepresentationSelection, 0) => ProductOperandKind::Data,
        (BuildOperation::RepresentationSelection, 1) => ProductOperandKind::Representation,
        _ => return Err("Build static operand has no declaration role at this position".into()),
    };
    Ok(role)
}

/// Complete every authored Product description after source assembly. All
/// operation classes share this join, including requests with no later planner.
/// Exact static-argument occurrences supply lexical/package visibility custody.
pub fn validate_build_product_operands(
    program: &dyn crate::CheckingInput,
) -> Result<(), diagnostics::Diagnostic> {
    for (expression, _) in program.expression_table.iter_expressions() {
        if !is_build_declaration_selection(program, expression) {
            continue;
        }
        let call = program.expression_table.call_node(expression).unwrap();
        for argument_index in 0..call.machine_arguments.len() {
            let occurrences = program
                .expression_table
                .authored_selection_occurrences(expression)
                .filter_map(|occurrence| program.authored_declaration_selections().get(occurrence))
                .filter(|selection| {
                    selection.top_level_static_argument_position() == Some(argument_index)
                })
                .collect::<Vec<_>>();
            let [selection] = occurrences.as_slice() else {
                return Err(diagnostics::Diagnostic::error(
                    "Build Product operand has no unique authored static-argument occurrence",
                ));
            };
            resolve_build_static_operand(
                program,
                expression,
                argument_index,
                selection.source_span(),
            )
            .map_err(|message| {
                diagnostics::Diagnostic::error(message).with_source_span(selection.source_span())
            })?;
        }
    }
    Ok(())
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

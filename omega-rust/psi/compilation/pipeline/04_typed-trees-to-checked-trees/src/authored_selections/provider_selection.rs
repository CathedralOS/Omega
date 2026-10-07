//! Typed authority for Build operations selecting product declarations.
//!
//! `Build::` identifies the toolchain namespace; it supplies no receiver value.
//! The first ordinary argument must be a mutable Build borrow. Execution checks
//! that borrow against the current activation's original cell, so a fabricated
//! Build cannot acquire authority merely by passing this type-level check.

use super::intrinsic_calls::exact_build_prelude_data;
use language_semantics::declaration_selection::BuildOperation;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, StaticMachineArgument};

pub(crate) fn is_build_declaration_selection(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> bool {
    use language_semantics::declaration_selection::AuthoredDeclarationSelectionIntrinsic as Intrinsic;
    let ExpressionNode::Call(call) = program.expression_table.expression(expression) else {
        return false;
    };
    matches!(
        super::intrinsic_calls::checked_expression_call_intrinsic(program, call),
        Some(Intrinsic::BuildProviderSelection | Intrinsic::BuildServiceExclusion)
    )
}

/// Rejoin a designated product operand only after exact operation admission.
/// Provider slots admit boundary requirements and operator families; service
/// exclusions admit only a boundary trait, never its implementation.
pub(crate) fn resolve_build_declaration_operand(
    program: &TypedTrees,
    expression: ExpressionHandle,
    argument_index: usize,
    occurrence: source::SourceSpan,
) -> Option<SymbolHandle> {
    if !is_build_declaration_selection(program, expression) {
        return None;
    }
    let ExpressionNode::Call(call) = program.expression_table.expression(expression) else {
        return None;
    };
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

pub(crate) fn provider_selection_expressions(
    program: &TypedTrees,
    machine: &typed_trees::machine::Machine,
) -> Vec<ExpressionHandle> {
    let mut expressions = Vec::new();
    for state in program.machine_states(machine) {
        for statement in program.statement_table.statements(state.statement_nodes) {
            crate::monomorphization::collect_statement_expression_trees(
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

pub(crate) fn is_build_provider_selection(
    program: &TypedTrees,
    expression: ExpressionHandle,
) -> bool {
    let ExpressionNode::Call(call) = program.expression_table.expression(expression) else {
        return false;
    };
    is_provider_selection_call(program, call)
}

pub(crate) fn is_provider_selection_call(
    program: &TypedTrees,
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
    program: &TypedTrees,
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

fn exact_mutable_build_argument(program: &TypedTrees, receiver: ExpressionHandle) -> bool {
    if !receiver.is_valid() {
        return false;
    }
    if let ExpressionNode::Borrow(borrow) = program.expression_table.expression(receiver) {
        return borrow.access == language_semantics::ReferenceAccess::Mutable
            && crate::flow::expression_type_symbol(program, borrow.target)
                .is_some_and(|symbol| exact_build_prelude_data(program, symbol, "Build"));
    }
    crate::flow::expression_place_type_reference(program, receiver, &[]).is_some_and(|reference| {
        matches!(program.type_reference_table.type_reference(reference),
            typed_trees::types::TypeReferenceNode::Reference { access, referee, .. }
                if *access == language_semantics::ReferenceAccess::Mutable
                    && exact_build_prelude_data(program, program.type_reference_table.type_symbol(*referee), "Build"))
    })
}

/// Resolve an already classified product operand, never an ordinary static use.
/// The collector of an executed selection independently rejoins the same path.
pub(crate) fn resolve_product_operand(
    program: &TypedTrees,
    argument: &StaticMachineArgument,
    occurrence: source::SourceSpan,
    subject: bool,
) -> Option<SymbolHandle> {
    let program = program.product_declarations();
    if argument.type_reference.is_valid()
        || argument.application.is_some()
        || argument.const_literal.is_some()
        || argument.evidence_projection.is_some()
        || argument.path.is_empty()
    {
        return None;
    }
    let path = argument
        .path
        .iter()
        .map(|name| name.as_str())
        .collect::<Vec<_>>()
        .join("::");
    let candidates = if subject {
        program
            .traits()
            .iter()
            .filter(|definition| definition.is_boundary)
            .map(|definition| (definition.symbol, definition.is_public))
            .chain(
                program
                    .machines()
                    .iter()
                    .filter(|machine| {
                        machine.supply_mode
                            == language_semantics::MachineSupplyMode::TopLevelRequirement
                    })
                    .map(|machine| (machine.symbol, machine.is_public)),
            )
            .chain(
                program
                    .operators()
                    .iter()
                    .chain(
                        program
                            .domain_definitions()
                            .iter()
                            .flat_map(|domain| program.domain_operators(domain)),
                    )
                    .filter(|operator| operator.is_boundary)
                    .map(|operator| (operator.symbol, operator.is_public)),
            )
            .collect::<Vec<_>>()
    } else {
        program
            .data_definitions()
            .iter()
            .map(|definition| (definition.symbol, definition.is_public))
            .chain(
                program
                    .conformances()
                    .iter()
                    .filter(|definition| {
                        definition.type_parameters.is_empty()
                            && definition.lifetime_parameters.is_empty()
                    })
                    .map(|definition| (definition.symbol, definition.is_public)),
            )
            .collect()
    };
    let token_bound_machines = program
        .machine_token_bindings()
        .iter()
        .filter(|binding| binding.is_boundary)
        .map(|binding| binding.symbol)
        .collect::<Vec<_>>();
    program.symbols.find_build_subject_declaration_from_source(
        &path,
        occurrence,
        candidates,
        &token_bound_machines,
    )
}

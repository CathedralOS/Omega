//! Typed authority for designated product operands of Build provider selection.

use super::intrinsic_calls::exact_build_prelude_data;
use language_semantics::declaration_selection::BuildOperation;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, StaticMachineArgument};

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
    BuildOperation::from_call_target(call.target.as_str())
        == Some(BuildOperation::ProviderSelection)
        && !call.target_symbol.is_valid()
        && exact_mutable_build_receiver(program, call.receiver)
}

pub(crate) fn exact_mutable_build_receiver(
    program: &TypedTrees,
    receiver: ExpressionHandle,
) -> bool {
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

pub(crate) fn exact_mutable_build_statement_receiver(
    program: &TypedTrees,
    call: &typed_trees::statement::TableCall,
) -> bool {
    super::operator_targets::type_reference_for_symbol(program, call.receiver_symbol)
        .is_some_and(|reference| {
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

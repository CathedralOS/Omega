//! Result-policy sites retain their selected operation, rather than inventing
//! a boundary crash bucket or a source `crash` terminator. The raw semantic
//! result is judged with the operand captures available before adaptation;
//! the adapted result's normal-return guarantees cannot exclude its own trap.

use checked_trees::{
    CheckFacts, CheckedArithmeticPolicyAdapter, CheckedValueOrigin, CrashSiteLocation,
};
use typed_trees::expression::ExpressionHandle;

pub(super) struct SelectedOperation {
    pub expression: ExpressionHandle,
    pub selected: symbols::SymbolHandle,
    pub adapter: CheckedArithmeticPolicyAdapter,
    pub location: CrashSiteLocation,
}

pub(super) fn collect(
    facts: &CheckFacts,
    machine: symbols::SymbolHandle,
) -> Vec<SelectedOperation> {
    facts
        .operators
        .resolved_expression_selections()
        .filter_map(|(expression, origin, selected, adapter)| {
            let CheckedArithmeticPolicyAdapter::FloatTrappingNonFinite { .. } = adapter else {
                return None;
            };
            let CheckedValueOrigin::StateStatement {
                machine_symbol,
                state_symbol,
                statement_index,
                ..
            } = origin
            else {
                return None;
            };
            (machine_symbol == machine).then(|| SelectedOperation {
                expression,
                selected,
                adapter,
                location: CrashSiteLocation::new(
                    state_symbol,
                    u32::try_from(statement_index).expect("statement ordinal fits u32"),
                ),
            })
        })
        .collect()
}

/// The operation's sealed raw-result equation supplies the exact finite
/// predicate. No query mentions its adapted result or the containing
/// statement's eventual postcondition. Missing/substituted captures select no
/// facts, just as for an ordinary selected-operation precondition.
pub(super) fn raw_result_is_finite(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    operation: &SelectedOperation,
) -> bool {
    let Some(operator) = typed_trees::operator::declaration_by_symbol(program, operation.selected)
    else {
        return false;
    };
    let Some(raw_result) = validation::float_result_semantic_contract(program, operator) else {
        return false;
    };
    let parameters = program.operator_parameters(operator);
    let operands = if let Some((_, selected_use)) =
        facts.operators.uses.iter().find(|(_, selected_use)| {
            selected_use.expression == operation.expression
                && selected_use.selected_operator_symbol == operation.selected
        }) {
        selected_use.operands(program)
    } else {
        program
            .expression_table
            .call_node(operation.expression)
            .and_then(|call| {
                crate::facts::operator_crashes::named_call_operands(program, call, parameters)
            })
    };
    let Some(operands) = operands else {
        return false;
    };
    if operands.len() != parameters.len() {
        return false;
    }
    if validation::closed_float_semantic_meaning_with_inputs(
        program, raw_result, parameters, &operands,
    )
    .is_some_and(|meaning| meaning.is_finite())
    {
        return true;
    }
    let contexts = super::super::operators::InvocationContexts::for_selected_expression(
        facts,
        operation.expression,
        operation.selected,
        &operands,
    );
    let Some(raw_label) = crate::labels::instantiated_float_semantic_label(
        program, facts, raw_result, parameters, &operands,
    ) else {
        return false;
    };
    // The Boolean query uses the existing exact fact-label vocabulary; the
    // predicate's identity comes from the closed sealed semantic catalog.
    let finite = program.operators().iter().find(|candidate| {
        validation::exact_toolchain_float_semantic_contract(program, candidate)
            .is_some_and(|(row, _)| row.name == "is_finite")
    });
    let Some(finite) = finite else {
        return false;
    };
    let finite_name = program
        .operator_path_members(finite.name)
        .iter()
        .map(|member| member.as_str())
        .collect::<Vec<_>>()
        .join("::");
    let required = format!("{finite_name}({raw_label})");
    contexts
        .for_expressions(program, parameters, [raw_result])
        .iter()
        .any(|context| {
            crate::labels::context_proves_boolean_label(
                program,
                &facts.semantic,
                *context,
                &required,
            ) || crate::labels::context_proves_float_semantic_predicate(
                program, facts, *context, &required,
            )
        })
}

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

pub(super) fn collect_by_machine(
    facts: &CheckFacts,
) -> symbols::SymbolKeyMap<symbols::SymbolHandle, Vec<SelectedOperation>> {
    let mut by_machine =
        symbols::SymbolKeyMap::<symbols::SymbolHandle, Vec<SelectedOperation>>::default();
    for (expression, origin, selected, adapter) in facts.operators.resolved_expression_selections()
    {
        let CheckedArithmeticPolicyAdapter::FloatTrappingNonFinite { .. } = adapter else {
            continue;
        };
        let CheckedValueOrigin::StateStatement {
            machine_symbol,
            state_symbol,
            statement_index,
            ..
        } = origin
        else {
            continue;
        };
        by_machine
            .entry(machine_symbol)
            .or_default()
            .push(SelectedOperation {
                expression,
                selected,
                adapter,
                location: CrashSiteLocation::new(
                    state_symbol,
                    u32::try_from(statement_index).expect("statement ordinal fits u32"),
                ),
            });
    }
    by_machine
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
    let contexts = super::super::operators::InvocationContexts::for_selected_expression(
        program,
        facts,
        operation.expression,
        operation.selected,
        &operands,
    );
    let Some(raw_term) = super::super::operators::instantiated_float_semantic_term(
        program,
        facts,
        raw_result,
        operation.selected,
        parameters,
        &operands,
        program
            .machine_holding_state(operation.location.state())
            .map_or(symbols::SymbolHandle::invalid(), |machine| machine.symbol),
        operation.location.state(),
    ) else {
        return false;
    };
    let Some(required) = super::super::operators::finite_float_semantic_term(program, raw_term)
    else {
        return false;
    };
    if super::super::operators::float_boolean_term_value(&required) == Some(true) {
        return true;
    }
    contexts
        .for_expressions(program, parameters, [raw_result])
        .iter()
        .any(|context| {
            super::super::operators::context_proves_float_semantic_term(
                program, facts, *context, &required,
            )
        })
}

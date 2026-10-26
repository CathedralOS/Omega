//! Retained structural bindings name their actual production owner.
//! Consumers share this query instead of rebuilding a partial producer list.
//! Named bindings belong to their declaration; anonymous operands belong to
//! the exact expression that produced them, never their enclosing statement.

use checked_trees::{
    CheckFacts, CheckedUnitEffectOperationPlan, CheckedUnitLocalBindingPlan, FlowCallFact,
};
use facts::PlaceRoot;
use symbols::SymbolHandle;

pub(super) fn structural_result_owner(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    state: SymbolHandle,
    calls: &[FlowCallFact],
    operation: &CheckedUnitEffectOperationPlan,
) -> Option<PlaceRoot> {
    let result = operation.structural_result_binding()?;
    if result.local.is_named() {
        let state = program.state_by_symbol(state)?;
        let local = program
            .statement_table
            .statements(state.statement_nodes)
            .get(usize::try_from(result.statement_index).ok()?)?
            .as_local_data()?;
        return (local.symbol == result.local.symbol)
            .then_some(PlaceRoot::Symbol(result.local.symbol));
    }
    let expression = if let Some(call) = operation.call() {
        let mut producers = calls.iter().filter(|source| {
            source.statement_index == call.coordinate.statement_index as usize
                && source.call_ordinal == call.coordinate.call_ordinal as usize
        });
        let source = producers.next()?;
        if producers.next().is_some() {
            return None;
        }
        source.authored_expression
    } else {
        match operation {
            CheckedUnitEffectOperationPlan::EstablishLocal(
                CheckedUnitLocalBindingPlan::StructuralValue { value, .. },
            ) if facts.values.structural_values.nodes.is_valid(*value) => {
                facts.values.structural_values.nodes.get(*value).expression
            }
            CheckedUnitEffectOperationPlan::EstablishLocal(
                CheckedUnitLocalBindingPlan::ScalarArray {
                    source:
                        checked_trees::CheckedArrayConstructionSource::CallArgument {
                            call_ordinal,
                            parameter_position,
                        },
                    ..
                },
            ) => {
                let machine = program.symbols.get(state).parent;
                let site = validation::find_call_site(
                    program,
                    machine,
                    state,
                    result.statement_index as usize,
                    *call_ordinal as usize,
                )?;
                *validation::call_site_argument_expressions(program, &site)
                    .get(*parameter_position as usize)?
            }
            CheckedUnitEffectOperationPlan::EstablishReference {
                formation_expression,
                ..
            } => *formation_expression,
            CheckedUnitEffectOperationPlan::MoveStructuralField { expression, .. } => *expression,
            _ => return None,
        }
    };
    expression
        .is_valid()
        .then_some(PlaceRoot::Expression(expression))
}

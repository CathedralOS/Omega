//! Named storage and immutable production results have different lifetimes.
//! A rebind transfers its RHS into a local home and retires that production
//! result; later operands and cleanup must name the home's current value,
//! not revive the local's initializer. Both consumers use this same ordered
//! operation-prefix query. It selects storage identity, never access authority.

use checked_trees::{CheckedUnitEffectOperationPlan, CheckedUnitStructuralArgumentSourcePlan};
use symbols::SymbolHandle;
use typed_trees::statement::StatementNode;

pub(crate) fn source(
    program: &dyn validation::CheckingInput,
    state: &typed_trees::state::State,
    operations: &[CheckedUnitEffectOperationPlan],
    symbol: SymbolHandle,
) -> Option<CheckedUnitStructuralArgumentSourcePlan> {
    for operation in operations.iter().rev() {
        if operation.ends_local_occurrence(symbol) {
            return Some(CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol });
        }
        if matches!(operation, CheckedUnitEffectOperationPlan::EstablishStructuralLocal { symbol: local, .. } if *local == symbol)
        {
            return Some(CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol });
        }
        if let Some(result) = operation.statement_structural_result_binding()
            && matches!(program.statement_table.statements(state.statement_nodes).get(result.statement_index as usize),
                Some(StatementNode::LocalData(local)) if local.symbol == symbol)
        {
            return Some(CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                binding_ordinal: result.binding_ordinal,
            });
        }
    }
    None
}

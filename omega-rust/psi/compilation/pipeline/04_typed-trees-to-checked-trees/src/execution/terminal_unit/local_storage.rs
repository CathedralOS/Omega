//! Named storage and immutable production results have different lifetimes.
//! A rebind transfers its RHS into a local home and retires that production
//! result; later operands and cleanup must name the home's current value,
//! not revive the local's initializer. Consumers use this same ordered
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
        if matches!(operation, CheckedUnitEffectOperationPlan::EstablishLocal(checked_trees::CheckedUnitLocalBindingPlan::Structural { symbol: local, .. }) if *local == symbol)
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

/// The production still occupying `symbol`'s home after `operations`: the
/// binding ordinal of its declaration's establishment or initializer result.
/// A rebind has replaced every earlier production, so none occupies it. An
/// in-place declaration is still that declaration's own production even
/// though operands name its home rather than the result.
pub(crate) fn occupant(
    program: &dyn validation::CheckingInput,
    state: &typed_trees::state::State,
    operations: &[CheckedUnitEffectOperationPlan],
    symbol: SymbolHandle,
) -> Option<u32> {
    match source(program, state, operations, symbol)? {
        CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal } => {
            Some(binding_ordinal)
        }
        _ => operations
            .iter()
            .rev()
            .find_map(|operation| match operation {
                _ if operation.ends_local_occurrence(symbol) => Some(None),
                CheckedUnitEffectOperationPlan::EstablishLocal(
                    checked_trees::CheckedUnitLocalBindingPlan::Structural {
                        symbol: local,
                        result,
                        ..
                    },
                ) if *local == symbol => Some(Some(result.binding_ordinal)),
                _ => None,
            })?,
    }
}

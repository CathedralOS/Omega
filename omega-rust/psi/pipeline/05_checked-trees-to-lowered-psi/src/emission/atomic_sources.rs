//! Source correspondence for one checked atomic event.
//!
//! The checked plan names an event, its place, its orderings, its operand
//! rows and its result binding. None of that is trusted here: each fact is
//! reread from the authored carrier through the shared decoder
//! (`typed_trees_to_checked_trees::validation::atomic_load_carrier` / `atomic_assignment_carrier`) and the
//! place through the same projected-receiver resolution a primitive store
//! replays, so a plan that names another field, another operation, another
//! ordering, a substituted operand or a different result local refuses.
//!
//! The writing carriers occupy two statements: the result placeholder local
//! and the carrier assignment. The placeholder plans nothing; the event's
//! coordinate is the carrier and its result binding is the placeholder's.

use typed_trees_to_checked_trees::checked_trees::statement::StatementNode;
use typed_trees_to_checked_trees::checked_trees::{
    CheckedAtomicAccessPlan, CheckedAtomicEvent, CheckedAtomicReadModifyWrite,
    CheckedScalarExpressionRole, CheckedTrees,
};
use typed_trees_to_checked_trees::validation::AtomicAccessOperation;

use crate::lowering_error::{LoweringError, unsupported};

/// Whether the statement at `index` is an atomic writing carrier's result
/// placeholder: an immutable local immediately followed by the carrier whose
/// generated result names it. It declares the prior's binding and plans no
/// operation of its own.
pub(crate) fn result_placeholder(
    checked: &CheckedTrees,
    statements: &[StatementNode],
    index: usize,
) -> bool {
    let (Some(StatementNode::LocalData(local)), Some(StatementNode::Assignment(assignment))) =
        (statements.get(index), statements.get(index + 1))
    else {
        return false;
    };
    !local.is_mutable
        && typed_trees_to_checked_trees::validation::atomic_assignment_carrier(checked, assignment)
            .is_some_and(|carrier| names_local(checked, carrier.result, local))
}

fn names_local(
    checked: &CheckedTrees,
    result: typed_trees_to_checked_trees::checked_trees::expression::ExpressionHandle,
    local: &typed_trees_to_checked_trees::checked_trees::statement::TableLocalData,
) -> bool {
    if !checked.expression_table.expression_is_valid(result) {
        return false;
    }
    let typed_trees_to_checked_trees::checked_trees::expression::ExpressionNode::Name(path) =
        checked.expression_table.expression(result)
    else {
        return false;
    };
    if path.symbol.is_valid() {
        return path.symbol == local.symbol;
    }
    matches!(
        checked.expression_table.name_path_members(path.members),
        [member] if *member == local.name
    )
}

/// Rejoin `access` to its authored carrier: the event, orderings and operand
/// rows, the result binding's statement and local, and the place rooted at
/// `destination` (the authored parameter the plan names).
pub(crate) fn validate(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state_symbol: symbols::SymbolHandle,
    destination: symbols::SymbolHandle,
    access: &CheckedAtomicAccessPlan,
) -> Result<(), LoweringError> {
    let (_, state) =
        crate::expression_preparation::source_custody::authored_state(checked, state_symbol)?;
    let statements = checked.statement_table.statements(state.statement_nodes);
    let statement = access.statement_index as usize;
    let carrier = match (statements.get(statement), &access.event) {
        (Some(StatementNode::LocalData(local)), CheckedAtomicEvent::Load { .. }) => {
            let carrier = typed_trees_to_checked_trees::validation::atomic_load_carrier(
                checked,
                local.initial_value,
            )
            .ok_or(LoweringError::Unsupported(
                "atomic load lost its authored carrier",
            ))?;
            if local.is_mutable
                || access.result.as_ref().is_none_or(|result| {
                    result.statement_index != access.statement_index
                        || checked.primitive_type_reference(local.type_reference)
                            != Some(result.primitive_type)
                })
            {
                return unsupported("atomic load result differs from its authored local");
            }
            carrier
        }
        (Some(StatementNode::Assignment(assignment)), event)
            if !matches!(event, CheckedAtomicEvent::Load { .. }) =>
        {
            let carrier = typed_trees_to_checked_trees::validation::atomic_assignment_carrier(
                checked, assignment,
            )
            .ok_or(LoweringError::Unsupported(
                "atomic event lost its authored carrier",
            ))?;
            match &access.result {
                Some(result) => {
                    let placeholder = result.statement_index as usize;
                    if placeholder + 1 != statement
                        || !result_placeholder(checked, statements, placeholder)
                        || !matches!(statements.get(placeholder),
                            Some(StatementNode::LocalData(local))
                                if checked.primitive_type_reference(local.type_reference)
                                    == Some(result.primitive_type))
                    {
                        return unsupported("atomic event prior differs from its placeholder");
                    }
                }
                None if carrier.result.is_valid() => {
                    return unsupported("atomic event dropped its authored result");
                }
                None => {}
            }
            carrier
        }
        _ => return unsupported("atomic event has no authored carrier at its statement"),
    };
    if !same_operation(carrier.operation, &access.event) {
        return unsupported("atomic event differs from its authored operation or ordering");
    }
    let operands = access.event.operands();
    if operands.len() != carrier.operands.len() {
        return unsupported("atomic event operand roster differs from its carrier");
    }
    for (ordinal, (operand, authored)) in operands.iter().zip(&carrier.operands).enumerate() {
        let role = CheckedScalarExpressionRole::AtomicOperand {
            operand_ordinal: u32::try_from(ordinal)
                .map_err(|_| LoweringError::Unsupported("atomic operand ordinal overflow"))?,
        };
        let (binding, retained) = checked
            .facts
            .values
            .scalar_expressions
            .bound_expression_at(state_symbol, access.statement_index, role)
            .ok_or(LoweringError::Unsupported(
                "atomic operand has no retained source row",
            ))?;
        if binding.expression != *authored
            || !matches!(operand, typed_trees_to_checked_trees::checked_trees::CheckedCallScalarArgument::Pure(value)
                if value == retained)
        {
            return unsupported("atomic operand differs from its authored operand");
        }
    }
    let target = crate::emission::call_source_custody::projected_receivers::store_destination(
        checked,
        machine,
        state_symbol,
        Some(statement),
        carrier.place,
    )?;
    let (field, carrier) = target.path.split_last().ok_or(LoweringError::Unsupported(
        "atomic event place names no field",
    ))?;
    if !destination.is_valid()
        || target.root != destination
        || carrier != access.carrier_path.as_slice()
        || !matches!(field, typed_trees_to_checked_trees::checked_trees::CheckedUnitStructuralPathSegment::Field(identity)
            if *identity == access.field_identity)
    {
        return unsupported("atomic event place differs from its authored place");
    }
    Ok(())
}

/// The plan's event and orderings are exactly the carrier's operation.
fn same_operation(operation: AtomicAccessOperation, event: &CheckedAtomicEvent) -> bool {
    use CheckedAtomicReadModifyWrite as Fetch;
    match (operation, event) {
        (AtomicAccessOperation::Load(authored), CheckedAtomicEvent::Load { ordering })
        | (AtomicAccessOperation::Store(authored), CheckedAtomicEvent::Store { ordering, .. })
        | (AtomicAccessOperation::Swap(authored), CheckedAtomicEvent::Swap { ordering, .. }) => {
            authored == *ordering
        }
        (
            AtomicAccessOperation::FetchAdd(authored)
            | AtomicAccessOperation::FetchSub(authored)
            | AtomicAccessOperation::FetchAnd(authored)
            | AtomicAccessOperation::FetchOr(authored)
            | AtomicAccessOperation::FetchXor(authored),
            CheckedAtomicEvent::ReadModifyWrite {
                operation: fetch,
                ordering,
                ..
            },
        ) => {
            authored == *ordering
                && matches!(
                    (operation, fetch),
                    (AtomicAccessOperation::FetchAdd(_), Fetch::FetchAdd)
                        | (AtomicAccessOperation::FetchSub(_), Fetch::FetchSub)
                        | (AtomicAccessOperation::FetchAnd(_), Fetch::FetchAnd)
                        | (AtomicAccessOperation::FetchOr(_), Fetch::FetchOr)
                        | (AtomicAccessOperation::FetchXor(_), Fetch::FetchXor)
                )
        }
        (
            AtomicAccessOperation::CompareExchange { success, failure },
            CheckedAtomicEvent::CompareExchange {
                success: planned_success,
                failure: planned_failure,
                ..
            },
        ) => success == *planned_success && failure == *planned_failure,
        _ => false,
    }
}

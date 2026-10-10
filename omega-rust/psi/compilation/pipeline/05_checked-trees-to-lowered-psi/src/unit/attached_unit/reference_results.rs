//! Reference carriers retain ingress provenance and the exact source loan end.
//! A consumer borrows the referent; it does not acquire its linear claims or
//! cleanup. The checked borrow-free storage judgment excludes nested carriers
//! without requiring the referent to be a freely copied owned snapshot.
use super::super::StructuralArgument;
use super::{
    LoweringError, Operation, OperationKind, StructuralMultiplicity, StructuralPlaceDeclaration,
    StructuralPlaceKind, StructuralTypeId, allocate_dense, lookup_type_id, place_id,
};
use crate::emission::operation_emission::buffer::{OperationBuffer, structural_result_declaration};
use checked_trees::CheckedUnitStructuralResultBindingPlan;

/// The checked expression the reference operation actually evaluates.
/// Source coordinates stay in this producer companion, never in Terminal.
pub(super) struct Formation<'checked> {
    pub(super) checked: &'checked checked_trees::CheckedTrees,
    pub(super) state: symbols::SymbolHandle,
    pub(super) statement_index: usize,
    pub(super) expression: checked_trees::expression::ExpressionHandle,
}

pub(super) fn emit(
    formation: Formation<'_>,
    result: &CheckedUnitStructuralResultBindingPlan,
    source: StructuralArgument,
    storage: Option<terminal_psi::ReferenceStorageView>,
    types: &[(String, StructuralTypeId)],
    next_place: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<StructuralPlaceDeclaration, LoweringError> {
    let structural_type = lookup_type_id(types, &result.type_identity)?;
    emit_carrier(
        formation,
        structural_type,
        source,
        storage,
        next_place,
        operations,
    )
}

pub(super) fn emit_carrier(
    formation: Formation<'_>,
    structural_type: StructuralTypeId,
    source: StructuralArgument,
    storage: Option<terminal_psi::ReferenceStorageView>,
    next_place: &mut u64,
    operations: &mut OperationBuffer,
) -> Result<StructuralPlaceDeclaration, LoweringError> {
    let id = operations.allocate();
    let place = place_id(allocate_dense(next_place)?);
    // The ordinary and constructed-reference routes share this capture. The
    // lowered source is already evaluated, including every selector snapshot.
    // These rows associate checked occurrences with emitted coordinates; they
    // neither replay the borrow judgment nor grant preservation permission.
    let borrow = &formation.checked.facts.borrow;
    if let Some(state) = borrow
        .states
        .iter()
        .find_map(|(_, state)| (state.state_symbol == formation.state).then_some(state))
    {
        for loan in state.loans.handles().filter(|loan| {
            let row = borrow.loans.get(*loan);
            row.statement_index == formation.statement_index
                && row.formation_expression == formation.expression
        }) {
            operations
                .loan_captures
                .push(lowered_psi::LoweredSourceLoanCapture {
                    loan,
                    formation: id,
                    carrier: place,
                    source: source.clone(),
                });
        }
    }
    operations.push(Operation {
        static_reach_binding: None,
        suspension_crossing: None,
        id,
        result: structural_result_declaration(
            place,
            structural_type,
            StructuralMultiplicity::Affine,
        ),
        kind: OperationKind::EstablishReference { source, storage },
    });
    Ok(StructuralPlaceDeclaration {
        id: place,
        kind: StructuralPlaceKind::OperationResult {
            producer: id,
            structural_type,
        },
    })
}

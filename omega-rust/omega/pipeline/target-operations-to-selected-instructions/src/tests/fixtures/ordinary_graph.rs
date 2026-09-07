//! Named access to ordinary graph rows in structural custody fixtures.
use legalized_operations::{
    LegalizedBoundarySettlement, LegalizedScalarCall, LegalizedScalarFunction,
    LegalizedScalarInstructionKind,
};

pub(in crate::tests) fn call(function: &LegalizedScalarFunction) -> &LegalizedScalarCall {
    function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .find_map(|row| match &row.kind {
            LegalizedScalarInstructionKind::Call(call) => Some(call),
            _ => None,
        })
        .expect("call fixture")
}

pub(in crate::tests) fn call_mut(
    function: &mut LegalizedScalarFunction,
) -> &mut LegalizedScalarCall {
    function
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.instructions)
        .find_map(|row| match &mut row.kind {
            LegalizedScalarInstructionKind::Call(call) => Some(call),
            _ => None,
        })
        .expect("call fixture")
}

pub(in crate::tests) fn settlements(
    function: &LegalizedScalarFunction,
) -> Vec<&LegalizedBoundarySettlement> {
    function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|row| match &row.kind {
            LegalizedScalarInstructionKind::BoundarySettlement(settlement) => Some(settlement),
            _ => None,
        })
        .collect()
}

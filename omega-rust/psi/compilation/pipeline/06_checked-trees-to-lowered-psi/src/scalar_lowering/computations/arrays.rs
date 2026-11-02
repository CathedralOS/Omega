//! Discover scalar-array places emitted by the common value producer.
use super::super::{Operation, OperationKind, StructuralPlaceDeclaration};
use crate::emission::operation_emission::place_declarations::operation_result_declaration;

pub(crate) fn declarations(
    operations: &[Operation],
) -> impl Iterator<Item = StructuralPlaceDeclaration> + '_ {
    operations.iter().filter_map(|operation| {
        if !matches!(operation.kind, OperationKind::EstablishScalarArray { .. }) {
            return None;
        }
        operation_result_declaration(operation)
    })
}

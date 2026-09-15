use crate::lowering_error::{LoweringError, unsupported};
use checked_trees::CheckedTrees;
use checked_trees::types::PrimitiveType;
use checked_trees::{
    CheckedStructuralAccess, CheckedUnitStructuralArgumentPlan,
    CheckedUnitStructuralArgumentSourcePlan,
};

#[allow(clippy::too_many_arguments)]
pub(crate) fn validate_source(
    checked: &CheckedTrees,
    machine: symbols::SymbolHandle,
    state: symbols::SymbolHandle,
    statement: u32,
    expression: checked_trees::expression::ExpressionHandle,
    subject: &CheckedUnitStructuralArgumentPlan,
    field: symbols::SymbolHandle,
    primitive: PrimitiveType,
) -> Result<(), LoweringError> {
    let source = validation::local_scalar_record_field(
        &checked.typed,
        machine,
        state,
        statement,
        expression,
    )
    .ok_or(LoweringError::Unsupported(
        "record read lost its authored local and field",
    ))?;
    if subject.source
        != (CheckedUnitStructuralArgumentSourcePlan::StructuralLocal {
            symbol: source.local,
        })
        || subject.access != CheckedStructuralAccess::SharedBorrow
        || subject.path
            != source
                .path
                .iter()
                .cloned()
                .map(checked_trees::CheckedUnitStructuralPathSegment::Field)
                .collect::<Vec<_>>()
        || subject.type_identity
            != checked
                .normalized_type_identity(source.carrier_type_reference)
                .as_str()
        || source.field != field
        || source.primitive_type != primitive
    {
        return unsupported("record read differs from its authored local and field");
    }
    Ok(())
}

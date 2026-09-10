//! Raw test entrances independently admit the supplied target/catalog join.
//! Production construction and replay borrow one immutable validated environment.
use super::*;

pub(in crate::selection) fn validate(
    function: usize,
    source: &LegalizedScalarFunction,
    selected: &SelectedFunction,
    native_target: target::NativeTarget,
    constraints: &SelectedSelectionConstraints,
    physical: &ValidatedPhysicalRegisterModel,
    catalog: &ValidatedRegisterConstraintCatalog,
) -> Result<(), SelectedInstructionError> {
    let environment = register_environment::validate_target_register_environment(
        native_target,
        physical.model().clone(),
        catalog.catalog().clone(),
    )
    .map_err(|_| SelectedInstructionError::FunctionProjectionMismatch { function })?;
    validate_with_environment(function, source, selected, constraints, &environment)
}

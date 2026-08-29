//! Common condition-input and physical-register context reconstruction.

use crate::selection::constraints::fixed_input_constraint;
use crate::selection::shared::*;

use super::model::ScalarConstructionContext;

pub(super) fn reconstruct<'a>(
    function: usize,
    source: &'a SourceFunction,
    constraints: &'a SelectedSelectionConstraints,
    physical: &'a ValidatedPhysicalRegisterModel,
    catalog: &'a ValidatedRegisterConstraintCatalog,
) -> Result<ScalarConstructionContext<'a>, SelectedInstructionError> {
    let input = fixed_input_constraint(
        source.machine,
        source.condition_source,
        source.condition_parameter_index,
        source.condition_register,
        &constraints.fixed_inputs,
    )
    .ok_or(SelectedInstructionError::MissingInputRegisterView { function })?;
    let input_view = input.fixed_view;
    let input_class = physical
        .model()
        .views
        .iter()
        .find(|view| view.id == input_view)
        .ok_or(SelectedInstructionError::MissingInputRegisterView { function })?
        .class;
    let u64_type =
        ScalarType::Integer(psi_core::IntegerType::new(IntegerSign::Unsigned, 64).expect("u64"));
    Ok(ScalarConstructionContext {
        function,
        source,
        constraints,
        physical,
        catalog,
        input_class,
        input_view,
        u64_type,
    })
}

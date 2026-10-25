//! Atomic selection admits an exact declared scalar field and access mode,
//! not merely a same-width memory slot. This shared query has no selected
//! stream or register state; producer and replay bind their own operands,
//! observed result, memory access records and native constraint below it.
use legalized_operations::{
    LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
};
use selected_instructions::AtomicForm;
use semantic_vocabulary::ScalarType;
use terminal_psi::StructuralAccess;

pub(super) fn form(
    source: &LegalizedScalarFunction,
    row: &LegalizedScalarInstruction,
) -> Option<(
    AtomicForm,
    crate::structural_inputs::structural_reference_input::PrimitiveAccessGeometry,
    ScalarType,
)> {
    let LegalizedScalarInstructionKind::AtomicAccess {
        location,
        field,
        scalar_type,
        field_byte_offset,
        event,
        outcome,
    } = &row.kind
    else {
        return None;
    };
    let form = AtomicForm::from_access(*scalar_type, *event)?;
    let signature = source.structural.as_ref()?;
    let root = super::reference_input::root_type(source, location.place)?;
    let root_access = if let Some(parameter) =
        super::arrival_parameter_input::parameter(source, location.place)
    {
        parameter.access
    } else {
        super::aggregate_result_input::home(source, location.place)?;
        StructuralAccess::Owned
    };
    if root_access != location.access
        || event.observes_resident() && root_access == StructuralAccess::WriteOnlyBorrow
        || row.result.is_some()
            != (event.observes_resident() && event.single_attempt_custody().is_none())
    {
        return None;
    }
    match (event.single_attempt_custody(), outcome) {
        (None, None) => {}
        (Some(_), Some((result, layout)))
            if result.is_unrestricted_plain()
                && result.qualification_establishments.is_empty()
                && signature
                    .structural_types
                    .iter()
                    .find(|row| row.id == result.structural_type)
                    .and_then(|row| {
                        target_operations::conventional_atomic_once_layout(row, *scalar_type)
                    })
                    .as_ref()
                    == Some(layout) => {}
        _ => return None,
    }
    let declaration = terminal_semantics::projected_field_declaration(
        signature.structural_types.iter(),
        root,
        &location.path,
        *field,
    )?;
    let logical_type = declaration.field_type.atomic_scalar_type()?;
    if target_operations::operations::atomic_storage_integer_type(*event, logical_type)?
        != *scalar_type
        || row
            .result
            .is_some_and(|result| result.scalar_type != logical_type)
    {
        return None;
    }
    let geometry = crate::structural_inputs::structural_reference_input::atomic_field_geometry(
        root,
        &location.path,
        *field,
        logical_type,
        &signature.structural_types,
    )?;
    (geometry.byte_offset == *field_byte_offset
        && geometry.byte_size == u32::from(form.width.bytes())
        && geometry.byte_offset % geometry.byte_size == 0)
        .then_some((form, geometry, logical_type))
}

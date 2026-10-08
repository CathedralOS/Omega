//! Atomic selection admits an exact declared scalar field and access mode,
//! not merely a same-width memory slot. This shared query has no selected
//! stream or register state; producer and replay bind their own operands,
//! observed result, memory access records and native constraint below it.
use legalized_operations::{
    LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
};
use selected_instructions::AtomicForm;
use semantic_vocabulary::ScalarType;
use terminal_psi::{StructuralAccess, StructuralFieldType};

pub(super) fn form(
    source: &LegalizedScalarFunction,
    row: &LegalizedScalarInstruction,
) -> Option<AtomicForm> {
    let LegalizedScalarInstructionKind::AtomicAccess {
        location,
        field,
        scalar_type,
        field_byte_offset,
        event,
    } = &row.kind
    else {
        return None;
    };
    let form = AtomicForm::from_access(*scalar_type, *event)?;
    let signature = source.structural.as_ref()?;
    let parameter = crate::selection::arrival_parameter_input::parameter(source, location.place)?;
    if parameter.access != location.access
        || !parameter.qualifications.is_empty()
        || !parameter.projected_qualifications.is_empty()
        || event.observes_resident() && parameter.access == StructuralAccess::WriteOnlyBorrow
        || row.result.is_some() != event.observes_resident()
        || row
            .result
            .is_some_and(|result| result.scalar_type != ScalarType::Integer(*scalar_type))
    {
        return None;
    }
    let (_, declaration) = terminal_semantics::static_field_projection(
        signature.structural_types.iter(),
        parameter.structural_type,
        &location.path,
        *field,
    )?;
    if declaration.field_type != StructuralFieldType::Atomic(ScalarType::Integer(*scalar_type)) {
        return None;
    }
    let (offset, size, indices) = crate::structural_inputs::structural_reference_input::store(
        parameter.structural_type,
        &location.path,
        *field,
        ScalarType::Integer(*scalar_type),
        &signature.structural_types,
    )?;
    (indices.is_empty()
        && offset == *field_byte_offset
        && size == u32::from(form.width.bytes())
        && offset % size == 0)
        .then_some(form)
}

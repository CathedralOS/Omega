//! Explicit zero-backed host roots: reject duplicate establishments and any
//! overlapping explicit initialization before binding invocation storage.

use crate::errors::TerminalInterpretError;
use crate::structural_inputs::TerminalStructuralInputs;

pub(crate) fn validate(inputs: TerminalStructuralInputs<'_>) -> Result<(), TerminalInterpretError> {
    for field in inputs.established_erased_fields {
        if !inputs
            .zero_initialized_arguments
            .contains(&field.argument_index)
            || field.field_path.is_empty()
            || field.carrier_type_identity.is_empty()
        {
            return Err(TerminalInterpretError::ZeroInitializedArgumentInvalid {
                argument_index: field.argument_index,
            });
        }
    }
    for (position, argument_index) in inputs
        .zero_initialized_arguments
        .iter()
        .copied()
        .enumerate()
    {
        let invalid = || TerminalInterpretError::ZeroInitializedArgumentInvalid { argument_index };
        let root = inputs
            .arguments
            .get(argument_index as usize)
            .ok_or_else(invalid)?;
        let overlaps = |other_index: u32, path: &[terminal_psi::StructuralPathSegment]| {
            inputs
                .arguments
                .get(other_index as usize)
                .is_some_and(|other| {
                    if root.opaque_identity != other.opaque_identity {
                        return false;
                    }
                    let mut other_path = other.path.clone();
                    other_path.extend_from_slice(path);
                    terminal_semantics::structural_paths_may_overlap(&root.path, &other_path)
                })
        };
        if inputs.zero_initialized_arguments[..position]
            .iter()
            .any(|other| overlaps(*other, &[]))
            || inputs
                .scalar_fields
                .iter()
                .any(|field| overlaps(field.argument_index, &field.path))
            || inputs
                .boolean_fields
                .iter()
                .any(|field| overlaps(field.argument_index, &field.path))
            || inputs
                .primitive_values
                .iter()
                .any(|value| overlaps(value.argument_index, &[]))
            || inputs
                .cases
                .iter()
                .any(|case| overlaps(case.argument_index, &case.path))
            || inputs
                .byte_arrays
                .iter()
                .any(|array| overlaps(array.argument_index, &array.path))
            || inputs
                .placed_view_establishments
                .iter()
                .any(|establishment| {
                    root.opaque_identity == establishment.referent.opaque_identity
                        && terminal_semantics::structural_paths_may_overlap(
                            &root.path,
                            &establishment.referent.path,
                        )
                })
        {
            return Err(invalid());
        }
    }
    Ok(())
}

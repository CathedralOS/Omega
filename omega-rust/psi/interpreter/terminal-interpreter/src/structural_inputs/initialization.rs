//! Complete host initializers cannot overlap one another or a partial input.
//! Validate that relationship before allocating any invocation backing.

use crate::errors::TerminalInterpretError;
use crate::structural_inputs::TerminalStructuralInputs;
use terminal_semantics::structural_paths_may_overlap;

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
    let initializers = || {
        inputs
            .zero_initialized_arguments
            .iter()
            .copied()
            .map(|argument_index| (argument_index, true))
            .chain(
                inputs
                    .contents
                    .iter()
                    .map(|contents| (contents.argument_index, false)),
            )
    };
    for (position, (argument_index, zeroed)) in initializers().enumerate() {
        let invalid = || {
            if zeroed {
                TerminalInterpretError::ZeroInitializedArgumentInvalid { argument_index }
            } else {
                TerminalInterpretError::StructuralContentsArgumentInvalid {
                    argument_index,
                    reason: "complete contents overlap another initializer or name no argument",
                }
            }
        };
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
                    structural_paths_may_overlap(&root.path, &other_path)
                })
        };
        if initializers()
            .take(position)
            .any(|(other, _)| overlaps(other, &[]))
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
                .byte_arrays
                .iter()
                .any(|array| overlaps(array.argument_index, &array.path))
            || inputs
                .access_view_establishments
                .iter()
                .any(|establishment| {
                    root.opaque_identity == establishment.referent.opaque_identity
                        && structural_paths_may_overlap(&root.path, &establishment.referent.path)
                })
        {
            return Err(invalid());
        }
    }
    Ok(())
}

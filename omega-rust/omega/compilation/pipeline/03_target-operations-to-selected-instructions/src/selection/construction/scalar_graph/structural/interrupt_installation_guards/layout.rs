//! Resolve source arguments before asking the common target geometry owner.
use super::*;
use semantic_vocabulary::StructuralTypeId;
use terminal_psi::StructuralTypeShape;
pub(super) type Layout = crate::InstalledInterruptOperandLayout;
fn argument_type(
    source: &LegalizedScalarFunction,
    argument: &terminal_psi::StructuralArgument,
) -> Result<StructuralTypeId, SelectedInstructionError> {
    let declarations = &source
        .structural
        .as_ref()
        .ok_or_else(|| failure("installation has no structural declarations"))?
        .structural_types;
    let root = crate::selection::reference_input::root_type(source, argument.place)
        .ok_or_else(|| failure("installation argument has no root type"))?;
    let selected = geometry::reference_formation_geometry(root, &argument.path, declarations)
        .and_then(|shape| shape.selected.as_structural())
        .ok_or_else(|| failure("installation argument lost its referent type"))?;
    // Reference formation may select the reference carrier itself at a whole
    // argument. Follow that exact declaration, never a matching catalog type.
    Ok(
        match terminal_psi::find_structural_type(declarations, selected).map(|row| &row.shape) {
            Some(StructuralTypeShape::Reference { referent, .. }) => *referent,
            _ => selected,
        },
    )
}

impl Layout {
    pub(super) fn prepare(
        source: &LegalizedScalarFunction,
        arguments: &[terminal_psi::StructuralArgument; 6],
    ) -> Result<Self, SelectedInstructionError> {
        let declarations = &source
            .structural
            .as_ref()
            .ok_or_else(|| failure("installation has no structural declarations"))?
            .structural_types;
        let mut types = Vec::with_capacity(6);
        for argument in arguments {
            types.push(argument_type(source, argument)?);
        }
        Self::derive(
            declarations,
            types
                .try_into()
                .map_err(|_| failure("installation argument roster changed"))?,
        )
        .map_err(failure)
    }
}

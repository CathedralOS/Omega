//! Reconstruct every scalar-leaf byte range of a zeroed establishment.
use calling_conventions::ValueShape;
use legalized_operations::{
    LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
};
use terminal_psi::StructuralMultiplicity;

/// Reconstruct a zeroed establishment's exact home shape and each scalar
/// leaf's `(byte offset, width)` in canonical leaf order. Construction and
/// independent instruction replay each consume this input.
pub(super) fn storage<'a>(
    source: &LegalizedScalarFunction,
    row: &'a LegalizedScalarInstruction,
) -> Option<(
    &'a terminal_psi::StructuralOperationResult,
    ValueShape,
    Vec<(u32, u8)>,
)> {
    let LegalizedScalarInstructionKind::EstablishZeroedStructural { result, shape } = &row.kind
    else {
        return None;
    };
    let declarations = &source.structural.as_ref()?.structural_types;
    if row.result.is_some()
        || result.multiplicity != StructuralMultiplicity::Unrestricted
        || !result.claims.is_empty()
        || !result.qualifications.is_empty()
        || !result.projected_qualifications.is_empty()
        || crate::structural_inputs::structural_reference_input::shape(
            result.structural_type,
            declarations,
        )? != *shape
    {
        return None;
    }
    let leaves =
        terminal_semantics::zeroed_structural_leaves(declarations.iter(), result.structural_type)?;
    let mut stores = Vec::with_capacity(leaves.len());
    for leaf in leaves {
        // A zeroed leaf path is fully static — every segment is a declared
        // field or a literal index — so the projection spells no runtime
        // element.
        let (offset, width, elements) =
            crate::structural_inputs::structural_reference_input::primitive_geometry(
                result.structural_type,
                leaf.path(),
                leaf.scalar_type(),
                declarations,
            )?;
        if !elements.is_empty() {
            return None;
        }
        stores.push((offset, width));
    }
    Some((result, *shape, stores))
}

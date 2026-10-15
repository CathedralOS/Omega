//! Reconstruct a memberwise structural local establishment's home shape.
use calling_conventions::ValueShape;
use legalized_operations::{
    LegalizedScalarFunction, LegalizedScalarInstruction, LegalizedScalarInstructionKind,
};

/// Resolve the establishment's exact home shape. The local's members arrive
/// as their own store operations, so this input names the whole home rather
/// than a leaf roster; construction and independent instruction replay each
/// consume it.
pub(super) fn storage<'a>(
    source: &LegalizedScalarFunction,
    row: &'a LegalizedScalarInstruction,
) -> Option<(&'a terminal_psi::StructuralOperationResult, ValueShape)> {
    let LegalizedScalarInstructionKind::EstablishStructuralLocal { result, shape } = &row.kind
    else {
        return None;
    };
    let declarations = &source.structural.as_ref()?.structural_types;
    if row.result.is_some()
        || !matches!(
            result.multiplicity,terminal_psi::nonlinear_multiplicity_pair_pattern!())
        || !result.claims.is_empty()
        // Only a descriptor home declares qualifications: byte predicates of
        // its contents, which leave its storage shape unchanged.
        || (!result.qualifications.is_empty()
            && !terminal_semantics::descriptor_results::is_descriptor(
                declarations.iter(),
                result.structural_type,
            ))
        || !result.projected_qualifications.is_empty()
        || crate::structural_inputs::structural_reference_input::shape(
            result.structural_type,
            declarations,
        )? != *shape
    {
        return None;
    }
    Some((result, *shape))
}

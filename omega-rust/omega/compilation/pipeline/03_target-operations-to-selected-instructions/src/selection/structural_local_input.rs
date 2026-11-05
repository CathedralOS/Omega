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
) -> Result<(&'a terminal_psi::StructuralOperationResult, ValueShape), &'static str> {
    let LegalizedScalarInstructionKind::EstablishStructuralLocal { result, shape } = &row.kind
    else {
        return Err("structural local input has another operation kind");
    };
    let declarations = &source
        .structural
        .as_ref()
        .ok_or("structural local has no structural catalog")?
        .structural_types;
    terminal_semantics::validate_vacant_structural_local(declarations.iter(), result)?;
    if row.result.is_some() {
        return Err("structural local unexpectedly publishes a scalar result");
    }
    let declared_shape = crate::structural_inputs::shape(result.structural_type, declarations)
        .ok_or("structural local has no closed storage shape")?;
    if declared_shape != *shape {
        return Err("structural local differs from its declared storage shape");
    }
    Ok((result, *shape))
}

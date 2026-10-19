//! Native geometry for the complete outcome of an admitted atomic attempt.
use calling_conventions::{ConventionalSumLayout, ValueShape};
use semantic_vocabulary::ScalarType;
use terminal_psi::StructuralTypeDeclaration;

/// Geometry of an already selected single-attempt event's complete outcome.
/// The plain scalar payload has its admitted native width; both observing
/// failure cases use the same placement, while success has no active payload.
pub fn conventional_atomic_once_layout(
    declaration: &StructuralTypeDeclaration,
    integer: semantic_vocabulary::IntegerType,
) -> Option<ConventionalSumLayout> {
    terminal_psi::types::atomic_once_outcome_cases(declaration, ScalarType::Integer(integer))?;
    if integer.is_address() || !matches!(integer.bits(), 8 | 16 | 32 | 64) {
        return None;
    }
    let bytes = u32::from(integer.bits() / 8);
    let field = ValueShape::integer(bytes, bytes as u16);
    calling_conventions::evaluate_conventional_sum_layout(&[], &[vec![field], vec![], vec![field]])
        .ok()
}

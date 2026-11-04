//! Native geometry for the complete outcome of an admitted atomic attempt.
use calling_conventions::{ConventionalSumLayout, ValueShape};
use semantic_vocabulary::ScalarType;
use terminal_psi::StructuralTypeDeclaration;

/// Geometry of an already selected single-attempt event's complete outcome.
/// Each canonical case carries its admitted scalar payload, when present.
/// Observing success is empty; non-observing success owns the displaced value.
pub fn conventional_atomic_once_layout(
    declaration: &StructuralTypeDeclaration,
    integer: semantic_vocabulary::IntegerType,
    custody: language_core::atomic::AtomicCompareExchangeOnceResultCustody,
) -> Option<ConventionalSumLayout> {
    terminal_psi::types::atomic_once_outcome_cases(
        declaration,
        ScalarType::Integer(integer),
        custody,
    )?;
    if integer.is_address() || !matches!(integer.bits(), 8 | 16 | 32 | 64) {
        return None;
    }
    let bytes = u32::from(integer.bits() / 8);
    let field = ValueShape::integer(bytes, bytes as u16);
    let cases = custody
        .outcome_identity
        .cases()
        .iter()
        .map(|case| {
            if case.payload.is_some() {
                vec![field]
            } else {
                Vec::new()
            }
        })
        .collect::<Vec<_>>();
    calling_conventions::evaluate_conventional_sum_layout(&[], &cases).ok()
}

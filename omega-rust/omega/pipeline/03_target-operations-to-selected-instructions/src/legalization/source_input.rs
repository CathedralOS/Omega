//! Borrow the existing verifier context rather than asserting cyclic authority.
use abstract_operations_to_abstract_operations::ValidatedOptimizedAbstractPlan;
use optimization_unit::PsiOptimizationUnit;
use terminal_psi_to_abstract_operations::VerifiedPsiOptimizationInput;

/// Source custody for the single legalization path. Raw units retain their
/// existing standalone checks; natural cycles additionally require the verified
/// artifact retained by the validated abstract phase. Construction and replay
/// independently authenticate its current graph before admitting those cycles.
#[derive(Clone, Copy)]
pub struct LegalizationSource<'source> {
    pub(super) unit: &'source PsiOptimizationUnit,
    pub(super) verified_input: Option<&'source VerifiedPsiOptimizationInput>,
}

impl<'source> From<&'source PsiOptimizationUnit> for LegalizationSource<'source> {
    fn from(unit: &'source PsiOptimizationUnit) -> Self {
        Self {
            unit,
            verified_input: None,
        }
    }
}

impl<'source> From<&'source ValidatedOptimizedAbstractPlan> for LegalizationSource<'source> {
    fn from(source: &'source ValidatedOptimizedAbstractPlan) -> Self {
        Self {
            unit: source.unit(),
            verified_input: Some(source.verified_input()),
        }
    }
}

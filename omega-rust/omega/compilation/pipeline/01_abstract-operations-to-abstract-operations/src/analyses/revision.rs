use optimization_core::{AnalysisKind, AnalysisSet};
use optimization_unit::PsiOptimizationUnit;

use super::control_flow::{AnalysisRootError, validate_unit_identity};
use super::{AnalysisProduct, compute_analysis};
use crate::RuleAnalysisView;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnalysisError {
    Root(AnalysisRootError),
    UnsupportedAnalysis(AnalysisKind),
}

failed_display!(AnalysisError, "Psi optimizer analysis failure");

/// The analyses of one content-validated unit revision. Every rule evaluated
/// against the revision shares each product it computes; a commit produces a
/// new unit, and the pass manager binds a new revision to it, so no product
/// outlives the unit it describes.
#[derive(Debug)]
pub struct AnalysisRevision<'a> {
    unit: &'a PsiOptimizationUnit,
    products: [Option<AnalysisProduct>; AnalysisKind::ALL.len()],
}

impl<'a> AnalysisRevision<'a> {
    /// Validate the unit's content identity once for all of this revision's
    /// rule requests. The borrow keeps the unit from changing beneath them.
    pub(crate) fn bind(unit: &'a PsiOptimizationUnit) -> Result<Self, AnalysisError> {
        validate_unit_identity(unit).map_err(AnalysisError::Root)?;
        Ok(Self {
            unit,
            products: std::array::from_fn(|_| None),
        })
    }

    /// Compute each requested product the first time this revision is asked
    /// for it. Products compute from the unit alone, so nothing a rule did not
    /// request is computed or visible to it.
    pub(crate) fn require_all(
        &mut self,
        requested: AnalysisSet,
    ) -> Result<RuleAnalysisView<'_>, AnalysisError> {
        for kind in requested.iter() {
            let slot = &mut self.products[analysis_index(kind)];
            if slot.is_none() {
                *slot = Some(
                    compute_analysis(self.unit, kind)
                        .ok_or(AnalysisError::UnsupportedAnalysis(kind))?,
                );
            }
        }
        Ok(RuleAnalysisView::new(&self.products, requested))
    }
}

/// `AnalysisKind` discriminants are the dense range `1..=ALL.len()`.
pub(crate) const fn analysis_index(kind: AnalysisKind) -> usize {
    kind as usize - 1
}

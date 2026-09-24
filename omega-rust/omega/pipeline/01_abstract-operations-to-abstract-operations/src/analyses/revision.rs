use optimization_core::AnalysisSet;
use optimization_unit::PsiOptimizationUnit;

use super::{AnalysisManager, AnalysisManagerError};
use crate::RuleAnalysisView;

/// An exact content-validated revision borrowed for a sequence of rule requests.
/// Neither its unit nor its cache can be replaced while the borrow is alive.
#[derive(Debug)]
pub struct AnalysisRevision<'a> {
    unit: &'a PsiOptimizationUnit,
    manager: &'a mut AnalysisManager,
}

impl<'a> AnalysisRevision<'a> {
    pub(super) fn new(unit: &'a PsiOptimizationUnit, manager: &'a mut AnalysisManager) -> Self {
        Self { unit, manager }
    }

    /// Resolve declared products without revalidating immutable content or
    /// copying payloads. Dependency-only products remain hidden from the rule.
    pub fn require_all(
        &mut self,
        requested: AnalysisSet,
    ) -> Result<RuleAnalysisView<'_>, AnalysisManagerError> {
        self.manager.require_view(self.unit, requested)
    }
}

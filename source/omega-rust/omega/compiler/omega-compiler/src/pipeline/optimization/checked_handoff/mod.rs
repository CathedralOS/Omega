//! Optimizer module role: executable entrance. Checked optimization-selection custody.
//!
//! The exact selection, its identity, and the independent report request leave
//! build evaluation together. Empty selections remain ordinary data and do not
//! instantiate optimizer machinery here.

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::pipeline) struct CheckedOptimizationHandoff {
    selections: omega_optimization_core::OptimizationSelections,
    selection_identity: omega_optimization_core::OptimizationSelectionIdentity,
    report: omega_optimization_pipeline::OptimizationReportRequest,
}

impl CheckedOptimizationHandoff {
    pub(in crate::pipeline) fn retain(
        selections: omega_optimization_core::OptimizationSelections,
        report: omega_optimization_pipeline::OptimizationReportRequest,
    ) -> Self {
        let selection_identity = selections.identity();
        Self {
            selections,
            selection_identity,
            report,
        }
    }

    pub(in crate::pipeline) const fn selections(
        &self,
    ) -> &omega_optimization_core::OptimizationSelections {
        &self.selections
    }

    pub(in crate::pipeline) const fn selection_identity(
        &self,
    ) -> omega_optimization_core::OptimizationSelectionIdentity {
        self.selection_identity
    }

    pub(in crate::pipeline) const fn report(
        &self,
    ) -> omega_optimization_pipeline::OptimizationReportRequest {
        self.report
    }
}

#[cfg(test)]
mod tests {
    use super::CheckedOptimizationHandoff;

    #[test]
    fn empty_selection_is_retained_without_enabling_optimizer_work() {
        let selections = omega_optimization_core::OptimizationSelections::default();
        let handoff = CheckedOptimizationHandoff::retain(
            selections.clone(),
            omega_optimization_pipeline::OptimizationReportRequest::Suppressed,
        );

        assert!(handoff.selections().is_empty());
        assert_eq!(handoff.selections(), &selections);
        assert_eq!(handoff.selection_identity(), selections.identity());
        assert_eq!(
            handoff.report(),
            omega_optimization_pipeline::OptimizationReportRequest::Suppressed
        );
    }
}

//! Optimizer module role: executable entrance. Checked optimization-selection custody.
//!
//! The exact selection, its identity, and the independent report request leave
//! build evaluation together. Empty selections remain ordinary data and do not
//! instantiate optimizer machinery here.

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::pipeline) struct CheckedOptimizationHandoff {
    selections: optimization_core::OptimizationSelections,
    selection_identity: optimization_core::OptimizationSelectionIdentity,
    report: optimization_core::OptimizationReportRequest,
}

impl CheckedOptimizationHandoff {
    pub(in crate::pipeline) fn retain(
        selections: optimization_core::OptimizationSelections,
        report: optimization_core::OptimizationReportRequest,
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
    ) -> &optimization_core::OptimizationSelections {
        &self.selections
    }

    pub(in crate::pipeline) const fn selection_identity(
        &self,
    ) -> optimization_core::OptimizationSelectionIdentity {
        self.selection_identity
    }

    pub(in crate::pipeline) const fn report(&self) -> optimization_core::OptimizationReportRequest {
        self.report
    }
}

#[cfg(test)]
mod tests {
    use super::CheckedOptimizationHandoff;

    #[test]
    fn empty_selection_is_retained_without_enabling_optimizer_work() {
        let selections = optimization_core::OptimizationSelections::default();
        let handoff = CheckedOptimizationHandoff::retain(
            selections.clone(),
            optimization_core::OptimizationReportRequest::Suppressed,
        );

        assert!(handoff.selections().is_empty());
        assert_eq!(handoff.selections(), &selections);
        assert_eq!(handoff.selection_identity(), selections.identity());
        assert_eq!(
            handoff.report(),
            optimization_core::OptimizationReportRequest::Suppressed
        );
    }
}

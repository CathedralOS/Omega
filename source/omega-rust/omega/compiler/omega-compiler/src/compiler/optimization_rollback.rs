use omega_optimization_core::{DuplicateOptimization, Optimization, OptimizationSelections};

use super::OptimizationRollbackReceipt;

/// A release-tooling overlay that can only subtract exact rules selected by
/// `build.omg`. It cannot add, alias, or reorder an optimization.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OptimizationRollback {
    requested_disabled: OptimizationSelections,
}

/// Complete native-realization view of one release rollback request.
///
/// The effective selection and optional report receipt are constructed
/// together so callers cannot reimplement the empty-request identity case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OptimizationRollbackSettlement {
    effective: OptimizationSelections,
    receipt: Option<OptimizationRollbackReceipt>,
}

impl OptimizationRollbackSettlement {
    pub const fn effective(&self) -> &OptimizationSelections {
        &self.effective
    }

    pub fn into_receipt(self) -> Option<OptimizationRollbackReceipt> {
        self.receipt
    }
}

impl OptimizationRollback {
    pub fn new(
        requested_disabled: impl IntoIterator<Item = Optimization>,
    ) -> Result<Self, OptimizationRollbackInputError> {
        OptimizationSelections::new(requested_disabled)
            .map(|requested_disabled| Self { requested_disabled })
            .map_err(|DuplicateOptimization(optimization)| {
                OptimizationRollbackInputError::DuplicateRule(optimization)
            })
    }

    pub fn from_exact_names<'name>(
        names: impl IntoIterator<Item = &'name str>,
    ) -> Result<Self, OptimizationRollbackInputError> {
        let requested_disabled = names
            .into_iter()
            .map(|name| {
                Optimization::from_build_case_name(name)
                    .ok_or_else(|| OptimizationRollbackInputError::UnknownName(name.to_owned()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Self::new(requested_disabled)
    }

    pub fn is_empty(&self) -> bool {
        self.requested_disabled.is_empty()
    }

    pub const fn requested_disabled(&self) -> &OptimizationSelections {
        &self.requested_disabled
    }

    /// Reconstruct the optional public report receipt for compatibility with
    /// callers that inspect rollback evidence independently. Production native
    /// realization consumes [`Self::settle`] so effective selection cannot
    /// detach from this receipt.
    pub fn reconcile(
        &self,
        build_selected: &OptimizationSelections,
    ) -> Option<OptimizationRollbackReceipt> {
        (!self.is_empty()).then(|| {
            OptimizationRollbackReceipt::new(
                build_selected.clone(),
                self.requested_disabled.clone(),
            )
        })
    }

    pub(crate) fn settle(
        &self,
        build_selected: &OptimizationSelections,
    ) -> OptimizationRollbackSettlement {
        let receipt = self.reconcile(build_selected);
        let effective = receipt.as_ref().map_or_else(
            || build_selected.clone(),
            |receipt| receipt.effective().clone(),
        );
        OptimizationRollbackSettlement { effective, receipt }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptimizationRollbackInputError {
    UnknownName(String),
    DuplicateRule(Optimization),
}

impl std::fmt::Display for OptimizationRollbackInputError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownName(name) => write!(
                formatter,
                "unknown exact optimization rollback name `{name}`"
            ),
            Self::DuplicateRule(optimization) => write!(
                formatter,
                "optimization rollback repeats `{}`",
                optimization.build_case_name()
            ),
        }
    }
}

impl std::error::Error for OptimizationRollbackInputError {}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_optimization_core::OptimizationExecutionPhase;

    const EXECUTION_PHASES: [OptimizationExecutionPhase; 5] = [
        OptimizationExecutionPhase::Psi,
        OptimizationExecutionPhase::SelectedLowering,
        OptimizationExecutionPhase::AllocationRecovery,
        OptimizationExecutionPhase::PostAllocationMachine,
        OptimizationExecutionPhase::FunctionRelativeLayout,
    ];

    #[test]
    fn empty_settlement_preserves_build_selection_without_a_receipt() {
        let selected = OptimizationSelections::new([
            Optimization::ControlFlowCleanup,
            Optimization::CopyPropagation,
        ])
        .expect("canonical build selection");
        let settlement = OptimizationRollback::default().settle(&selected);

        assert_eq!(settlement.effective(), &selected);
        assert_eq!(settlement.into_receipt(), None);
    }

    #[test]
    fn nonempty_settlement_keeps_effective_selection_and_receipt_coherent() {
        let selected = OptimizationSelections::new([
            Optimization::ControlFlowCleanup,
            Optimization::CopyPropagation,
        ])
        .expect("canonical build selection");
        let rollback = OptimizationRollback::new([Optimization::CopyPropagation])
            .expect("canonical rollback selection");
        let settlement = rollback.settle(&selected);
        let expected = OptimizationSelections::new([Optimization::ControlFlowCleanup])
            .expect("canonical effective selection");

        assert_eq!(settlement.effective(), &expected);
        let receipt = settlement
            .into_receipt()
            .expect("a nonempty request retains one report receipt");
        assert_eq!(receipt.build_selected(), &selected);
        assert_eq!(receipt.effective(), &expected);
        assert!(receipt.is_consistent());
    }

    #[test]
    fn every_exact_rule_is_subtractive_phase_local_and_idempotent() {
        let all = OptimizationSelections::new(Optimization::ALL)
            .expect("the closed optimization vocabulary is duplicate-free");

        for disabled in Optimization::ALL {
            let rollback = OptimizationRollback::from_exact_names([disabled.build_case_name()])
                .expect("every build case name is an exact rollback name");
            let receipt = rollback
                .reconcile(&all)
                .expect("a nonempty rollback request leaves custody");
            let expected_effective = OptimizationSelections::new(
                Optimization::ALL
                    .into_iter()
                    .filter(|optimization| *optimization != disabled),
            )
            .expect("a vocabulary subset remains duplicate-free");

            assert_eq!(receipt.build_selected(), &all, "{disabled:?}");
            assert_eq!(
                receipt.requested_disabled().as_slice(),
                &[disabled],
                "{disabled:?}"
            );
            assert_eq!(
                receipt.actually_disabled().as_slice(),
                &[disabled],
                "{disabled:?}"
            );
            assert_eq!(receipt.effective(), &expected_effective, "{disabled:?}");
            assert!(receipt.is_consistent(), "{disabled:?}");

            for phase in EXECUTION_PHASES {
                let expected_phase = OptimizationSelections::new(
                    Optimization::ALL.into_iter().filter(|optimization| {
                        *optimization != disabled && optimization.execution_phase() == phase
                    }),
                )
                .expect("a phase vocabulary subset remains duplicate-free");
                let effective_phase = receipt.effective().for_phase(phase);
                assert_eq!(effective_phase, expected_phase, "{disabled:?} in {phase:?}");
                assert!(
                    !effective_phase.contains(disabled),
                    "{disabled:?} leaked into {phase:?}"
                );
            }

            let repeated = rollback
                .reconcile(receipt.effective())
                .expect("the authored rollback request remains visible");
            assert!(repeated.actually_disabled().is_empty(), "{disabled:?}");
            assert_eq!(repeated.effective(), receipt.effective(), "{disabled:?}");
        }
    }

    #[test]
    fn exact_names_are_canonical_subtractive_and_idempotent_for_absent_rules() {
        let rollback = OptimizationRollback::from_exact_names([
            "X86SelectXorZeroI64MaterializationV1",
            "CopyPropagation",
        ])
        .unwrap();
        let selected = OptimizationSelections::new([
            Optimization::ControlFlowCleanup,
            Optimization::CopyPropagation,
        ])
        .unwrap();
        let receipt = rollback.reconcile(&selected).unwrap();
        assert_eq!(
            receipt.requested_disabled().as_slice(),
            &[
                Optimization::CopyPropagation,
                Optimization::X86SelectXorZeroI64MaterializationV1,
            ]
        );
        assert_eq!(
            receipt.actually_disabled().as_slice(),
            &[Optimization::CopyPropagation]
        );
        assert_eq!(
            receipt.effective().as_slice(),
            &[Optimization::ControlFlowCleanup]
        );
    }

    #[test]
    fn unknown_and_duplicate_names_fail_closed() {
        assert_eq!(
            OptimizationRollback::from_exact_names(["copy_propagation"]),
            Err(OptimizationRollbackInputError::UnknownName(
                "copy_propagation".into()
            ))
        );
        assert_eq!(
            OptimizationRollback::from_exact_names(["CopyPropagation", "CopyPropagation"]),
            Err(OptimizationRollbackInputError::DuplicateRule(
                Optimization::CopyPropagation
            ))
        );
    }
}

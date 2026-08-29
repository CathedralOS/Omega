use omega_optimization_core::{DuplicateOptimization, Optimization, OptimizationSelections};

use super::OptimizationRollbackReceipt;

/// A release-tooling overlay that can only subtract exact rules selected by
/// `build.omg`. It cannot add, alias, or reorder an optimization.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OptimizationRollback {
    requested_disabled: OptimizationSelections,
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

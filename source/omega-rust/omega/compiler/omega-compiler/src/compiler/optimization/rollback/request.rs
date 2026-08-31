use omega_optimization_core::{DuplicateOptimization, Optimization, OptimizationSelections};

use super::OptimizationRollback;

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

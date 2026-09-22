use super::{
    Optimization, OptimizationExecutionPhase, OptimizationSelectionIdentity, OptimizationSelections,
};
use std::fmt;

/// Canonical selection vocabulary accepted after sealed Terminal Psi.
///
/// Earlier-phase names are unrepresentable at the native-lowering API rather
/// than accepted through the global build vocabulary and rejected later.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PostTerminalOptimizationSelections {
    pub(super) selected: OptimizationSelections,
}

/// Omega-owned projection containing only phases that execute after Terminal
/// Psi has been published.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostTerminalOptimizationSelectionProjection {
    pub(super) complete_selection: OptimizationSelectionIdentity,
    pub(super) selected: PostTerminalOptimizationSelections,
}

impl PostTerminalOptimizationSelectionProjection {
    pub const fn complete_selection(&self) -> OptimizationSelectionIdentity {
        self.complete_selection
    }

    pub const fn selections(&self) -> &PostTerminalOptimizationSelections {
        &self.selected
    }
}

impl PostTerminalOptimizationSelections {
    pub fn new(selected: OptimizationSelections) -> Result<Self, PreTerminalOptimizationSelection> {
        if let Some(optimization) = selected.as_slice().iter().copied().find(|optimization| {
            matches!(
                optimization.execution_phase(),
                OptimizationExecutionPhase::CheckedTrees | OptimizationExecutionPhase::Psi
            )
        }) {
            return Err(PreTerminalOptimizationSelection(optimization));
        }
        Ok(Self { selected })
    }

    pub const fn selections(&self) -> &OptimizationSelections {
        &self.selected
    }

    pub fn as_slice(&self) -> &[Optimization] {
        self.selected.as_slice()
    }

    pub fn is_empty(&self) -> bool {
        self.selected.is_empty()
    }

    pub fn identity(&self) -> OptimizationSelectionIdentity {
        self.selected.identity()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PreTerminalOptimizationSelection(pub Optimization);

impl fmt::Display for PreTerminalOptimizationSelection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "pre-Terminal optimization `{}` is not a post-Terminal selection",
            self.0.build_case_name()
        )
    }
}

impl std::error::Error for PreTerminalOptimizationSelection {}

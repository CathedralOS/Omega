// Post-Terminal optimization selection vocabulary (`PostTerminalOptimizationSelections`)
// and projection (`PostTerminalOptimizationSelectionProjection`).
//
// Sealed Terminal Psi artifacts have already executed `Psi` optimizations
// prior to publication, so post-Terminal lowering accepts only
// phases where `is_post_terminal()` holds while retaining `complete_selection`
// and `complete_selections` for compiler-private custody verification.

use super::{Optimization, OptimizationSelectionIdentity, OptimizationSelections};
use std::fmt;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PostTerminalOptimizationSelections {
    pub(super) selected: OptimizationSelections, // Canonical subset restricted to post-Terminal phases.
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PostTerminalOptimizationSelectionProjection {
    pub(super) complete_selection: OptimizationSelectionIdentity, // Digest of the full authored selection.
    pub(super) complete_selections: OptimizationSelections, // Full authored selection retained for custody checks.
    pub(super) selected: PostTerminalOptimizationSelections, // Post-Terminal phase subset consumed by native lowering.
}

impl PostTerminalOptimizationSelectionProjection {
    pub const fn complete_selections(&self) -> &OptimizationSelections {
        &self.complete_selections
    }

    pub const fn complete_selection(&self) -> OptimizationSelectionIdentity {
        self.complete_selection
    }

    pub const fn selections(&self) -> &PostTerminalOptimizationSelections {
        &self.selected
    }
}

impl PostTerminalOptimizationSelections {
    pub fn new(selected: OptimizationSelections) -> Result<Self, PreTerminalOptimizationSelection> {
        if let Some(optimization) = selected
            .as_slice()
            .iter()
            .copied()
            .find(|optimization| !optimization.execution_phase().is_post_terminal())
        {
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

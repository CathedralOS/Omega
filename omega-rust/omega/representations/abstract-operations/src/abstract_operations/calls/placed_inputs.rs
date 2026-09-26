//! Plan-laid inputs retained separately from owned structural values.

use crate::AbstractOperationPlan;
use terminal_psi::TerminalPlacedViewInput;

/// Exact semantic custody for plan-laid inputs retained beside ordinary
/// abstract operations.
///
/// A `Placed<P, T>` input is deliberately not a structural parameter: its
/// pointer-shaped runtime representation does not grant ownership of `T`, and
/// its sealed placement commitment is not a substitute for installed backing.
/// Keeping this carrier separate prevents ordinary lowering consumers from
/// silently interpreting the view as structural authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AbstractOperationPlanWithPlacedViewInputs {
    pub plan: AbstractOperationPlan,
    pub placed_view_inputs: Vec<TerminalPlacedViewInput>,
}

//! Authored successor operands and ownership cleanup at edges and returns.
//!
//! Cleanup owner rosters use the shared ownership-facts query: a root can die
//! on several mutually exclusive exits without being disposed twice. Edge
//! bindings and return transfers select the actual residual cleanup below.
use super::{CheckedComposedUnitControlStatePlan, CheckedStructuralControlSuccessorPlan};

pub(in crate::unit::attached_unit::composed_control) fn successors(
    state: &CheckedComposedUnitControlStatePlan,
) -> Vec<&CheckedStructuralControlSuccessorPlan> {
    state.successors()
}

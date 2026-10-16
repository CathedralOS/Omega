//! Exact legality changes, retained as spans rather than per-phase copies.
//!
//! Physical unit fragments/actions and this value's fixed constraints are the
//! only point-dependent inputs. Between their boundaries the candidate domain
//! is constant. Derive once there; keep source-fragment boundaries so consumers
//! can authenticate topology without reconstructing the expanded point stream.

use register_model::{RegisterClass, RegisterViewId};
use selected_instructions::SelectedBlockId;

use super::{fixed_views, view_candidates::CandidateViews};
use crate::AllocationLegalityError;
use register_homes::VirtualLegalitySpan;
use selected_instructions::{LiveRangePoint, VirtualFixedConstraintSite, VirtualLiveRange};

#[allow(clippy::too_many_arguments)]
pub(super) fn compute(
    function_index: usize,
    register: &VirtualLiveRange,
    class: &RegisterClass,
    available: &[RegisterViewId],
    entry_point: Option<(SelectedBlockId, LiveRangePoint)>,
    views: &mut CandidateViews<'_>,
) -> Result<Vec<VirtualLegalitySpan>, AllocationLegalityError> {
    // Architectural boundaries are already sorted for the immutable function.
    // Merge them by cursor with this register's fixed boundaries below; copying
    // and sorting the complete function list for every register repeats setup
    // even when a register touches only one small fragment.
    let mut boundaries = Vec::new();
    for constraint in &register.fixed_constraints {
        let point = match constraint.site {
            VirtualFixedConstraintSite::Entry => entry_point.map(|(_, point)| point),
            VirtualFixedConstraintSite::Operand { point, .. } => Some(point),
        };
        if let Some(point) = point {
            boundaries.push(point);
            if let Some(end) = point.0.checked_add(1) {
                boundaries.push(LiveRangePoint(end));
            }
        }
    }
    boundaries.sort_unstable();
    boundaries.dedup();
    let mut spans: Vec<VirtualLegalitySpan> = Vec::new();
    for fragment in &register.fragments {
        if fragment.start >= fragment.end {
            continue;
        }
        let fragment_offset = spans.len();
        let mut point = fragment.start;
        let mut architectural_index = views
            .boundaries()
            .partition_point(|boundary| *boundary <= point);
        let mut fixed_index = boundaries.partition_point(|boundary| *boundary <= point);
        while point < fragment.end {
            let architectural = views.boundaries().get(architectural_index).copied();
            let fixed = boundaries.get(fixed_index).copied();
            let end = architectural
                .into_iter()
                .chain(fixed)
                .min()
                .unwrap_or(fragment.end)
                .min(fragment.end);
            if architectural == Some(end) {
                architectural_index += 1;
            }
            if fixed == Some(end) {
                fixed_index += 1;
            }
            let fixed = fixed_views::at_live_point(
                function_index,
                register,
                fragment.block,
                point,
                entry_point,
            )?;
            let mut candidates = views.unconstrained(class, available, fragment.block, point);
            views.restrict_to_fixed(
                function_index,
                register,
                fragment.block,
                point,
                fixed,
                &mut candidates,
            )?;
            if candidates.is_empty() {
                return Err(AllocationLegalityError::NoCandidateViews {
                    function: function_index,
                    register: register.virtual_register.0,
                    block: fragment.block.0,
                    point: point.0,
                });
            }
            if spans.len() > fragment_offset
                && spans
                    .last()
                    .is_some_and(|span| span.candidates == candidates)
            {
                spans.last_mut().expect("current fragment has a span").end = end;
            } else {
                spans.push(VirtualLegalitySpan {
                    block: fragment.block,
                    start: point,
                    end,
                    candidates,
                });
            }
            point = end;
        }
    }
    Ok(spans)
}

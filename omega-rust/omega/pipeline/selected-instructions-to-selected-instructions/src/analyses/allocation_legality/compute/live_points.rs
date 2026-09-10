//! Candidate derivation at ordinary live-range points.

use register_model::{RegisterClass, RegisterViewId};
use selected_instructions::SelectedBlockId;

use super::{fixed_views, view_candidates::CandidateViews};
use crate::{AllocationLegalityError, LiveRangePoint, VirtualLiveRange, VirtualPointLegality};

#[allow(clippy::too_many_arguments)]
pub(super) fn compute(
    function_index: usize,
    register: &VirtualLiveRange,
    class: &RegisterClass,
    available: &[RegisterViewId],
    entry_point: Option<(SelectedBlockId, LiveRangePoint)>,
    views: &mut CandidateViews<'_>,
) -> Result<Vec<VirtualPointLegality>, AllocationLegalityError> {
    let mut points = Vec::new();
    for fragment in &register.fragments {
        for raw_point in fragment.start.0..fragment.end.0 {
            let point = LiveRangePoint(raw_point);
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
            points.push(VirtualPointLegality {
                block: fragment.block,
                point,
                candidates,
            });
        }
    }
    Ok(points)
}

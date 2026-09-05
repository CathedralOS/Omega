use omega_optimization_core::ExternalDecisionLog;

use super::ExternalPolicySurfaceMismatch;

/// Independently require the response to preserve every compiler-authored
/// request field. Strict schema decoding already proves that each changed
/// action belongs to its point's finite legal action set.
pub(super) fn require_action_only_change(
    request: &ExternalDecisionLog,
    response: &ExternalDecisionLog,
) -> Result<(), ExternalPolicySurfaceMismatch> {
    if request.context() != response.context() {
        return Err(ExternalPolicySurfaceMismatch::Context);
    }
    if request.points().len() != response.points().len() {
        return Err(ExternalPolicySurfaceMismatch::PointCount);
    }
    for (ordinal, (request, response)) in request.points().iter().zip(response.points()).enumerate()
    {
        if request.input() != response.input() {
            return Err(ExternalPolicySurfaceMismatch::PointInput { ordinal });
        }
        if request.rule() != response.rule() {
            return Err(ExternalPolicySurfaceMismatch::PointRule { ordinal });
        }
        if request.legal_candidates() != response.legal_candidates() {
            return Err(ExternalPolicySurfaceMismatch::CandidateSurface { ordinal });
        }
    }
    Ok(())
}

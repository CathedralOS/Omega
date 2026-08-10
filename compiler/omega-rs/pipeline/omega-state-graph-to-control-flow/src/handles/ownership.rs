use omega_control_flow::StatePermissionEvent;
use psi_arena::HandleSpan;

use super::remap_span;

pub(crate) fn remap_permission_event_span(
    permissions: HandleSpan<omega_state_graph::StatePermissionEvent>,
) -> HandleSpan<StatePermissionEvent> {
    remap_span(permissions)
}

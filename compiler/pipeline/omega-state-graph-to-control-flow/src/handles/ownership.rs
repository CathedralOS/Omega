use omega_control_flow::{StateDropEvent, StateMoveEvent};
use omega_core::arena::HandleSpan;

use super::remap_span;

pub(crate) fn remap_move_event_span(
    moves: HandleSpan<omega_state_graph::StateMoveEvent>,
) -> HandleSpan<StateMoveEvent> {
    remap_span(moves)
}

pub(crate) fn remap_drop_event_span(
    drops: HandleSpan<omega_state_graph::StateDropEvent>,
) -> HandleSpan<StateDropEvent> {
    remap_span(drops)
}

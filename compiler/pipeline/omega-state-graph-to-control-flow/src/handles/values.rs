use omega_control_flow::StateValueFact;
use omega_core::arena::HandleSpan;

use super::remap_span;

pub(crate) fn remap_value_span(
    values: HandleSpan<omega_state_graph::StateValueFact>,
) -> HandleSpan<StateValueFact> {
    remap_span(values)
}

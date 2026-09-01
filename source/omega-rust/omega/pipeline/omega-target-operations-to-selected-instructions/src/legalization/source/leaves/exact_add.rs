//! Exact-add leaf routing from most specific retained graph to direct binary form.

use super::{
    DerivedValue, LeafContext, active_resident_exact_add_bridge_chain,
    active_resident_exact_add_chain, direct_exact_binary,
};
use crate::legalization::source::shared::*;

pub(in crate::legalization::source) use active_resident_exact_add_bridge_chain::is_active_resident_exact_add_bridge_chain;
pub(in crate::legalization::source) use active_resident_exact_add_chain::is_active_resident_exact_add_chain;

pub(super) fn derive<'a>(
    context: &LeafContext<'a>,
    expression: &TargetIntegerExpression,
) -> Result<DerivedValue<'a>, LegalizationError> {
    if is_active_resident_exact_add_bridge_chain(expression) {
        active_resident_exact_add_bridge_chain::derive(context, expression)
    } else if is_active_resident_exact_add_chain(expression) {
        active_resident_exact_add_chain::derive(context, expression)
    } else {
        direct_exact_binary::derive_add(context, expression)
    }
}

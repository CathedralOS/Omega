//! Resolve the installed address with the same producer used by admission.

use super::{SelectedInstructionError, failure};
use calling_conventions::InterruptGateAddress;
use layout_plans::LayoutPlacementReport;

pub(super) fn prepare(
    placements: &[LayoutPlacementReport],
    table_bytes: u32,
) -> Result<InterruptGateAddress, SelectedInstructionError> {
    calling_conventions::interrupt_gate_address(placements, table_bytes).map_err(failure)
}

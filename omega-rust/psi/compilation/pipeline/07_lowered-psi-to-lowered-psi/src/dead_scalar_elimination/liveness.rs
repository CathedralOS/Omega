//! Backward scalar demand is shared with generated block transport.
//! The optional optimizer alone removes unused total scalar operations.

use semantic_vocabulary::ValueId;
use terminal_psi::TerminalMachine;
use terminal_semantics::scalar_demand::{ScalarDemandMode, complete_scalar_demand};

pub(super) fn eliminate(
    machine: &mut TerminalMachine,
    retained_values: &[ValueId],
    protected_blocks: impl IntoIterator<Item = semantic_vocabulary::BlockId>,
) {
    complete_scalar_demand(
        machine,
        retained_values.iter().copied(),
        protected_blocks,
        ScalarDemandMode::DropUnusedTotalOperations,
    );
}

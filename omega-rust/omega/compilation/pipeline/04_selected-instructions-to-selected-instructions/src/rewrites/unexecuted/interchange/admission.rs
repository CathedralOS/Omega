//! Shared admission for the in-block commuting run interchange: locate the
//! four named members bounding two disjoint runs inside one block's body —
//! the earlier run's first and last and the later run's first and last —
//! and prove the window they bound independent: no register or
//! condition-state hazard between any member and any instruction outside
//! its own run inside the window, every roster row that newly trades order
//! commuting with every row of the position it crosses, no call,
//! hosted-effect, or terminator barrier anywhere in the window, and no
//! boundary settlement inside its span.
use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::SelectedInstructionId;

use super::InterchangeError;
use crate::ValidatedSelectedAnalysis;

pub(super) struct Admission {
    pub block_index: usize,
    /// The earlier run's first member index in the block body.
    pub earlier_first_index: usize,
    /// The earlier run's last member index; the earlier run is the
    /// contiguous span `earlier_first_index..=earlier_last_index` of at
    /// least two members.
    pub earlier_last_index: usize,
    /// The later run's first member index, strictly past the earlier run;
    /// the positions between the runs form the interior, possibly empty.
    pub later_first_index: usize,
    /// The later run's last member index; the window the interchange
    /// crosses is `earlier_first_index..=later_last_index`.
    pub later_last_index: usize,
}

pub(super) fn admit(
    source: &impl ValidatedSelectedAnalysis,
    function_index: usize,
    earlier_first: SelectedInstructionId,
    earlier_last: SelectedInstructionId,
    later_first: SelectedInstructionId,
    later_last: SelectedInstructionId,
    environment: &ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Admission, InterchangeError> {
    let derived = super::derive_interchange(
        source,
        function_index,
        earlier_first,
        earlier_last,
        later_first,
        later_last,
        environment,
        budget,
    )?;
    Ok(Admission {
        block_index: derived.block_index,
        earlier_first_index: derived.earlier_first_index,
        earlier_last_index: derived.earlier_last_index,
        later_first_index: derived.later_first_index,
        later_last_index: derived.later_last_index,
    })
}

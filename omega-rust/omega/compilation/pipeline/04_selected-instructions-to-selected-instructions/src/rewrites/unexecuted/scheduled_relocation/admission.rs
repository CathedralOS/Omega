//! The path-derived relocation window, now through the shared derivation
//! and audit: `crossed_window` enumerates every acyclic path between the
//! run's block and the destination's — in the direction this family's
//! own dominance bounds established — and returns the crossed positions
//! and crossed edges; `admit_run_relocation` proves that window
//! independent once. What stays local is what makes this family the
//! general admission: the sink/hoist once-per-traversal bounds
//! (dominance, re-entry, continuations), the member's pure-work bound,
//! and the skipped-edge dead-path audit a sink owes.

use optimization_core::OptimizationWorkBudget;
use register_environment::ValidatedTargetRegisterEnvironment;
use selected_instructions::SelectedInstructionId;

use super::{Reconstructed, ScheduledRelocationError, reconstruct};
use crate::ValidatedSelectedAnalysis;

/// The admitted run relocation: where the run leaves and where it lands.
pub(super) struct Admission {
    /// The block the contiguous run vacates.
    pub(super) block_index: usize,
    /// The first body index the run occupies.
    pub(super) member_first: usize,
    /// The last body index the run occupies — `member_first` for a
    /// one-member run.
    pub(super) member_last: usize,
    /// The block the run lands in.
    pub(super) target_index: usize,
    /// The body index the run's first member takes; naming the
    /// destination's terminator-carried instruction lands the run at the
    /// body end.
    pub(super) landing_index: usize,
}

/// Admit `members` — a contiguous run of body instructions named in their
/// own order — to leave their block and land on `destination`'s position
/// in a block their block dominates (a sink), or land on a position in a
/// block dominating theirs whose continuations all reach theirs (a
/// hoist), when the shared audits clear the derived window: every path
/// between them simple and through plain `Source` blocks, every crossed
/// edge plain and free of member interference, every crossed position
/// schedulable and uncoupled, no boundary settlement observing a changed
/// executed prefix, and the measured audit within budget.
#[allow(clippy::too_many_arguments)]
pub(super) fn admit<'source>(
    source: &'source impl ValidatedSelectedAnalysis,
    function_index: usize,
    members: &[SelectedInstructionId],
    destination: SelectedInstructionId,
    environment: &'source ValidatedTargetRegisterEnvironment,
    budget: OptimizationWorkBudget,
) -> Result<Admission, ScheduledRelocationError> {
    let reconstructed: Reconstructed = reconstruct(
        source,
        function_index,
        members,
        destination,
        environment,
        budget,
    )?;
    Ok(Admission {
        block_index: reconstructed.block_index,
        member_first: reconstructed.member_first,
        member_last: reconstructed.member_last,
        target_index: reconstructed.target_index,
        landing_index: reconstructed.landing_index,
    })
}

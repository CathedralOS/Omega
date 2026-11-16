//! The one place image emission reads stage 02's working program.
//!
//! Ordinary emission builds the object from the prepared execution plan and
//! the stages after it. The replay validators (`verify_retained_target_operations`
//! and the object, composition and image replays) also check the emitted
//! object against what stage 02 lowered, so they read the working program
//! through this function. `mbx xtask check-boundary` restricts the stage-02
//! accessor to this file and rejects production references to this module or
//! the retained-source validators, including imports under another name.

use object_file::StagedOptimizedRelocationFreeObjectContainer;
use target_operations::TargetOperationPlan;

pub(crate) fn target_operations(
    source: &StagedOptimizedRelocationFreeObjectContainer,
) -> &TargetOperationPlan {
    source
        .source()
        .source()
        .source()
        .source()
        .optimized_target()
        .working_program_for_validation()
}

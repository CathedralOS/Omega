//! The one place image emission reads stage 02's working program.
//!
//! Ordinary emission builds the object from the current program and the
//! facts the native driver carried. The replay validators
//! (`verify_retained_target_operations` and the object, composition and image
//! replays) also check the emitted object against what stage 02 lowered, so
//! they read the working program through this function. It exists only when
//! the compile retained stage 02's output for an explicit audit
//! (`RetainedTargetOperations`). `mbx xtask check-boundary` restricts the
//! stage-02 accessor to this file and rejects production references to this
//! module or the retained-source validators, including imports under another
//! name.

use target_operations::TargetOperationPlan;

pub(crate) fn target_operations(
    original: &crate::RetainedTargetOperations,
) -> &TargetOperationPlan {
    original.target().working_program_for_validation()
}

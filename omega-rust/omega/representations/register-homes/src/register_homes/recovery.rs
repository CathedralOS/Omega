//! Pressure-victim and recovery-classification data; admission belongs to transforms.

pub mod classification;
pub mod fixed_view_copy;
pub mod spill_choice;

pub use classification::{
    FunctionRecoveryClassification, NoAdmittedRecoveryReason, PressureRecoveryClassification,
    RecoveryClassification, RecoveryClassificationDecodeError, RecoveryClassificationIdentity,
    RecoveryClassificationPlan, RecoveryClassificationPolicy, RecoveryFutureUse,
    RecoveryVictimRole, recovery_classification_identity,
};
pub use fixed_view_copy::{
    FixedViewCopy, FixedViewCopyDecodeError, FixedViewCopyDestination, FixedViewCopyPlan,
    FixedViewCopyPolicy, FixedViewCopySourceEvidence, fixed_view_copy_identity,
};
pub use spill_choice::{
    FunctionSpillChoices, PressureContender, PressureResident, SpillChoice, SpillChoiceDecodeError,
    SpillChoiceIdentity, SpillChoicePlan, SpillChoicePolicy, spill_choice_identity,
};

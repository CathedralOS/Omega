//! Pressure-victim and recovery-classification data; admission belongs to transforms.

pub mod classification;
pub mod fixed_view_copy;
pub mod spill_choice;

pub use classification::{
    FunctionRecoveryClassification, NoAdmittedRecoveryReason, PressureRecoveryClassification,
    RecoveryClassification, RecoveryClassificationIdentity,
    RecoveryClassificationPlan, RecoveryClassificationPolicy, RecoveryFutureUse,
    RecoveryVictimRole, recovery_classification_identity,
};
pub(crate) use classification::{ RecoveryClassificationDecodeError};
pub use fixed_view_copy::{
    FixedViewCopy, FixedViewCopyDestination, FixedViewCopyPlan,
    FixedViewCopyPolicy, FixedViewCopySourceEvidence, fixed_view_copy_identity,
};
pub(crate) use fixed_view_copy::{ FixedViewCopyDecodeError};
pub use spill_choice::{
    FunctionSpillChoices, PressureContender, PressureResident, SpillChoice,
    SpillChoiceIdentity, SpillChoicePlan, SpillChoicePolicy, spill_choice_identity,
};
pub(crate) use spill_choice::{ SpillChoiceDecodeError};

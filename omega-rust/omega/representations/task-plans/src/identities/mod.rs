//! Normalized identities: every nonzero `u64` coordinate a plan, runtime,
//! activation, storage lease or lifecycle claim is known by.

use crate::TaskPlanDiagnostic;

macro_rules! normalized_id {
    ($name:ident, $label:literal) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(pub(crate) u64);

        impl $name {
            pub fn from_normalized_identity(identity: u64) -> Result<Self, TaskPlanDiagnostic> {
                if identity == 0 {
                    return Err(TaskPlanDiagnostic(format!(
                        "normalized {} identity cannot be zero",
                        $label
                    )));
                }
                Ok(Self(identity))
            }

            pub const fn normalized_identity(self) -> u64 {
                self.0
            }
        }
    };
}

normalized_id!(MachineContractId, "machine-contract");
normalized_id!(MachineEntryId, "machine-entry");
normalized_id!(ValueLayoutId, "value-layout");
normalized_id!(CallingPlanId, "calling-plan");
normalized_id!(StackRepresentationId, "stack-representation");
normalized_id!(TaskStackFrameId, "task-stack-frame");
normalized_id!(TaskStackFrameValidationId, "task-stack-frame-validation");
normalized_id!(
    AdmittedStackContributionReportId,
    "admitted-stack-contribution-report"
);
normalized_id!(
    SameStackContributionAdmissionReceiptId,
    "same-stack-contribution-admission-receipt"
);
normalized_id!(TaskStackCompositionId, "task-stack-composition");
normalized_id!(StackPlanProjectionId, "stack-plan-projection");
normalized_id!(TaskRuntimeId, "task-runtime");
normalized_id!(TaskRuntimeInstanceId, "task-runtime-instance");
normalized_id!(TaskRuntimeInvocationId, "task-runtime-invocation");
normalized_id!(
    TaskRuntimeInvocationReceiptId,
    "task-runtime-invocation-receipt"
);
normalized_id!(
    TaskRuntimeInvocationBindingId,
    "task-runtime-invocation-binding"
);
normalized_id!(ActivationPlanId, "activation-plan");
normalized_id!(
    ExecutorPreservationEvidenceId,
    "executor-preservation-evidence"
);
normalized_id!(ExecutorSelectionId, "executor-selection");
normalized_id!(ActivationInstanceId, "activation-instance");
normalized_id!(TaskStorageOwnerId, "task-storage-owner");
normalized_id!(TaskStorageLeaseId, "task-storage-lease");
normalized_id!(TaskArgumentCustodyId, "task-argument-custody");
normalized_id!(TaskLifecycleClaimId, "task-lifecycle-claim");

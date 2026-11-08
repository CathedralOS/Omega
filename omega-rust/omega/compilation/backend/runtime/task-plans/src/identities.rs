/*
We define the non-zero 64-bit report coordinates (`normalized_id!`) and
domain-separated 32-byte SHA-256 commitment wrappers (`commitment_digest!`)
shared across task activation plans, WCSU stack composition, and runtime
selection.
*/

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

macro_rules! commitment_digest {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name([u8; 32]);

        impl $name {
            pub const fn from_digest(digest: [u8; 32]) -> Self {
                Self(digest)
            }

            pub const fn as_bytes(self) -> [u8; 32] {
                self.0
            }

            pub fn is_zero(self) -> bool {
                self.0 == [0; 32]
            }
        }
    };
}

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
normalized_id!(LiveCarryPlaceId, "live-carry-place");
normalized_id!(LiveCarryTypeId, "live-carry-type");
normalized_id!(CompositionModelId, "composition-model");

pub(crate) use commitment_digest;

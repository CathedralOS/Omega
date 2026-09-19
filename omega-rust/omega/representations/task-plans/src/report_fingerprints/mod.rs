//! Compact, non-authoritative report fingerprints over validated plans,
//! selections, claims and receipts.

use crate::stack_composition::WcsuStackPlanProjection;
use crate::{
    ActivationInstanceId, ActivationPlanCandidate, ExecutorPreservationAxis,
    ExecutorSelectionCandidate, MovedTaskArguments, TaskActivationPlanFact,
    TaskRuntimeInvocationReceiptCandidate, TaskStartOperation, TaskStorageBinding,
    ValidatedActivationPlan, ValidatedExecutorSelection, ValidatedTaskRuntimeInvocationReceipt,
};

pub(crate) fn activation_plan_report_fingerprint(
    plan: &ActivationPlanCandidate,
    wcsu_stack_projection: Option<&WcsuStackPlanProjection>,
) -> u64 {
    let mut fingerprint = Fingerprint::new();
    fingerprint.word(plan.machine_contract.normalized_identity());
    fingerprint.word(plan.entry.normalized_identity());
    fingerprint.word(plan.argument_layout.identity.normalized_identity());
    fingerprint.word(plan.terminal_outcome_layout.normalized_identity());
    fingerprint.word(plan.calling_plan.normalized_identity());
    fingerprint.word(plan.stack_plan.bytes);
    fingerprint.word(plan.stack_plan.alignment);
    fingerprint.word(plan.stack_plan.representation.normalized_identity());
    if let Some(projection) = wcsu_stack_projection {
        fingerprint.byte(1);
        fingerprint.word(projection.identity().normalized_identity());
    }
    fingerprint.flag(plan.may_suspend);
    fingerprint.flag(plan.may_block);
    fingerprint.word(plan.canonical_suspension_crossings.len() as u64);
    for crossing in &plan.canonical_suspension_crossings {
        fingerprint.word(crossing.identity.get());
        fingerprint.flag(crossing.suspension_allowed);
        fingerprint.flag(crossing.preserve_cpu);
        fingerprint.flag(crossing.preserve_host_thread);
    }
    fingerprint.flag(plan.carry_obligations.preserve_cpu);
    fingerprint.flag(plan.carry_obligations.preserve_host_thread);
    fingerprint.flag(plan.cancellation_required);
    fingerprint.finish()
}

pub(crate) fn executor_selection_report_fingerprint(
    plan: &ValidatedActivationPlan,
    candidate: &ExecutorSelectionCandidate,
) -> u64 {
    let mut fingerprint = Fingerprint::new();
    fingerprint.word(plan.normalized_identity().normalized_identity());
    fingerprint.word(candidate.runtime.normalized_identity());
    fingerprint.word(candidate.runtime_instance.normalized_identity());
    fingerprint.word(candidate.preservation.len() as u64);
    for evidence in &candidate.preservation {
        fingerprint.byte(match evidence.axis() {
            ExecutorPreservationAxis::Cpu => 1,
            ExecutorPreservationAxis::HostThread => 2,
        });
        fingerprint.word(evidence.identity().normalized_identity());
    }
    fingerprint.finish()
}

pub(crate) fn task_claim_report_fingerprint(
    invocation: &ValidatedTaskRuntimeInvocationReceipt,
    activation: ActivationInstanceId,
    arguments: &MovedTaskArguments,
    storage: TaskStorageBinding,
) -> u64 {
    let mut fingerprint = Fingerprint::new();
    fingerprint.word(invocation.identity().normalized_identity());
    fingerprint.word(invocation.candidate().invocation.normalized_identity());
    fingerprint.word(invocation.candidate().receipt.normalized_identity());
    fingerprint.word(activation.normalized_identity());
    fingerprint.word(arguments.custody().normalized_identity());
    // The claim commits to the exact marshalled bytes that crossed the
    // boundary, not only the custody coordinate they were presented under.
    fingerprint.word(arguments.image().len() as u64);
    for byte in arguments.image() {
        fingerprint.byte(*byte);
    }
    match storage {
        TaskStorageBinding::Persistent(provenance) => {
            fingerprint.byte(1);
            fingerprint.word(provenance.owner.normalized_identity());
            fingerprint.word(provenance.lease.normalized_identity());
        }
        TaskStorageBinding::InlineCompletion => fingerprint.byte(2),
    }
    fingerprint.finish()
}

pub(crate) fn runtime_invocation_report_fingerprint(
    activation: &TaskActivationPlanFact,
    candidate: &TaskRuntimeInvocationReceiptCandidate,
    executor_selection: &ValidatedExecutorSelection,
) -> u64 {
    let mut fingerprint = Fingerprint::new();
    fingerprint.word(candidate.receipt.normalized_identity());
    fingerprint.word(candidate.invocation.normalized_identity());
    fingerprint.word(candidate.runtime.normalized_identity());
    fingerprint.word(candidate.runtime_instance.normalized_identity());
    fingerprint.byte(match candidate.operation {
        TaskStartOperation::Start => 1,
        TaskStartOperation::TryStart => 2,
    });
    fingerprint.string(&candidate.provider_plan_name);
    fingerprint.string(&candidate.requirement_identity);
    fingerprint.word(candidate.activation_plan.normalized_identity());
    fingerprint.word(executor_selection.identity().normalized_identity());
    for byte in activation.specialization_commitment.as_bytes() {
        fingerprint.byte(byte);
    }
    fingerprint.finish()
}

struct Fingerprint(u64);

impl Fingerprint {
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;

    const fn new() -> Self {
        Self(Self::OFFSET)
    }

    fn byte(&mut self, byte: u8) {
        self.0 ^= u64::from(byte);
        self.0 = self.0.wrapping_mul(Self::PRIME);
    }

    fn flag(&mut self, value: bool) {
        self.byte(u8::from(value));
    }

    fn word(&mut self, value: u64) {
        for byte in value.to_le_bytes() {
            self.byte(byte);
        }
    }

    fn string(&mut self, value: &str) {
        for byte in value.as_bytes() {
            self.byte(*byte);
        }
        self.byte(0);
    }

    fn finish(self) -> u64 {
        // Normalized IDs reserve zero as the invalid sentinel. FNV-1a
        // reaching zero is extraordinarily unlikely, but the representation
        // must remain total rather than manufacturing an invalid ID.
        if self.0 == 0 { Self::OFFSET } else { self.0 }
    }
}

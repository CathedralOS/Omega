//! Normalized activation demands and task-runtime behavior admission.
//!
//! Suspension safety is a local code/effect check. CPU affinity, host-thread
//! affinity, and address stability are demand/behavior joins selected by the
//! runtime's preemption granularity.

use omega_core::trust::{TrustCommitment, TrustReceipt};
use std::collections::{BTreeMap, BTreeSet};

macro_rules! normalized_id {
    ($name:ident, $label:literal) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(u64);

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
normalized_id!(TaskRuntimeId, "task-runtime");
normalized_id!(TaskRuntimeInstanceId, "task-runtime-instance");
normalized_id!(ActivationPlanId, "activation-plan");
normalized_id!(ActivationAdmissionId, "activation-admission");
normalized_id!(ActivationInstanceId, "activation-instance");
normalized_id!(TaskStorageOwnerId, "task-storage-owner");
normalized_id!(TaskStorageLeaseId, "task-storage-lease");
normalized_id!(TaskLifecycleClaimId, "task-lifecycle-claim");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SameCpuDemand {
    Any,
    Same,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SameThreadDemand {
    Any,
    Same,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddressStabilityDemand {
    Movable,
    Stable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MigrationDemand {
    pub cpu: SameCpuDemand,
    pub thread: SameThreadDemand,
    pub address: AddressStabilityDemand,
}

impl MigrationDemand {
    pub const fn unconstrained() -> Self {
        Self {
            cpu: SameCpuDemand::Any,
            thread: SameThreadDemand::Any,
            address: AddressStabilityDemand::Movable,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistinctActivationRequirement {
    Required,
    InlineCompletionAllowed,
}

/// Provider-independent output of compile-time machine target elaboration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivationPlanCandidate {
    pub machine_contract: MachineContractId,
    pub entry: MachineEntryId,
    pub argument_layout: ValueLayoutId,
    pub terminal_outcome_layout: ValueLayoutId,
    pub calling_plan: CallingPlanId,
    pub continuation_bytes: u64,
    pub continuation_alignment: u64,
    pub may_suspend: bool,
    pub may_block: bool,
    /// Result of local canonical-liveness × carry checking at possible parks.
    pub suspension_crossings_safe: bool,
    pub safe_point_migration: MigrationDemand,
    /// Required because asynchronous preemption makes every instruction a
    /// potential provider crossing. `None` means no such analysis was emitted.
    pub asynchronous_migration: Option<MigrationDemand>,
    pub cancellation_required: bool,
    pub activation: DistinctActivationRequirement,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedActivationPlan(ActivationPlanCandidate);

impl ValidatedActivationPlan {
    pub const fn candidate(&self) -> &ActivationPlanCandidate {
        &self.0
    }

    /// Normalized identity of the complete provider-independent demand. A
    /// provider admission receipt binds to this identity rather than relying
    /// on a caller-selected label or the start specialization alone.
    pub fn normalized_identity(&self) -> ActivationPlanId {
        ActivationPlanId(fingerprint_activation_plan(&self.0))
    }
}

pub fn validate_activation_plan(
    candidate: ActivationPlanCandidate,
) -> Result<ValidatedActivationPlan, TaskPlanDiagnostic> {
    if candidate.continuation_bytes == 0 {
        return Err(TaskPlanDiagnostic(
            "activation continuation size must be nonzero".into(),
        ));
    }
    if candidate.continuation_alignment == 0 || !candidate.continuation_alignment.is_power_of_two()
    {
        return Err(TaskPlanDiagnostic(format!(
            "activation continuation alignment {} is not a nonzero power of two",
            candidate.continuation_alignment
        )));
    }
    if candidate.may_suspend && !candidate.suspension_crossings_safe {
        return Err(TaskPlanDiagnostic(
            "a possible suspension crossing carries a value that forbids suspension".into(),
        ));
    }
    Ok(ValidatedActivationPlan(candidate))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreemptionGranularity {
    SafePoints,
    Asynchronous,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuMigrationBehavior {
    MayMigrate,
    Pinned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreadMigrationBehavior {
    MayMigrate,
    Pinned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContinuationStorageBehavior {
    Movable,
    Stable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InlineCompletionBehavior {
    Never,
    MayCompleteInline,
}

/// Freely constructible behavior claim for one provider plan. This value is
/// not admission: activation planning may consume it only after an exact
/// shared-spine `TrustReceipt` qualifies it as `AdmittedTaskRuntimeContract`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskRuntimeContract {
    pub provider_plan: String,
    pub provider_plan_fingerprint: u64,
    pub max_continuation_bytes: u64,
    pub max_continuation_alignment: u64,
    pub preemption: PreemptionGranularity,
    pub cpu_migration: CpuMigrationBehavior,
    pub thread_migration: ThreadMigrationBehavior,
    pub continuation_storage: ContinuationStorageBehavior,
    pub cancellation: bool,
    pub inline_completion: InlineCompletionBehavior,
}

impl TaskRuntimeContract {
    pub fn pessimistic(provider_plan: impl Into<String>, provider_plan_fingerprint: u64) -> Self {
        Self {
            provider_plan: provider_plan.into(),
            provider_plan_fingerprint,
            max_continuation_bytes: 0,
            max_continuation_alignment: 1,
            preemption: PreemptionGranularity::Asynchronous,
            cpu_migration: CpuMigrationBehavior::MayMigrate,
            thread_migration: ThreadMigrationBehavior::MayMigrate,
            continuation_storage: ContinuationStorageBehavior::Movable,
            cancellation: false,
            inline_completion: InlineCompletionBehavior::MayCompleteInline,
        }
    }

    /// Canonical statement admitted by the provider-plan receipt. The base
    /// plan identity and every behavior promise participate; presentation
    /// name and receipt provenance do not.
    pub fn statement_fingerprint(&self) -> u64 {
        fingerprint_runtime_contract(self)
    }
}

/// Provider behavior accepted through the common grant/receipt spine. The
/// runtime identity is normalizer-owned: it hashes the provider-plan identity
/// and complete behavior promise, never the receipt's provenance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedTaskRuntimeContract {
    runtime: TaskRuntimeId,
    contract: TaskRuntimeContract,
    receipt: TrustReceipt,
}

impl AdmittedTaskRuntimeContract {
    pub const fn runtime(&self) -> TaskRuntimeId {
        self.runtime
    }

    pub const fn contract(&self) -> &TaskRuntimeContract {
        &self.contract
    }

    pub const fn receipt(&self) -> &TrustReceipt {
        &self.receipt
    }
}

/// Qualify a freely authored behavior claim with the exact provider-plan
/// receipt produced by the shared admission pipeline. A package cannot make a
/// narrower runtime promise trusted merely by constructing plan data.
pub fn admit_task_runtime(
    contract: TaskRuntimeContract,
    receipt: TrustReceipt,
) -> Result<AdmittedTaskRuntimeContract, TaskPlanDiagnostic> {
    if contract.provider_plan.is_empty() {
        return Err(TaskPlanDiagnostic(
            "task runtime provider-plan name cannot be empty".into(),
        ));
    }
    if contract.provider_plan_fingerprint == 0 {
        return Err(TaskPlanDiagnostic(
            "task runtime provider-plan fingerprint cannot be zero".into(),
        ));
    }
    let statement_fingerprint = contract.statement_fingerprint();
    if receipt.commitment != TrustCommitment::ProviderPlan(contract.provider_plan.clone())
        || receipt.statement_hash != statement_fingerprint
    {
        return Err(TaskPlanDiagnostic(
            "task runtime receipt does not bind the exact normalized provider plan".into(),
        ));
    }
    if contract.max_continuation_alignment == 0
        || !contract.max_continuation_alignment.is_power_of_two()
    {
        return Err(TaskPlanDiagnostic(format!(
            "task runtime continuation alignment {} is not a nonzero power of two",
            contract.max_continuation_alignment
        )));
    }
    let runtime = TaskRuntimeId(statement_fingerprint);
    Ok(AdmittedTaskRuntimeContract {
        runtime,
        contract,
        receipt,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskRuntimeAdmission {
    identity: ActivationAdmissionId,
    runtime: TaskRuntimeId,
    machine_contract: MachineContractId,
    selected_migration_demand: MigrationDemand,
    inline_completion: InlineCompletionBehavior,
}

impl TaskRuntimeAdmission {
    pub const fn identity(&self) -> ActivationAdmissionId {
        self.identity
    }

    pub const fn runtime(&self) -> TaskRuntimeId {
        self.runtime
    }

    pub const fn selected_migration_demand(&self) -> MigrationDemand {
        self.selected_migration_demand
    }

    pub const fn inline_completion(&self) -> InlineCompletionBehavior {
        self.inline_completion
    }
}

pub fn admit_activation(
    plan: &ValidatedActivationPlan,
    runtime: &AdmittedTaskRuntimeContract,
) -> Result<TaskRuntimeAdmission, TaskPlanDiagnostic> {
    let candidate = plan.candidate();
    let contract = runtime.contract();
    if candidate.continuation_bytes > contract.max_continuation_bytes {
        return Err(TaskPlanDiagnostic(format!(
            "activation needs {} continuation bytes but runtime admits {}",
            candidate.continuation_bytes, contract.max_continuation_bytes
        )));
    }
    if candidate.continuation_alignment > contract.max_continuation_alignment
        || !contract
            .max_continuation_alignment
            .is_multiple_of(candidate.continuation_alignment)
    {
        return Err(TaskPlanDiagnostic(format!(
            "activation alignment {} is incompatible with runtime alignment {}",
            candidate.continuation_alignment, contract.max_continuation_alignment
        )));
    }
    if candidate.cancellation_required && !contract.cancellation {
        return Err(TaskPlanDiagnostic(
            "activation requires cancellation but runtime does not provide it".into(),
        ));
    }
    if contract.inline_completion == InlineCompletionBehavior::MayCompleteInline
        && candidate.activation == DistinctActivationRequirement::Required
    {
        return Err(TaskPlanDiagnostic(
            "runtime may complete inline but activation contract requires distinct execution"
                .into(),
        ));
    }

    let demand = match contract.preemption {
        PreemptionGranularity::SafePoints => candidate.safe_point_migration,
        PreemptionGranularity::Asynchronous => {
            candidate.asynchronous_migration.ok_or_else(|| {
                TaskPlanDiagnostic(
                    "asynchronous runtime requires an all-instruction carry analysis".into(),
                )
            })?
        }
    };
    if demand.cpu == SameCpuDemand::Same && contract.cpu_migration != CpuMigrationBehavior::Pinned {
        return Err(TaskPlanDiagnostic(
            "activation requires same-CPU execution but runtime may migrate it".into(),
        ));
    }
    if demand.thread == SameThreadDemand::Same
        && contract.thread_migration != ThreadMigrationBehavior::Pinned
    {
        return Err(TaskPlanDiagnostic(
            "activation requires same-host-thread execution but runtime may migrate it".into(),
        ));
    }
    if demand.address == AddressStabilityDemand::Stable
        && contract.continuation_storage != ContinuationStorageBehavior::Stable
    {
        return Err(TaskPlanDiagnostic(
            "activation requires stable addresses but runtime may move continuation storage".into(),
        ));
    }

    Ok(TaskRuntimeAdmission {
        identity: ActivationAdmissionId(fingerprint_admission(plan, runtime)),
        runtime: runtime.runtime,
        machine_contract: candidate.machine_contract,
        selected_migration_demand: demand,
        inline_completion: contract.inline_completion,
    })
}

/// Provider-normalized identity of one persistent activation-storage lease.
///
/// This record is provenance, not the source-visible lease authority. The
/// provider owns minting and transfers the corresponding linear lease through
/// the task-start outcome; the lifecycle ledger retains only the exact
/// owner/lease edge needed to reject premature reclamation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TaskStorageProvenance {
    pub owner: TaskStorageOwnerId,
    pub lease: TaskStorageLeaseId,
}

/// Physical-storage relationship selected for one accepted activation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStorageBinding {
    Persistent(TaskStorageProvenance),
    /// The activation completed during start and retained no persistent
    /// activation storage. Its lifecycle claim still requires settlement.
    InlineCompletion,
}

/// Auditable dependency retained for every live task claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaskDependencyRecord {
    pub claim: TaskLifecycleClaimId,
    pub runtime: TaskRuntimeId,
    pub runtime_instance: TaskRuntimeInstanceId,
    pub admission: ActivationAdmissionId,
    pub activation: ActivationInstanceId,
    pub storage: TaskStorageBinding,
}

/// Source-level `Task<T>` is linear. This normalized carrier mirrors that
/// property by withholding `Clone`/`Copy` and exposing no public constructor.
#[derive(Debug, PartialEq, Eq)]
pub struct TaskLifecycleClaim {
    record: TaskDependencyRecord,
}

impl TaskLifecycleClaim {
    pub const fn identity(&self) -> TaskLifecycleClaimId {
        self.record.claim
    }

    pub const fn runtime_instance(&self) -> TaskRuntimeInstanceId {
        self.record.runtime_instance
    }

    pub const fn activation(&self) -> ActivationInstanceId {
        self.record.activation
    }

    pub const fn storage(&self) -> TaskStorageBinding {
        self.record.storage
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct SettledTaskLifecycle {
    record: TaskDependencyRecord,
}

impl SettledTaskLifecycle {
    pub const fn identity(&self) -> TaskLifecycleClaimId {
        self.record.claim
    }

    /// Returns the exact storage relationship released by terminal task
    /// settlement. A provider may reclaim/recycle persistent storage only
    /// after receiving this result.
    pub const fn released_storage(&self) -> TaskStorageBinding {
        self.record.storage
    }
}

#[derive(Debug)]
pub struct TaskSettlementError {
    claim: TaskLifecycleClaim,
    diagnostic: TaskPlanDiagnostic,
}

impl TaskSettlementError {
    pub const fn diagnostic(&self) -> &TaskPlanDiagnostic {
        &self.diagnostic
    }

    pub fn into_claim(self) -> TaskLifecycleClaim {
        self.claim
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct ClosedTaskRuntime {
    runtime: TaskRuntimeId,
    instance: TaskRuntimeInstanceId,
}

impl ClosedTaskRuntime {
    pub const fn runtime(&self) -> TaskRuntimeId {
        self.runtime
    }

    pub const fn instance(&self) -> TaskRuntimeInstanceId {
        self.instance
    }
}

#[derive(Debug)]
pub struct TaskRuntimeCloseError {
    ledger: TaskLifecycleLedger,
    diagnostic: TaskPlanDiagnostic,
}

impl TaskRuntimeCloseError {
    pub const fn diagnostic(&self) -> &TaskPlanDiagnostic {
        &self.diagnostic
    }

    pub fn into_ledger(self) -> TaskLifecycleLedger {
        self.ledger
    }
}

/// Provider-local accounting for operational custody, physical storage, and
/// the independently held lifecycle claim.
///
/// A runtime instance has one ledger. Accepting an activation records the
/// exact admitted provider and storage dependency before a `Task<T>` claim is
/// issued. Terminal settlement removes that dependency. Runtime close and
/// storage reclamation fail while any matching child claim remains live.
#[derive(Debug)]
pub struct TaskLifecycleLedger {
    runtime: TaskRuntimeId,
    instance: TaskRuntimeInstanceId,
    live: BTreeMap<TaskLifecycleClaimId, TaskDependencyRecord>,
    used_activations: BTreeSet<ActivationInstanceId>,
    used_storage_leases: BTreeSet<TaskStorageProvenance>,
}

impl TaskLifecycleLedger {
    pub fn new(runtime: &AdmittedTaskRuntimeContract, instance: TaskRuntimeInstanceId) -> Self {
        Self {
            runtime: runtime.runtime(),
            instance,
            live: BTreeMap::new(),
            used_activations: BTreeSet::new(),
            used_storage_leases: BTreeSet::new(),
        }
    }

    pub fn records(&self) -> impl Iterator<Item = &TaskDependencyRecord> {
        self.live.values()
    }

    pub fn accept_activation(
        &mut self,
        admission: &TaskRuntimeAdmission,
        activation: ActivationInstanceId,
        storage: TaskStorageBinding,
    ) -> Result<TaskLifecycleClaim, TaskPlanDiagnostic> {
        if admission.runtime != self.runtime {
            return Err(TaskPlanDiagnostic(
                "task claim admission belongs to a different runtime provider".into(),
            ));
        }
        if self.used_activations.contains(&activation) {
            return Err(TaskPlanDiagnostic(
                "task activation identity has already been accepted by this runtime instance"
                    .into(),
            ));
        }
        match storage {
            TaskStorageBinding::Persistent(provenance) => {
                if self.used_storage_leases.contains(&provenance) {
                    return Err(TaskPlanDiagnostic(
                        "task storage lease identity has already been used; storage reuse requires a new lease era"
                            .into(),
                    ));
                }
            }
            TaskStorageBinding::InlineCompletion
                if admission.inline_completion != InlineCompletionBehavior::MayCompleteInline =>
            {
                return Err(TaskPlanDiagnostic(
                    "runtime admission does not permit inline completion without persistent activation storage"
                        .into(),
                ));
            }
            TaskStorageBinding::InlineCompletion => {}
        }

        let claim = TaskLifecycleClaimId(fingerprint_task_claim(
            admission,
            self.instance,
            activation,
            storage,
        ));
        if self.live.contains_key(&claim) {
            return Err(TaskPlanDiagnostic(
                "normalized task lifecycle claim identity collides with a live claim".into(),
            ));
        }
        let record = TaskDependencyRecord {
            claim,
            runtime: self.runtime,
            runtime_instance: self.instance,
            admission: admission.identity,
            activation,
            storage,
        };
        self.used_activations.insert(activation);
        if let TaskStorageBinding::Persistent(provenance) = storage {
            self.used_storage_leases.insert(provenance);
        }
        self.live.insert(claim, record);
        Ok(TaskLifecycleClaim { record })
    }

    /// Cancellation requests preserve the lifecycle obligation. This check is
    /// intentionally read-only: only terminal settlement removes the record.
    pub fn validate_cancellation_request(
        &self,
        claim: &TaskLifecycleClaim,
    ) -> Result<(), TaskPlanDiagnostic> {
        if self.live.get(&claim.record.claim) == Some(&claim.record) {
            Ok(())
        } else {
            Err(TaskPlanDiagnostic(
                "cancellation requires the exact live task lifecycle claim".into(),
            ))
        }
    }

    pub fn settle(
        &mut self,
        claim: TaskLifecycleClaim,
    ) -> Result<SettledTaskLifecycle, TaskSettlementError> {
        if self.live.get(&claim.record.claim) != Some(&claim.record) {
            return Err(TaskSettlementError {
                claim,
                diagnostic: TaskPlanDiagnostic(
                    "task settlement requires the exact live runtime/activation/storage claim"
                        .into(),
                ),
            });
        }
        self.live.remove(&claim.record.claim);
        Ok(SettledTaskLifecycle {
            record: claim.record,
        })
    }

    /// Validate the provider's storage-reclaim precondition. The storage
    /// authority itself remains outside this normalized ledger.
    pub fn validate_storage_reclaim(
        &self,
        storage: TaskStorageProvenance,
    ) -> Result<(), TaskPlanDiagnostic> {
        if self
            .live
            .values()
            .any(|record| record.storage == TaskStorageBinding::Persistent(storage))
        {
            return Err(TaskPlanDiagnostic(
                "task storage cannot be reclaimed while a dependent lifecycle claim is live".into(),
            ));
        }
        Ok(())
    }

    /// Close consumes the provider-local ledger only when every child task
    /// claim has been terminally settled or transferred out through a future
    /// explicitly accounted operation.
    pub fn close(self) -> Result<ClosedTaskRuntime, TaskRuntimeCloseError> {
        if !self.live.is_empty() {
            return Err(TaskRuntimeCloseError {
                diagnostic: TaskPlanDiagnostic(format!(
                    "task runtime cannot close while {} dependent lifecycle claim(s) remain live",
                    self.live.len()
                )),
                ledger: self,
            });
        }
        Ok(ClosedTaskRuntime {
            runtime: self.runtime,
            instance: self.instance,
        })
    }
}

fn fingerprint_activation_plan(plan: &ActivationPlanCandidate) -> u64 {
    let mut fingerprint = Fingerprint::new();
    fingerprint.word(plan.machine_contract.normalized_identity());
    fingerprint.word(plan.entry.normalized_identity());
    fingerprint.word(plan.argument_layout.normalized_identity());
    fingerprint.word(plan.terminal_outcome_layout.normalized_identity());
    fingerprint.word(plan.calling_plan.normalized_identity());
    fingerprint.word(plan.continuation_bytes);
    fingerprint.word(plan.continuation_alignment);
    fingerprint.flag(plan.may_suspend);
    fingerprint.flag(plan.may_block);
    fingerprint.flag(plan.suspension_crossings_safe);
    fingerprint.migration(plan.safe_point_migration);
    match plan.asynchronous_migration {
        Some(demand) => {
            fingerprint.byte(1);
            fingerprint.migration(demand);
        }
        None => fingerprint.byte(0),
    }
    fingerprint.flag(plan.cancellation_required);
    fingerprint.byte(match plan.activation {
        DistinctActivationRequirement::Required => 1,
        DistinctActivationRequirement::InlineCompletionAllowed => 2,
    });
    fingerprint.finish()
}

fn fingerprint_runtime_contract(runtime: &TaskRuntimeContract) -> u64 {
    let mut fingerprint = Fingerprint::new();
    fingerprint.word(runtime.provider_plan_fingerprint);
    fingerprint.word(runtime.max_continuation_bytes);
    fingerprint.word(runtime.max_continuation_alignment);
    fingerprint.byte(match runtime.preemption {
        PreemptionGranularity::SafePoints => 1,
        PreemptionGranularity::Asynchronous => 2,
    });
    fingerprint.byte(match runtime.cpu_migration {
        CpuMigrationBehavior::MayMigrate => 1,
        CpuMigrationBehavior::Pinned => 2,
    });
    fingerprint.byte(match runtime.thread_migration {
        ThreadMigrationBehavior::MayMigrate => 1,
        ThreadMigrationBehavior::Pinned => 2,
    });
    fingerprint.byte(match runtime.continuation_storage {
        ContinuationStorageBehavior::Movable => 1,
        ContinuationStorageBehavior::Stable => 2,
    });
    fingerprint.flag(runtime.cancellation);
    fingerprint.byte(match runtime.inline_completion {
        InlineCompletionBehavior::Never => 1,
        InlineCompletionBehavior::MayCompleteInline => 2,
    });
    fingerprint.finish()
}

fn fingerprint_admission(
    plan: &ValidatedActivationPlan,
    runtime: &AdmittedTaskRuntimeContract,
) -> u64 {
    let mut fingerprint = Fingerprint::new();
    fingerprint.word(plan.normalized_identity().normalized_identity());
    fingerprint.word(runtime.runtime.normalized_identity());
    fingerprint.finish()
}

fn fingerprint_task_claim(
    admission: &TaskRuntimeAdmission,
    runtime_instance: TaskRuntimeInstanceId,
    activation: ActivationInstanceId,
    storage: TaskStorageBinding,
) -> u64 {
    let mut fingerprint = Fingerprint::new();
    fingerprint.word(admission.identity.normalized_identity());
    fingerprint.word(admission.runtime.normalized_identity());
    fingerprint.word(runtime_instance.normalized_identity());
    fingerprint.word(activation.normalized_identity());
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

    fn migration(&mut self, demand: MigrationDemand) {
        self.byte(match demand.cpu {
            SameCpuDemand::Any => 1,
            SameCpuDemand::Same => 2,
        });
        self.byte(match demand.thread {
            SameThreadDemand::Any => 1,
            SameThreadDemand::Same => 2,
        });
        self.byte(match demand.address {
            AddressStabilityDemand::Movable => 1,
            AddressStabilityDemand::Stable => 2,
        });
    }

    fn finish(self) -> u64 {
        // Normalized IDs reserve zero as the invalid sentinel. FNV-1a
        // reaching zero is extraordinarily unlikely, but the representation
        // must remain total rather than manufacturing an invalid ID.
        if self.0 == 0 { Self::OFFSET } else { self.0 }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskPlanDiagnostic(pub String);

impl std::fmt::Display for TaskPlanDiagnostic {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for TaskPlanDiagnostic {}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_core::trust::TrustProvenance;

    const PROVIDER_PLAN: &str = "test::TaskRuntimeProvider";
    const PROVIDER_PLAN_FINGERPRINT: u64 = 0x7a5c_1138_9b2d_4401;

    fn id<T>(identity: u64, constructor: fn(u64) -> Result<T, TaskPlanDiagnostic>) -> T {
        constructor(identity).expect("normalized identity")
    }

    fn candidate() -> ActivationPlanCandidate {
        ActivationPlanCandidate {
            machine_contract: id(1, MachineContractId::from_normalized_identity),
            entry: id(2, MachineEntryId::from_normalized_identity),
            argument_layout: id(3, ValueLayoutId::from_normalized_identity),
            terminal_outcome_layout: id(4, ValueLayoutId::from_normalized_identity),
            calling_plan: id(5, CallingPlanId::from_normalized_identity),
            continuation_bytes: 4096,
            continuation_alignment: 16,
            may_suspend: true,
            may_block: false,
            suspension_crossings_safe: true,
            safe_point_migration: MigrationDemand {
                cpu: SameCpuDemand::Same,
                thread: SameThreadDemand::Any,
                address: AddressStabilityDemand::Stable,
            },
            asynchronous_migration: None,
            cancellation_required: true,
            activation: DistinctActivationRequirement::Required,
        }
    }

    fn runtime_claim() -> TaskRuntimeContract {
        TaskRuntimeContract {
            provider_plan: PROVIDER_PLAN.into(),
            provider_plan_fingerprint: PROVIDER_PLAN_FINGERPRINT,
            max_continuation_bytes: 8192,
            max_continuation_alignment: 64,
            preemption: PreemptionGranularity::SafePoints,
            cpu_migration: CpuMigrationBehavior::Pinned,
            thread_migration: ThreadMigrationBehavior::MayMigrate,
            continuation_storage: ContinuationStorageBehavior::Stable,
            cancellation: true,
            inline_completion: InlineCompletionBehavior::Never,
        }
    }

    fn runtime_receipt(claim: &TaskRuntimeContract, provenance: TrustProvenance) -> TrustReceipt {
        TrustReceipt {
            commitment: TrustCommitment::ProviderPlan(PROVIDER_PLAN.into()),
            statement_hash: claim.statement_fingerprint(),
            provenance,
        }
    }

    fn admitted_runtime(claim: TaskRuntimeContract) -> AdmittedTaskRuntimeContract {
        let receipt = runtime_receipt(&claim, TrustProvenance::RootGrant);
        admit_task_runtime(claim, receipt).expect("task runtime provider admission")
    }

    fn runtime() -> AdmittedTaskRuntimeContract {
        admitted_runtime(runtime_claim())
    }

    #[test]
    fn safe_point_runtime_admits_matching_storage_and_carry_demands() {
        let plan = validate_activation_plan(candidate()).expect("activation plan");
        let admission = admit_activation(&plan, &runtime()).expect("runtime admission");
        assert_ne!(admission.runtime().normalized_identity(), 0);
        assert_eq!(
            admission.selected_migration_demand().cpu,
            SameCpuDemand::Same
        );
    }

    #[test]
    fn suspension_is_rejected_locally_before_runtime_selection() {
        let mut unsafe_plan = candidate();
        unsafe_plan.suspension_crossings_safe = false;
        let error = validate_activation_plan(unsafe_plan).expect_err("unsafe park");
        assert!(error.0.contains("suspension crossing"));
    }

    #[test]
    fn asynchronous_runtime_requires_its_own_liveness_envelope() {
        let plan = validate_activation_plan(candidate()).expect("activation plan");
        let mut asynchronous = runtime_claim();
        asynchronous.preemption = PreemptionGranularity::Asynchronous;
        let asynchronous = admitted_runtime(asynchronous);
        let error = admit_activation(&plan, &asynchronous)
            .expect_err("safe-point analysis cannot justify async preemption");
        assert!(error.0.contains("all-instruction"));
    }

    #[test]
    fn asynchronous_runtime_admits_a_matching_all_instruction_envelope() {
        let mut asynchronous_candidate = candidate();
        asynchronous_candidate.asynchronous_migration = Some(MigrationDemand {
            cpu: SameCpuDemand::Same,
            thread: SameThreadDemand::Any,
            address: AddressStabilityDemand::Stable,
        });
        let plan = validate_activation_plan(asynchronous_candidate).expect("activation plan");
        let mut asynchronous = runtime_claim();
        asynchronous.preemption = PreemptionGranularity::Asynchronous;
        let asynchronous = admitted_runtime(asynchronous);

        let admission =
            admit_activation(&plan, &asynchronous).expect("asynchronous runtime admission");
        assert_eq!(
            admission.selected_migration_demand(),
            MigrationDemand {
                cpu: SameCpuDemand::Same,
                thread: SameThreadDemand::Any,
                address: AddressStabilityDemand::Stable,
            }
        );
    }

    #[test]
    fn migration_storage_and_inline_behavior_fail_closed() {
        let plan = validate_activation_plan(candidate()).expect("activation plan");
        let mut migrating = runtime_claim();
        migrating.cpu_migration = CpuMigrationBehavior::MayMigrate;
        let migrating = admitted_runtime(migrating);
        let error = admit_activation(&plan, &migrating).expect_err("same CPU demand");
        assert!(error.0.contains("same-CPU"));

        let mut movable = runtime_claim();
        movable.continuation_storage = ContinuationStorageBehavior::Movable;
        let movable = admitted_runtime(movable);
        let error = admit_activation(&plan, &movable).expect_err("stable address demand");
        assert!(error.0.contains("stable addresses"));

        let mut inline = runtime_claim();
        inline.inline_completion = InlineCompletionBehavior::MayCompleteInline;
        let inline = admitted_runtime(inline);
        let error = admit_activation(&plan, &inline).expect_err("distinct activation required");
        assert!(error.0.contains("inline"));
    }

    #[test]
    fn pessimistic_runtime_cannot_accidentally_admit_work() {
        let plan = validate_activation_plan(candidate()).expect("activation plan");
        let runtime = admitted_runtime(TaskRuntimeContract::pessimistic(
            PROVIDER_PLAN,
            PROVIDER_PLAN_FINGERPRINT,
        ));
        assert!(admit_activation(&plan, &runtime).is_err());
    }

    #[test]
    fn runtime_behavior_requires_the_exact_shared_provider_receipt() {
        let wrong_plan = TrustReceipt {
            commitment: TrustCommitment::ProviderPlan("other::provider".into()),
            statement_hash: runtime_claim().statement_fingerprint(),
            provenance: TrustProvenance::RootGrant,
        };
        let error = admit_task_runtime(runtime_claim(), wrong_plan).expect_err("wrong plan");
        assert!(error.0.contains("exact normalized provider plan"));

        let mut drifted = runtime_receipt(&runtime_claim(), TrustProvenance::RootGrant);
        drifted.statement_hash ^= 1;
        let error = admit_task_runtime(runtime_claim(), drifted).expect_err("drifted plan");
        assert!(error.0.contains("exact normalized provider plan"));

        let approved = runtime_claim();
        let receipt = runtime_receipt(&approved, TrustProvenance::RootGrant);
        let mut stronger_claim = approved;
        stronger_claim.thread_migration = ThreadMigrationBehavior::Pinned;
        let error =
            admit_task_runtime(stronger_claim, receipt).expect_err("unapproved behavior change");
        assert!(error.0.contains("exact normalized provider plan"));
    }

    #[test]
    fn receipt_provenance_is_evidence_not_runtime_identity() {
        let dev = admit_task_runtime(
            runtime_claim(),
            runtime_receipt(&runtime_claim(), TrustProvenance::OwnPackageDev),
        )
        .expect("dev admission");
        let root = runtime();
        assert_eq!(dev.runtime(), root.runtime());
        assert_ne!(dev.receipt().provenance, root.receipt().provenance);
    }

    #[test]
    fn normalized_plan_and_admission_identities_bind_every_checked_input() {
        let plan = validate_activation_plan(candidate()).expect("activation plan");
        let plan_identity = plan.normalized_identity();
        let admission = admit_activation(&plan, &runtime()).expect("runtime admission");

        let mut changed_candidate = candidate();
        changed_candidate.continuation_bytes += 1;
        let changed_plan =
            validate_activation_plan(changed_candidate).expect("changed activation plan");
        assert_ne!(plan_identity, changed_plan.normalized_identity());

        let mut changed_runtime = runtime_claim();
        changed_runtime.max_continuation_bytes += 1;
        let changed_runtime = admitted_runtime(changed_runtime);
        let changed_admission =
            admit_activation(&plan, &changed_runtime).expect("changed runtime admission");
        assert_ne!(admission.identity(), changed_admission.identity());
    }

    #[test]
    fn lifecycle_claim_pins_provider_and_storage_until_terminal_settlement() {
        let runtime = runtime();
        let plan = validate_activation_plan(candidate()).expect("activation plan");
        let admission = admit_activation(&plan, &runtime).expect("runtime admission");
        let instance = id(100, TaskRuntimeInstanceId::from_normalized_identity);
        let activation = id(101, ActivationInstanceId::from_normalized_identity);
        let storage = TaskStorageProvenance {
            owner: id(102, TaskStorageOwnerId::from_normalized_identity),
            lease: id(103, TaskStorageLeaseId::from_normalized_identity),
        };
        let mut ledger = TaskLifecycleLedger::new(&runtime, instance);
        let claim = ledger
            .accept_activation(
                &admission,
                activation,
                TaskStorageBinding::Persistent(storage),
            )
            .expect("accepted activation");

        assert_eq!(ledger.records().count(), 1);
        ledger
            .validate_cancellation_request(&claim)
            .expect("cancellation preserves the live claim");
        let reclaim = ledger
            .validate_storage_reclaim(storage)
            .expect_err("live task pins its child storage");
        assert!(reclaim.0.contains("dependent lifecycle claim"));

        let close = ledger.close().expect_err("live child blocks runtime close");
        assert!(close.diagnostic().0.contains("1 dependent"));
        let mut ledger = close.into_ledger();
        let settled = ledger.settle(claim).expect("terminal settlement");
        assert_eq!(
            settled.released_storage(),
            TaskStorageBinding::Persistent(storage)
        );
        ledger
            .validate_storage_reclaim(storage)
            .expect("settlement releases the storage dependency");
        let closed = ledger.close().expect("settled runtime closes");
        assert_eq!(closed.instance(), instance);
        assert_eq!(closed.runtime(), runtime.runtime());
    }

    #[test]
    fn lifecycle_rejects_replayed_activation_and_storage_eras() {
        let runtime = runtime();
        let plan = validate_activation_plan(candidate()).expect("activation plan");
        let admission = admit_activation(&plan, &runtime).expect("runtime admission");
        let instance = id(110, TaskRuntimeInstanceId::from_normalized_identity);
        let first_activation = id(111, ActivationInstanceId::from_normalized_identity);
        let second_activation = id(112, ActivationInstanceId::from_normalized_identity);
        let storage = TaskStorageProvenance {
            owner: id(113, TaskStorageOwnerId::from_normalized_identity),
            lease: id(114, TaskStorageLeaseId::from_normalized_identity),
        };
        let alternate_storage = TaskStorageProvenance {
            owner: storage.owner,
            lease: id(115, TaskStorageLeaseId::from_normalized_identity),
        };
        let mut ledger = TaskLifecycleLedger::new(&runtime, instance);
        let claim = ledger
            .accept_activation(
                &admission,
                first_activation,
                TaskStorageBinding::Persistent(storage),
            )
            .expect("first activation");

        let replayed_activation = ledger
            .accept_activation(
                &admission,
                first_activation,
                TaskStorageBinding::Persistent(alternate_storage),
            )
            .expect_err("activation identity cannot replay");
        assert!(replayed_activation.0.contains("already been accepted"));

        let replayed_lease = ledger
            .accept_activation(
                &admission,
                second_activation,
                TaskStorageBinding::Persistent(storage),
            )
            .expect_err("storage lease era cannot back two claims");
        assert!(replayed_lease.0.contains("new lease era"));

        ledger.settle(claim).expect("settle first activation");
        let replay_after_settlement = ledger
            .accept_activation(
                &admission,
                second_activation,
                TaskStorageBinding::Persistent(storage),
            )
            .expect_err("settlement does not make an old lease identity fresh");
        assert!(replay_after_settlement.0.contains("new lease era"));
    }

    #[test]
    fn failed_cross_runtime_settlement_returns_the_linear_claim() {
        let runtime = runtime();
        let plan = validate_activation_plan(candidate()).expect("activation plan");
        let admission = admit_activation(&plan, &runtime).expect("runtime admission");
        let mut owner = TaskLifecycleLedger::new(
            &runtime,
            id(120, TaskRuntimeInstanceId::from_normalized_identity),
        );
        let mut wrong_instance = TaskLifecycleLedger::new(
            &runtime,
            id(121, TaskRuntimeInstanceId::from_normalized_identity),
        );
        let claim = owner
            .accept_activation(
                &admission,
                id(122, ActivationInstanceId::from_normalized_identity),
                TaskStorageBinding::Persistent(TaskStorageProvenance {
                    owner: id(123, TaskStorageOwnerId::from_normalized_identity),
                    lease: id(124, TaskStorageLeaseId::from_normalized_identity),
                }),
            )
            .expect("accepted activation");

        let error = wrong_instance
            .settle(claim)
            .expect_err("another runtime instance cannot settle this claim");
        assert!(error.diagnostic().0.contains("exact live"));
        let claim = error.into_claim();
        owner
            .settle(claim)
            .expect("failed settlement preserved the claim");
    }

    #[test]
    fn no_storage_binding_requires_admitted_inline_completion() {
        let runtime = runtime();
        let plan = validate_activation_plan(candidate()).expect("activation plan");
        let admission = admit_activation(&plan, &runtime).expect("runtime admission");
        let mut ledger = TaskLifecycleLedger::new(
            &runtime,
            id(130, TaskRuntimeInstanceId::from_normalized_identity),
        );
        let error = ledger
            .accept_activation(
                &admission,
                id(131, ActivationInstanceId::from_normalized_identity),
                TaskStorageBinding::InlineCompletion,
            )
            .expect_err("non-inline runtime cannot omit persistent storage");
        assert!(error.0.contains("does not permit inline completion"));

        let mut inline_candidate = candidate();
        inline_candidate.activation = DistinctActivationRequirement::InlineCompletionAllowed;
        let inline_plan =
            validate_activation_plan(inline_candidate).expect("inline-capable activation");
        let mut inline_runtime_claim = runtime_claim();
        inline_runtime_claim.inline_completion = InlineCompletionBehavior::MayCompleteInline;
        let inline_runtime = admitted_runtime(inline_runtime_claim);
        let inline_admission =
            admit_activation(&inline_plan, &inline_runtime).expect("inline admission");
        let mut inline_ledger = TaskLifecycleLedger::new(
            &inline_runtime,
            id(132, TaskRuntimeInstanceId::from_normalized_identity),
        );
        let claim = inline_ledger
            .accept_activation(
                &inline_admission,
                id(133, ActivationInstanceId::from_normalized_identity),
                TaskStorageBinding::InlineCompletion,
            )
            .expect("inline completion still returns a live lifecycle claim");
        assert_eq!(claim.storage(), TaskStorageBinding::InlineCompletion);
        inline_ledger
            .settle(claim)
            .expect("completed inline claim still requires settlement");
    }
}

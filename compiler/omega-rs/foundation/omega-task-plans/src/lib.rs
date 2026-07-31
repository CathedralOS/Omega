//! Provider-independent task activation plans and lifecycle accounting.
//!
//! An activation plan describes one fixed, nonmoving stack, the canonical
//! semantic suspension crossings, and only the CPU/thread preservation those
//! crossings demand. Executor selection consumes exact per-axis checked or
//! admitted evidence; this crate deliberately does not publish a generalized
//! runtime behavior record.

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
normalized_id!(StackRepresentationId, "stack-representation");
normalized_id!(SuspensionCrossingId, "suspension-crossing");
normalized_id!(TaskRuntimeId, "task-runtime");
normalized_id!(TaskRuntimeInstanceId, "task-runtime-instance");
normalized_id!(ActivationPlanId, "activation-plan");
normalized_id!(
    ExecutorPreservationEvidenceId,
    "executor-preservation-evidence"
);
normalized_id!(ExecutorSelectionId, "executor-selection");
normalized_id!(ActivationInstanceId, "activation-instance");
normalized_id!(TaskStorageOwnerId, "task-storage-owner");
normalized_id!(TaskStorageLeaseId, "task-storage-lease");
normalized_id!(TaskLifecycleClaimId, "task-lifecycle-claim");

/// Fixed, nonmoving stack resource required by one activation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StackPlan {
    pub bytes: u64,
    pub alignment: u64,
    pub representation: StackRepresentationId,
}

/// One canonical semantic crossing at which the activation can park.
///
/// `identity` binds the detailed checked-tree crossing record retained in the
/// carry artifact. The local permission and preservation columns are repeated
/// here so activation-plan consumers do not reinterpret source or liveness.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CanonicalSuspensionCrossing {
    pub identity: SuspensionCrossingId,
    pub suspension_allowed: bool,
    pub preserve_cpu: bool,
    pub preserve_host_thread: bool,
}

/// Activation-wide scheduler preservation derived by joining the canonical
/// suspension crossings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActivationCarryObligations {
    pub preserve_cpu: bool,
    pub preserve_host_thread: bool,
}

impl ActivationCarryObligations {
    pub const fn none() -> Self {
        Self {
            preserve_cpu: false,
            preserve_host_thread: false,
        }
    }

    fn required_by_crossings(crossings: &[CanonicalSuspensionCrossing]) -> Self {
        crossings
            .iter()
            .fold(Self::none(), |mut obligations, crossing| {
                obligations.preserve_cpu |= crossing.preserve_cpu;
                obligations.preserve_host_thread |= crossing.preserve_host_thread;
                obligations
            })
    }
}

/// Provider-independent output of compile-time machine target elaboration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivationPlanCandidate {
    pub machine_contract: MachineContractId,
    pub entry: MachineEntryId,
    pub argument_layout: ValueLayoutId,
    pub terminal_outcome_layout: ValueLayoutId,
    pub calling_plan: CallingPlanId,
    pub stack_plan: StackPlan,
    pub may_suspend: bool,
    pub may_block: bool,
    pub canonical_suspension_crossings: Vec<CanonicalSuspensionCrossing>,
    pub carry_obligations: ActivationCarryObligations,
    pub cancellation_required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedActivationPlan(ActivationPlanCandidate);

impl ValidatedActivationPlan {
    pub const fn candidate(&self) -> &ActivationPlanCandidate {
        &self.0
    }

    /// Normalized identity of the complete provider-independent plan.
    pub fn normalized_identity(&self) -> ActivationPlanId {
        ActivationPlanId(fingerprint_activation_plan(&self.0))
    }
}

pub fn validate_activation_plan(
    candidate: ActivationPlanCandidate,
) -> Result<ValidatedActivationPlan, TaskPlanDiagnostic> {
    if candidate.stack_plan.bytes == 0 {
        return Err(TaskPlanDiagnostic(
            "activation stack size must be nonzero".into(),
        ));
    }
    if candidate.stack_plan.alignment == 0 || !candidate.stack_plan.alignment.is_power_of_two() {
        return Err(TaskPlanDiagnostic(format!(
            "activation stack alignment {} is not a nonzero power of two",
            candidate.stack_plan.alignment
        )));
    }
    if candidate.may_suspend && candidate.canonical_suspension_crossings.is_empty() {
        return Err(TaskPlanDiagnostic(
            "a suspending activation has no canonical suspension crossings".into(),
        ));
    }
    if !candidate.may_suspend && !candidate.canonical_suspension_crossings.is_empty() {
        return Err(TaskPlanDiagnostic(
            "a non-suspending activation cannot publish suspension crossings".into(),
        ));
    }
    if candidate
        .canonical_suspension_crossings
        .iter()
        .any(|crossing| !crossing.suspension_allowed)
    {
        return Err(TaskPlanDiagnostic(
            "a possible suspension crossing carries a value that forbids suspension".into(),
        ));
    }
    let crossing_requirements = ActivationCarryObligations::required_by_crossings(
        &candidate.canonical_suspension_crossings,
    );
    if (crossing_requirements.preserve_cpu && !candidate.carry_obligations.preserve_cpu)
        || (crossing_requirements.preserve_host_thread
            && !candidate.carry_obligations.preserve_host_thread)
    {
        return Err(TaskPlanDiagnostic(
            "activation-wide CPU/thread preservation understates a canonical crossing".into(),
        ));
    }
    Ok(ValidatedActivationPlan(candidate))
}

/// One independent affinity axis an executor can prove it preserves.
///
/// Suspension and address stability are deliberately absent. Suspension is a
/// local liveness judgment, while address stability follows from the selected
/// fixed nonmoving stack lease.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ExecutorPreservationAxis {
    Cpu,
    HostThread,
}

/// Exact checked-conformance or admission-receipt evidence for one axis.
///
/// The normalized identity is produced by provider selection from the
/// conformance/receipt it validated. This is evidence identity, not a freely
/// authored runtime behavior bit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExecutorPreservationEvidence {
    axis: ExecutorPreservationAxis,
    identity: ExecutorPreservationEvidenceId,
}

impl ExecutorPreservationEvidence {
    pub const fn new(
        axis: ExecutorPreservationAxis,
        identity: ExecutorPreservationEvidenceId,
    ) -> Self {
        Self { axis, identity }
    }

    pub const fn axis(self) -> ExecutorPreservationAxis {
        self.axis
    }

    pub const fn identity(self) -> ExecutorPreservationEvidenceId {
        self.identity
    }
}

/// Exact runtime instance and preservation evidence selected for one
/// provider-independent activation plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutorSelectionCandidate {
    pub runtime: TaskRuntimeId,
    pub runtime_instance: TaskRuntimeInstanceId,
    pub preservation: Vec<ExecutorPreservationEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedExecutorSelection {
    identity: ExecutorSelectionId,
    candidate: ExecutorSelectionCandidate,
    plan: ValidatedActivationPlan,
}

impl ValidatedExecutorSelection {
    pub const fn identity(&self) -> ExecutorSelectionId {
        self.identity
    }

    pub const fn candidate(&self) -> &ExecutorSelectionCandidate {
        &self.candidate
    }

    pub const fn plan(&self) -> &ValidatedActivationPlan {
        &self.plan
    }
}

/// Validate one already resolved executor selection against the activation's
/// demanded CPU/thread preservation. Checked providers and opaque admitted
/// providers both arrive as exact per-axis evidence identities; absence fails
/// closed, and multiple identities for one axis reject rather than guessing.
pub fn validate_executor_selection(
    plan: &ValidatedActivationPlan,
    mut candidate: ExecutorSelectionCandidate,
) -> Result<ValidatedExecutorSelection, TaskPlanDiagnostic> {
    candidate
        .preservation
        .sort_by_key(|evidence| evidence.axis());
    for duplicate in candidate.preservation.windows(2) {
        if duplicate[0].axis() == duplicate[1].axis() {
            return Err(TaskPlanDiagnostic(format!(
                "selected executor supplies more than one {} preservation identity; selection must retain one exact checked conformance or admission receipt",
                axis_label(duplicate[0].axis()),
            )));
        }
    }

    let establishes = |axis| {
        candidate
            .preservation
            .iter()
            .any(|evidence| evidence.axis() == axis)
    };
    let obligations = plan.candidate().carry_obligations;
    if obligations.preserve_cpu && !establishes(ExecutorPreservationAxis::Cpu) {
        return Err(TaskPlanDiagnostic(
            "selected executor does not establish CPU preservation required by the activation"
                .into(),
        ));
    }
    if obligations.preserve_host_thread && !establishes(ExecutorPreservationAxis::HostThread) {
        return Err(TaskPlanDiagnostic(
            "selected executor does not establish host-thread preservation required by the activation"
                .into(),
        ));
    }

    Ok(ValidatedExecutorSelection {
        identity: ExecutorSelectionId(fingerprint_executor_selection(plan, &candidate)),
        candidate,
        plan: plan.clone(),
    })
}

const fn axis_label(axis: ExecutorPreservationAxis) -> &'static str {
    match axis {
        ExecutorPreservationAxis::Cpu => "CPU",
        ExecutorPreservationAxis::HostThread => "host-thread",
    }
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
    pub activation_plan: ActivationPlanId,
    pub executor_selection: ExecutorSelectionId,
    pub activation: ActivationInstanceId,
    pub storage: TaskStorageBinding,
}

/// Source-level `Task<T>` is linear. This normalized carrier mirrors that
/// property by withholding `Clone`/`Copy` and exposing no public constructor.
#[derive(Debug, PartialEq, Eq)]
pub struct TaskLifecycleClaim {
    dependency: LiveTaskDependency,
}

impl TaskLifecycleClaim {
    pub const fn identity(&self) -> TaskLifecycleClaimId {
        self.dependency.record.claim
    }

    pub const fn runtime_instance(&self) -> TaskRuntimeInstanceId {
        self.dependency.record.runtime_instance
    }

    pub const fn activation(&self) -> ActivationInstanceId {
        self.dependency.record.activation
    }

    pub const fn storage(&self) -> TaskStorageBinding {
        self.dependency.record.storage
    }
}

/// Exact executor selection and activation plan retained behind one live
/// lifecycle claim.
///
/// `TaskDependencyRecord` remains the compact report form. Custody and
/// settlement compare this carrier so compact identity collisions cannot move
/// a claim between distinct activation plans.
#[derive(Debug, Clone, PartialEq, Eq)]
struct LiveTaskDependency {
    record: TaskDependencyRecord,
    selection: Box<ValidatedExecutorSelection>,
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
/// A selected runtime instance has one ledger. Recording an already accepted
/// activation binds its exact plan and storage dependency before a `Task<T>`
/// claim is issued. Terminal settlement removes that dependency. Runtime close
/// and storage reclamation fail while any matching child claim remains live.
#[derive(Debug)]
pub struct TaskLifecycleLedger {
    runtime: TaskRuntimeId,
    instance: TaskRuntimeInstanceId,
    live: BTreeMap<TaskLifecycleClaimId, LiveTaskDependency>,
    used_activations: BTreeSet<ActivationInstanceId>,
    used_storage_leases: BTreeSet<TaskStorageProvenance>,
}

impl TaskLifecycleLedger {
    /// Create empty accounting for one runtime instance. Each accepted
    /// activation must still supply its exact validated executor selection.
    pub fn new(runtime: TaskRuntimeId, instance: TaskRuntimeInstanceId) -> Self {
        Self {
            runtime,
            instance,
            live: BTreeMap::new(),
            used_activations: BTreeSet::new(),
            used_storage_leases: BTreeSet::new(),
        }
    }

    pub fn records(&self) -> impl Iterator<Item = &TaskDependencyRecord> {
        self.live.values().map(|dependency| &dependency.record)
    }

    pub fn accept_activation(
        &mut self,
        selection: &ValidatedExecutorSelection,
        activation: ActivationInstanceId,
        storage: TaskStorageBinding,
    ) -> Result<TaskLifecycleClaim, TaskPlanDiagnostic> {
        if selection.candidate().runtime != self.runtime
            || selection.candidate().runtime_instance != self.instance
        {
            return Err(TaskPlanDiagnostic(
                "task activation selection belongs to a different runtime instance".into(),
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
            TaskStorageBinding::InlineCompletion => {}
        }

        let claim = TaskLifecycleClaimId(fingerprint_task_claim(selection, activation, storage));
        if self.live.contains_key(&claim) {
            return Err(TaskPlanDiagnostic(
                "normalized task lifecycle claim identity collides with a live claim".into(),
            ));
        }
        let record = TaskDependencyRecord {
            claim,
            runtime: self.runtime,
            runtime_instance: self.instance,
            activation_plan: selection.plan().normalized_identity(),
            executor_selection: selection.identity(),
            activation,
            storage,
        };
        self.used_activations.insert(activation);
        if let TaskStorageBinding::Persistent(provenance) = storage {
            self.used_storage_leases.insert(provenance);
        }
        let dependency = LiveTaskDependency {
            record,
            selection: Box::new(selection.clone()),
        };
        self.live.insert(claim, dependency.clone());
        Ok(TaskLifecycleClaim { dependency })
    }

    /// Cancellation requests preserve the lifecycle obligation. This check is
    /// intentionally read-only: only terminal settlement removes the record.
    pub fn validate_cancellation_request(
        &self,
        claim: &TaskLifecycleClaim,
    ) -> Result<(), TaskPlanDiagnostic> {
        if self.live.get(&claim.dependency.record.claim) == Some(&claim.dependency) {
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
        if self.live.get(&claim.dependency.record.claim) != Some(&claim.dependency) {
            return Err(TaskSettlementError {
                claim,
                diagnostic: TaskPlanDiagnostic(
                    "task settlement requires the exact live runtime/activation/storage claim"
                        .into(),
                ),
            });
        }
        self.live.remove(&claim.dependency.record.claim);
        Ok(SettledTaskLifecycle {
            record: claim.dependency.record,
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
            .any(|dependency| dependency.record.storage == TaskStorageBinding::Persistent(storage))
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
    fingerprint.word(plan.stack_plan.bytes);
    fingerprint.word(plan.stack_plan.alignment);
    fingerprint.word(plan.stack_plan.representation.normalized_identity());
    fingerprint.flag(plan.may_suspend);
    fingerprint.flag(plan.may_block);
    fingerprint.word(plan.canonical_suspension_crossings.len() as u64);
    for crossing in &plan.canonical_suspension_crossings {
        fingerprint.word(crossing.identity.normalized_identity());
        fingerprint.flag(crossing.suspension_allowed);
        fingerprint.flag(crossing.preserve_cpu);
        fingerprint.flag(crossing.preserve_host_thread);
    }
    fingerprint.flag(plan.carry_obligations.preserve_cpu);
    fingerprint.flag(plan.carry_obligations.preserve_host_thread);
    fingerprint.flag(plan.cancellation_required);
    fingerprint.finish()
}

fn fingerprint_executor_selection(
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

fn fingerprint_task_claim(
    selection: &ValidatedExecutorSelection,
    activation: ActivationInstanceId,
    storage: TaskStorageBinding,
) -> u64 {
    let mut fingerprint = Fingerprint::new();
    fingerprint.word(selection.identity().normalized_identity());
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
            stack_plan: StackPlan {
                bytes: 4096,
                alignment: 16,
                representation: id(6, StackRepresentationId::from_normalized_identity),
            },
            may_suspend: true,
            may_block: false,
            canonical_suspension_crossings: vec![CanonicalSuspensionCrossing {
                identity: id(7, SuspensionCrossingId::from_normalized_identity),
                suspension_allowed: true,
                preserve_cpu: true,
                preserve_host_thread: false,
            }],
            carry_obligations: ActivationCarryObligations {
                preserve_cpu: true,
                preserve_host_thread: false,
            },
            cancellation_required: true,
        }
    }

    fn runtime() -> TaskRuntimeId {
        id(80, TaskRuntimeId::from_normalized_identity)
    }

    fn executor_selection(
        plan: &ValidatedActivationPlan,
        instance: TaskRuntimeInstanceId,
    ) -> ValidatedExecutorSelection {
        validate_executor_selection(
            plan,
            ExecutorSelectionCandidate {
                runtime: runtime(),
                runtime_instance: instance,
                preservation: vec![ExecutorPreservationEvidence::new(
                    ExecutorPreservationAxis::Cpu,
                    id(81, ExecutorPreservationEvidenceId::from_normalized_identity),
                )],
            },
        )
        .expect("matching executor selection")
    }

    #[test]
    fn fixed_stack_and_canonical_crossings_form_a_valid_plan() {
        let plan = validate_activation_plan(candidate()).expect("activation plan");
        assert_eq!(plan.candidate().stack_plan.bytes, 4096);
        assert!(plan.candidate().carry_obligations.preserve_cpu);
        assert_ne!(plan.normalized_identity().normalized_identity(), 0);
    }

    #[test]
    fn stack_and_crossing_validation_fail_closed() {
        let mut zero_stack = candidate();
        zero_stack.stack_plan.bytes = 0;
        assert!(
            validate_activation_plan(zero_stack)
                .expect_err("zero stack")
                .0
                .contains("stack size")
        );

        let mut missing_crossing = candidate();
        missing_crossing.canonical_suspension_crossings.clear();
        missing_crossing.carry_obligations = ActivationCarryObligations::none();
        assert!(
            validate_activation_plan(missing_crossing)
                .expect_err("missing crossing")
                .0
                .contains("no canonical")
        );

        let mut unsafe_crossing = candidate();
        unsafe_crossing.canonical_suspension_crossings[0].suspension_allowed = false;
        assert!(
            validate_activation_plan(unsafe_crossing)
                .expect_err("unsafe crossing")
                .0
                .contains("forbids suspension")
        );

        let mut understated = candidate();
        understated.carry_obligations.preserve_cpu = false;
        assert!(
            validate_activation_plan(understated)
                .expect_err("understated preservation")
                .0
                .contains("understates")
        );
    }

    #[test]
    fn normalized_plan_identity_binds_stack_crossings_and_preservation() {
        let plan = validate_activation_plan(candidate()).expect("activation plan");

        let mut changed_stack = candidate();
        changed_stack.stack_plan.bytes += 1;
        assert_ne!(
            plan.normalized_identity(),
            validate_activation_plan(changed_stack)
                .expect("changed stack")
                .normalized_identity()
        );

        let mut changed_crossing = candidate();
        changed_crossing.canonical_suspension_crossings[0].preserve_host_thread = true;
        changed_crossing.carry_obligations.preserve_host_thread = true;
        assert_ne!(
            plan.normalized_identity(),
            validate_activation_plan(changed_crossing)
                .expect("changed crossing")
                .normalized_identity()
        );
    }

    #[test]
    fn incompatible_affinity_executor_selection_rejects() {
        let plan = validate_activation_plan(candidate()).expect("activation plan");
        let diagnostic = validate_executor_selection(
            &plan,
            ExecutorSelectionCandidate {
                runtime: runtime(),
                runtime_instance: id(90, TaskRuntimeInstanceId::from_normalized_identity),
                preservation: vec![ExecutorPreservationEvidence::new(
                    ExecutorPreservationAxis::HostThread,
                    id(91, ExecutorPreservationEvidenceId::from_normalized_identity),
                )],
            },
        )
        .expect_err("host-thread evidence cannot satisfy a CPU-pinned activation");

        assert!(
            diagnostic
                .0
                .contains("selected executor does not establish CPU preservation")
        );
    }

    #[test]
    fn executor_selection_requires_only_the_activation_affinity_axes() {
        let mut portable = candidate();
        portable.canonical_suspension_crossings[0].preserve_cpu = false;
        portable.carry_obligations = ActivationCarryObligations::none();
        let plan = validate_activation_plan(portable).expect("portable activation plan");
        let instance = id(92, TaskRuntimeInstanceId::from_normalized_identity);
        let selection = validate_executor_selection(
            &plan,
            ExecutorSelectionCandidate {
                runtime: runtime(),
                runtime_instance: instance,
                preservation: Vec::new(),
            },
        )
        .expect("portable activation needs no affinity evidence");

        assert_eq!(selection.candidate().runtime_instance, instance);
        assert_eq!(selection.plan(), &plan);
        assert_ne!(selection.identity().normalized_identity(), 0);
    }

    #[test]
    fn lifecycle_claim_pins_runtime_plan_and_storage_until_settlement() {
        let plan = validate_activation_plan(candidate()).expect("activation plan");
        let instance = id(100, TaskRuntimeInstanceId::from_normalized_identity);
        let activation = id(101, ActivationInstanceId::from_normalized_identity);
        let storage = TaskStorageProvenance {
            owner: id(102, TaskStorageOwnerId::from_normalized_identity),
            lease: id(103, TaskStorageLeaseId::from_normalized_identity),
        };
        let selection = executor_selection(&plan, instance);
        let mut ledger = TaskLifecycleLedger::new(runtime(), instance);
        let claim = ledger
            .accept_activation(
                &selection,
                activation,
                TaskStorageBinding::Persistent(storage),
            )
            .expect("accepted activation");

        let record = ledger.records().next().expect("one live dependency");
        assert_eq!(record.executor_selection, selection.identity());
        ledger
            .validate_cancellation_request(&claim)
            .expect("cancellation preserves the claim");
        assert!(ledger.validate_storage_reclaim(storage).is_err());
        let close = ledger.close().expect_err("live child blocks runtime close");
        let mut ledger = close.into_ledger();
        let settled = ledger.settle(claim).expect("terminal settlement");
        assert_eq!(
            settled.released_storage(),
            TaskStorageBinding::Persistent(storage)
        );
        ledger
            .validate_storage_reclaim(storage)
            .expect("settlement releases storage");
        assert_eq!(ledger.close().expect("runtime closes").runtime(), runtime());
    }

    #[test]
    fn lifecycle_rejects_replayed_activation_and_storage_eras() {
        let plan = validate_activation_plan(candidate()).expect("activation plan");
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
        let selection = executor_selection(&plan, instance);
        let mut ledger = TaskLifecycleLedger::new(runtime(), instance);
        let claim = ledger
            .accept_activation(
                &selection,
                first_activation,
                TaskStorageBinding::Persistent(storage),
            )
            .expect("first activation");

        assert!(
            ledger
                .accept_activation(
                    &selection,
                    first_activation,
                    TaskStorageBinding::Persistent(alternate_storage),
                )
                .expect_err("activation replay")
                .0
                .contains("already been accepted")
        );
        assert!(
            ledger
                .accept_activation(
                    &selection,
                    second_activation,
                    TaskStorageBinding::Persistent(storage),
                )
                .expect_err("lease replay")
                .0
                .contains("new lease era")
        );
        ledger.settle(claim).expect("settle first activation");
        assert!(
            ledger
                .accept_activation(
                    &selection,
                    second_activation,
                    TaskStorageBinding::Persistent(storage),
                )
                .is_err()
        );
    }

    #[test]
    fn failed_cross_runtime_settlement_returns_the_linear_claim() {
        let plan = validate_activation_plan(candidate()).expect("activation plan");
        let owner_instance = id(120, TaskRuntimeInstanceId::from_normalized_identity);
        let mut owner = TaskLifecycleLedger::new(runtime(), owner_instance);
        let mut wrong_instance = TaskLifecycleLedger::new(
            runtime(),
            id(121, TaskRuntimeInstanceId::from_normalized_identity),
        );
        let selection = executor_selection(&plan, owner_instance);
        let claim = owner
            .accept_activation(
                &selection,
                id(122, ActivationInstanceId::from_normalized_identity),
                TaskStorageBinding::InlineCompletion,
            )
            .expect("accepted activation");
        let error = wrong_instance
            .settle(claim)
            .expect_err("another runtime instance cannot settle this claim");
        owner
            .settle(error.into_claim())
            .expect("failed settlement preserves the claim");
    }
}

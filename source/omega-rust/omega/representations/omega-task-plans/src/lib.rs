//! Provider-independent task activation plans and lifecycle accounting.
//!
//! An activation plan describes one fixed, nonmoving stack, the canonical
//! semantic suspension crossings, and only the CPU/thread preservation those
//! crossings demand. Executor selection consumes exact per-axis checked or
//! admitted evidence; this crate deliberately does not publish a generalized
//! runtime behavior record.

use std::collections::{BTreeMap, BTreeSet};

mod wcsu;
pub use wcsu::{
    AdmittedSameStackContribution, ComposedTaskStackDemand,
    SameStackContributionAdmissionCandidate, SameStackContributionCommitment,
    SameStackProviderPlanCommitment, StackCallContribution, TaskStackFrameSummary,
    ValidatedTaskStackFrameSummary, WcsuStackPlanProjection, admit_same_stack_contribution,
    compose_task_stack_demand, project_wcsu_stack_plan, validate_task_stack_frame_summary,
};

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
normalized_id!(SuspensionCrossingId, "suspension-crossing");
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
normalized_id!(TaskLifecycleClaimId, "task-lifecycle-claim");

/// Physical fixed-stack shape retained by the existing activation sidecar.
///
/// This three-field carrier is not WCSU admission. The compiler's current
/// local layout bridge still produces it while whole-call-graph collection is
/// incomplete. New foundation work must obtain the same shape from
/// [`project_wcsu_stack_plan`] and validate it with
/// [`validate_wcsu_activation_plan`].
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
pub struct ValidatedActivationPlan {
    candidate: ActivationPlanCandidate,
    wcsu_stack_projection: Option<WcsuStackPlanProjection>,
}

impl ValidatedActivationPlan {
    pub const fn candidate(&self) -> &ActivationPlanCandidate {
        &self.candidate
    }

    /// Exact whole-call-graph evidence used to produce the stack shape.
    ///
    /// `None` identifies the temporary compiler-local layout bridge. It is
    /// deliberately not upgraded into WCSU evidence by activation validation.
    pub const fn wcsu_stack_projection(&self) -> Option<&WcsuStackPlanProjection> {
        self.wcsu_stack_projection.as_ref()
    }

    /// Normalized identity of the complete provider-independent plan.
    pub fn normalized_identity(&self) -> ActivationPlanId {
        ActivationPlanId(activation_plan_report_fingerprint(
            &self.candidate,
            self.wcsu_stack_projection.as_ref(),
        ))
    }
}

pub fn validate_activation_plan(
    candidate: ActivationPlanCandidate,
) -> Result<ValidatedActivationPlan, TaskPlanDiagnostic> {
    validate_activation_plan_shape(&candidate)?;
    Ok(ValidatedActivationPlan {
        candidate,
        wcsu_stack_projection: None,
    })
}

/// Validate an activation whose fixed-stack shape is projected from sealed
/// whole-call-graph WCSU evidence.
///
/// The projection must retain its exact normalized identity and must reproduce
/// every public shape field. Supplying an unrelated byte/alignment tuple or a
/// different representation therefore fails rather than inheriting the
/// composition's authority.
pub fn validate_wcsu_activation_plan(
    candidate: ActivationPlanCandidate,
    projection: WcsuStackPlanProjection,
) -> Result<ValidatedActivationPlan, TaskPlanDiagnostic> {
    validate_activation_plan_shape(&candidate)?;
    if !projection.has_valid_identity() {
        return Err(TaskPlanDiagnostic(
            "WCSU stack-plan projection identity does not match its retained composition evidence"
                .into(),
        ));
    }
    if candidate.stack_plan != projection.stack_plan() {
        return Err(TaskPlanDiagnostic(
            "activation stack shape does not exactly match its WCSU stack-plan projection".into(),
        ));
    }
    Ok(ValidatedActivationPlan {
        candidate,
        wcsu_stack_projection: Some(projection),
    })
}

fn validate_activation_plan_shape(
    candidate: &ActivationPlanCandidate,
) -> Result<(), TaskPlanDiagnostic> {
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
    Ok(())
}

/// Which ordinary `TaskRuntime` operation requested one concrete activation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStartOperation {
    Start,
    TryStart,
}

/// Domain-separated SHA-256 commitment to the exact checked TaskRuntime
/// requirement, operation, target entry, and target contract selected by one
/// task activation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TaskSpecializationCommitment([u8; 32]);

impl TaskSpecializationCommitment {
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

/// Exact selected runtime evidence paired with one Omega activation plan.
/// This is post-check provider realization state, not part of Psi checked
/// semantics.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedTaskRuntimeProviderFact {
    pub runtime: TaskRuntimeId,
    pub provider_plan_name: String,
    pub requirement_identity: String,
}

/// One target/layout-specific task activation elaborated by Omega after Psi
/// semantic checking.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskActivationPlanFact {
    pub start_requirement: psi_symbols::SymbolHandle,
    pub target_machine: psi_symbols::SymbolHandle,
    pub target_entry: psi_symbols::SymbolHandle,
    /// Historical compact compatibility/report coordinate.
    pub specialization_report_fingerprint: u64,
    /// Strong identity of the exact checked specialization structure.
    pub specialization_commitment: TaskSpecializationCommitment,
    pub operation: TaskStartOperation,
    pub selected_runtime: SelectedTaskRuntimeProviderFact,
    pub plan: ValidatedActivationPlan,
}

/// Omega-owned task activation sidecar for one checked compilation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TaskActivationPlanSet {
    pub activations: Vec<TaskActivationPlanFact>,
}

impl TaskActivationPlanSet {
    pub fn as_slice(&self) -> &[TaskActivationPlanFact] {
        &self.activations
    }

    pub fn is_empty(&self) -> bool {
        self.activations.is_empty()
    }
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
        identity: ExecutorSelectionId(executor_selection_report_fingerprint(plan, &candidate)),
        candidate,
        plan: plan.clone(),
    })
}

/// Provider-authored evidence for one dynamic invocation of an already
/// selected `TaskRuntime::{start,try_start}<M>` specialization.
///
/// This is deliberately a candidate rather than authority by construction.
/// Validation binds every copied field back to the Omega-owned static
/// activation fact before lifecycle accounting may consume the receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskRuntimeInvocationReceiptCandidate {
    pub receipt: TaskRuntimeInvocationReceiptId,
    pub invocation: TaskRuntimeInvocationId,
    pub runtime: TaskRuntimeId,
    pub runtime_instance: TaskRuntimeInstanceId,
    pub operation: TaskStartOperation,
    pub provider_plan_name: String,
    pub requirement_identity: String,
    pub activation_plan: ActivationPlanId,
    pub preservation: Vec<ExecutorPreservationEvidence>,
}

/// Normalized projection of the static activation fact retained at runtime.
/// Checked-tree symbol handles remain in the compilation sidecar; runtime
/// accounting needs only the specialization, operation, provider, and plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskRuntimeActivationBinding {
    pub specialization_report_fingerprint: u64,
    pub specialization_commitment: TaskSpecializationCommitment,
    pub operation: TaskStartOperation,
    pub selected_runtime: SelectedTaskRuntimeProviderFact,
    pub activation_plan: ActivationPlanId,
}

/// Exact static/dynamic binding retained for one provider-accepted task start.
/// The retained carrier is independent of checked-tree symbol handles: only
/// normalized Omega realization identity crosses into runtime accounting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedTaskRuntimeInvocationReceipt {
    identity: TaskRuntimeInvocationBindingId,
    candidate: TaskRuntimeInvocationReceiptCandidate,
    activation: TaskRuntimeActivationBinding,
    executor_selection: ValidatedExecutorSelection,
}

impl ValidatedTaskRuntimeInvocationReceipt {
    pub const fn identity(&self) -> TaskRuntimeInvocationBindingId {
        self.identity
    }

    pub const fn candidate(&self) -> &TaskRuntimeInvocationReceiptCandidate {
        &self.candidate
    }

    pub const fn activation(&self) -> &TaskRuntimeActivationBinding {
        &self.activation
    }

    pub const fn executor_selection(&self) -> &ValidatedExecutorSelection {
        &self.executor_selection
    }
}

/// Bind a provider invocation receipt to one exact post-check activation fact.
/// Static provider/requirement/operation drift rejects before the normalized
/// lifecycle ledger can acquire operational or storage custody.
pub fn validate_task_runtime_invocation_receipt(
    activation: &TaskActivationPlanFact,
    candidate: TaskRuntimeInvocationReceiptCandidate,
) -> Result<ValidatedTaskRuntimeInvocationReceipt, TaskPlanDiagnostic> {
    if activation.specialization_commitment.is_zero() {
        return Err(TaskPlanDiagnostic(
            "task runtime activation has an empty specialization commitment".into(),
        ));
    }
    if candidate.runtime != activation.selected_runtime.runtime {
        return Err(TaskPlanDiagnostic(
            "task runtime invocation receipt names a different selected runtime".into(),
        ));
    }
    if candidate.provider_plan_name != activation.selected_runtime.provider_plan_name {
        return Err(TaskPlanDiagnostic(
            "task runtime invocation receipt names a different selected provider plan".into(),
        ));
    }
    if candidate.requirement_identity != activation.selected_runtime.requirement_identity {
        return Err(TaskPlanDiagnostic(
            "task runtime invocation receipt names a different start requirement".into(),
        ));
    }
    if candidate.operation != activation.operation {
        return Err(TaskPlanDiagnostic(
            "task runtime invocation receipt names a different start operation".into(),
        ));
    }
    if candidate.activation_plan != activation.plan.normalized_identity() {
        return Err(TaskPlanDiagnostic(
            "task runtime invocation receipt names a different activation plan".into(),
        ));
    }
    let executor_selection = validate_executor_selection(
        &activation.plan,
        ExecutorSelectionCandidate {
            runtime: candidate.runtime,
            runtime_instance: candidate.runtime_instance,
            preservation: candidate.preservation.clone(),
        },
    )?;
    Ok(ValidatedTaskRuntimeInvocationReceipt {
        identity: TaskRuntimeInvocationBindingId(runtime_invocation_report_fingerprint(
            activation,
            &candidate,
            &executor_selection,
        )),
        candidate,
        activation: TaskRuntimeActivationBinding {
            specialization_report_fingerprint: activation.specialization_report_fingerprint,
            specialization_commitment: activation.specialization_commitment,
            operation: activation.operation,
            selected_runtime: activation.selected_runtime.clone(),
            activation_plan: activation.plan.normalized_identity(),
        },
        executor_selection,
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
    pub invocation_binding: TaskRuntimeInvocationBindingId,
    pub invocation: TaskRuntimeInvocationId,
    pub invocation_receipt: TaskRuntimeInvocationReceiptId,
    pub operation: TaskStartOperation,
    pub activation: ActivationInstanceId,
    pub storage: TaskStorageBinding,
}

/// Source-level `Task<T>` is linear. This normalized carrier mirrors that
/// property by withholding `Clone`/`Copy` and exposing no public constructor.
#[derive(Debug, PartialEq, Eq)]
pub struct TaskLifecycleClaim {
    dependency: Box<LiveTaskDependency>,
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

/// Exact validated provider invocation retained behind one live lifecycle
/// claim, including its selected executor and activation plan.
///
/// `TaskDependencyRecord` remains the compact report form. Custody and
/// settlement compare this carrier so compact identity collisions cannot move
/// a claim between distinct invocations or activation plans.
#[derive(Debug, Clone, PartialEq, Eq)]
struct LiveTaskDependency {
    record: TaskDependencyRecord,
    invocation: Box<ValidatedTaskRuntimeInvocationReceipt>,
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
    ledger: Box<TaskLifecycleLedger>,
    diagnostic: TaskPlanDiagnostic,
}

impl TaskRuntimeCloseError {
    pub const fn diagnostic(&self) -> &TaskPlanDiagnostic {
        &self.diagnostic
    }

    pub fn into_ledger(self) -> TaskLifecycleLedger {
        *self.ledger
    }
}

/// Provider-local accounting for operational custody, physical storage, and
/// the independently held lifecycle claim.
///
/// A selected runtime instance has one ledger. Recording an already accepted
/// invocation binds its exact static provider/operation selection, dynamic
/// receipt, activation plan, and storage dependency before a `Task<T>` claim
/// is issued. Terminal settlement removes that dependency. Runtime close and
/// storage reclamation fail while any matching child claim remains live.
#[derive(Debug)]
pub struct TaskLifecycleLedger {
    runtime: TaskRuntimeId,
    instance: TaskRuntimeInstanceId,
    live: BTreeMap<TaskLifecycleClaimId, LiveTaskDependency>,
    used_activations: BTreeSet<ActivationInstanceId>,
    used_invocations: BTreeSet<TaskRuntimeInvocationId>,
    used_invocation_receipts: BTreeSet<TaskRuntimeInvocationReceiptId>,
    used_storage_leases: BTreeSet<TaskStorageProvenance>,
}

impl TaskLifecycleLedger {
    /// Create empty accounting for one runtime instance. Each accepted start
    /// must still supply its exact validated provider invocation receipt.
    pub fn new(runtime: TaskRuntimeId, instance: TaskRuntimeInstanceId) -> Self {
        Self {
            runtime,
            instance,
            live: BTreeMap::new(),
            used_activations: BTreeSet::new(),
            used_invocations: BTreeSet::new(),
            used_invocation_receipts: BTreeSet::new(),
            used_storage_leases: BTreeSet::new(),
        }
    }

    pub fn records(&self) -> impl Iterator<Item = &TaskDependencyRecord> {
        self.live.values().map(|dependency| &dependency.record)
    }

    pub fn accept_invocation(
        &mut self,
        invocation: &ValidatedTaskRuntimeInvocationReceipt,
        activation: ActivationInstanceId,
        storage: TaskStorageBinding,
    ) -> Result<TaskLifecycleClaim, TaskPlanDiagnostic> {
        if invocation.candidate().runtime != self.runtime
            || invocation.candidate().runtime_instance != self.instance
        {
            return Err(TaskPlanDiagnostic(
                "task invocation receipt belongs to a different runtime instance".into(),
            ));
        }
        if self
            .used_invocations
            .contains(&invocation.candidate().invocation)
        {
            return Err(TaskPlanDiagnostic(
                "task runtime invocation identity has already been accepted by this runtime instance"
                    .into(),
            ));
        }
        if self
            .used_invocation_receipts
            .contains(&invocation.candidate().receipt)
        {
            return Err(TaskPlanDiagnostic(
                "task runtime invocation receipt has already been accepted by this runtime instance"
                    .into(),
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

        let claim = TaskLifecycleClaimId(task_claim_report_fingerprint(
            invocation, activation, storage,
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
            activation_plan: invocation.activation().activation_plan,
            executor_selection: invocation.executor_selection().identity(),
            invocation_binding: invocation.identity(),
            invocation: invocation.candidate().invocation,
            invocation_receipt: invocation.candidate().receipt,
            operation: invocation.candidate().operation,
            activation,
            storage,
        };
        self.used_invocations
            .insert(invocation.candidate().invocation);
        self.used_invocation_receipts
            .insert(invocation.candidate().receipt);
        self.used_activations.insert(activation);
        if let TaskStorageBinding::Persistent(provenance) = storage {
            self.used_storage_leases.insert(provenance);
        }
        let dependency = LiveTaskDependency {
            record,
            invocation: Box::new(invocation.clone()),
        };
        self.live.insert(claim, dependency.clone());
        Ok(TaskLifecycleClaim {
            dependency: Box::new(dependency),
        })
    }

    /// Cancellation requests preserve the lifecycle obligation. This check is
    /// intentionally read-only: only terminal settlement removes the record.
    pub fn validate_cancellation_request(
        &self,
        claim: &TaskLifecycleClaim,
    ) -> Result<(), TaskPlanDiagnostic> {
        if self.live.get(&claim.dependency.record.claim) == Some(claim.dependency.as_ref()) {
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
        if self.live.get(&claim.dependency.record.claim) != Some(claim.dependency.as_ref()) {
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
                ledger: Box::new(self),
            });
        }
        Ok(ClosedTaskRuntime {
            runtime: self.runtime,
            instance: self.instance,
        })
    }
}

fn activation_plan_report_fingerprint(
    plan: &ActivationPlanCandidate,
    wcsu_stack_projection: Option<&WcsuStackPlanProjection>,
) -> u64 {
    let mut fingerprint = Fingerprint::new();
    fingerprint.word(plan.machine_contract.normalized_identity());
    fingerprint.word(plan.entry.normalized_identity());
    fingerprint.word(plan.argument_layout.normalized_identity());
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

fn executor_selection_report_fingerprint(
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

fn task_claim_report_fingerprint(
    invocation: &ValidatedTaskRuntimeInvocationReceipt,
    activation: ActivationInstanceId,
    storage: TaskStorageBinding,
) -> u64 {
    let mut fingerprint = Fingerprint::new();
    fingerprint.word(invocation.identity().normalized_identity());
    fingerprint.word(invocation.candidate().invocation.normalized_identity());
    fingerprint.word(invocation.candidate().receipt.normalized_identity());
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

fn runtime_invocation_report_fingerprint(
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

    fn wcsu_projection(validation_identity: u64) -> WcsuStackPlanProjection {
        let root = id(30, TaskStackFrameId::from_normalized_identity);
        let frame = validate_task_stack_frame_summary(TaskStackFrameSummary {
            frame: root,
            local_bytes: 4096,
            alignment: 16,
            validation: id(
                validation_identity,
                TaskStackFrameValidationId::from_normalized_identity,
            ),
            calls: Vec::new(),
        })
        .expect("validated WCSU frame");
        let demand = compose_task_stack_demand(root, [frame]).expect("composed WCSU demand");
        project_wcsu_stack_plan(
            &demand,
            id(6, StackRepresentationId::from_normalized_identity),
        )
    }

    fn runtime() -> TaskRuntimeId {
        id(80, TaskRuntimeId::from_normalized_identity)
    }

    fn activation_fact(plan: &ValidatedActivationPlan) -> TaskActivationPlanFact {
        TaskActivationPlanFact {
            start_requirement: psi_symbols::SymbolHandle::invalid(),
            target_machine: psi_symbols::SymbolHandle::invalid(),
            target_entry: psi_symbols::SymbolHandle::invalid(),
            specialization_report_fingerprint: 79,
            specialization_commitment: TaskSpecializationCommitment::from_digest([7; 32]),
            operation: TaskStartOperation::Start,
            selected_runtime: SelectedTaskRuntimeProviderFact {
                runtime: runtime(),
                provider_plan_name: "LocalTaskRuntime::satisfies::TaskRuntime".into(),
                requirement_identity: "TaskRuntime::start".into(),
            },
            plan: plan.clone(),
        }
    }

    fn invocation_receipt(
        plan: &ValidatedActivationPlan,
        instance: TaskRuntimeInstanceId,
        invocation: u64,
        receipt: u64,
    ) -> ValidatedTaskRuntimeInvocationReceipt {
        let activation = activation_fact(plan);
        validate_task_runtime_invocation_receipt(
            &activation,
            TaskRuntimeInvocationReceiptCandidate {
                receipt: id(
                    receipt,
                    TaskRuntimeInvocationReceiptId::from_normalized_identity,
                ),
                invocation: id(
                    invocation,
                    TaskRuntimeInvocationId::from_normalized_identity,
                ),
                runtime: runtime(),
                runtime_instance: instance,
                operation: TaskStartOperation::Start,
                provider_plan_name: activation.selected_runtime.provider_plan_name.clone(),
                requirement_identity: activation.selected_runtime.requirement_identity.clone(),
                activation_plan: plan.normalized_identity(),
                preservation: vec![ExecutorPreservationEvidence::new(
                    ExecutorPreservationAxis::Cpu,
                    id(81, ExecutorPreservationEvidenceId::from_normalized_identity),
                )],
            },
        )
        .expect("matching task runtime invocation receipt")
    }

    #[test]
    fn fixed_stack_and_canonical_crossings_form_a_valid_plan() {
        let plan = validate_activation_plan(candidate()).expect("activation plan");
        assert_eq!(plan.candidate().stack_plan.bytes, 4096);
        assert_eq!(plan.wcsu_stack_projection(), None);
        assert!(plan.candidate().carry_obligations.preserve_cpu);
        assert_ne!(plan.normalized_identity().normalized_identity(), 0);
    }

    #[test]
    fn wcsu_projection_validates_exact_activation_stack_shape() {
        let projection = wcsu_projection(31);
        let projection_identity = projection.identity();
        let mut exact = candidate();
        exact.stack_plan = projection.stack_plan();
        let plan = validate_wcsu_activation_plan(exact.clone(), projection)
            .expect("WCSU-backed activation plan");

        assert_eq!(
            plan.wcsu_stack_projection().map(|value| value.identity()),
            Some(projection_identity)
        );

        let mut changed_bytes = exact.clone();
        changed_bytes.stack_plan.bytes += 1;
        assert!(
            validate_wcsu_activation_plan(changed_bytes, wcsu_projection(31))
                .expect_err("byte substitution")
                .0
                .contains("does not exactly match")
        );

        let mut changed_representation = exact;
        changed_representation.stack_plan.representation =
            id(32, StackRepresentationId::from_normalized_identity);
        assert!(
            validate_wcsu_activation_plan(changed_representation, wcsu_projection(31))
                .expect_err("representation substitution")
                .0
                .contains("does not exactly match")
        );
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
    fn activation_identity_binds_exact_wcsu_projection_identity() {
        let first_projection = wcsu_projection(40);
        let second_projection = wcsu_projection(41);
        assert_eq!(
            first_projection.stack_plan(),
            second_projection.stack_plan()
        );
        assert_ne!(first_projection.identity(), second_projection.identity());

        let mut first_candidate = candidate();
        first_candidate.stack_plan = first_projection.stack_plan();
        let mut second_candidate = candidate();
        second_candidate.stack_plan = second_projection.stack_plan();
        let first = validate_wcsu_activation_plan(first_candidate, first_projection)
            .expect("first WCSU-backed activation");
        let second = validate_wcsu_activation_plan(second_candidate, second_projection)
            .expect("second WCSU-backed activation");

        assert_ne!(first.normalized_identity(), second.normalized_identity());
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
    fn invocation_receipt_binds_the_selected_provider_operation_and_plan() {
        let plan = validate_activation_plan(candidate()).expect("activation plan");
        let activation = activation_fact(&plan);
        let instance = id(93, TaskRuntimeInstanceId::from_normalized_identity);
        let base = TaskRuntimeInvocationReceiptCandidate {
            receipt: id(94, TaskRuntimeInvocationReceiptId::from_normalized_identity),
            invocation: id(95, TaskRuntimeInvocationId::from_normalized_identity),
            runtime: runtime(),
            runtime_instance: instance,
            operation: TaskStartOperation::Start,
            provider_plan_name: activation.selected_runtime.provider_plan_name.clone(),
            requirement_identity: activation.selected_runtime.requirement_identity.clone(),
            activation_plan: plan.normalized_identity(),
            preservation: vec![ExecutorPreservationEvidence::new(
                ExecutorPreservationAxis::Cpu,
                id(96, ExecutorPreservationEvidenceId::from_normalized_identity),
            )],
        };
        let validated = validate_task_runtime_invocation_receipt(&activation, base.clone())
            .expect("exact invocation receipt");
        assert_eq!(validated.candidate(), &base);
        assert_eq!(
            validated.activation().specialization_report_fingerprint,
            activation.specialization_report_fingerprint
        );
        assert_eq!(
            validated.activation().specialization_commitment,
            activation.specialization_commitment
        );
        assert_eq!(
            validated.activation().selected_runtime,
            activation.selected_runtime
        );
        assert_eq!(
            validated.activation().activation_plan,
            plan.normalized_identity()
        );
        assert_ne!(validated.identity().normalized_identity(), 0);
        assert_eq!(
            validated.executor_selection().candidate().runtime_instance,
            instance
        );

        let mut wrong_provider = base.clone();
        wrong_provider.provider_plan_name = "OtherRuntime".into();
        assert!(
            validate_task_runtime_invocation_receipt(&activation, wrong_provider)
                .expect_err("provider drift")
                .0
                .contains("different selected provider plan")
        );

        let mut wrong_operation = base.clone();
        wrong_operation.operation = TaskStartOperation::TryStart;
        assert!(
            validate_task_runtime_invocation_receipt(&activation, wrong_operation)
                .expect_err("operation drift")
                .0
                .contains("different start operation")
        );

        let mut wrong_plan = base;
        let mut changed = candidate();
        changed.stack_plan.bytes += 16;
        wrong_plan.activation_plan = validate_activation_plan(changed)
            .expect("changed plan")
            .normalized_identity();
        assert!(
            validate_task_runtime_invocation_receipt(&activation, wrong_plan)
                .expect_err("plan drift")
                .0
                .contains("different activation plan")
        );
    }

    #[test]
    fn compact_equal_specialization_commitments_bind_distinct_runtime_invocations() {
        let plan = validate_activation_plan(candidate()).expect("activation plan");
        let first = activation_fact(&plan);
        let mut substituted = first.clone();
        substituted.specialization_commitment = TaskSpecializationCommitment::from_digest([8; 32]);
        assert_eq!(
            first.specialization_report_fingerprint,
            substituted.specialization_report_fingerprint
        );
        let candidate = TaskRuntimeInvocationReceiptCandidate {
            receipt: id(
                194,
                TaskRuntimeInvocationReceiptId::from_normalized_identity,
            ),
            invocation: id(195, TaskRuntimeInvocationId::from_normalized_identity),
            runtime: runtime(),
            runtime_instance: id(193, TaskRuntimeInstanceId::from_normalized_identity),
            operation: TaskStartOperation::Start,
            provider_plan_name: first.selected_runtime.provider_plan_name.clone(),
            requirement_identity: first.selected_runtime.requirement_identity.clone(),
            activation_plan: plan.normalized_identity(),
            preservation: vec![ExecutorPreservationEvidence::new(
                ExecutorPreservationAxis::Cpu,
                id(
                    196,
                    ExecutorPreservationEvidenceId::from_normalized_identity,
                ),
            )],
        };
        let first_validated = validate_task_runtime_invocation_receipt(&first, candidate.clone())
            .expect("first exact specialization");
        let substituted_validated =
            validate_task_runtime_invocation_receipt(&substituted, candidate)
                .expect("substituted exact specialization");
        assert_ne!(first_validated.identity(), substituted_validated.identity());
        assert_ne!(
            first_validated.activation().specialization_commitment,
            substituted_validated.activation().specialization_commitment
        );

        let mut report_only = first.clone();
        report_only.specialization_report_fingerprint ^= 1;
        let report_only_validated = validate_task_runtime_invocation_receipt(
            &report_only,
            first_validated.candidate().clone(),
        )
        .expect("report-only coordinate drift does not change authority");
        assert_eq!(first_validated.identity(), report_only_validated.identity());
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
        let receipt = invocation_receipt(&plan, instance, 104, 105);
        let mut ledger = TaskLifecycleLedger::new(runtime(), instance);
        let claim = ledger
            .accept_invocation(
                &receipt,
                activation,
                TaskStorageBinding::Persistent(storage),
            )
            .expect("accepted activation");

        let record = ledger.records().next().expect("one live dependency");
        assert_eq!(
            record.executor_selection,
            receipt.executor_selection().identity()
        );
        assert_eq!(record.invocation, receipt.candidate().invocation);
        assert_eq!(record.invocation_receipt, receipt.candidate().receipt);
        assert_eq!(record.invocation_binding, receipt.identity());
        assert_eq!(record.operation, TaskStartOperation::Start);
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
        let receipt = invocation_receipt(&plan, instance, 116, 117);
        let mut ledger = TaskLifecycleLedger::new(runtime(), instance);
        let claim = ledger
            .accept_invocation(
                &receipt,
                first_activation,
                TaskStorageBinding::Persistent(storage),
            )
            .expect("first activation");

        let replayed_activation_receipt = invocation_receipt(&plan, instance, 118, 119);
        assert!(
            ledger
                .accept_invocation(
                    &replayed_activation_receipt,
                    first_activation,
                    TaskStorageBinding::Persistent(alternate_storage),
                )
                .expect_err("activation replay")
                .0
                .contains("already been accepted")
        );
        let replayed_storage_receipt = invocation_receipt(&plan, instance, 120, 121);
        assert!(
            ledger
                .accept_invocation(
                    &replayed_storage_receipt,
                    second_activation,
                    TaskStorageBinding::Persistent(storage),
                )
                .expect_err("lease replay")
                .0
                .contains("new lease era")
        );
        ledger.settle(claim).expect("settle first activation");
        let post_settlement_receipt = invocation_receipt(&plan, instance, 122, 123);
        assert!(
            ledger
                .accept_invocation(
                    &post_settlement_receipt,
                    second_activation,
                    TaskStorageBinding::Persistent(storage),
                )
                .is_err()
        );
    }

    #[test]
    fn lifecycle_rejects_replayed_invocations_and_provider_receipts() {
        let plan = validate_activation_plan(candidate()).expect("activation plan");
        let instance = id(130, TaskRuntimeInstanceId::from_normalized_identity);
        let first = invocation_receipt(&plan, instance, 131, 132);
        let mut ledger = TaskLifecycleLedger::new(runtime(), instance);
        let claim = ledger
            .accept_invocation(
                &first,
                id(133, ActivationInstanceId::from_normalized_identity),
                TaskStorageBinding::InlineCompletion,
            )
            .expect("first invocation");

        assert!(
            ledger
                .accept_invocation(
                    &first,
                    id(134, ActivationInstanceId::from_normalized_identity),
                    TaskStorageBinding::InlineCompletion,
                )
                .expect_err("invocation replay")
                .0
                .contains("invocation identity has already been accepted")
        );

        let mut repeated_receipt = invocation_receipt(&plan, instance, 135, 136);
        repeated_receipt.candidate.receipt = first.candidate().receipt;
        assert!(
            ledger
                .accept_invocation(
                    &repeated_receipt,
                    id(137, ActivationInstanceId::from_normalized_identity),
                    TaskStorageBinding::InlineCompletion,
                )
                .expect_err("provider receipt replay")
                .0
                .contains("invocation receipt has already been accepted")
        );

        ledger.settle(claim).expect("settle first invocation");
        assert!(
            ledger
                .accept_invocation(
                    &first,
                    id(138, ActivationInstanceId::from_normalized_identity),
                    TaskStorageBinding::InlineCompletion,
                )
                .is_err(),
            "settlement does not make a provider invocation receipt replayable"
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
        let receipt = invocation_receipt(&plan, owner_instance, 124, 125);
        let claim = owner
            .accept_invocation(
                &receipt,
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

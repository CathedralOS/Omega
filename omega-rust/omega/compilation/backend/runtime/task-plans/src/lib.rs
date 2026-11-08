/*
We define provider-independent task activation plans (`ValidatedActivationPlan`,
`ActivationPlanCandidate`), their canonical packed argument marshalling layouts
(`TaskArgumentLayout`), suspension-crossing live frontiers
(`CanonicalSuspensionCrossing`, `LiveCarryDemand`), and whole-call-graph stack
composition (`stack_composition.rs`).

An activation plan describes one fixed, nonmoving stack, the canonical
suspension crossings—each retaining the exact live frontier of places, linear
claims, and four-axis carry demands—and the CPU/host-thread preservation those
crossings require. Plan validation (`validate_wcsu_activation_plan`) checks
that crossing identities are strictly ordered, no live place forbids
suspension, each crossing's preservation flags equal the exact join of its live
frontier, and the fixed stack shape matches a sealed `WcsuStackPlanProjection`.
*/

/*
Why require `CanonicalSuspensionCrossing` to carry the full `LiveCarryDemand`
frontier when its `preserve_cpu` and `preserve_host_thread` booleans already
summarize scheduler obligations? A parked continuation's soundness depends on
more than thread affinity: every live loan attached to a place must remain
alive and address-stable in its declared `LiveCarryStorage` class across the
park, and any drift between the frontier and the summary bits indicates
corrupted or fabricated evidence. Re-verifying the join at plan validation
ensures a candidate cannot publish a frontier that disagrees with the
preservation it declares.
*/

/*
Start in this file for the activation plan types, canonical argument packing,
and `TaskActivationPlanSet` sidecar records. `activation.rs` closes portable
`TerminalTaskActivationDemands` under a selected runtime and target profile
(`realize_task_activations`), `stack_composition.rs` owns acyclic same-stack
WCSU composition and unresolved-call tracking, and `identities.rs` defines the
non-zero report coordinates and 32-byte commitment wrappers.
*/

pub(crate) use optimization_core::digest_encoder;
mod activation;
mod identities;
mod source_guards;
mod stack_composition;

pub use activation::realize_task_activations;
pub use identities::{
    ActivationPlanId, AdmittedStackContributionReportId, CallingPlanId, CompositionModelId,
    ExecutorPreservationEvidenceId, ExecutorSelectionId, LiveCarryPlaceId, LiveCarryTypeId,
    MachineContractId, MachineEntryId, SameStackContributionAdmissionReceiptId,
    StackPlanProjectionId, StackRepresentationId, TaskRuntimeId, TaskRuntimeInstanceId,
    TaskRuntimeInvocationBindingId, TaskRuntimeInvocationId, TaskRuntimeInvocationReceiptId,
    TaskStackCompositionId, TaskStackFrameId, TaskStackFrameValidationId, ValueLayoutId,
};
pub use semantic_vocabulary::{ClaimId, SuspensionCrossingId};
pub use stack_composition::{
    AdmittedSameStackContribution, CallTargetBinding, SameStackContributionAdmissionCandidate,
    SameStackContributionCommitment, SameStackProviderPlanCommitment, StackCallContribution,
    TaskStackFrameSummary, UnresolvedCallKind, UnresolvedCallSite, ValidatedTaskStackFrameSummary,
    admit_same_stack_contribution, compose_task_stack_demand, project_wcsu_stack_plan,
    task_stack_frame_validation_identity, validate_task_stack_frame_summary,
};

use identities::commitment_digest;
use language_core::{CarryAddress, CarryCpu, CarryHostThread, CarryPolicy, CarrySuspension};
use semantic_vocabulary::Fnv1a as Fingerprint;
use stack_composition::WcsuStackPlanProjection;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskPlanDiagnostic(pub String);

impl std::fmt::Display for TaskPlanDiagnostic {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for TaskPlanDiagnostic {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStartOperation {
    Start,
    TryStart,
}

commitment_digest!(TaskSpecializationCommitment);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedTaskRuntimeProviderFact {
    pub runtime: TaskRuntimeId,
    pub provider_plan_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskActivationPlanFact {
    pub start_requirement: terminal_psi::TerminalIdentityIndex,
    pub target_machine: terminal_psi::TerminalIdentityIndex,
    pub target_entry: terminal_psi::TerminalIdentityIndex,
    pub specialization_report_fingerprint: u64, // Historical compact compatibility/report coordinate.
    pub specialization_commitment: TaskSpecializationCommitment, // Strong SHA-256 identity of the checked specialization structure.
    pub operation: TaskStartOperation,
    pub selected_runtime: SelectedTaskRuntimeProviderFact,
    pub plan: ValidatedActivationPlan,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TaskActivationPlanSet {
    pub identities: terminal_psi::TerminalIdentityTable,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StackPlan {
    pub bytes: u64,
    pub alignment: u64,
    pub representation: StackRepresentationId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaskArgumentExtent {
    pub offset: u64,
    pub bytes: u64,
    pub alignment: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskArgumentLayout {
    pub identity: ValueLayoutId,
    pub bytes: u64, // Total bytes of the marshalled image, including interior and trailing padding.
    pub alignment: u64, // Widest field alignment, or 1 for an empty bundle.
    pub fields: Vec<TaskArgumentExtent>, // Per-argument placement in source parameter order.
}

impl TaskArgumentLayout {
    pub fn new(
        identity: ValueLayoutId,
        arguments: &[(u64, u64)],
    ) -> Result<Self, TaskPlanDiagnostic> {
        let (fields, bytes, alignment) = pack_task_argument_fields(arguments.iter().copied())?;
        Ok(Self {
            identity,
            bytes,
            alignment,
            fields,
        })
    }

    pub fn is_canonical(&self) -> bool {
        canonical_argument_layout(self).is_ok_and(|canonical| canonical == *self)
    }
}

const MAX_TASK_ARGUMENT_ALIGNMENT: u64 = 1 << 16;

pub(crate) fn validate_alignment(alignment: u64, subject: &str) -> Result<(), TaskPlanDiagnostic> {
    if alignment == 0 || !alignment.is_power_of_two() {
        return Err(TaskPlanDiagnostic(format!(
            "{subject} alignment {alignment} is not a nonzero power of two"
        )));
    }
    Ok(())
}

fn pack_task_argument_fields(
    arguments: impl Iterator<Item = (u64, u64)>,
) -> Result<(Vec<TaskArgumentExtent>, u64, u64), TaskPlanDiagnostic> {
    let mut fields = Vec::new();
    let mut cursor = 0u64;
    let mut image_alignment = 1u64;
    for (index, (bytes, alignment)) in arguments.enumerate() {
        validate_alignment(alignment, &format!("task argument field {index}"))?;
        if alignment > MAX_TASK_ARGUMENT_ALIGNMENT {
            return Err(TaskPlanDiagnostic(format!(
                "task argument field {index} alignment {alignment} exceeds the marshalling \
                 bound {MAX_TASK_ARGUMENT_ALIGNMENT}"
            )));
        }
        image_alignment = image_alignment.max(alignment);
        let offset = cursor
            .checked_add(alignment - 1)
            .map(|padded| padded & !(alignment - 1))
            .ok_or_else(|| {
                TaskPlanDiagnostic(format!(
                    "task argument field {index} offset overflows the marshalling image"
                ))
            })?;
        fields.push(TaskArgumentExtent {
            offset,
            bytes,
            alignment,
        });
        cursor = offset.checked_add(bytes).ok_or_else(|| {
            TaskPlanDiagnostic(format!(
                "task argument field {index} extent overflows the marshalling image"
            ))
        })?;
    }
    let bytes = cursor
        .checked_add(image_alignment - 1)
        .map(|padded| padded & !(image_alignment - 1))
        .ok_or_else(|| TaskPlanDiagnostic("task argument image extent overflows".into()))?;
    Ok((fields, bytes, image_alignment))
}

fn canonical_argument_layout(
    layout: &TaskArgumentLayout,
) -> Result<TaskArgumentLayout, TaskPlanDiagnostic> {
    let (fields, bytes, alignment) = pack_task_argument_fields(
        layout
            .fields
            .iter()
            .map(|field| (field.bytes, field.alignment)),
    )?;
    Ok(TaskArgumentLayout {
        identity: layout.identity,
        bytes,
        alignment,
        fields,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiveCarryStorage {
    Persistent,   // Resident in the machine's persistent attached/owned layout.
    Parameter,    // Entry/state parameter held in the activation's fixed frame.
    Local,        // Lexical local held in the activation's fixed frame.
    CallArgument, // Value materialized for the suspending call itself.
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveCarryDemand {
    pub place: LiveCarryPlaceId,
    pub ty: LiveCarryTypeId,
    pub storage: LiveCarryStorage,
    pub claims: Vec<ClaimId>, // Complete linear-claim identities attached to this place at the crossing.
    pub effective: CarryPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalSuspensionCrossing {
    pub identity: SuspensionCrossingId,
    pub suspension_allowed: bool,
    pub preserve_cpu: bool,
    pub preserve_host_thread: bool,
    pub live_carry: Vec<LiveCarryDemand>, // Exact ordered live frontier retained across this crossing.
}

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivationPlanCandidate {
    pub machine_contract: MachineContractId,
    pub entry: MachineEntryId,
    pub argument_layout: TaskArgumentLayout,
    pub terminal_outcome_layout: ValueLayoutId,
    pub calling_plan: CallingPlanId,
    pub stack_plan: StackPlan,
    pub may_suspend: bool,
    pub may_block: bool,
    pub canonical_suspension_crossings: Vec<CanonicalSuspensionCrossing>, // Ordered by canonical crossing identity.
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

    pub const fn wcsu_stack_projection(&self) -> Option<&WcsuStackPlanProjection> {
        self.wcsu_stack_projection.as_ref()
    }

    pub fn normalized_identity(&self) -> ActivationPlanId {
        ActivationPlanId(activation_plan_report_fingerprint(
            &self.candidate,
            self.wcsu_stack_projection.as_ref(),
        ))
    }
}

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
    validate_alignment(candidate.stack_plan.alignment, "activation stack")?;
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
    let mut previous_crossing = None;
    for crossing in &candidate.canonical_suspension_crossings {
        if previous_crossing == Some(crossing.identity) {
            return Err(TaskPlanDiagnostic(
                "canonical suspension crossings carry a duplicate crossing identity".into(),
            ));
        }
        if previous_crossing.is_some_and(|previous| crossing.identity < previous) {
            return Err(TaskPlanDiagnostic(
                "canonical suspension crossings are not in canonical identity order".into(),
            ));
        }
        previous_crossing = Some(crossing.identity);
        let mut places = Vec::new();
        for live in &crossing.live_carry {
            if places.contains(&live.place) {
                return Err(TaskPlanDiagnostic(
                    "a canonical suspension crossing carries duplicate live places".into(),
                ));
            }
            places.push(live.place);
            let mut claims = Vec::new();
            for claim in &live.claims {
                if claims.contains(claim) {
                    return Err(TaskPlanDiagnostic(
                        "a live place at a suspension crossing carries a duplicate claim".into(),
                    ));
                }
                claims.push(*claim);
            }
            if live.effective.suspension == CarrySuspension::Forbidden {
                return Err(TaskPlanDiagnostic(
                    "a canonical suspension crossing carries a live value that forbids suspension"
                        .into(),
                ));
            }
        }
        if crossing.preserve_cpu
            != crossing
                .live_carry
                .iter()
                .any(|live| live.effective.cpu == CarryCpu::Origin)
            || crossing.preserve_host_thread
                != crossing
                    .live_carry
                    .iter()
                    .any(|live| live.effective.host_thread == CarryHostThread::Origin)
        {
            return Err(TaskPlanDiagnostic(
                "a canonical suspension crossing's preservation does not match its live \
                 frontier"
                    .into(),
            ));
        }
    }
    let canonical_layout = canonical_argument_layout(&candidate.argument_layout)?;
    if canonical_layout != candidate.argument_layout {
        return Err(TaskPlanDiagnostic(
            "activation argument layout is not the canonical packed marshalling layout".into(),
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

fn activation_plan_report_fingerprint(
    plan: &ActivationPlanCandidate,
    wcsu_stack_projection: Option<&WcsuStackPlanProjection>,
) -> u64 {
    let mut fingerprint = Fingerprint::new();
    fingerprint.u64(plan.machine_contract.normalized_identity());
    fingerprint.u64(plan.entry.normalized_identity());
    fingerprint.u64(plan.argument_layout.identity.normalized_identity());
    fingerprint.u64(plan.terminal_outcome_layout.normalized_identity());
    fingerprint.u64(plan.calling_plan.normalized_identity());
    fingerprint.u64(plan.stack_plan.bytes);
    fingerprint.u64(plan.stack_plan.alignment);
    fingerprint.u64(plan.stack_plan.representation.normalized_identity());
    if let Some(projection) = wcsu_stack_projection {
        fingerprint.byte(1);
        fingerprint.u64(projection.identity().normalized_identity());
    }
    fingerprint.flag(plan.may_suspend);
    fingerprint.flag(plan.may_block);
    fingerprint.u64(plan.canonical_suspension_crossings.len() as u64);
    for crossing in &plan.canonical_suspension_crossings {
        fingerprint.u64(crossing.identity.get());
        fingerprint.flag(crossing.suspension_allowed);
        fingerprint.flag(crossing.preserve_cpu);
        fingerprint.flag(crossing.preserve_host_thread);
        fingerprint.u64(crossing.live_carry.len() as u64);
        for live in &crossing.live_carry {
            fingerprint.u64(live.place.normalized_identity());
            fingerprint.u64(live.ty.normalized_identity());
            fingerprint.byte(match live.storage {
                LiveCarryStorage::Persistent => 1,
                LiveCarryStorage::Parameter => 2,
                LiveCarryStorage::Local => 3,
                LiveCarryStorage::CallArgument => 4,
            });
            fingerprint.u64(live.claims.len() as u64);
            for claim in &live.claims {
                fingerprint.u64(claim.get());
            }
            fingerprint.flag(live.effective.suspension == CarrySuspension::Allowed);
            fingerprint.flag(live.effective.cpu == CarryCpu::Origin);
            fingerprint.flag(live.effective.host_thread == CarryHostThread::Origin);
            fingerprint.byte(match live.effective.address {
                CarryAddress::Movable => 1,
                CarryAddress::Stable => 2,
            });
        }
    }
    fingerprint.flag(plan.carry_obligations.preserve_cpu);
    fingerprint.flag(plan.carry_obligations.preserve_host_thread);
    fingerprint.flag(plan.cancellation_required);
    fingerprint.finish_or(Fingerprint::OFFSET_BASIS)
}

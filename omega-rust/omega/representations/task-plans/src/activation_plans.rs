//! The activation plan: one fixed, nonmoving stack, the canonical
//! suspension crossings, the preservation those crossings demand, and the
//! validators that seal a candidate from a local stack plan or a WCSU
//! projection.

pub(crate) mod activation_plan_facts;
pub(crate) mod diagnostic;

use crate::report_fingerprints::activation_plan_report_fingerprint;
use crate::stack_composition::WcsuStackPlanProjection;
use crate::{
    ActivationPlanId, CallingPlanId, LiveCarryPlaceId, LiveCarryTypeId, MachineContractId,
    MachineEntryId, StackRepresentationId, TaskPlanDiagnostic, ValueLayoutId,
};
use language_core::{CarryCpu, CarryHostThread, CarryPolicy, CarrySuspension};
use semantic_vocabulary::{ClaimId, SuspensionCrossingId};

/// Physical fixed-stack shape retained by the activation sidecar.
///
/// This three-field carrier is not WCSU admission on its own. Activation
/// elaboration obtains the shape from [`project_wcsu_stack_plan`] over a
/// sealed whole-call-graph demand and validates it with
/// [`validate_wcsu_activation_plan`]; a bare tuple carries no composition
/// evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StackPlan {
    pub bytes: u64,
    pub alignment: u64,
    pub representation: StackRepresentationId,
}

/// One moved argument's placement inside the packed marshalling image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaskArgumentExtent {
    /// Byte offset inside the marshalled image.
    pub offset: u64,
    /// Exact byte extent the argument occupies.
    pub bytes: u64,
    /// Alignment the argument is placed at.
    pub alignment: u64,
}

/// The exact packed layout a task start's moved arguments marshal under.
///
/// `identity` remains the compact report coordinate derived from the checked
/// parameter list. `fields`, `bytes` and `alignment` describe the marshalling
/// image itself: each argument occupies its canonical aligned extent in
/// source parameter order, and the image is the contiguous byte array a
/// provider writes into the activation's argument area. Plan validation and
/// marshalling both require the canonical packing
/// [`pack_task_argument_fields`] derives, so a literal-built descriptor
/// cannot carry overlapping fields or unconstrained padding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskArgumentLayout {
    /// Compact report coordinate of the exact checked argument layout.
    pub identity: ValueLayoutId,
    /// Total bytes of the marshalled image, including interior and trailing
    /// padding.
    pub bytes: u64,
    /// Alignment the marshalled image is placed at: the widest field
    /// alignment, or one for an empty bundle.
    pub alignment: u64,
    /// Per-argument placement in source parameter order.
    pub fields: Vec<TaskArgumentExtent>,
}

impl TaskArgumentLayout {
    /// The canonical marshalling layout for `arguments` supplied as
    /// `(bytes, alignment)` pairs in source parameter order.
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

    /// Whether this descriptor is exactly the canonical packing of its own
    /// field demands — what [`TaskArgumentLayout::new`] produces. Activation
    /// validation and argument marshalling both require it so a hand-built
    /// layout cannot carry non-canonical offsets or free padding.
    pub fn is_canonical(&self) -> bool {
        canonical_argument_layout(self).is_ok_and(|canonical| canonical == *self)
    }
}

/// Largest field alignment the marshalling image supports. Every
/// source-representable argument alignment fits; the bound keeps a hostile
/// or corrupted field table from inflating padding into an unbounded image.
const MAX_TASK_ARGUMENT_ALIGNMENT: u64 = 1 << 16;

/// Pack `(bytes, alignment)` argument demands into their canonical field
/// extents: each argument lands at the next offset aligned to its own
/// alignment, and the image extent rounds the final cursor up to the widest
/// field alignment (one for an empty bundle).
fn pack_task_argument_fields(
    arguments: impl Iterator<Item = (u64, u64)>,
) -> Result<(Vec<TaskArgumentExtent>, u64, u64), TaskPlanDiagnostic> {
    let mut fields = Vec::new();
    let mut cursor = 0u64;
    let mut image_alignment = 1u64;
    for (index, (bytes, alignment)) in arguments.enumerate() {
        if alignment == 0 || !alignment.is_power_of_two() {
            return Err(TaskPlanDiagnostic(format!(
                "task argument field {index} alignment {alignment} is not a nonzero power of two"
            )));
        }
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

/// Re-derive the canonical descriptor a stored field table claims to be.
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

/// The storage class holding one live value across a parking crossing.
///
/// The class names whose lifetime and pinning the park relies on: the
/// fixed, nonmoving activation stack keeps `Parameter`, `Local` and
/// `CallArgument` places stable across the suspension, while `Persistent`
/// storage belongs to the machine's own attached layout and outlives the
/// crossing independently.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiveCarryStorage {
    /// Resident in the machine's persistent attached/owned layout.
    Persistent,
    /// Entry/state parameter held in the activation's fixed frame.
    Parameter,
    /// Lexical local held in the activation's fixed frame.
    Local,
    /// Value materialized for the suspending call itself.
    CallArgument,
}

/// One exact live place a parked continuation retains at a canonical
/// crossing.
///
/// This roster is the plan's suspension-safe-loan evidence. Each row names
/// the exact place, the checked type it carries, the storage class whose
/// lifetime and pinning the park relies on, the complete linear-claim
/// identities attached to that place — the loans that must stay alive and
/// address-stable through the suspension — and the four-axis demand the
/// value contributes. A live reference carrier alone never appears here;
/// only checked live places with their exact claim rosters do. A row whose
/// `effective` forbids suspension is rejected at plan validation rather
/// than silently licensing or silently dropping the loan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveCarryDemand {
    /// Stable identity of this exact live place at this crossing.
    pub place: LiveCarryPlaceId,
    /// Stable identity of the value's checked type.
    pub ty: LiveCarryTypeId,
    /// Storage class holding the value across the park.
    pub storage: LiveCarryStorage,
    /// Complete linear-claim identities attached to this place at the
    /// crossing. An empty roster means no live loan crosses here; it is
    /// never reconstructed from the type or storage class.
    pub claims: Vec<ClaimId>,
    /// The four-axis carry demand this value contributes to the crossing.
    pub effective: CarryPolicy,
}

/// One canonical semantic crossing at which the activation can park.
///
/// `identity` binds the detailed checked-tree crossing record retained in the
/// carry artifact. The local permission and preservation columns are repeated
/// here so activation-plan consumers do not reinterpret source or liveness.
/// `live_carry` retains the crossing's exact ordered live frontier: the
/// permission and preservation columns must be exactly the join of those
/// rows, so a candidate cannot publish a frontier that disagrees with the
/// preservation it declares.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalSuspensionCrossing {
    pub identity: SuspensionCrossingId,
    pub suspension_allowed: bool,
    pub preserve_cpu: bool,
    pub preserve_host_thread: bool,
    /// The exact ordered live frontier retained across this crossing: one
    /// row per live place, in checked order.
    pub live_carry: Vec<LiveCarryDemand>,
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
    /// The exact packed layout moved start arguments marshal under. The
    /// retained field table — not just its `identity` coordinate — is what
    /// the provider boundary checks a presented bundle against.
    pub argument_layout: TaskArgumentLayout,
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
    for crossing in &candidate.canonical_suspension_crossings {
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
            // The checked producer already refuses a suspension-forbidden
            // live value at check time; a row that forbids suspension here
            // is fabricated or drifted evidence, so it fails closed rather
            // than licensing or silently dropping the loan.
            if live.effective.suspension == CarrySuspension::Forbidden {
                return Err(TaskPlanDiagnostic(
                    "a canonical suspension crossing carries a live value that forbids suspension"
                        .into(),
                ));
            }
        }
        // The crossing's preservation bits are the join of its live
        // frontier. The checked producer derives them from the same
        // intersect, so an exact mismatch means the frontier and the
        // declared preservation disagree — neither may be trusted.
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

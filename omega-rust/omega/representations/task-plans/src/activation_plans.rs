//! The activation plan: one fixed, nonmoving stack, the canonical
//! suspension crossings, the preservation those crossings demand, and the
//! validators that seal a candidate from a local stack plan or a WCSU
//! projection.

pub(crate) mod activation_plan_facts;
pub(crate) mod diagnostic;

use crate::report_fingerprints::activation_plan_report_fingerprint;
use crate::stack_composition::WcsuStackPlanProjection;
use crate::{
    ActivationPlanId, CallingPlanId, MachineContractId, MachineEntryId, StackRepresentationId,
    TaskPlanDiagnostic, ValueLayoutId,
};
use semantic_vocabulary::SuspensionCrossingId;

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

//! Boundary entry plans: pairing a call plan with a state plan for one
//! inbound root, evaluating and validating one, and the rejection when a
//! policy cannot serve a signature.

use crate::calling_conventions::callback_materializations::{
    CallbackMaterializationContext, validate_callback_materializations,
};
use crate::calling_conventions::plans::call_plan_evaluation::validate_call_plan_structure;
use crate::calling_conventions::plans::plan_identity::Fnv1a;
use crate::calling_conventions::plans::state_footprints::machine_state_for_registers;
use crate::calling_conventions::plans::{
    BoundaryPlanDiagnostic, CallPlan, CallSignature, CallingPolicy, ConcreteVariadicCallSignature,
    EntryStack, MachineRegime, MachineState, MachineStateSet, PlanDiagnostic, Preemption,
    StatePlan, ValueLocation, evaluate_call_plan, evaluate_darwin_aapcs64_variadic_call_plan,
    validate_call_plan,
};
use target::Architecture;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryEntryPlan {
    pub call: CallPlan,
    pub state: StatePlan,
}

/// The compile-time result published by an implementation of the source
/// `CallingPolicy::plan` relationship. A rejected policy is deliberately not
/// representable as a validated plan and therefore cannot acquire contract
/// identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundaryPlanResult {
    Accepted(BoundaryEntryPlan),
    Rejected(CallingPolicyRejection),
}

/// Structured policy-authored context for a boundary signature the policy
/// cannot represent. The compiler retains this distinct from validator
/// failures in an allegedly accepted plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallingPolicyRejection {
    reason: String,
}

impl CallingPolicyRejection {
    pub fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }

    pub fn reason(&self) -> &str {
        &self.reason
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedBoundaryEntryPlan(BoundaryEntryPlan);

impl ValidatedBoundaryEntryPlan {
    pub const fn plan(&self) -> &BoundaryEntryPlan {
        &self.0
    }

    /// Deterministic compatibility/report coordinate for the canonical public
    /// contract. Strong replay uses [`Self::contract_commitment_digest`].
    pub fn contract_report_fingerprint(&self) -> u64 {
        let mut hash = Fnv1a::new();
        hash.call_plan(&self.0.call);
        hash.state_plan(&self.0.state);
        hash.finish()
    }

    /// Domain-separated commitment to the complete canonical boundary plan.
    pub fn contract_commitment_digest(&self) -> [u8; 32] {
        let mut hash = Fnv1a::with_strong_domain(b"omega.boundary-calling-plan.v1");
        hash.call_plan(&self.0.call);
        hash.state_plan(&self.0.state);
        hash.finish_strong()
    }
}

/// Concrete state policy for ordinary call/return entries, including hosted
/// process roots and firmware handoffs. No interrupted activation exists, so
/// the entry stub owes no save/restore; its transitive state ceiling is exactly
/// the machine-state classes touched by the ABI's ordinary volatile registers
/// plus its caller-volatile condition flags.
pub fn evaluate_ordinary_boundary_entry_plan(
    policy: CallingPolicy,
    signature: &CallSignature,
) -> Result<ValidatedBoundaryEntryPlan, PlanDiagnostic> {
    let call = evaluate_call_plan(policy, signature)?;
    evaluate_ordinary_boundary_entry_plan_from_call(call, signature)
}

/// Concrete state policy for the compiler-selected implicit entry of a
/// freestanding program. Unlike a hosted ordinary call, this root is the
/// admitted owns-the-machine domain: checked instruction contracts may use
/// instruction, stack, and control state in addition to the ordinary ABI
/// volatile banks. An explicit source-selected boundary plan remains
/// authoritative and must not be widened through this compatibility path.
pub fn evaluate_freestanding_program_entry_plan(
    policy: CallingPolicy,
    signature: &CallSignature,
) -> Result<ValidatedBoundaryEntryPlan, PlanDiagnostic> {
    let ordinary = evaluate_ordinary_boundary_entry_plan(policy, signature)?;
    let mut plan = ordinary.plan().clone();
    plan.state.permitted_transitive_use =
        plan.state
            .permitted_transitive_use
            .union(MachineStateSet::new([
                MachineState::InstructionPointer,
                MachineState::StackPointer,
                MachineState::ControlState,
            ]));
    validate_boundary_entry_plan(plan, signature)
}

pub fn evaluate_darwin_aapcs64_variadic_boundary_entry_plan(
    signature: &ConcreteVariadicCallSignature,
) -> Result<ValidatedBoundaryEntryPlan, PlanDiagnostic> {
    let call = evaluate_darwin_aapcs64_variadic_call_plan(signature)?;
    evaluate_ordinary_boundary_entry_plan_from_call(call, &signature.flattened())
}

fn evaluate_ordinary_boundary_entry_plan_from_call(
    call: CallPlan,
    signature: &CallSignature,
) -> Result<ValidatedBoundaryEntryPlan, PlanDiagnostic> {
    let permitted_transitive_use = machine_state_for_registers(&call.ordinary_clobbers)
        .union(MachineStateSet::new([MachineState::Flags]));
    let initial_regime = match call.policy.architecture() {
        Architecture::X86_64 => MachineRegime::X86Long64,
        Architecture::Aarch64 => MachineRegime::Aarch64A64 { exception_level: 0 },
    };
    validate_boundary_entry_plan(
        BoundaryEntryPlan {
            call,
            state: StatePlan {
                initial_regime,
                interrupted_state: MachineStateSet::default(),
                saved_state: MachineStateSet::default(),
                restored_state: MachineStateSet::default(),
                permitted_transitive_use,
                stack: EntryStack::ProviderSelected,
                preemption: Preemption::NotApplicable,
            },
        },
        signature,
    )
}

pub fn validate_boundary_entry_plan(
    mut plan: BoundaryEntryPlan,
    signature: &CallSignature,
) -> Result<ValidatedBoundaryEntryPlan, PlanDiagnostic> {
    canonicalize_boundary_entry_plan(&mut plan);
    validate_call_plan(&plan.call, signature)?;
    validate_boundary_state_plan(plan, signature)
}

/// Validate a registrar boundary whose outbound plan carries private callback
/// materialization rows. The context is deliberately required: a bare plan
/// cannot establish that nominal binder, native-parameter, or layout-slot
/// identities exist or are compatible.
pub fn validate_boundary_entry_plan_with_callback_materializations(
    mut plan: BoundaryEntryPlan,
    signature: &CallSignature,
    context: &CallbackMaterializationContext,
) -> Result<ValidatedBoundaryEntryPlan, PlanDiagnostic> {
    canonicalize_boundary_entry_plan(&mut plan);
    validate_call_plan_structure(&plan.call, signature)?;
    validate_callback_materializations(&plan.call.callback_materializations, context)?;
    validate_boundary_state_plan(plan, signature)
}

fn validate_boundary_state_plan(
    plan: BoundaryEntryPlan,
    _signature: &CallSignature,
) -> Result<ValidatedBoundaryEntryPlan, PlanDiagnostic> {
    if plan.call.policy.architecture() != plan.state.initial_regime.architecture() {
        return Err(PlanDiagnostic(
            "calling policy and initial machine regime name different architectures".into(),
        ));
    }
    if !plan
        .state
        .interrupted_state
        .contains_all(plan.state.saved_state)
    {
        return Err(PlanDiagnostic(
            "saved machine state is not part of the interrupted state".into(),
        ));
    }
    if !plan
        .state
        .saved_state
        .contains_all(plan.state.restored_state)
        || !plan
            .state
            .restored_state
            .contains_all(plan.state.saved_state)
    {
        return Err(PlanDiagnostic(
            "entry plan must restore exactly the machine state it saves".into(),
        ));
    }
    let endangered = plan
        .state
        .permitted_transitive_use
        .intersection(plan.state.interrupted_state);
    if !plan.state.saved_state.contains_all(endangered) {
        return Err(PlanDiagnostic(
            "permitted transitive machine-state use includes interrupted state the entry stub does not save"
                .into(),
        ));
    }
    let ordinary_clobber_state = machine_state_for_registers(&plan.call.ordinary_clobbers);
    if !plan
        .state
        .permitted_transitive_use
        .contains_all(ordinary_clobber_state)
    {
        return Err(PlanDiagnostic(
            "ordinary call clobbers exceed the entry plan's permitted machine-state ceiling".into(),
        ));
    }
    Ok(ValidatedBoundaryEntryPlan(plan))
}

/// Turns a source policy result into the only artifact allowed to contribute
/// requirement identity. Policy rejection and compiler validation failure stay
/// distinguishable so callers can attach the appropriate declaration-site
/// diagnostic.
pub fn validate_boundary_plan_result(
    result: BoundaryPlanResult,
    signature: &CallSignature,
) -> Result<ValidatedBoundaryEntryPlan, BoundaryPlanDiagnostic> {
    match result {
        BoundaryPlanResult::Accepted(plan) => validate_boundary_entry_plan(plan, signature)
            .map_err(BoundaryPlanDiagnostic::InvalidAcceptedPlan),
        BoundaryPlanResult::Rejected(rejection) => Err(BoundaryPlanDiagnostic::Rejected(rejection)),
    }
}

fn canonicalize_boundary_entry_plan(plan: &mut BoundaryEntryPlan) {
    for placement in plan
        .call
        .parameters
        .iter_mut()
        .chain(plan.call.result.iter_mut())
    {
        placement.locations.sort_by_key(value_location_byte_offset);
    }
    plan.call
        .callback_materializations
        .sort_by_key(|row| row.binder);
}

fn value_location_byte_offset(location: &ValueLocation) -> u16 {
    match location {
        ValueLocation::Register {
            value_byte_offset, ..
        }
        | ValueLocation::Stack {
            value_byte_offset, ..
        } => *value_byte_offset,
        ValueLocation::Indirect { .. } => 0,
    }
}

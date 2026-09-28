//! Machine-state footprints: the evidence a provider exit and a composed
//! entry may touch, and the validators that hold each under its ceiling.

use crate::plans::plan_identity::Fnv1a;
use crate::plans::{
    BoundaryEntryPlan, EntryControl, MachineRegister, MachineState, MachineStateSet,
    PlanDiagnostic, RegisterSet, ValidatedBoundaryEntryPlan,
};
use target::Architecture;

/// Implementation evidence. This is intentionally not a field of
/// `BoundaryEntryPlan`: changing allocation or emitted code revalidates the
/// provider artifact without changing the published requirement identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateFootprintEvidence {
    registers: RegisterSet,
    machine_state: MachineStateSet,
}

impl StateFootprintEvidence {
    pub fn new(registers: RegisterSet, additional_machine_state: MachineStateSet) -> Self {
        let register_state = machine_state_for_registers(&registers);
        Self {
            registers,
            machine_state: additional_machine_state.union(register_state),
        }
    }

    pub fn registers(&self) -> &RegisterSet {
        &self.registers
    }

    pub const fn machine_state(&self) -> MachineStateSet {
        self.machine_state
    }
}

/// Provider evidence for the control-state transition that leaves one
/// externally entered boundary. This is implementation evidence, not part of
/// the public `CallPlan + StatePlan` identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderExitRealization {
    pub control: EntryControl,
    pub restored_state: MachineStateSet,
}

/// Verify that a provider's realized exit is exactly the exit admitted by the
/// boundary contract. Footprint validation covers state touched by the body;
/// this separate check prevents an otherwise-valid footprint from returning
/// through the wrong control mechanism or restore set. The plan's
/// `initial_regime` is an entry fact, so this evidence deliberately does not
/// invent a same-regime exit promise.
pub fn validate_provider_exit_realization(
    plan: &BoundaryEntryPlan,
    realization: &ProviderExitRealization,
) -> Result<(), PlanDiagnostic> {
    if realization.control != plan.call.entry_control {
        return Err(PlanDiagnostic(
            "provider exit control does not match the admitted CallPlan".into(),
        ));
    }
    if realization.restored_state != plan.state.restored_state {
        return Err(PlanDiagnostic(
            "provider exit restored-state set does not match the admitted StatePlan".into(),
        ));
    }
    Ok(())
}

/// Compose implementation evidence from independently derived code fragments.
/// Register sets and machine-state classes are mathematical unions, so the
/// result is deterministic across fragment ordering and repeated evidence.
/// This remains implementation evidence: it does not enter boundary contract
/// identity and does not claim to be a final placed-artifact certificate.
pub fn compose_state_footprints<'a>(
    fragments: impl IntoIterator<Item = &'a StateFootprintEvidence>,
) -> StateFootprintEvidence {
    let mut registers = Vec::new();
    let mut machine_state = MachineStateSet::empty();
    for fragment in fragments {
        registers.extend_from_slice(fragment.registers().as_slice());
        machine_state = machine_state.union(fragment.machine_state());
    }
    StateFootprintEvidence::new(RegisterSet::new(registers), machine_state)
}

/// Compose fragment evidence and validate the whole transitive footprint
/// against one already-validated boundary plan. Returning the normalized
/// aggregate lets later object/final-image consumers retain exactly the
/// evidence that was checked without publishing it as requirement identity.
pub fn validate_composed_state_footprint<'a>(
    validated: &ValidatedBoundaryEntryPlan,
    fragments: impl IntoIterator<Item = &'a StateFootprintEvidence>,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    let composed = compose_state_footprints(fragments);
    validate_state_footprint(validated, &composed)?;
    Ok(composed)
}

pub fn validate_state_footprint(
    validated: &ValidatedBoundaryEntryPlan,
    evidence: &StateFootprintEvidence,
) -> Result<(), PlanDiagnostic> {
    validate_state_footprint_under_ceiling(
        validated,
        evidence,
        validated.plan().state.permitted_transitive_use,
    )
}

/// Validate compiler-owned ordinary call-entry/return mechanics. Their
/// stack-pointer, control-transfer, and canonical floating-control-state
/// effects are prescribed by `CallReturn`; they are not handler-body
/// transitive use and therefore sit outside that ceiling. All other machine
/// state remains constrained by the ordinary transitive ceiling, and
/// interrupted state still has to be saved.
pub fn validate_call_return_mechanics_footprint(
    validated: &ValidatedBoundaryEntryPlan,
    evidence: &StateFootprintEvidence,
) -> Result<(), PlanDiagnostic> {
    if validated.plan().call.entry_control != EntryControl::CallReturn {
        return Err(PlanDiagnostic(
            "ordinary call-return footprint evidence requires CallReturn entry control".into(),
        ));
    }
    let prescribed_control = MachineStateSet::new([
        MachineState::InstructionPointer,
        MachineState::StackPointer,
        MachineState::ControlState,
    ]);
    validate_state_footprint_under_ceiling(
        validated,
        evidence,
        validated
            .plan()
            .state
            .permitted_transitive_use
            .union(prescribed_control),
    )
}

/// Validate one outbound call leaf inside an ordinary call-return activation.
/// The callee or supervisor may consume the plan's complete volatile ceiling;
/// control transfer is prescribed by the selected outbound entry mechanism,
/// rather than being ordinary handler-body transitive use.
pub fn validate_outbound_call_footprint(
    validated: &ValidatedBoundaryEntryPlan,
    evidence: &StateFootprintEvidence,
) -> Result<(), PlanDiagnostic> {
    if validated.plan().call.entry_control != EntryControl::CallReturn {
        return Err(PlanDiagnostic(
            "outbound call footprint evidence requires an enclosing CallReturn activation".into(),
        ));
    }
    validate_state_footprint_under_ceiling(
        validated,
        evidence,
        validated
            .plan()
            .state
            .permitted_transitive_use
            .union(MachineStateSet::new([
                MachineState::InstructionPointer,
                MachineState::StackPointer,
                MachineState::ControlState,
            ])),
    )
}

/// Validate a recursive runtime-value evaluator used by guards or ordinary
/// binary writes. Its x86 lowering may use balanced push/pop pairs while
/// evaluating `Binary` operands; that stack effect is prescribed only for an
/// ordinary call-return activation. Every other state class remains under the
/// boundary's transitive ceiling.
pub fn validate_runtime_value_guard_footprint(
    validated: &ValidatedBoundaryEntryPlan,
    evidence: &StateFootprintEvidence,
) -> Result<(), PlanDiagnostic> {
    let stack_use = MachineStateSet::new([MachineState::StackPointer]);
    let control_use = MachineStateSet::new([MachineState::ControlState]);
    let uses_stack = evidence.machine_state().contains_all(stack_use);
    let uses_control = evidence.machine_state().contains_all(control_use);
    if uses_stack
        && (validated.plan().call.policy.architecture() != Architecture::X86_64
            || validated.plan().call.entry_control != EntryControl::CallReturn)
    {
        return Err(PlanDiagnostic(
            "runtime-value guard stack scratch requires an x86 call-return activation".into(),
        ));
    }
    if uses_control && validated.plan().call.entry_control != EntryControl::CallReturn {
        return Err(PlanDiagnostic(
            "runtime-value guard directed rounding requires a call-return activation".into(),
        ));
    }
    validate_state_footprint_under_ceiling(
        validated,
        evidence,
        validated
            .plan()
            .state
            .permitted_transitive_use
            .union(if uses_stack {
                stack_use
            } else {
                MachineStateSet::empty()
            })
            .union(if uses_control {
                control_use
            } else {
                MachineStateSet::empty()
            }),
    )
}

fn validate_state_footprint_under_ceiling(
    validated: &ValidatedBoundaryEntryPlan,
    evidence: &StateFootprintEvidence,
    permitted_state: MachineStateSet,
) -> Result<(), PlanDiagnostic> {
    let plan = validated.plan();
    for register in evidence.registers().as_slice() {
        if register.architecture() != plan.call.policy.architecture() {
            return Err(PlanDiagnostic(format!(
                "footprint register {register:?} belongs to the wrong architecture"
            )));
        }
    }
    if !permitted_state.contains_all(evidence.machine_state()) {
        return Err(PlanDiagnostic(format!(
            "emitted machine-state footprint {:?} exceeds the entry plan ceiling {:?}",
            evidence.machine_state(),
            permitted_state
        )));
    }
    let unsaved =
        MachineStateSet(plan.state.interrupted_state.bits() & !plan.state.saved_state.bits());
    if !evidence.machine_state().intersection(unsaved).is_empty() {
        return Err(PlanDiagnostic(
            "emitted footprint clobbers interrupted machine state that is not saved".into(),
        ));
    }
    Ok(())
}

impl StateFootprintEvidence {
    /// Non-authoritative report coordinate over the retained exact register
    /// and machine-state evidence.
    pub fn evidence_report_fingerprint(&self) -> u64 {
        let mut hash = Fnv1a::new();
        hash.u8(0xe1);
        hash.register_set(self.registers());
        hash.u16(self.machine_state().bits());
        hash.finish()
    }
}

pub(crate) fn machine_state_for_registers(registers: &RegisterSet) -> MachineStateSet {
    MachineStateSet::new(registers.as_slice().iter().map(|register| match register {
        MachineRegister::X86Xmm(_) | MachineRegister::Aarch64V(_) => MachineState::VectorRegisters,
        _ => MachineState::GeneralRegisters,
    }))
}

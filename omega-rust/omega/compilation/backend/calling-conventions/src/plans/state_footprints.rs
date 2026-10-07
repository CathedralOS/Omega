//! Machine-state footprints: the evidence a provider exit and a composed
//! entry may touch. Fragment composition retains the emitted facts without
//! replaying compiler-owned plans against them.

use crate::plans::plan_identity::Fnv1a;
use crate::plans::{
    BoundaryEntryPlan, EntryControl, MachineRegister, MachineState, MachineStateSet,
    PlanDiagnostic, RegisterSet,
};

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

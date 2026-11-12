/*
We pair evaluated `CallPlan`s with `StatePlan`s to form canonical boundary
entry plans (`BoundaryEntryPlan`, `ValidatedBoundaryEntryPlan`), validate
accepted and rejected policy outputs (`BoundaryPlanResult`,
`CallingPolicyRejection`, `PlanDiagnostic`, `BoundaryPlanDiagnostic`), and
verify implementation state-footprint and generated-entry evidence
(`StateFootprintEvidence`, `StateEffectsEvidence`, `GeneratedEntryEvidence`).
We canonicalize fragment and callback materialization ordering before computing
contract fingerprints or domain-separated commitments, while keeping emitted
footprint and instruction evidence strictly separate from contract identity.
*/

mod call_plan_evaluation;
mod plan_identity;
#[cfg(test)]
mod tests;
mod vocabulary;

use crate::callback_materializations::{
    CallbackMaterializationContext, validate_callback_materializations,
};
use call_plan_evaluation::validate_call_plan_structure;
use plan_identity::Fnv1a;
use target::Architecture;

pub use call_plan_evaluation::evaluate_call_plan;
pub(crate) use call_plan_evaluation::validate_call_plan;
pub use vocabulary::{
    CallPlan, CallSignature, CallingPolicy, EntryControl, EntryStack, IndirectPointerLocation,
    MachineRegime, MachineRegister, MachineState, MachineStateSet, Preemption, RegisterSet,
    StatePlan, SystemVEightbyteClass, ValueClass, ValueLocation, ValuePlacement, ValueShape,
    byte_size_at_offset_zero, encode_state_plan_identity, gpr_code,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanDiagnostic(pub String);

impl std::fmt::Display for PlanDiagnostic {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for PlanDiagnostic {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundaryPlanDiagnostic {
    Rejected(CallingPolicyRejection),
    InvalidAcceptedPlan(PlanDiagnostic),
}

impl std::fmt::Display for BoundaryPlanDiagnostic {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rejected(rejection) => {
                write!(
                    formatter,
                    "calling policy rejected the boundary: {}",
                    rejection.reason()
                )
            }
            Self::InvalidAcceptedPlan(diagnostic) => {
                write!(
                    formatter,
                    "calling policy accepted an invalid plan: {diagnostic}"
                )
            }
        }
    }
}

impl std::error::Error for BoundaryPlanDiagnostic {}

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateEffectsEvidence {
    reads: RegisterSet,
    footprint: StateFootprintEvidence,
    clobbered_state: MachineStateSet,
}

impl StateEffectsEvidence {
    pub fn empty() -> Self {
        Self::new(
            RegisterSet::default(),
            RegisterSet::default(),
            MachineStateSet::empty(),
            MachineStateSet::empty(),
        )
    }

    pub fn new(
        reads: RegisterSet,
        writes: RegisterSet,
        additional_use: MachineStateSet,
        additional_clobbers: MachineStateSet,
    ) -> Self {
        let clobbered_state = machine_state_for_registers(&writes).union(additional_clobbers);
        Self {
            footprint: StateFootprintEvidence::new(
                writes,
                additional_use
                    .union(clobbered_state)
                    .union(machine_state_for_registers(&reads)),
            ),
            reads,
            clobbered_state,
        }
    }

    pub fn conservative(footprint: StateFootprintEvidence) -> Self {
        Self {
            reads: RegisterSet::default(),
            clobbered_state: footprint.machine_state(),
            footprint,
        }
    }

    pub fn footprint(&self) -> &StateFootprintEvidence {
        &self.footprint
    }

    pub fn registers_read(&self) -> &RegisterSet {
        &self.reads
    }

    pub fn registers(&self) -> &RegisterSet {
        self.footprint.registers()
    }

    pub fn machine_state(&self) -> MachineStateSet {
        self.footprint.machine_state()
    }

    pub fn clobbered_state(&self) -> MachineStateSet {
        self.clobbered_state
    }

    pub fn compose(&self, other: &Self) -> Self {
        Self::new(
            RegisterSet::new(
                self.reads
                    .as_slice()
                    .iter()
                    .copied()
                    .chain(other.reads.as_slice().iter().copied()),
            ),
            RegisterSet::new(
                self.registers()
                    .as_slice()
                    .iter()
                    .copied()
                    .chain(other.registers().as_slice().iter().copied()),
            ),
            self.machine_state().union(other.machine_state()),
            self.clobbered_state.union(other.clobbered_state),
        )
    }

    pub fn validate(&self, plan: &StatePlan) -> Result<(), &'static str> {
        if !plan
            .permitted_transitive_use
            .contains_all(self.machine_state())
        {
            return Err("complete entry code uses machine state outside its declared ceiling");
        }
        if !plan
            .saved_state
            .contains_all(self.clobbered_state.intersection(plan.interrupted_state))
        {
            return Err(
                "complete entry code overwrites interrupted machine state it does not save",
            );
        }
        Ok(())
    }
}

impl From<StateFootprintEvidence> for StateEffectsEvidence {
    fn from(footprint: StateFootprintEvidence) -> Self {
        Self::conservative(footprint)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GeneratedEntryControl {
    FallThrough,
    Branch {
        target_byte_offset: i64,
        conditional: bool,
    },
    Call {
        field_byte_offset: u32,
    },
    LocalCall {
        target_byte_offset: i64,
    },
    Return,
    InterruptReturn,
    Stop,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedEntryInstruction {
    pub byte_offset: u32,
    pub byte_count: u32,
    pub effects: StateEffectsEvidence,
    pub control: GeneratedEntryControl,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedEntryEvidence {
    pub instructions: Vec<GeneratedEntryInstruction>,
    pub effects: StateEffectsEvidence,
}

impl GeneratedEntryEvidence {
    pub fn from_target_instructions(
        byte_count: usize,
        instructions: Vec<GeneratedEntryInstruction>,
        body_call_field: Option<u32>,
    ) -> Result<Self, &'static str> {
        let mut cursor = 0usize;
        let mut calls = Vec::new();
        let mut effects = StateEffectsEvidence::new(
            RegisterSet::default(),
            RegisterSet::default(),
            MachineStateSet::empty(),
            MachineStateSet::empty(),
        );
        for row in &instructions {
            if body_call_field.is_none()
                && matches!(row.control, GeneratedEntryControl::LocalCall { .. })
            {
                return Err("generated frame protocol contains an unaccounted call");
            }
            if row.byte_count == 0 || row.byte_offset as usize != cursor {
                return Err("generated entry instruction coverage has a gap, overlap or empty row");
            }
            cursor = cursor
                .checked_add(row.byte_count as usize)
                .ok_or("generated entry byte coverage overflows")?;
            if cursor > byte_count {
                return Err("generated entry instruction exceeds its byte image");
            }
            if let GeneratedEntryControl::Call { field_byte_offset } = row.control {
                calls.push(field_byte_offset);
            }
            effects = effects.compose(&row.effects);
        }
        if cursor != byte_count {
            return Err("generated entry instructions do not cover every byte");
        }
        if calls.as_slice() != body_call_field.as_slice() {
            return Err("generated entry body call differs from its exact relocation");
        }
        for row in &instructions {
            if let GeneratedEntryControl::Branch {
                target_byte_offset, ..
            }
            | GeneratedEntryControl::LocalCall { target_byte_offset } = row.control
                && !instructions
                    .iter()
                    .any(|target| i64::from(target.byte_offset) == target_byte_offset)
            {
                return Err("generated entry branch leaves its instruction boundaries");
            }
        }
        Ok(Self {
            instructions,
            effects,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundaryEntryPlan {
    pub call: CallPlan,
    pub state: StatePlan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoundaryPlanResult {
    Accepted(BoundaryEntryPlan),
    Rejected(CallingPolicyRejection),
}

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

    pub fn contract_report_fingerprint(&self) -> u64 {
        let mut hash = Fnv1a::new();
        hash.call_plan(&self.0.call);
        hash.state_plan(&self.0.state);
        hash.finish()
    }

    pub fn contract_commitment_digest(&self) -> [u8; 32] {
        let mut hash = Fnv1a::with_strong_domain(b"omega.boundary-calling-plan.v2");
        hash.call_plan(&self.0.call);
        hash.state_plan(&self.0.state);
        hash.finish_strong()
    }
}

pub fn evaluate_ordinary_boundary_entry_plan(
    policy: CallingPolicy,
    signature: &CallSignature,
) -> Result<ValidatedBoundaryEntryPlan, PlanDiagnostic> {
    let call = evaluate_call_plan(policy, signature)?;
    evaluate_ordinary_boundary_entry_plan_from_call(call, signature)
}

fn evaluate_ordinary_boundary_entry_plan_from_call(
    call: CallPlan,
    signature: &CallSignature,
) -> Result<ValidatedBoundaryEntryPlan, PlanDiagnostic> {
    // The floating-control envelope around a returning import restores the
    // callee-preserved MXCSR/FPCR controls, so the ceiling includes them.
    let permitted_transitive_use =
        machine_state_for_registers(&call.ordinary_clobbers).union(MachineStateSet::new([
            MachineState::Flags,
            MachineState::InstructionPointer,
            MachineState::StackPointer,
            MachineState::ControlState,
        ]));
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
    if plan.state.saved_state != plan.state.restored_state {
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

fn value_location_byte_offset(location: &ValueLocation) -> u32 {
    match location {
        crate::register_or_stack_pair_pattern!(value_byte_offset, ..) => *value_byte_offset,
        ValueLocation::Indirect { .. } => 0,
    }
}

//! Compute register and machine-state footprints from physical or independently decoded effects.
use calling_conventions::{
    MachineRegister, MachineState, MachineStateSet, Preemption, RegisterSet, StateEffectsEvidence,
};
use machine_code::SelectedFormDecodedFootprint;
use register_model::{PhysicalRegisterModel, RegisterUnitId, RegisterUnitKind, RegisterViewId};
use selected_instructions::{
    MachineEncodedControlEffect, MachineEncodedEffects, MachineEncodedStackEffect,
};
use semantic_vocabulary::MachineId;
use std::collections::BTreeSet;

#[derive(Debug)]
pub enum Error {
    Mismatch(&'static str),
    Unsupported(&'static str),
    SelectedOperand(crate::selected::Error),
}

impl From<crate::selected::Error> for Error {
    fn from(error: crate::selected::Error) -> Self {
        Self::SelectedOperand(error)
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Mismatch(reason) | Self::Unsupported(reason) => f.write_str(reason),
            Self::SelectedOperand(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for Error {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BodyCpuEffects {
    effects: StateEffectsEvidence,
    generated_effects: StateEffectsEvidence,
    floating_control: bool,
    opaque_control: bool,
}

impl BodyCpuEffects {
    /// Include the frame, calls, and transitive state observed in this body.
    pub fn with_generated_effects(
        mut self,
        generated: StateEffectsEvidence,
        frame_floating: bool,
        opaque: bool,
    ) -> Self {
        self.effects = self.effects.compose(&generated);
        self.generated_effects = generated;
        self.floating_control |= frame_floating;
        self.opaque_control = opaque;
        self
    }

    pub fn effects(&self) -> &StateEffectsEvidence {
        &self.effects
    }

    pub fn generated_effects(&self) -> &StateEffectsEvidence {
        &self.generated_effects
    }

    pub const fn floating_control(&self) -> bool {
        self.floating_control
    }

    pub fn require_without_floating_state(&self) -> Result<(), Error> {
        if self.floating_control {
            return Err(Error::Mismatch(
                "installed body executes floating control without a CPU-state premise",
            ));
        }
        if self.opaque_control {
            return Err(Error::Mismatch(
                "opaque callee CPU-state ceiling does not exclude floating control",
            ));
        }
        if self
            .effects
            .machine_state()
            .contains_all(MachineStateSet::new([MachineState::VectorRegisters]))
        {
            return Err(Error::Mismatch(
                "installed body executes vector state without a CPU-state premise",
            ));
        }
        Ok(())
    }

    pub fn into_effects(self) -> StateEffectsEvidence {
        self.effects
    }
}

#[derive(Default)]
pub struct Footprint {
    units: BTreeSet<RegisterUnitId>,
    reads: BTreeSet<RegisterUnitId>,
    states: MachineStateSet,
    clobbered: MachineStateSet,
}

impl Footprint {
    pub fn add_states(&mut self, used: MachineStateSet, clobbered: MachineStateSet) {
        self.states = self.states.union(used);
        self.clobbered = self.clobbered.union(clobbered);
    }
    pub fn clobber(&mut self, state: MachineState) {
        self.clobbered = self.clobbered.union(MachineStateSet::new([state]));
    }
    pub fn write_unit(&mut self, unit: RegisterUnitId) {
        self.units.insert(unit);
    }

    pub fn state(&mut self, state: MachineState) {
        self.states = self.states.union(MachineStateSet::new([state]));
    }

    pub fn view(
        &mut self,
        model: &PhysicalRegisterModel,
        view: RegisterViewId,
    ) -> Result<(), Error> {
        self.units.extend(
            &model
                .view(view)
                .ok_or(Error::Mismatch(
                    "interrupt effect names unknown register view",
                ))?
                .write_units,
        );
        Ok(())
    }

    pub fn read_view(
        &mut self,
        model: &PhysicalRegisterModel,
        view: RegisterViewId,
    ) -> Result<(), Error> {
        self.reads.extend(
            &model
                .view(view)
                .ok_or(Error::Mismatch(
                    "interrupt read names unknown register view",
                ))?
                .units,
        );
        Ok(())
    }

    pub fn decoded(
        &mut self,
        model: &PhysicalRegisterModel,
        row: &SelectedFormDecodedFootprint,
    ) -> Result<(), Error> {
        for view in &row.register_reads {
            self.read_view(model, *view)?;
        }
        for view in &row.register_writes {
            self.view(model, *view)?;
        }
        self.units.extend(&row.implicit_defs);
        self.units.extend(&row.implicit_clobbers);
        self.effects(model, &row.encoded)
    }

    pub fn effects(
        &mut self,
        model: &PhysicalRegisterModel,
        effects: &MachineEncodedEffects,
    ) -> Result<(), Error> {
        self.reads.extend(&effects.implicit_unit_uses);
        self.units.extend(&effects.implicit_unit_defs);
        self.units.extend(&effects.implicit_unit_clobbers);
        match effects.stack {
            MachineEncodedStackEffect::UnchangedV1 => {}
            MachineEncodedStackEffect::TemporaryBytesV1 { stack_pointer, .. }
            | MachineEncodedStackEffect::PopBytesV1 { stack_pointer, .. }
            | MachineEncodedStackEffect::CallReturnAddressLifecycleV1 { stack_pointer, .. } => {
                self.view(model, stack_pointer)?;
                self.state(MachineState::StackPointer);
                self.clobbered = self
                    .clobbered
                    .union(MachineStateSet::new([MachineState::StackPointer]));
            }
        }
        if effects.control != MachineEncodedControlEffect::FallThroughV1 {
            self.state(MachineState::InstructionPointer);
            self.clobbered = self
                .clobbered
                .union(MachineStateSet::new([MachineState::InstructionPointer]));
        }
        Ok(())
    }

    pub fn finish(mut self, model: &PhysicalRegisterModel) -> Result<BodyCpuEffects, Error> {
        let mut registers = Vec::new();
        let mut reads = Vec::new();
        let mut floating_control = false;
        for unit in self.units.union(&self.reads) {
            let declaration = model.unit(*unit).ok_or(Error::Mismatch(
                "interrupt effect names unknown register unit",
            ))?;
            floating_control |= declaration.kind == RegisterUnitKind::FloatingControl;
            let state = match declaration.kind {
                RegisterUnitKind::IntegerLane => MachineState::GeneralRegisters,
                RegisterUnitKind::VectorLane => MachineState::VectorRegisters,
                RegisterUnitKind::Flags => MachineState::Flags,
                RegisterUnitKind::StackPointer => MachineState::StackPointer,
                RegisterUnitKind::InstructionPointer => MachineState::InstructionPointer,
                RegisterUnitKind::FloatingControl => MachineState::ControlState,
                RegisterUnitKind::Zero => continue,
            };
            self.states = self.states.union(MachineStateSet::new([state]));
            if self.units.contains(unit) {
                self.clobbered = self.clobbered.union(MachineStateSet::new([state]));
            }
            if self.reads.contains(unit)
                && let Some(register) = register_for_unit(model, *unit)
            {
                reads.push(register);
            }
            if self.units.contains(unit)
                && matches!(
                    declaration.kind,
                    RegisterUnitKind::IntegerLane
                        | RegisterUnitKind::VectorLane
                        | RegisterUnitKind::StackPointer
                )
            {
                if let Some(register) = register_for_unit(model, *unit) {
                    registers.push(register);
                } else if declaration.kind != RegisterUnitKind::StackPointer {
                    return Err(Error::Unsupported(
                        "entry register unit has no calling-plan identity",
                    ));
                }
            }
        }
        Ok(BodyCpuEffects {
            effects: StateEffectsEvidence::new(
                RegisterSet::new(reads),
                RegisterSet::new(registers),
                self.states,
                self.clobbered,
            ),
            generated_effects: StateEffectsEvidence::empty(),
            floating_control,
            opaque_control: false,
        })
    }
}

fn register_for_unit(
    model: &PhysicalRegisterModel,
    unit: RegisterUnitId,
) -> Option<MachineRegister> {
    use MachineRegister::*;
    if model.view_named("x0").is_some() {
        for index in 0..31 {
            if model
                .view_named(&format!("x{index}"))
                .is_some_and(|view| view.units.contains(&unit))
            {
                return Some(Aarch64X(index));
            }
        }
        for index in 0..32 {
            if model
                .view_named(&format!("q{index}"))
                .or_else(|| model.view_named(&format!("d{index}")))
                .is_some_and(|view| view.units.contains(&unit))
            {
                return Some(Aarch64V(index));
            }
        }
        return None;
    }
    let general = [
        ("rax", X86Rax),
        ("rcx", X86Rcx),
        ("rdx", X86Rdx),
        ("rbx", X86Rbx),
        ("rsp", X86Rsp),
        ("rbp", X86Rbp),
        ("rsi", X86Rsi),
        ("rdi", X86Rdi),
        ("r8", X86R8),
        ("r9", X86R9),
        ("r10", X86R10),
        ("r11", X86R11),
        ("r12", X86R12),
        ("r13", X86R13),
        ("r14", X86R14),
        ("r15", X86R15),
    ];
    for (name, register) in general {
        if model
            .view_named(name)
            .is_some_and(|view| view.units.contains(&unit))
        {
            return Some(register);
        }
    }
    (0..16)
        .find(|index| {
            model
                .view_named(&format!("xmm{index}"))
                .is_some_and(|view| view.units.contains(&unit))
        })
        .map(X86Xmm)
}

pub fn authenticated_callable_cpu_targets<'a>(
    body: &target_vocabulary::NativeFunctionPublication,
    invocations: &'a terminal_semantics::callable_values::ReachableCallableInvocations,
    machine: MachineId,
    operation: semantic_vocabulary::OperationId,
    callee: semantic_vocabulary::ValueId,
    contract: semantic_vocabulary::CallableContractId,
) -> Result<Option<&'a [MachineId]>, Error> {
    if body.call_target(operation)
        != Some(target_vocabulary::NativeCallTarget::Callable { callee, contract })
    {
        return Err(Error::Mismatch(
            "callable CPU footprint changed its evaluated source callee",
        ));
    }
    let Some(invocation) = invocations
        .operation(machine, operation)
        .map_err(Error::Mismatch)?
    else {
        if invocations
            .reached_machines
            .binary_search(&machine)
            .is_err()
        {
            return Err(Error::Unsupported(
                "callable CPU footprint lacks an admitted invocation context for its native caller",
            ));
        }
        // This authenticated source occurrence is outside every
        // feasible execution path from the observed entry.
        return Ok(None);
    };
    if invocation.invocation.target
        != terminal_semantics::callable_values::CallableInvocationTarget::Value(callee, contract)
        || invocation.invocation.boundary.is_some()
    {
        return Err(Error::Mismatch(
            "callable CPU footprint changed its retained invocation",
        ));
    }
    if !invocation.targets_are_resolved() {
        return Err(Error::Unsupported(
            "callable CPU footprint has unresolved code and no admitted physical state ceiling",
        ));
    }
    Ok(Some(invocation.selected_machines.as_slice()))
}

pub fn callable_cpu_targets<'a>(
    source: &selected_instructions::SelectedProgramContracts,
    invocations: &'a terminal_semantics::callable_values::ReachableCallableInvocations,
    machine: MachineId,
    owner: target_vocabulary::CallSiteOwner,
    target: target_vocabulary::NativeCallTarget,
) -> Result<Option<&'a [MachineId]>, Error> {
    let target_vocabulary::NativeCallTarget::Callable { callee, contract } = target else {
        return Err(Error::Mismatch(
            "callable CPU footprint has another native call target",
        ));
    };
    let target_vocabulary::CallSiteOwner::Operation(operation) = owner else {
        return Err(Error::Mismatch(
            "callable CPU footprint has another source invocation route",
        ));
    };
    let body = source
        .publication()
        .function(machine)
        .ok_or(Error::Mismatch(
            "callable CPU footprint has no current caller publication facts",
        ))?;
    authenticated_callable_cpu_targets(body, invocations, machine, operation, callee, contract)
}

pub fn check_mask_operation(
    preemption: Preemption,
    operation: terminal_psi::RegisterOperation,
) -> Result<(), Error> {
    use terminal_psi::{InterruptControlKind, RegisterOperation, SystemRegister};
    if preemption == Preemption::Masked
        && matches!(
            operation,
            RegisterOperation::InterruptControl(InterruptControlKind::Enable)
                | RegisterOperation::SystemWrite(SystemRegister::Rflags)
        )
    {
        return Err(Error::Unsupported(
            "masked interrupt handler can enable nested interrupts",
        ));
    }
    Ok(())
}

pub fn is_call(control: MachineEncodedControlEffect) -> bool {
    matches!(
        control,
        MachineEncodedControlEffect::DirectRelativeCallV1
            | MachineEncodedControlEffect::IndirectExternalCallV1
            | MachineEncodedControlEffect::IndirectTableCallOrTrapV1
            | MachineEncodedControlEffect::SupervisorCallV1
            | MachineEncodedControlEffect::HostedExitOrTrapV1
            | MachineEncodedControlEffect::HostedReadReturnOrTrapV1
            | MachineEncodedControlEffect::HostedWriteReturnOrTrapV1
    )
}

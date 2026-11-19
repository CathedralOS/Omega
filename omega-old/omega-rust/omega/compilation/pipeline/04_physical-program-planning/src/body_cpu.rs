//! Measure the closed selected instruction and frame footprint before encoding.
use calling_conventions::{
    MachineState, MachineStateSet, Preemption, RegisterSet, StateEffectsEvidence,
};
use installation_admission::cpu::{
    BodyCpuEffects, Error, Footprint, callable_cpu_targets, check_mask_operation, is_call,
};
use machine_code::SelectedFormDecodedFootprint;
use register_model::RegisterUnitKind;
use selected_instructions::MachineAlternativeFamily;
use semantic_vocabulary::MachineId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedBodyCpuState {
    effects: BodyCpuEffects,
    physical: [u8; 32],
    frame: machine_code::TargetFrameLayoutIdentity,
    entry: MachineId,
    preemption: Preemption,
}
impl PlannedBodyCpuState {
    pub fn observations(&self) -> &BodyCpuEffects {
        &self.effects
    }
    pub fn into_effects(self) -> StateEffectsEvidence {
        self.effects.into_effects()
    }
    pub fn require_body(
        &self,
        frame: &crate::PhysicalFrameProgram,
        entry: MachineId,
        preemption: Preemption,
    ) -> Result<(), Error> {
        if self.physical != frame.instructions.identity
            || self.frame != frame.frame.receipt().identity()
            || self.entry != entry
            || self.preemption != preemption
        {
            return Err(Error::Mismatch(
                "installed CPU-state measurement names another physical body or entry contract",
            ));
        }
        Ok(())
    }
}
impl std::ops::Deref for PlannedBodyCpuState {
    type Target = BodyCpuEffects;
    fn deref(&self) -> &Self::Target {
        &self.effects
    }
}

fn planned_call_effects(architecture: target::Architecture) -> StateEffectsEvidence {
    use calling_conventions::MachineRegister as R;
    use calling_conventions::MachineState as S;
    match architecture {
        target::Architecture::X86_64 => StateEffectsEvidence::new(
            RegisterSet::new([R::X86Rsp]),
            RegisterSet::new([R::X86Rsp]),
            MachineStateSet::new([S::StackPointer]),
            MachineStateSet::new([S::StackPointer, S::InstructionPointer]),
        ),
        target::Architecture::Aarch64 => StateEffectsEvidence::new(
            RegisterSet::default(),
            RegisterSet::new([R::Aarch64X(30)]),
            MachineStateSet::empty(),
            MachineStateSet::new([S::InstructionPointer]),
        ),
    }
}

pub fn plan(
    frame_program: &crate::PhysicalFrameProgram,
    entry: MachineId,
    preemption: Preemption,
) -> Result<PlannedBodyCpuState, Error> {
    let source = &frame_program.instructions;
    let target = source.machine.target;
    let callable_invocations = source
        .contracts
        .execution()
        .and_then(|facts| facts.callable_invocations_from(entry))
        .ok_or(Error::Mismatch(
            "entry has no exact prepared callable invocation summary",
        ))?;
    let model = source.physical().model();
    let call_effects = planned_call_effects(target.architecture);
    let mut footprint = Footprint::default();
    let mut generated_effects = StateEffectsEvidence::new(
        RegisterSet::default(),
        RegisterSet::default(),
        MachineStateSet::empty(),
        MachineStateSet::empty(),
    );
    let mut visited = arena::IdSet::<MachineId>::default();
    let mut pending = vec![entry];
    pending.extend(callable_invocations.roots.iter().copied());
    let mut frame_floating_control = false;
    let mut opaque_control = false;
    while let Some(machine) = pending.pop() {
        if !visited.insert(machine) {
            continue;
        }
        let function = source
            .machine
            .functions
            .iter()
            .find(|function| function.machine == machine)
            .ok_or(Error::Mismatch("interrupt callee lacks final native body"))?;
        let selected = source
            .function(machine)
            .ok_or(Error::Mismatch("interrupt callee lacks selected contract"))?;
        let frame_layout = frame_program
            .frame
            .plan()
            .functions
            .iter()
            .find(|frame| frame.machine == machine)
            .ok_or(Error::Mismatch(
                "entry callee lacks its actual frame layout",
            ))?;
        for restore in &frame_layout.unwind.restores {
            let view = model
                .view(restore.view)
                .ok_or(Error::Mismatch("entry frame names unknown register view"))?;
            for unit in view.units.iter().chain(&view.write_units) {
                let unit = model
                    .unit(*unit)
                    .ok_or(Error::Mismatch("entry frame names unknown register unit"))?;
                frame_floating_control |= unit.kind == RegisterUnitKind::FloatingControl;
            }
        }
        generated_effects =
            generated_effects.compose(frame_program.protocols.function_effects(machine).ok_or(
                Error::Mismatch("entry callee lacks its planned frame effects"),
            )?);
        for instruction in selected.instructions() {
            if let selected_instructions::SelectedInstructionKind::RegisterAccess { operation } =
                instruction.kind
            {
                check_mask_operation(preemption, operation)?;
            }
        }
        for row in function.instructions() {
            let instruction = selected
                .blocks
                .iter()
                .flat_map(|block| {
                    block.instructions.iter().chain(std::iter::once(
                        selected_instructions::terminator_instruction(&block.terminator),
                    ))
                })
                .find(|instruction| instruction.id == row.instruction)
                .ok_or(Error::Mismatch(
                    "physical instruction has no exact selected contract",
                ))?;
            if let Some(callee) = instruction
                .kind
                .call_target()
                .and_then(|target| target.machine())
            {
                generated_effects = generated_effects.compose(&call_effects);
                pending.push(callee);
                continue;
            }
            let effects = &row.alternative.encoded;
            let operands = |ordinals: &[u16]| {
                ordinals
                    .iter()
                    .map(|ordinal| row.operand_footprint(*ordinal).map(|operand| operand.view))
                    .collect::<Option<Vec<_>>>()
                    .ok_or(Error::Mismatch("physical effect has no exact operand"))
            };
            let encoded = SelectedFormDecodedFootprint {
                register_reads: operands(&effects.external_operand_reads)?,
                register_writes: operands(&effects.external_operand_writes)?,
                implicit_defs: effects.implicit_unit_defs.clone(),
                implicit_clobbers: effects.implicit_unit_clobbers.clone(),
                encoded: effects.clone(),
            };
            if is_call(encoded.encoded.control) {
                let instruction = selected
                    .instructions()
                    .find(|instruction| instruction.id == row.instruction)
                    .ok_or(Error::Mismatch(
                        "native call has no exact selected instruction",
                    ))?;
                if matches!(
                    instruction.kind.call_target(),
                    Some(selected_instructions::SelectedCallTarget::Callable { .. })
                ) {
                    let call =
                        selected_instructions::calls::dynamic::call_record(selected, instruction)
                            .ok_or(Error::Mismatch(
                            "callable CPU footprint lost its selected call contract",
                        ))?;
                    if let Some(callees) = callable_cpu_targets(
                        &source.contracts,
                        callable_invocations,
                        machine,
                        call.owner,
                        call.call.target,
                    )? {
                        pending.extend(callees.iter().copied());
                    }
                    footprint.decoded(model, &encoded)?;
                    continue;
                }
                let call = selected
                    .normalized_foreign_calls
                    .iter()
                    .find(|call| call.instruction == row.instruction)
                    .ok_or(Error::Unsupported(
                        "interrupt indirect call lacks admitted provider state",
                    ))?;
                if !call.call.callbacks.is_empty() {
                    return Err(Error::Unsupported(
                        "interrupt provider callback closure has no retained state footprint",
                    ));
                }
                if preemption == Preemption::Masked
                    && call.call.binding.boundary_entry_plan.state.preemption != Preemption::Masked
                {
                    return Err(Error::Unsupported(
                        "opaque interrupt callee does not promise masked execution",
                    ));
                }
                let ceiling = call
                    .call
                    .binding
                    .boundary_entry_plan
                    .state
                    .permitted_transitive_use;
                opaque_control |= !ceiling
                    .intersection(MachineStateSet::new([
                        MachineState::ControlState,
                        MachineState::ExtendedState,
                    ]))
                    .is_empty();
                footprint.add_states(ceiling, ceiling);
            }
            footprint.decoded(model, &encoded)?;
            match row.alternative.key.family {
                MachineAlternativeFamily::CodeSegmentReload
                | MachineAlternativeFamily::DataSegmentReload => {
                    footprint.state(MachineState::ControlState);
                    footprint.state(MachineState::SegmentState);
                    footprint.clobber(MachineState::SegmentState);
                }
                MachineAlternativeFamily::SegmentSelectorRead => {
                    footprint.state(MachineState::SegmentState);
                }
                MachineAlternativeFamily::ControlRegisterRead
                | MachineAlternativeFamily::GlobalDescriptorTableLoad
                | MachineAlternativeFamily::GlobalDescriptorTableStore
                | MachineAlternativeFamily::TaskRegisterActivate
                | MachineAlternativeFamily::InstalledInterruptCpuCheck
                | MachineAlternativeFamily::InstalledInterruptTaskCommit
                | MachineAlternativeFamily::InstalledInterruptTablePublication
                | MachineAlternativeFamily::TaskRegisterRead
                | MachineAlternativeFamily::ControlRegisterWrite => {
                    footprint.state(MachineState::ControlState);
                    if matches!(
                        row.alternative.key.family,
                        MachineAlternativeFamily::GlobalDescriptorTableLoad
                            | MachineAlternativeFamily::TaskRegisterActivate
                            | MachineAlternativeFamily::InstalledInterruptTaskCommit
                            | MachineAlternativeFamily::InstalledInterruptTablePublication
                            | MachineAlternativeFamily::ControlRegisterWrite
                    ) {
                        footprint.clobber(MachineState::ControlState);
                    }
                }
                MachineAlternativeFamily::ModelSpecificRead
                | MachineAlternativeFamily::ModelSpecificWrite => {
                    if target.architecture != target::Architecture::X86_64 {
                        return Err(Error::Unsupported(
                            "MSR architectural state requires x86-64",
                        ));
                    }
                    let instruction = selected
                        .instructions()
                        .find(|instruction| instruction.id == row.instruction)
                        .ok_or(Error::Mismatch(
                            "MSR row has no function-local selected instruction",
                        ))?;
                    let expected = if row.alternative.key.family
                        == MachineAlternativeFamily::ModelSpecificRead
                    {
                        selected_instructions::RegisterOperation::ModelSpecificRead
                    } else {
                        selected_instructions::RegisterOperation::ModelSpecificWrite
                    };
                    if instruction.kind
                        != (selected_instructions::SelectedInstructionKind::RegisterAccess {
                            operation: expected,
                        })
                    {
                        return Err(Error::Mismatch("MSR row and selected operation disagree"));
                    }
                    let selector = installation_admission::selected::selected_constant_operand(
                        selected,
                        row.instruction,
                        0,
                    )?
                    .and_then(|value| u32::try_from(value).ok());
                    let architectural =
                        target_x86_64::x86_64_model_specific_register_state(expected, selector)
                            .map_err(Error::Unsupported)?;
                    footprint.add_states(
                        architectural.machine_state(),
                        architectural.clobbered_state(),
                    );
                }
                _ => {}
            }
        }
    }
    let effects = footprint.finish(model)?.with_generated_effects(
        generated_effects,
        frame_floating_control,
        opaque_control,
    );
    Ok(PlannedBodyCpuState {
        effects,
        physical: source.identity,
        frame: frame_program.frame.receipt().identity(),
        entry,
        preemption,
    })
}

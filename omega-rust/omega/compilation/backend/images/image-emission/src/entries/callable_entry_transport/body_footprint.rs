//! Collect state touched by the admitted native body and its actual callees.
//!
//! A call's allocation clobbers describe what its ABI permits, not what its
//! checked callee writes. The x86 direct-call template is only CALL rel32;
//! replace that summary with its stack/control transport and visit the callee.
//! Ordinary forms keep their retained decoded effects. No bytes are decoded here.

use std::collections::{BTreeMap, BTreeSet};

use calling_conventions::{
    MachineRegister, MachineState, MachineStateSet, Preemption, RegisterSet, StateFootprintEvidence,
};
use machine_code::{
    ResolvedBranchEvidence, SelectedFormDecodedFootprint, SelectedFormEncodingRow,
    SelectedFormInternalMachineFixupKind,
};
use register_model::{PhysicalRegisterModel, RegisterUnitId, RegisterUnitKind, RegisterViewId};
use selected_instructions::{
    MachineAlternativeFamily, MachineEncodedControlEffect, MachineEncodedEffects,
    MachineEncodedStackEffect, SelectedInstructionId,
};

use crate::{FunctionFragmentObjectArtifactError as Error, ObjectArtifact};
use semantic_vocabulary::MachineId;

pub(super) fn collect(
    body: &ObjectArtifact,
    preemption: Preemption,
) -> Result<StateFootprintEvidence, Error> {
    if body.target.architecture != target::Architecture::X86_64 {
        return Err(Error::Unsupported(
            "interrupt body footprint requires x86-64 effects",
        ));
    }
    let replay = body.fragment_replay.as_ref().ok_or(Error::Mismatch(
        "interrupt body lacks retained emission custody",
    ))?;
    let source = replay.0.source().source().source().source();
    let model = source.register_environment().physical().model();
    let encoding = source.encoding();
    // Omega 07 writes rows in this retained physical order. Each physical
    // block already includes its terminator, unlike SelectedFunction::instructions.
    // Instruction numbers restart in each function, so both coordinates matter.
    let encoded_rows = index_encoded_rows(
        source
            .program()
            .machine
            .functions
            .iter()
            .flat_map(|function| {
                function
                    .instructions()
                    .map(|instruction| (function.machine, instruction.instruction))
            }),
        encoding.rows(),
    )?;
    let mut footprint = Footprint::default();
    let mut visited = BTreeSet::new();
    let mut pending = vec![body.entry];
    while let Some(machine) = pending.pop() {
        if !visited.insert(machine) {
            continue;
        }
        let function = source
            .resolved_layout()
            .function(machine)
            .ok_or(Error::Mismatch("interrupt callee lacks final native body"))?;
        let selected = source
            .selected_plan()
            .function(machine)
            .ok_or(Error::Mismatch("interrupt callee lacks selected contract"))?;
        let frame = source
            .frame_layout()
            .functions
            .iter()
            .find(|frame| frame.machine == machine)
            .ok_or(Error::Mismatch("interrupt callee lacks admitted frame"))?;
        // A handler promised to run with interrupts masked cannot enable
        // nested delivery, even if IRETQ would restore the original flags later.
        for instruction in selected.instructions() {
            if let selected_instructions::SelectedInstructionKind::RegisterAccess { operation } =
                instruction.kind
            {
                check_mask_operation(preemption, operation)?;
            }
        }
        footprint.frame(model, frame)?;
        for row in function.blocks.iter().flat_map(|block| &block.instructions) {
            if let Some(fixup) = row.internal_machine_fixup() {
                if fixup.kind != SelectedFormInternalMachineFixupKind::X86Relative32FromNextInstructionToInternalMachineV1 {
                    return Err(Error::Unsupported("interrupt internal call has another transport"));
                }
                // This admitted fixup belongs to the five-byte CALL template,
                // not an argument-copy envelope. The callee supplies its writes.
                footprint.direct_call(model)?;
                pending.push(fixup.callee);
                continue;
            }
            if let Some(branch) = &row.branch {
                let effects = match branch.as_ref() {
                    ResolvedBranchEvidence::Conditional(branch) => {
                        for view in &branch.decoded_register_reads {
                            footprint.read_view(model, *view)?;
                        }
                        &branch.decoded_effects
                    }
                    ResolvedBranchEvidence::Jump(branch) => &branch.decoded_effects,
                };
                footprint.effects(model, effects)?;
                continue;
            }
            let encoded = encoded_rows
                .get(&(machine, row.instruction))
                .and_then(|row| encoding.encoded().footprint(row.footprint))
                .ok_or(Error::Mismatch(
                    "interrupt instruction lacks retained encoded effects",
                ))?;
            if is_call(encoded.encoded.control) {
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
                // An opaque implementation has a class ceiling rather than an
                // inspectable body. Keep that admitted ceiling AND all encoded
                // transport effects; never narrow it to the result register.
                footprint.states = footprint.states.union(
                    call.call
                        .binding
                        .boundary_entry_plan
                        .state
                        .permitted_transitive_use,
                );
            }
            footprint.decoded(model, encoded)?;
            match row.alternative.family {
                MachineAlternativeFamily::ControlRegisterRead
                | MachineAlternativeFamily::GlobalDescriptorTableLoad
                | MachineAlternativeFamily::GlobalDescriptorTableStore
                | MachineAlternativeFamily::TaskRegisterActivate
                | MachineAlternativeFamily::TaskRegisterRead
                | MachineAlternativeFamily::CodeSegmentReload
                | MachineAlternativeFamily::DataSegmentReload
                | MachineAlternativeFamily::SegmentSelectorRead
                | MachineAlternativeFamily::ControlRegisterWrite => {
                    footprint.state(MachineState::ControlState)
                }
                // A dynamic MSR selector can affect several architectural state
                // classes. The current emitted row does not retain that summary.
                MachineAlternativeFamily::ModelSpecificRead
                | MachineAlternativeFamily::ModelSpecificWrite => {
                    return Err(Error::Unsupported(
                        "interrupt MSR access lacks architectural state footprint",
                    ));
                }
                _ => {}
            }
        }
    }
    footprint.finish(model)
}

/// Join the admitted producer's complete physical order to its encoded rows.
/// The caller supplies physical instructions, including every block terminator.
fn index_encoded_rows(
    owners: impl IntoIterator<Item = (MachineId, SelectedInstructionId)>,
    rows: &[SelectedFormEncodingRow],
) -> Result<BTreeMap<(MachineId, SelectedInstructionId), &SelectedFormEncodingRow>, Error> {
    let mut encoded = rows.iter();
    let mut indexed = BTreeMap::new();
    for owner in owners {
        let row = encoded.next().ok_or(Error::Mismatch(
            "interrupt encoding omits a physical instruction",
        ))?;
        if row.instruction != owner.1 {
            return Err(Error::Mismatch(
                "interrupt encoding differs from physical instruction order",
            ));
        }
        if indexed.insert(owner, row).is_some() {
            return Err(Error::Mismatch(
                "interrupt function repeats a physical instruction identity",
            ));
        }
    }
    if encoded.next().is_some() {
        return Err(Error::Mismatch(
            "interrupt encoding has an unowned physical instruction",
        ));
    }
    Ok(indexed)
}

fn check_mask_operation(
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

fn is_call(control: MachineEncodedControlEffect) -> bool {
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

#[derive(Default)]
struct Footprint {
    units: BTreeSet<RegisterUnitId>,
    reads: BTreeSet<RegisterUnitId>,
    states: MachineStateSet,
}

impl Footprint {
    fn state(&mut self, state: MachineState) {
        self.states = self.states.union(MachineStateSet::new([state]));
    }

    fn view(&mut self, model: &PhysicalRegisterModel, view: RegisterViewId) -> Result<(), Error> {
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

    fn read_view(
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

    fn decoded(
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

    fn effects(
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
            }
        }
        if effects.control != MachineEncodedControlEffect::FallThroughV1 {
            self.state(MachineState::InstructionPointer);
        }
        Ok(())
    }

    fn direct_call(&mut self, model: &PhysicalRegisterModel) -> Result<(), Error> {
        self.view(
            model,
            model
                .view_named("rsp")
                .ok_or(Error::Mismatch("interrupt call lacks stack register"))?
                .id,
        )?;
        self.state(MachineState::StackPointer);
        self.state(MachineState::InstructionPointer);
        Ok(())
    }

    fn frame(
        &mut self,
        model: &PhysicalRegisterModel,
        frame: &machine_code::FunctionTargetFrameLayout,
    ) -> Result<(), Error> {
        if frame.frame_size_bytes != frame.red_zone_resident_bytes {
            self.view(model, frame.stack_pointer)?;
            self.state(MachineState::StackPointer);
            // The admitted x86 fixed-frame protocol uses SUB/ADD RSP; its
            // optional page-touch CMP also writes flags and no other register.
            self.state(MachineState::Flags);
        }
        for slot in &frame.callee_save_slots {
            self.view(model, slot.storage_view)?;
        }
        Ok(())
    }

    fn finish(mut self, model: &PhysicalRegisterModel) -> Result<StateFootprintEvidence, Error> {
        let mut registers = Vec::new();
        for unit in self.units.union(&self.reads) {
            let declaration = model.unit(*unit).ok_or(Error::Mismatch(
                "interrupt effect names unknown register unit",
            ))?;
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
            if self.units.contains(unit)
                && matches!(
                    declaration.kind,
                    RegisterUnitKind::IntegerLane
                        | RegisterUnitKind::VectorLane
                        | RegisterUnitKind::StackPointer
                )
            {
                let register = x86_register_for_unit(model, *unit).ok_or(Error::Unsupported(
                    "interrupt register unit has no calling-plan identity",
                ))?;
                registers.push(register);
            }
        }
        Ok(StateFootprintEvidence::new(
            RegisterSet::new(registers),
            self.states,
        ))
    }
}

fn x86_register_for_unit(
    model: &PhysicalRegisterModel,
    unit: RegisterUnitId,
) -> Option<MachineRegister> {
    use MachineRegister::*;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masked_handlers_cannot_enable_interrupts_or_restore_unknown_flags() {
        use terminal_psi::{InterruptControlKind, RegisterOperation, SystemRegister};
        assert!(
            check_mask_operation(
                Preemption::Masked,
                RegisterOperation::InterruptControl(InterruptControlKind::Disable)
            )
            .is_ok()
        );
        assert!(
            check_mask_operation(
                Preemption::Masked,
                RegisterOperation::SystemRead(SystemRegister::Rflags)
            )
            .is_ok()
        );
        assert!(
            check_mask_operation(
                Preemption::Masked,
                RegisterOperation::InterruptControl(InterruptControlKind::Enable)
            )
            .is_err()
        );
        assert!(
            check_mask_operation(
                Preemption::Masked,
                RegisterOperation::SystemWrite(SystemRegister::Rflags)
            )
            .is_err()
        );
    }

    #[test]
    fn checked_call_transport_does_not_invent_vector_writes() {
        let model = &isa_x86_64::x86_64_physical_register_model();
        let mut footprint = Footprint::default();
        footprint.direct_call(model).unwrap();
        footprint
            .view(model, model.view_named("eax").unwrap().id)
            .unwrap();
        let result = footprint.finish(model).unwrap();
        assert!(result.registers().contains(MachineRegister::X86Rax));
        assert!(result.registers().contains(MachineRegister::X86Rsp));
        assert!(
            !result
                .machine_state()
                .contains_all(MachineStateSet::new([MachineState::VectorRegisters]))
        );
        assert!(result.machine_state().contains_all(MachineStateSet::new([
            MachineState::InstructionPointer,
            MachineState::StackPointer
        ])));
    }

    #[test]
    fn vector_flags_and_unknown_units_remain_visible() {
        let model = &isa_x86_64::x86_64_physical_register_model();
        let mut footprint = Footprint::default();
        footprint.direct_call(model).unwrap();
        footprint
            .view(model, model.view_named("xmm3").unwrap().id)
            .unwrap();
        footprint
            .units
            .extend(&model.view_named("rflags").unwrap().write_units);
        let result = footprint.finish(model).unwrap();
        assert!(result.registers().contains(MachineRegister::X86Xmm(3)));
        assert!(result.machine_state().contains_all(MachineStateSet::new([
            MachineState::VectorRegisters,
            MachineState::Flags
        ])));
        let mut invalid = Footprint::default();
        invalid.units.insert(RegisterUnitId(u16::MAX));
        assert!(invalid.finish(model).is_err());
    }

    #[test]
    fn read_only_vector_use_is_not_reported_as_a_register_write() {
        let model = &isa_x86_64::x86_64_physical_register_model();
        let mut footprint = Footprint::default();
        footprint
            .read_view(model, model.view_named("xmm5").unwrap().id)
            .unwrap();
        let result = footprint.finish(model).unwrap();
        assert!(
            result
                .machine_state()
                .contains_all(MachineStateSet::new([MachineState::VectorRegisters]))
        );
        assert!(!result.registers().contains(MachineRegister::X86Xmm(5)));
    }

    #[test]
    fn repeated_instruction_numbers_keep_each_functions_own_effects() {
        use machine_code::{
            SelectedFormEncodedRowStore, SelectedFormEncodingState, SelectedFormMachineDisposition,
        };
        use selected_instructions::MachineAlternativeKey;
        let model = &isa_x86_64::x86_64_physical_register_model();
        let mut store = SelectedFormEncodedRowStore::new();
        let mut make_row = |instruction, view: Option<&str>| {
            let writes = view
                .into_iter()
                .map(|name| model.view_named(name).unwrap().id)
                .collect();
            // Synthetic encoded records isolate the owner join; this test does
            // not grant byte admission or claim that these rows are executable.
            let (code, footprint) = store.intern(
                &[],
                SelectedFormDecodedFootprint {
                    register_reads: vec![],
                    register_writes: writes,
                    implicit_defs: vec![],
                    implicit_clobbers: vec![],
                    encoded: MachineEncodedEffects::fallthrough_v1(vec![], vec![]),
                },
            );
            SelectedFormEncodingRow {
                instruction: SelectedInstructionId(instruction),
                alternative: MachineAlternativeKey {
                    family: MachineAlternativeFamily::CopyI64,
                    variant: 0,
                },
                machine_disposition: SelectedFormMachineDisposition::RetainedV1,
                state: SelectedFormEncodingState::Encoded,
                address: None,
                code,
                footprint,
            }
        };
        let rows = [
            make_row(0, Some("eax")),
            make_row(1, None),
            make_row(0, Some("xmm3")),
            make_row(1, None),
        ];
        let first = MachineId::new(10).unwrap();
        let second = MachineId::new(20).unwrap();
        // Each function has instruction 0 followed by terminator 1.
        let owners = [
            (first, SelectedInstructionId(0)),
            (first, SelectedInstructionId(1)),
            (second, SelectedInstructionId(0)),
            (second, SelectedInstructionId(1)),
        ];
        let indexed = index_encoded_rows(owners, &rows).unwrap();
        let mut scalar = Footprint::default();
        scalar
            .decoded(
                model,
                store
                    .footprint(indexed[&(first, SelectedInstructionId(0))].footprint)
                    .unwrap(),
            )
            .unwrap();
        let scalar = scalar.finish(model).unwrap();
        assert!(scalar.registers().contains(MachineRegister::X86Rax));
        assert!(
            !scalar
                .machine_state()
                .contains_all(MachineStateSet::new([MachineState::VectorRegisters]))
        );
        let mut vector = Footprint::default();
        vector
            .decoded(
                model,
                store
                    .footprint(indexed[&(second, SelectedInstructionId(0))].footprint)
                    .unwrap(),
            )
            .unwrap();
        assert!(
            vector
                .finish(model)
                .unwrap()
                .registers()
                .contains(MachineRegister::X86Xmm(3))
        );
        // Omitting a terminator cannot shift the following function's owner.
        assert!(index_encoded_rows([owners[0], owners[2], owners[3]], &rows).is_err());
        assert!(index_encoded_rows(owners, &rows[..3]).is_err());
        assert!(index_encoded_rows(owners[..3].iter().copied(), &rows).is_err());
    }
}

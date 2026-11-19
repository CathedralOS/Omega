/*
We bind execution entry points to sealed object bodies: `physical_entry` binds
incoming CPU and stack-switch requirements, `callable_entry_transport` pairs a
callable entry bridge with its body footprint, `hosted_entry_storage`,
`hosted_receiver`, and `hosted_unit_entry` synthesize and verify hosted process
and receiver entry shims on x86-64 and AArch64, and `foreign_adapter` emits
standalone native foreign-call adapter images.
*/

pub(crate) mod callable_entry_transport {
    mod body_footprint {
        #[cfg(test)]
        use installation_admission::cpu::{
            Error as CpuError, authenticated_callable_cpu_targets, check_mask_operation,
        };
        use installation_admission::cpu::{Footprint, callable_cpu_targets, is_call};

        use calling_conventions::{
            MachineState, MachineStateSet, Preemption, RegisterSet, StateEffectsEvidence,
        };
        use machine_code::{
            ResolvedBranchEvidence, SelectedFormInternalMachineFixupKind,
            placed_body::PlacedInstructionEffects,
        };
        use register_model::RegisterUnitKind;
        use selected_instructions::{MachineAlternativeFamily, SelectedInstructionId};

        use crate::{FunctionFragmentObjectArtifactError as Error, ObjectArtifact};
        use semantic_vocabulary::MachineId;

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct NativeBodyCpuState(installation_admission::cpu::BodyCpuEffects);

        impl NativeBodyCpuState {
            pub fn observations(&self) -> &installation_admission::cpu::BodyCpuEffects {
                &self.0
            }

            pub fn effects(&self) -> &StateEffectsEvidence {
                self.0.effects()
            }
            pub fn generated_effects(&self) -> &StateEffectsEvidence {
                self.0.generated_effects()
            }
            pub fn floating_control(&self) -> bool {
                self.0.floating_control()
            }
            pub fn require_without_floating_state(&self) -> Result<(), Error> {
                self.0.require_without_floating_state().map_err(Into::into)
            }
            pub(super) fn into_effects(self) -> StateEffectsEvidence {
                self.0.into_effects()
            }
        }

        pub(super) fn collect(
            body: &ObjectArtifact,
            preemption: Preemption,
        ) -> Result<NativeBodyCpuState, Error> {
            collect_from(body, body.entry, preemption)
        }

        pub(super) fn collect_from(
            body: &ObjectArtifact,
            entry: MachineId,
            preemption: Preemption,
        ) -> Result<NativeBodyCpuState, Error> {
            let source = body.emission_input.readback();
            let callable_invocations = crate::callable_calls::prepare_with(
                body.call_origins(),
                &body.physical_facts().contracts,
                entry,
            )
            .map_err(|reason| Error::PhysicalEntry(diagnostics::Diagnostic::error(reason)))?;
            let model = source.physical.model();
            let origins = &body.emission_input.emission_facts().physical.cpu;
            let encoded_rows = index_encoded_rows(
                origins.instruction_roster.iter().copied(),
                &source.instruction_effects,
            )?;
            let call_effects = direct_call_effects(body.target.architecture)?;
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
                    .layout
                    .function(machine)
                    .ok_or(Error::Mismatch("interrupt callee lacks final native body"))?;
                let origin = origins.function(machine).ok_or(Error::Mismatch(
                    "interrupt callee lacks physical CPU origins",
                ))?;
                let protocols = &source.protocol;
                let frame = protocols
                    .functions
                    .iter()
                    .find(|frame| frame.machine == machine)
                    .ok_or(Error::Mismatch(
                        "entry callee lacks its actual frame protocol",
                    ))?;
                let frame_layout = source
                    .frame
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
                for span in [frame.prologue, frame.epilogue] {
                    let bytes = span.bytes(&protocols.bytes).ok_or(Error::Mismatch(
                        "entry frame protocol has a stale byte span",
                    ))?;
                    let decoded = match body.target.architecture {
                        target::Architecture::X86_64 => target_x86_64::decode_x86_64_generated_entry(bytes, None),
                        target::Architecture::Aarch64 => target_aarch64::decode_aarch64_generated_entry(bytes, None),
                    }.map_err(|reason| Error::NativeEntryBridge(physical_recipes::native_entry_bridge::NativeEntryBridgeError::GeneratedEntryEffects(reason)))?;
                    generated_effects = generated_effects.compose(&decoded.effects);
                }
                if preemption == Preemption::Masked && !origin.unmasks_interrupts.is_empty() {
                    return Err(Error::Unsupported(
                        "masked interrupt handler can enable nested interrupts",
                    ));
                }
                for row in function.blocks.iter().flat_map(|block| &block.instructions) {
                    if let Some(fixup) = row.internal_machine_fixup() {
                        if !matches!((body.target.architecture, fixup.kind),
                            (target::Architecture::X86_64, SelectedFormInternalMachineFixupKind::X86Relative32FromNextInstructionToInternalMachineV1)
                            | (target::Architecture::Aarch64, SelectedFormInternalMachineFixupKind::Aarch64BranchLinkImmediate26FromInstructionToInternalMachineV1)) {
                            return Err(Error::Unsupported("entry internal call has another target transport"));
                        }
                        generated_effects = generated_effects.compose(&call_effects);
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
                    let encoded = encoded_row(&encoded_rows, machine, row.instruction)
                        .and_then(PlacedInstructionEffects::footprint)
                        .ok_or(Error::Mismatch(
                            "interrupt instruction lacks retained encoded effects",
                        ))?;
                    if is_call(encoded.encoded.control) {
                        let mut targets = origin
                            .indirect_calls
                            .iter()
                            .filter(|(instruction, _)| *instruction == row.instruction);
                        let target = targets.next().map(|(_, target)| *target);
                        if targets.next().is_some() {
                            return Err(Error::Mismatch(
                                "native call has ambiguous physical source",
                            ));
                        }
                        if matches!(
                            target,
                            Some(selected_instructions::SelectedCallTarget::Callable { .. })
                        ) {
                            let calls =
                                body.call_origins()
                                    .function(machine)
                                    .ok_or(Error::Mismatch(
                                        "callable CPU footprint lost its physical caller",
                                    ))?;
                            let mut matching = calls
                                .calls
                                .iter()
                                .filter(|call| call.instruction == row.instruction);
                            let call = matching.next().ok_or(Error::Mismatch(
                                "callable CPU footprint lost its selected call contract",
                            ))?;
                            if matching.next().is_some()
                                || call
                                    .occurrence
                                    .as_ref()
                                    .is_none_or(|occurrence| occurrence.target != target)
                            {
                                return Err(Error::Mismatch(
                                    "callable CPU footprint lost its selected call contract",
                                ));
                            }
                            if let Some(callees) = callable_cpu_targets(
                                &body.physical_facts().contracts,
                                callable_invocations,
                                machine,
                                call.owner,
                                call.call.target,
                            )? {
                                pending.extend(callees.iter().copied());
                            }
                            footprint.decoded(model, encoded)?;
                            continue;
                        }
                        let plans = body.foreign_call_plans();
                        let mut calls = plans.calls.iter().filter(|call| {
                            call.machine == machine && call.record.instruction == row.instruction
                        });
                        let call = &calls
                            .next()
                            .ok_or(Error::Unsupported(
                                "interrupt indirect call lacks admitted provider state",
                            ))?
                            .record;
                        if calls.next().is_some() {
                            return Err(Error::Mismatch(
                                "interrupt indirect call has ambiguous physical source",
                            ));
                        }
                        if !call.call.callbacks.is_empty() {
                            return Err(Error::Unsupported(
                                "interrupt provider callback closure has no retained state footprint",
                            ));
                        }
                        if preemption == Preemption::Masked
                            && call.call.binding.boundary_entry_plan.state.preemption
                                != Preemption::Masked
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
                    footprint.decoded(model, encoded)?;
                    match row.alternative.family {
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
                                row.alternative.family,
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
                            if body.target.architecture != target::Architecture::X86_64 {
                                return Err(Error::Unsupported(
                                    "MSR architectural state requires x86-64",
                                ));
                            }
                            let access = if row.alternative.family
                                == MachineAlternativeFamily::ModelSpecificRead
                            {
                                machine_code::cpu_origins::ModelSpecificAccess::Read
                            } else {
                                machine_code::cpu_origins::ModelSpecificAccess::Write
                            };
                            let mut registers = origin
                                .model_specific_registers
                                .iter()
                                .filter(|query| query.instruction == row.instruction);
                            let query = registers.next().ok_or(Error::Mismatch(
                                "MSR row has no function-local physical source",
                            ))?;
                            if registers.next().is_some() || query.access != access {
                                return Err(Error::Mismatch(
                                    "MSR row and selected operation disagree",
                                ));
                            }
                            let expected = match access {
                                machine_code::cpu_origins::ModelSpecificAccess::Read => {
                                    selected_instructions::RegisterOperation::ModelSpecificRead
                                }
                                machine_code::cpu_origins::ModelSpecificAccess::Write => {
                                    selected_instructions::RegisterOperation::ModelSpecificWrite
                                }
                            };
                            let selector = query
                                .selector
                                .as_ref()
                                .map_err(|detail| Error::PhysicalSourceObservation(detail.clone()))?
                                .and_then(|value| u32::try_from(value).ok());
                            let architectural =
                                target_x86_64::x86_64_model_specific_register_state(
                                    expected, selector,
                                )
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
            Ok(NativeBodyCpuState(effects))
        }

        fn index_encoded_rows(
            owners: impl IntoIterator<Item = (MachineId, SelectedInstructionId)>,
            rows: &[PlacedInstructionEffects],
        ) -> Result<Vec<&PlacedInstructionEffects>, Error> {
            let mut encoded = rows.iter();
            let mut indexed = Vec::with_capacity(rows.len());
            for owner in owners {
                let row = encoded.next().ok_or(Error::Mismatch(
                    "interrupt encoding omits a physical instruction",
                ))?;
                if (row.machine, row.instruction) != owner {
                    return Err(Error::Mismatch(
                        "interrupt encoding differs from physical instruction order",
                    ));
                }
                indexed.push(row);
            }
            if encoded.next().is_some() {
                return Err(Error::Mismatch(
                    "interrupt encoding has an unowned physical instruction",
                ));
            }
            indexed.sort_unstable_by_key(|row| (row.machine, row.instruction));
            if indexed.windows(2).any(|pair| {
                pair[0].machine == pair[1].machine && pair[0].instruction == pair[1].instruction
            }) {
                return Err(Error::Mismatch(
                    "interrupt function repeats a physical instruction identity",
                ));
            }
            Ok(indexed)
        }

        fn encoded_row<'a>(
            rows: &[&'a PlacedInstructionEffects],
            machine: MachineId,
            instruction: SelectedInstructionId,
        ) -> Option<&'a PlacedInstructionEffects> {
            rows.binary_search_by_key(&(machine, instruction), |row| {
                (row.machine, row.instruction)
            })
            .ok()
            .map(|position| rows[position])
        }

        fn direct_call_effects(
            architecture: target::Architecture,
        ) -> Result<StateEffectsEvidence, Error> {
            let decoded = match architecture {
                target::Architecture::X86_64 => target_x86_64::decode_x86_64_generated_entry(
                    &target_x86_64::ORDINARY_DIRECT_CALL_BYTES,
                    Some(1),
                ),
                target::Architecture::Aarch64 => target_aarch64::decode_aarch64_generated_entry(
                    &target_aarch64::ORDINARY_DIRECT_CALL_BYTES,
                    Some(0),
                ),
            }
            .map_err(|reason| {
                Error::NativeEntryBridge(
                    physical_recipes::native_entry_bridge::NativeEntryBridgeError::GeneratedEntryEffects(
                        reason,
                    ),
                )
            })?;
            Ok(decoded.effects)
        }

        #[cfg(test)]
        mod tests {
            use super::*;
            use calling_conventions::MachineRegister;
            use machine_code::SelectedFormDecodedFootprint;
            use register_model::RegisterUnitId;
            use selected_instructions::MachineEncodedEffects;

            #[test]
            fn callable_cpu_targets_need_the_exact_source_occurrence_and_resolved_context() {
                use semantic_vocabulary::{
                    BlockId, CallableContractId, CallableInvocationSite, InvocationSite,
                    MachineDescriptorId, OperationId, ValueId,
                };
                use terminal_semantics::callable_values::{
                    CallableInvocation, CallableInvocationTarget, CallableValuePossibilities,
                    ReachableCallableInvocation, ReachableCallableInvocations,
                    ReachedCallableTarget,
                };
                let machine = MachineId::new(1).unwrap();
                let target = MachineId::new(2).unwrap();
                let operation = OperationId::new(3).unwrap();
                let callee = ValueId::new(4).unwrap();
                let contract = CallableContractId::new(5).unwrap();
                let body = target_vocabulary::NativeFunctionPublication {
                    machine,
                    operation_count: 1,
                    sites: vec![(
                        target_vocabulary::publication::PsiProvenance::Operation(operation),
                        0,
                    )],
                    byte_literals: Vec::new(),
                    call_targets: vec![(
                        operation,
                        target_operations::NativeCallTarget::Callable { callee, contract },
                    )],
                };
                let context = ReachableCallableInvocations {
                    roots: vec![machine],
                    reached_machines: vec![machine, target],
                    inputs: Vec::new(),
                    invocations: vec![ReachableCallableInvocation {
                        caller: machine,
                        invocation: CallableInvocation {
                            site: CallableInvocationSite::Invocation(InvocationSite::Operation(
                                operation,
                            )),
                            operation: Some(operation),
                            boundary: None,
                            position: (BlockId::new(1).unwrap(), 0),
                            successor: None,
                            target: CallableInvocationTarget::Value(callee, contract),
                            scalar_arguments: Vec::new(),
                            structural_arguments: Vec::new(),
                            structural_actual_types: Vec::new(),
                        },
                        target: ReachedCallableTarget::Callable(CallableValuePossibilities {
                            descriptors: vec![MachineDescriptorId::new(1).unwrap()],
                            inputs: Vec::new(),
                            unresolved_ceilings: Vec::new(),
                        }),
                        selected_machines: vec![target],
                    }],
                };
                let query = |body: &_, context: &ReachableCallableInvocations| {
                    authenticated_callable_cpu_targets(
                        body, context, machine, operation, callee, contract,
                    )
                    .map(|targets| targets.map(<[MachineId]>::to_vec))
                };
                assert_eq!(query(&body, &context).unwrap(), Some(vec![target]));
                let mut foreign = body.clone();
                foreign.call_targets[0].1 = target_operations::NativeCallTarget::Callable {
                    callee: ValueId::new(6).unwrap(),
                    contract,
                };
                assert!(matches!(
                    query(&foreign, &context),
                    Err(CpuError::Mismatch(
                        "callable CPU footprint changed its evaluated source callee"
                    ))
                ));
                foreign.call_targets.clear();
                assert!(query(&foreign, &context).is_err());
                let mut unresolved = context.clone();
                let ReachedCallableTarget::Callable(values) = &mut unresolved.invocations[0].target
                else {
                    panic!("the test uses callable values")
                };
                values.unresolved_ceilings.push(contract);
                assert!(matches!(
                    query(&body, &unresolved),
                    Err(CpuError::Unsupported(
                        "callable CPU footprint has unresolved code and no admitted physical state ceiling"
                    ))
                ));
                let mut unreachable = context.clone();
                unreachable.invocations.clear();
                assert_eq!(query(&body, &unreachable).unwrap(), None);
                unreachable.reached_machines.clear();
                assert!(matches!(
                    query(&body, &unreachable),
                    Err(CpuError::Unsupported(
                        "callable CPU footprint lacks an admitted invocation context for its native caller"
                    ))
                ));
                let mut foreign = context;
                foreign.invocations[0].invocation.target =
                    CallableInvocationTarget::Value(ValueId::new(6).unwrap(), contract);
                assert!(matches!(
                    query(&body, &foreign),
                    Err(CpuError::Mismatch(
                        "callable CPU footprint changed its retained invocation"
                    ))
                ));
            }

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
                let model = &target_x86_64::x86_64_physical_register_model();
                let mut footprint = Footprint::default();
                footprint
                    .view(model, model.view_named("eax").unwrap().id)
                    .unwrap();
                let result = footprint
                    .finish(model)
                    .unwrap()
                    .into_effects()
                    .compose(&direct_call_effects(target::Architecture::X86_64).unwrap());
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
                let model = &target_x86_64::x86_64_physical_register_model();
                let mut footprint = Footprint::default();
                footprint
                    .view(model, model.view_named("xmm3").unwrap().id)
                    .unwrap();
                for unit in &model.view_named("rflags").unwrap().write_units {
                    footprint.write_unit(*unit);
                }
                let result = footprint
                    .finish(model)
                    .unwrap()
                    .into_effects()
                    .compose(&direct_call_effects(target::Architecture::X86_64).unwrap());
                assert!(result.registers().contains(MachineRegister::X86Xmm(3)));
                assert!(result.machine_state().contains_all(MachineStateSet::new([
                    MachineState::VectorRegisters,
                    MachineState::Flags
                ])));
                let mut invalid = Footprint::default();
                invalid.write_unit(RegisterUnitId(u16::MAX));
                assert!(invalid.finish(model).is_err());
            }

            #[test]
            fn read_only_vector_use_is_not_reported_as_a_register_write() {
                let model = &target_x86_64::x86_64_physical_register_model();
                let mut footprint = Footprint::default();
                footprint
                    .read_view(model, model.view_named("xmm5").unwrap().id)
                    .unwrap();
                let result = footprint.finish(model).unwrap().into_effects();
                assert!(
                    result
                        .machine_state()
                        .contains_all(MachineStateSet::new([MachineState::VectorRegisters]))
                );
                assert!(!result.registers().contains(MachineRegister::X86Xmm(5)));
                assert!(result.registers_read().contains(MachineRegister::X86Xmm(5)));
                assert!(result.clobbered_state().is_empty());
            }

            #[test]
            fn repeated_instruction_numbers_keep_each_functions_own_effects() {
                use machine_code::{
                    SelectedFormEncodedRowStore, SelectedFormEncodingRow,
                    SelectedFormEncodingState, SelectedFormMachineDisposition,
                };
                use selected_instructions::MachineAlternativeKey;
                let model = &target_x86_64::x86_64_physical_register_model();
                let mut store = SelectedFormEncodedRowStore::new();
                let mut make_row = |instruction, view: Option<&str>| {
                    let writes = view
                        .into_iter()
                        .map(|name| model.view_named(name).unwrap().id)
                        .collect();
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
                let owners = [
                    (first, SelectedInstructionId(0)),
                    (first, SelectedInstructionId(1)),
                    (second, SelectedInstructionId(0)),
                    (second, SelectedInstructionId(1)),
                ];
                let rows = owners
                    .iter()
                    .zip(&rows)
                    .map(|(&(machine, _), row)| {
                        PlacedInstructionEffects::from_footprint(
                            machine,
                            row.instruction,
                            &store,
                            row.footprint,
                        )
                    })
                    .collect::<Vec<_>>();
                let indexed = index_encoded_rows(owners, &rows).unwrap();
                let mut scalar = Footprint::default();
                scalar
                    .decoded(
                        model,
                        encoded_row(&indexed, first, SelectedInstructionId(0))
                            .unwrap()
                            .footprint()
                            .unwrap(),
                    )
                    .unwrap();
                let scalar = scalar.finish(model).unwrap().into_effects();
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
                        encoded_row(&indexed, second, SelectedInstructionId(0))
                            .unwrap()
                            .footprint()
                            .unwrap(),
                    )
                    .unwrap();
                assert!(
                    vector
                        .finish(model)
                        .unwrap()
                        .into_effects()
                        .registers()
                        .contains(MachineRegister::X86Xmm(3))
                );
                assert!(index_encoded_rows([owners[0], owners[2], owners[3]], &rows).is_err());
                assert!(index_encoded_rows(owners, &rows[..3]).is_err());
                assert!(index_encoded_rows(owners[..3].iter().copied(), &rows).is_err());
                assert!(
                    index_encoded_rows([owners[0], owners[1], owners[0], owners[1]], &rows)
                        .is_err()
                );
                assert!(encoded_row(&indexed, first, SelectedInstructionId(2)).is_none());
                assert!(
                    encoded_row(
                        &indexed,
                        MachineId::new(21).unwrap(),
                        SelectedInstructionId(0)
                    )
                    .is_none()
                );
            }
        }
    }

    pub use body_footprint::NativeBodyCpuState;

    use std::sync::Arc;

    use crate::{
        FunctionFragmentObjectArtifactError as Error, ObjectArtifact, PrivateFunctionObject,
    };
    use calling_conventions::BoundaryEntryPlan;
    use function_identity::MachineFunctionIdentity;
    use physical_recipes::native_entry_bridge::{
        InstalledInterruptEntryPlan, NativeEntryBridgeRequest,
    };

    pub fn observe_native_body_cpu_state(
        body: &ObjectArtifact,
        preemption: calling_conventions::Preemption,
    ) -> Result<NativeBodyCpuState, Error> {
        body_footprint::collect(
            body.composition_source.as_deref().unwrap_or(body),
            preemption,
        )
    }

    pub fn observe_native_function_cpu_state(
        body: &ObjectArtifact,
        entry: semantic_vocabulary::MachineId,
        preemption: calling_conventions::Preemption,
    ) -> Result<NativeBodyCpuState, Error> {
        body_footprint::collect_from(
            body.composition_source.as_deref().unwrap_or(body),
            entry,
            preemption,
        )
    }

    pub fn observe_native_constant_operand(
        body: &ObjectArtifact,
        machine: semantic_vocabulary::MachineId,
        instruction: selected_instructions::SelectedInstructionId,
        operand: u16,
    ) -> Result<Option<u64>, Error> {
        let body = body.composition_source.as_deref().unwrap_or(body);
        let replay = body.fragment_replay.as_ref().ok_or(Error::Mismatch(
            "constant operand requires retained native source",
        ))?;
        crate::object::function_fragments::selected_constant_operand(
            replay,
            machine,
            instruction,
            operand,
        )
    }

    pub type CallableEntryTransport = object_file::CallableEntryTransport;
    pub type EncodedPrivateEntry = object_file::PrivateEntryObject;
    use physical_recipes::callable_entry::{identity_transport, native_entry_request_from_abi};
    pub(crate) fn validate_callable_transport(
        body: &ObjectArtifact,
        transport: &CallableEntryTransport,
    ) -> Result<(), Error> {
        if transport.context().psi != body.psi() || transport.context().entry != body.entry() {
            return Err(Error::Mismatch(
                "callable entry transport belongs to another body, ABI or state promise",
            ));
        }
        check_entry_contract(
            body,
            &transport.context().inbound,
            &transport.context().ordinals,
            None,
            transport.bridge(),
            &transport.context().effects,
        )
    }

    pub fn bind_private_function_entry(
        encoded: EncodedPrivateEntry,
        body: Arc<ObjectArtifact>,
    ) -> Result<PrivateFunctionObject, Error> {
        let (identity, transport) = encoded.into_parts();
        let (context, bridge) = transport.into_parts();
        if body.psi() != context.psi || body.entry() != context.entry {
            return Err(Error::Mismatch(
                "private entry plan belongs to another body",
            ));
        }
        Ok(PrivateFunctionObject {
            identity,
            body,
            entry_bridge: bridge,
            entry_effects: context.effects,
            inbound: context.inbound,
            inbound_ordinals: context.ordinals,
        })
    }

    fn check_entry_contract(
        body: &ObjectArtifact,
        inbound: &BoundaryEntryPlan,
        ordinals: &[u32],
        installed: Option<(MachineFunctionIdentity, InstalledInterruptEntryPlan)>,
        bridge: Option<&crate::NativeEntryBridge>,
        effects: &calling_conventions::StateEffectsEvidence,
    ) -> Result<(), Error> {
        if let Some((identity, _)) = installed {
            let execution = body.installed_entry_execution().ok_or(Error::Mismatch(
                "installed private entry has no retained execution domain",
            ))?;
            if execution.entry().callback_function != identity
                || execution.entry().entry().inbound() != inbound
                || execution.entry().entry().inbound_ordinals() != ordinals
            {
                return Err(Error::Mismatch(
                    "installed private entry changed its execution domain",
                ));
            }
        } else if body.installed_entry_execution().is_some() {
            return Err(Error::Mismatch(
                "a privileged installed body cannot become an ordinary callable entry",
            ));
        }
        let request = native_entry_request(body, inbound, ordinals)?;
        let needs_bridge = installed.is_some() || !identity_transport(&request);
        match bridge {
            Some(bridge)
                if needs_bridge
                    && bridge.request() == &request
                    && bridge.installed_interrupt() == installed.map(|(_, plan)| plan)
                    && bridge.complete_effects() == effects =>
            {
                crate::published_entry_bytes::verify_entry_bridge_bytes(bridge, bridge.bytes())
                    .map_err(Error::NativeEntryBridge)
            }
            None if !needs_bridge
                && &request
                    .checked_complete_effects(&calling_conventions::StateEffectsEvidence::empty())
                    .map_err(Error::NativeEntryBridge)?
                    == effects =>
            {
                Ok(())
            }
            _ => Err(Error::Mismatch(
                "callable entry transport belongs to another body, ABI or state promise",
            )),
        }
    }

    pub(crate) fn native_entry_request(
        body: &ObjectArtifact,
        inbound: &BoundaryEntryPlan,
        inbound_ordinals: &[u32],
    ) -> Result<NativeEntryBridgeRequest, Error> {
        let abi = body
            .physical_facts()
            .contracts
            .function_abi(body.entry)
            .ok_or(Error::Mismatch("callable body entry plan missing"))?;
        native_entry_request_from_abi(
            body.target,
            abi,
            body_footprint::collect(body, inbound.state.preemption)?.into_effects(),
            inbound,
            inbound_ordinals,
        )
        .map_err(|error| match error {
            physical_recipes::callable_entry::EntryRequestError::Mismatch(reason) => {
                Error::Mismatch(reason)
            }
            physical_recipes::callable_entry::EntryRequestError::Unsupported(reason) => {
                Error::Unsupported(reason)
            }
            physical_recipes::callable_entry::EntryRequestError::Overflow => Error::Overflow,
        })
    }

    pub(crate) fn validate_entry_transport(
        private: &crate::ObjectCompilerPrivateFunction,
    ) -> Result<(), Error> {
        let installed = private
            .entry_bridge
            .as_ref()
            .and_then(|bridge| bridge.installed_interrupt());
        check_entry_contract(
            &private.body,
            &private.inbound,
            &private.inbound_ordinals,
            installed.map(|plan| (private.identity, plan)),
            private.entry_bridge.as_ref(),
            &private.entry_effects,
        )
    }
}

pub(crate) mod foreign_adapter {
    use crate::NativeForeignBridge;
    use crate::object::component_shared_image::emit_shared_function_container;
    use diagnostics::Diagnostic;
    use image::FinalImageInput;
    use sha2::{Digest, Sha256};

    #[derive(Debug)]
    pub struct NativeForeignAdapterImage {
        bridge: NativeForeignBridge,
        locator: target::NormalizedForeignLocator,
        application: [u8; 32],
        export: String,
        image_digest: [u8; 32],
        container: crate::ComponentSharedContainer,
    }

    impl NativeForeignAdapterImage {
        pub fn runtime_identity(&self) -> calling_conventions::PublishedForeignAdapterIdentity {
            calling_conventions::PublishedForeignAdapterIdentity {
                bridge: self.bridge().request().clone(),
                locator: self.locator().clone(),
                application: *self.application(),
                image_digest: self.image_digest,
            }
        }

        pub const fn image_digest(&self) -> &[u8; 32] {
            &self.image_digest
        }

        pub const fn bridge(&self) -> &NativeForeignBridge {
            &self.bridge
        }
        pub const fn locator(&self) -> &target::NormalizedForeignLocator {
            &self.locator
        }
        pub const fn application(&self) -> &[u8; 32] {
            &self.application
        }
        pub fn export(&self) -> &str {
            &self.export
        }
        pub const fn container(&self) -> &crate::ComponentSharedContainer {
            &self.container
        }
    }

    #[derive(Debug)]
    pub struct NativeForeignAdapterEmissionError {
        pub bridge: NativeForeignBridge,
        pub locator: target::NormalizedForeignLocator,
        pub application: [u8; 32],
        pub diagnostic: Diagnostic,
    }

    /// Publish the closed adapter object with the import body planned for its application.
    ///
    /// A caller-selected issuing type cannot install an adapter.
    /// ```compile_fail,E0308
    /// fn publish(adapter: physical_recipes::native_foreign_bridge::ForeignAdapterPlan<()>) {
    ///     let _ = image_emission::emit_native_foreign_adapter(adapter);
    /// }
    /// ```
    ///
    /// An issued bridge alone has no bound import program.
    /// ```compile_fail,E0308
    /// fn publish(bridge: image_emission::NativeForeignBridge) {
    ///     let _ = image_emission::emit_native_foreign_adapter(bridge);
    /// }
    /// ```
    ///
    /// Even with a genuine object, its public readback cannot replace the import code.
    /// ```compile_fail,E0308
    /// fn replace(object: object_file::ForeignAdapterObject,
    ///            imports: machine_code::import_code::EncodedImportProgram) {
    ///     let mut parts = object.into_parts();
    ///     parts.imports = imports;
    ///     let _ = image_emission::emit_native_foreign_adapter(parts);
    /// }
    /// ```
    pub fn emit_native_foreign_adapter(
        object: object_file::ForeignAdapterObject,
    ) -> Result<NativeForeignAdapterImage, Box<NativeForeignAdapterEmissionError>> {
        let object_file::ForeignAdapterObjectParts {
            bridge,
            locator,
            application,
            imports,
            object,
            relocations,
            export,
            export_commitment,
        } = object.into_parts();
        let name = object
            .layout
            .symbols
            .get(object.layout.entry_symbol)
            .name
            .as_bytes()
            .to_vec();
        let image = FinalImageInput {
            target: object.target,
            import_program: imports,
            object: &object,
            relocations: &relocations,
            text_bytes: bridge.bytes(),
            data_bytes: &[],
        }
        .into_image();
        let entry = image.symbol_table.entry_symbol;
        let emitted = emit_shared_function_container(image, entry, export_commitment, name)
            .map_err(|(_, diagnostic)| diagnostic);
        match emitted {
            Ok(container) => Ok(NativeForeignAdapterImage {
                bridge,
                locator,
                application,
                export,
                image_digest: Sha256::digest(container.contents()).into(),
                container,
            }),
            Err(diagnostic) => Err(Box::new(NativeForeignAdapterEmissionError {
                bridge,
                locator,
                application,
                diagnostic,
            })),
        }
    }
}

pub(crate) mod hosted_entry_storage;

pub(crate) mod hosted_receiver {
    pub(crate) mod instructions {
        mod aarch64 {
            use super::super::{HostedEntryPartitions, invalid, recipe_index_for_runtime_position};
            use diagnostics::Diagnostic;

            pub(crate) fn validate(
                target: target::NativeTarget,
                bytes: &[u8],
                address: u64,
                selected_entry: u64,
                bss: u64,
                partitions: HostedEntryPartitions,
                recipes: &[psi_entry_memory::CheckedPrivateEntryRamRecipe],
            ) -> Result<(), Diagnostic> {
                let fail = || invalid(target);
                if ![
                    target::NativeTarget::macos_arm64(),
                    target::NativeTarget::linux_arm64(),
                ]
                .contains(&target)
                {
                    return Err(fail());
                }
                let (chunks, remainder) = bytes.as_chunks::<4>();
                if !remainder.is_empty() || !address.is_multiple_of(4) {
                    return Err(fail());
                }
                let words = chunks
                    .iter()
                    .map(|bytes| u32::from_le_bytes(*bytes))
                    .collect::<Vec<_>>();
                let mut reader = Reader {
                    words: &words,
                    position: 0,
                    address,
                    target,
                };
                let (continuation, stack_top, allocation) =
                    super::expected_addresses(bss, partitions, &fail)?;
                reader.address(9, continuation)?;
                reader.exact(0x9100_03ea)?;
                reader.exact(0xa900_792a)?;
                reader.address(10, stack_top)?;
                reader.exact(0x9100_015f)?;
                let mut next = allocation;
                let mut originals = Vec::new();
                let mut headers = Vec::new();
                for recipe in recipes {
                    let alignment = recipe.alignment();
                    if !alignment.is_power_of_two() {
                        return Err(fail());
                    }
                    let original = next
                        .checked_add(alignment - 1)
                        .map(|value| value & !(alignment - 1))
                        .ok_or_else(fail)?;
                    let header = original
                        .checked_add(recipe.contents().len() as u64)
                        .ok_or_else(fail)?;
                    next = header
                        .checked_add(recipe.observation_header().len() as u64)
                        .ok_or_else(fail)?;
                    originals.push(original);
                    headers.push(header);
                    reader.initialize(original, recipe.contents())?;
                }
                for ordinal in 0..recipes.len() {
                    if ordinal >= 8 {
                        return Err(fail());
                    }
                    let selected =
                        recipe_index_for_runtime_position(recipes, ordinal).ok_or_else(fail)?;
                    reader.address(ordinal as u32, originals[selected])?;
                }
                let pc = reader.pc()?;
                let call = reader.take()?;
                if call & 0xfc00_0000 != 0x9400_0000
                    || pc.checked_add_signed(((i64::from(call & 0x03ff_ffff) << 38) >> 38) * 4)
                        != Some(selected_entry)
                {
                    return Err(fail());
                }
                for ((recipe, original), header) in recipes.iter().zip(originals).zip(headers) {
                    let metadata = recipe.observation_header();
                    reader.initialize(header, &metadata)?;
                    for (source, count) in [
                        (header, metadata.len()),
                        (original, recipe.contents().len()),
                    ] {
                        reader.exact(0xd280_0060)?; // Only the fresh runtime FD.
                        reader.address(1, source)?;
                        reader.count(2, count)?;
                        if target == target::NativeTarget::macos_arm64() {
                            reader.exact(0xd280_0090)?; // Darwin write: X16 = 4.
                            reader.exact(0xd400_1001)?;
                            reader.conditional_branch(2, 5)?; // Carry error, before CMP.
                        } else {
                            reader.exact(0xd280_0808)?; // Linux write: X8 = 64.
                            reader.exact(0xd400_0001)?;
                        }
                        reader.count(9, count)?;
                        reader.exact(0xeb09_001f)?;
                        reader.conditional_branch(0, 5)?; // Full write skips failure only.
                        reader.exact(0xd280_1900)?; // A failed observation exits 200.
                        let exit = if target == target::NativeTarget::macos_arm64() {
                            [0xd280_0030, 0xd400_1001, 0xd420_0000]
                        } else {
                            [0xd280_0bc8, 0xd400_0001, 0xd420_0000]
                        };
                        for word in exit {
                            reader.exact(word)?;
                        }
                    }
                }
                reader.address(9, continuation)?;
                for word in [0xa940_792a, 0x9100_015f, 0x5280_0000] {
                    reader.exact(word)?;
                }
                if target == target::NativeTarget::macos_arm64() {
                    reader.exact(0xd65f_03c0)?; // LC_MAIN returns to its loader.
                } else {
                    for word in [0xd280_0bc8, 0xd400_0001, 0xd420_0000] {
                        reader.exact(word)?; // ELF entry completes through exit_group(94).
                    }
                }
                if reader.position != words.len() {
                    return Err(fail());
                }
                Ok(())
            }

            struct Reader<'a> {
                words: &'a [u32],
                position: usize,
                address: u64,
                target: target::NativeTarget,
            }
            impl Reader<'_> {
                fn take(&mut self) -> Result<u32, Diagnostic> {
                    let word = self
                        .words
                        .get(self.position)
                        .copied()
                        .ok_or_else(|| invalid(self.target))?;
                    self.position += 1;
                    Ok(word)
                }
                fn pc(&self) -> Result<u64, Diagnostic> {
                    self.address
                        .checked_add(
                            (self.position as u64)
                                .checked_mul(4)
                                .ok_or_else(|| invalid(self.target))?,
                        )
                        .ok_or_else(|| invalid(self.target))
                }
                fn exact(&mut self, expected: u32) -> Result<(), Diagnostic> {
                    if self.take()? != expected {
                        return Err(invalid(self.target));
                    }
                    Ok(())
                }
                fn address(&mut self, register: u32, expected: u64) -> Result<(), Diagnostic> {
                    let target = self.target;
                    let fail = || invalid(target);
                    let pc = self.pc()?;
                    let page = self.take()?;
                    let add = self.take()?;
                    if page & 0x9f00_001f != 0x9000_0000 | register
                        || add & 0xffc0_03ff != 0x9100_0000 | (register << 5) | register
                    {
                        return Err(fail());
                    }
                    let immediate = ((page >> 5) & 0x7ffff) << 2 | ((page >> 29) & 3);
                    let signed_pages = (i64::from(immediate) << 43) >> 43;
                    let actual = (pc & !0xfff)
                        .checked_add_signed(signed_pages * 4096)
                        .and_then(|value| value.checked_add(u64::from((add >> 10) & 0xfff)));
                    if actual != Some(expected) {
                        return Err(fail());
                    }
                    Ok(())
                }
                fn initialize(&mut self, original: u64, bytes: &[u8]) -> Result<(), Diagnostic> {
                    self.address(9, original)?;
                    for byte in bytes {
                        let immediate = self.take()?;
                        if immediate & 0xffe0_001f != 0x5280_000a
                            || (immediate >> 5) & 0xffff != u32::from(*byte)
                        {
                            return Err(invalid(self.target));
                        }
                        self.exact(0x3900_012a)?; // Store W10 to current original byte.
                        self.exact(0x9100_0529)?; // Advance exactly one byte, no gap or alias.
                    }
                    Ok(())
                }
                fn count(&mut self, register: u32, expected: usize) -> Result<(), Diagnostic> {
                    let low = self.take()?;
                    let high = self.take()?;
                    if low & 0xffe0_001f != 0xd280_0000 | register
                        || high & 0xffe0_001f != 0xf2a0_0000 | register
                        || u64::from(((low >> 5) & 0xffff) | (((high >> 5) & 0xffff) << 16))
                            != expected as u64
                    {
                        return Err(invalid(self.target));
                    }
                    Ok(())
                }
                fn conditional_branch(
                    &mut self,
                    condition: u32,
                    distance: i64,
                ) -> Result<(), Diagnostic> {
                    let word = self.take()?;
                    if word & 0xff00_001f != 0x5400_0000 | condition
                        || ((i64::from((word >> 5) & 0x7ffff) << 45) >> 45) != distance
                    {
                        return Err(invalid(self.target));
                    }
                    Ok(())
                }
            }

            #[cfg(test)]
            mod tests {
                use super::Reader;

                fn macos_reader(words: &[u32], address: u64) -> Reader<'_> {
                    Reader {
                        words,
                        position: 0,
                        address,
                        target: target::NativeTarget::macos_arm64(),
                    }
                }

                #[test]
                fn original_byte_initialization_refuses_other_residence_or_missing_store() {
                    let words = [
                        0x9000_0009,
                        0x9108_0129,
                        0x5280_016a,
                        0x3900_012a,
                        0x9100_0529,
                    ];
                    let mut reader = macos_reader(&words, 0x1000);
                    assert!(reader.initialize(0x1200, &[11]).is_ok());
                    let mut other = macos_reader(&words, 0x1000);
                    assert!(other.initialize(0x1204, &[11]).is_err());
                    for (position, replacement) in
                        [(2, 0x5280_018a), (3, 0xd503_201f), (4, 0x9100_0929)]
                    {
                        let mut corrupted = words;
                        corrupted[position] = replacement;
                        let mut reader = macos_reader(&corrupted, 0x1000);
                        assert!(reader.initialize(0x1200, &[11]).is_err());
                    }
                }

                #[test]
                fn original_argument_register_and_full_width_observation_count_are_exact() {
                    let addresses = [0x9000_0000, 0x9108_0000];
                    let mut reader = macos_reader(&addresses, 0x1000);
                    assert!(reader.address(0, 0x1200).is_ok());
                    let mut substituted = macos_reader(&addresses, 0x1000);
                    assert!(substituted.address(1, 0x1200).is_err());
                    let count = [0xd280_0002, 0xf2a0_0022];
                    let mut reader = macos_reader(&count, 0);
                    assert!(reader.count(2, 65536).is_ok());
                    let mut truncated = macos_reader(&count, 0);
                    assert!(truncated.count(2, 0).is_err());
                }

                #[test]
                fn observation_error_path_cannot_skip_the_fail_closed_exit() {
                    for word in [0x5400_0082, 0x5400_00a3, 0x1400_0005] {
                        let words = [word];
                        let mut reader = macos_reader(&words, 0);
                        assert!(reader.conditional_branch(2, 5).is_err());
                    }
                    let words = [0x5400_00a2];
                    let mut reader = macos_reader(&words, 0);
                    assert!(reader.conditional_branch(2, 5).is_ok());
                }
            }
        }

        mod windows_x86_64 {
            use super::{
                HostedEntryPartitions, expected_addresses, recipe_index_for_runtime_position,
                rip_relative,
            };
            use diagnostics::Diagnostic;

            struct Reader<'bytes> {
                bytes: &'bytes [u8],
                address: u64,
                position: usize,
                failure: u64,
            }
            impl Reader<'_> {
                fn reject(reason: &str) -> Diagnostic {
                    Diagnostic::error(format!("Windows installed-entry replay: {reason}"))
                }
                fn bytes(&mut self, expected: &[u8], reason: &str) -> Result<(), Diagnostic> {
                    let end = self
                        .position
                        .checked_add(expected.len())
                        .ok_or_else(|| Self::reject(reason))?;
                    if self.bytes.get(self.position..end) != Some(expected) {
                        return Err(Self::reject(reason));
                    }
                    self.position = end;
                    Ok(())
                }
                fn pointer(
                    &mut self,
                    prefix: &[u8],
                    target: u64,
                    reason: &str,
                ) -> Result<(), Diagnostic> {
                    self.bytes(prefix, reason)?;
                    if rip_relative(self.bytes, self.address, self.position) != Some(target) {
                        return Err(Self::reject(reason));
                    }
                    self.position += 4;
                    Ok(())
                }
                fn failure(&mut self, condition: u8) -> Result<(), Diagnostic> {
                    self.pointer(
                        &[0x0f, condition],
                        self.failure,
                        "observation failure does not restore the original continuation",
                    )
                }
                fn initialize(&mut self, original: u64, contents: &[u8]) -> Result<(), Diagnostic> {
                    for (index, byte) in contents.iter().enumerate() {
                        let target = original
                            .checked_add(index as u64)
                            .and_then(|value| value.checked_sub(1))
                            .ok_or_else(|| Self::reject("initialized byte address overflow"))?;
                        self.pointer(
                            &[0xc6, 0x05],
                            target,
                            "initialization targets another residence",
                        )?;
                        self.bytes(&[*byte], "initialized value differs from checked recipe")?;
                    }
                    Ok(())
                }
            }

            pub(in crate::entries::hosted_receiver) fn validate(
                bytes: &[u8],
                address: u64,
                selected_entry: u64,
                output: &image::EmittedImageOutput,
                partitions: HostedEntryPartitions,
                recipes: &[psi_entry_memory::CheckedPrivateEntryRamRecipe],
            ) -> Result<(), Diagnostic> {
                let invalid = || Reader::reject("invalid bridge partitions or import residency");
                let (saved, stack_top, receiver) = expected_addresses(
                    output.final_image_layout.bss_address,
                    partitions,
                    &invalid,
                )?;
                if partitions.stack_byte_count < 80 {
                    return Err(invalid());
                }
                let mut imports = Vec::new();
                for export in [
                    "GetEnvironmentVariableA",
                    "CreateFileA",
                    "WriteFile",
                    "FlushFileBuffers",
                    "CloseHandle",
                ] {
                    let locator = target::normalize_foreign_locator(
                        target::ForeignLocatorCandidate::PeByName {
                            library: b"kernel32.dll".to_vec(),
                            export: export.as_bytes().to_vec(),
                        },
                        target::TargetProfile::WindowsX64,
                    )
                    .map_err(|_| invalid())?;
                    let name = object_file::normalized_foreign_import_symbol_name(&locator);
                    let mut matching = output.executable_regions.regions.iter().filter(|region| {
                        region.origin == image::FinalExecutableRegionOrigin::ImportThunk
                            && region.symbol == name
                    });
                    let region = matching.next().ok_or_else(invalid)?;
                    if matching.next().is_some() {
                        return Err(invalid());
                    }
                    imports.push(
                        output
                            .final_image_layout
                            .text_address
                            .checked_add(region.section_offset as u64)
                            .ok_or_else(invalid)?,
                    );
                }
                let failed = bytes.len().checked_sub(13).ok_or_else(invalid)?;
                let mut reader = Reader {
                    bytes,
                    address,
                    position: 0,
                    failure: address.checked_add(failed as u64).ok_or_else(invalid)?,
                };
                macro_rules! p {
                    ($a:expr => $b:expr => $c:expr) => {
                        reader.pointer($a, $b, $c)
                    };
                }
                reader.pointer(&[0x48, 0x89, 0x25], saved, "loader stack was not preserved")?;
                reader.bytes(
                    &[0x48, 0x83, 0xe4, 0xf0],
                    "Win32 calls do not retain the guarded loader stack",
                )?;
                reader.bytes(
                    &[0x48, 0x81, 0xec, 0, 1, 0, 0],
                    "bridge stack frame is not bounded",
                )?;
                for (index, byte) in b"OMEGA_PRIVATE_OBSERVATION_PIPE\0".iter().enumerate() {
                    reader.bytes(
                        &[0xc6, 0x44, 0x24, 64 + index as u8, *byte],
                        "private pipe variable changed",
                    )?;
                }
                reader.bytes(
                    &[
                        0x48, 0x8d, 0x4c, 0x24, 64, 0x48, 0x8d, 0x54, 0x24, 96, 0x41, 0xb8, 128, 0,
                        0, 0,
                    ],
                    "pipe name is not bounded bridge-local storage",
                )?;
                reader.pointer(&[0xe8], imports[0], "pipe name lookup calls another import")?;
                reader.bytes(&[0x85, 0xc0], "missing pipe name accepted")?;
                reader.failure(0x84)?;
                reader.bytes(&[0x3d, 128, 0, 0, 0], "truncated pipe name accepted")?;
                reader.failure(0x83)?;
                reader.bytes(
                    &[
                        0x48, 0x8d, 0x4c, 0x24, 96, 0xba, 0, 0, 0, 0x40, 0x45, 0x31, 0xc0, 0x45,
                        0x31, 0xc9,
                    ],
                    "pipe open lost its private write-only mode",
                )?;
                for (offset, value) in [(32, 3u32), (40, 0), (48, 0)] {
                    reader.bytes(
                        &[0x48, 0xc7, 0x44, 0x24, offset],
                        "pipe open stack argument changed",
                    )?;
                    reader.bytes(
                        &value.to_le_bytes(),
                        "pipe open creates or inherits unrelated storage",
                    )?;
                }
                reader.pointer(&[0xe8], imports[1], "pipe open calls another import")?;
                reader.bytes(&[0x48, 0x83, 0xf8, 0xff], "invalid pipe handle accepted")?;
                reader.failure(0x84)?;
                p!(&[0x48, 0x89, 0x05] => saved + 8 => "pipe handle overwrote loader continuation")?;
                p!(&[0x48, 0x8d, 0x25] => stack_top => "callee stack is not its disjoint partition")?;
                reader.bytes(
                    &[0x48, 0x83, 0xec, 64],
                    "semantic call lost its shadow/stack arguments",
                )?;
                let mut end = receiver;
                let mut originals = Vec::new();
                let mut headers = Vec::new();
                for recipe in recipes {
                    if !recipe.alignment().is_power_of_two() {
                        return Err(invalid());
                    }
                    let start = end
                        .checked_add(recipe.alignment() - 1)
                        .map(|value| value & !(recipe.alignment() - 1))
                        .ok_or_else(invalid)?;
                    reader.initialize(start, recipe.contents())?;
                    let header = start
                        .checked_add(recipe.contents().len() as u64)
                        .ok_or_else(invalid)?;
                    originals.push(start);
                    headers.push(header);
                    end = header
                        .checked_add(recipe.observation_header().len() as u64)
                        .ok_or_else(invalid)?;
                }
                if end
                    != output
                        .final_image_layout
                        .bss_address
                        .checked_add(partitions.end_offset)
                        .ok_or_else(invalid)?
                {
                    return Err(invalid());
                }
                for ordinal in 0..recipes.len() {
                    let index =
                        recipe_index_for_runtime_position(recipes, ordinal).ok_or_else(invalid)?;
                    let prefix = match ordinal {
                        0 => [0x48, 0x8d, 0x0d],
                        1 => [0x48, 0x8d, 0x15],
                        2 => [0x4c, 0x8d, 0x05],
                        3 => [0x4c, 0x8d, 0x0d],
                        4 => [0x48, 0x8d, 0x05],
                        _ => return Err(invalid()),
                    };
                    p!(&prefix => originals[index] => "semantic argument names another original allocation")?;
                    if ordinal == 4 {
                        reader.bytes(
                            &[0x48, 0x89, 0x44, 0x24, 32],
                            "fifth argument is outside its ABI stack slot",
                        )?;
                    }
                }
                p!(&[0xe8] => selected_entry => "bridge calls another semantic entry")?;
                p!(&[0x48, 0x8b, 0x25] => saved => "observer did not restore loader stack")?;
                reader.bytes(
                    &[0x48, 0x83, 0xe4, 0xf0, 0x48, 0x83, 0xec, 64],
                    "observer Win32 calls lost their guarded stack frame",
                )?;
                for ((recipe, original), header) in recipes.iter().zip(originals).zip(headers) {
                    let contents = recipe.observation_header();
                    reader.initialize(header, &contents)?;
                    for (source, length) in [
                        (header, contents.len()),
                        (original, recipe.contents().len()),
                    ] {
                        p!(&[0x48, 0x8b, 0x0d] => saved + 8 => "observation uses another pipe handle")?;
                        p!(&[0x48, 0x8d, 0x15] => source => "observation reads another allocation")?;
                        reader.bytes(&[0x41, 0xb8], "observation length register changed")?;
                        reader
                            .bytes(&(length as u32).to_le_bytes(), "observation extent changed")?;
                        reader.bytes(
                            &[
                                0x4c, 0x8d, 0x4c, 0x24, 48, 0x48, 0xc7, 0x44, 0x24, 32, 0, 0, 0, 0,
                            ],
                            "write completion is not synchronous bridge-local storage",
                        )?;
                        reader.pointer(&[0xe8], imports[2], "observation calls another writer")?;
                        reader.bytes(&[0x85, 0xc0], "failed write accepted")?;
                        reader.failure(0x84)?;
                        reader.bytes(&[0x81, 0x7c, 0x24, 48], "missing completed byte count")?;
                        reader.bytes(&(length as u32).to_le_bytes(), "partial write accepted")?;
                        reader.failure(0x85)?;
                    }
                }
                for import in &imports[3..] {
                    p!(&[0x48, 0x8b, 0x0d] => saved + 8 => "retirement uses another handle")?;
                    reader.pointer(&[0xe8], *import, "retirement calls another import")?;
                    reader.bytes(&[0x85, 0xc0], "failed retirement accepted")?;
                    reader.failure(0x84)?;
                }
                reader.bytes(&[0x31, 0xc0, 0xeb, 5], "normal completion is not zero")?;
                if reader.position != failed {
                    return Err(invalid());
                }
                reader.bytes(
                    &[0xb8, 200, 0, 0, 0],
                    "observation failure reported success",
                )?;
                p!(&[0x48, 0x8b, 0x25] => saved => "original loader stack was not restored")?;
                reader.bytes(&[0xc3], "bridge does not resume its loader")?;
                if reader.position != bytes.len() {
                    return Err(Reader::reject("extra instructions after bridge completion"));
                }
                Ok(())
            }
        }

        pub(crate) use aarch64::validate as validate_aarch64;
        pub(super) use windows_x86_64::validate as validate_windows_x86_64;

        use diagnostics::Diagnostic;

        use super::{HostedEntryPartitions, invalid, recipe_index_for_runtime_position};

        fn expected_addresses(
            bss_address: u64,
            partitions: HostedEntryPartitions,
            invalid: &dyn Fn() -> Diagnostic,
        ) -> Result<(u64, u64, u64), Diagnostic> {
            Ok((
                bss_address
                    .checked_add(partitions.saved_continuation_offset)
                    .ok_or_else(invalid)?,
                bss_address
                    .checked_add(partitions.stack_offset)
                    .and_then(|v| v.checked_add(partitions.stack_byte_count))
                    .ok_or_else(invalid)?,
                bss_address
                    .checked_add(partitions.allocation_offset)
                    .ok_or_else(invalid)?,
            ))
        }

        fn rip_relative(bytes: &[u8], address: u64, field: usize) -> Option<u64> {
            let field_address = address.checked_add(field as u64)?;
            let displacement = bytes
                .get(field..field.checked_add(4)?)
                .and_then(|bytes| <[u8; 4]>::try_from(bytes).ok())
                .map(i32::from_le_bytes)?;
            field_address
                .checked_add(4)?
                .checked_add_signed(i64::from(displacement))
        }

        pub(super) fn validate_x86_64(
            bytes: &[u8],
            address: u64,
            selected_entry: u64,
            bss_address: u64,
            partitions: HostedEntryPartitions,
            private_entry_ram: &[psi_entry_memory::CheckedPrivateEntryRamRecipe],
        ) -> Result<(), Diagnostic> {
            let invalid = || invalid(target::NativeTarget::linux_x64());
            let (continuation, stack_top, receiver) =
                expected_addresses(bss_address, partitions, &invalid)?;
            if bytes.get(0..3) != Some(&[0x48, 0x89, 0x25])
                || bytes.get(7..10) != Some(&[0x48, 0x8d, 0x25])
                || rip_relative(bytes, address, 3) != Some(continuation)
                || rip_relative(bytes, address, 10) != Some(stack_top)
            {
                return Err(invalid());
            }
            let mut offset = 14;
            let mut end = receiver;
            let mut originals = Vec::new();
            let mut headers = Vec::new();
            for recipe in private_entry_ram {
                let alignment = recipe.alignment();
                if !alignment.is_power_of_two() {
                    return Err(invalid());
                }
                let start = end
                    .checked_add(alignment - 1)
                    .map(|value| value & !(alignment - 1))
                    .ok_or_else(invalid)?;
                let header = start
                    .checked_add(recipe.contents().len() as u64)
                    .ok_or_else(invalid)?;
                headers.push(header);
                end = header
                    .checked_add(recipe.observation_header().len() as u64)
                    .ok_or_else(invalid)?;
                originals.push(start);
                for (index, byte) in recipe.contents().iter().enumerate() {
                    if bytes.get(offset..offset + 2) != Some(&[0xc6, 0x05])
                        || bytes.get(offset + 6) != Some(byte)
                        || rip_relative(bytes, address, offset + 2)
                            .and_then(|target| target.checked_add(1))
                            != start.checked_add(index as u64)
                    {
                        return Err(invalid());
                    }
                    offset += 7;
                }
            }
            if end
                != bss_address
                    .checked_add(partitions.end_offset)
                    .ok_or_else(invalid)?
            {
                return Err(invalid());
            }
            let argument_encodings = [
                (0x48, 0x3d),
                (0x48, 0x35),
                (0x48, 0x15),
                (0x48, 0x0d),
                (0x4c, 0x05),
                (0x4c, 0x0d),
            ];
            for ordinal in 0..private_entry_ram.len() {
                let index = recipe_index_for_runtime_position(private_entry_ram, ordinal)
                    .ok_or_else(invalid)?;
                let expected = originals[index];
                let (rex, modrm) = argument_encodings
                    .get(ordinal)
                    .copied()
                    .ok_or_else(invalid)?;
                if bytes.get(offset..offset + 3) != Some(&[rex, 0x8d, modrm])
                    || rip_relative(bytes, address, offset + 3) != Some(expected)
                {
                    return Err(invalid());
                }
                offset += 7;
            }
            if bytes.get(offset) != Some(&0xe8)
                || rip_relative(bytes, address, offset + 1) != Some(selected_entry)
            {
                return Err(invalid());
            }
            offset += 5;
            for ((recipe, original), header_address) in
                private_entry_ram.iter().zip(&originals).zip(&headers)
            {
                let header = recipe.observation_header();
                for (index, byte) in header.iter().enumerate() {
                    if bytes.get(offset..offset + 2) != Some(&[0xc6, 0x05])
                        || bytes.get(offset + 6) != Some(byte)
                        || rip_relative(bytes, address, offset + 2)
                            .and_then(|target| target.checked_add(1))
                            != header_address.checked_add(index as u64)
                    {
                        return Err(invalid());
                    }
                    offset += 7;
                }
                for (source, length) in [
                    (*header_address, header.len()),
                    (*original, recipe.contents().len()),
                ] {
                    if bytes.get(offset..offset + 10) != Some(&[0xb8, 1, 0, 0, 0, 0xbf, 3, 0, 0, 0])
                        || bytes.get(offset + 10..offset + 13) != Some(&[0x48, 0x8d, 0x35])
                        || rip_relative(bytes, address, offset + 13) != Some(source)
                        || bytes.get(offset + 17) != Some(&0xba)
                        || bytes.get(offset + 18..offset + 22)
                            != Some(&(length as u32).to_le_bytes())
                        || bytes.get(offset + 22..offset + 26) != Some(&[0x0f, 0x05, 0x48, 0x3d])
                        || bytes.get(offset + 26..offset + 30)
                            != Some(&(length as u32).to_le_bytes())
                        || bytes.get(offset + 30..offset + 46)
                            != Some(&[
                                0x74, 14, 0xbf, 200, 0, 0, 0, 0xb8, 0xe7, 0, 0, 0, 0x0f, 0x05,
                                0x0f, 0x0b,
                            ])
                    {
                        return Err(invalid());
                    }
                    offset += 46;
                }
            }
            if bytes.get(offset..)
                != Some(&[0x31, 0xff, 0xb8, 0xe7, 0, 0, 0, 0x0f, 0x05, 0x0f, 0x0b])
            {
                return Err(invalid());
            }
            Ok(())
        }
    }

    pub(crate) use instructions::validate_aarch64 as validate_aarch64_entry;

    use super::hosted_entry_storage::HostedEntryPartitions;
    use diagnostics::Diagnostic;
    use program_entry_contracts::SelectedProgramEntrySignature;
    use verified_optimization_input::hosted_receiver::AdmittedHostedReceiver;
    use verified_optimization_input::hosted_receiver::{
        ReceiverArgumentInput, physical_contract_matches,
    };
    pub use verified_optimization_input::hosted_receiver::{
        invalid, recipe_index_for_runtime_position,
    };

    pub(crate) fn validate_binding(
        artifact: &crate::ObjectArtifact,
        binding: &AdmittedHostedReceiver,
    ) -> Result<(), Diagnostic> {
        let invalid = || invalid(artifact.target);
        if !physical_contract_matches(binding.physical_contract(), artifact.target)
            || artifact.profile != Some(binding.physical_contract().target_slot().owner)
            || binding.physical_contract().target_slot() != binding.source().target_slot()
            || binding
                .physical_contract()
                .target_slot()
                .owner
                .native_target()
                != artifact.target
            || crate::derive_stack_demand(artifact, artifact.entry).map_err(|_| invalid())?
                != artifact
                    .hosted_entry
                    .as_ref()
                    .ok_or_else(invalid)?
                    .context()
                    .demand
        {
            return Err(invalid());
        }
        validate_entry_arguments(
            artifact,
            binding.source(),
            binding.checked_entry(),
            binding.private_entry_ram(),
        )?;
        Ok(())
    }

    fn validate_entry_arguments(
        artifact: &crate::ObjectArtifact,
        source: &SelectedProgramEntrySignature,
        checked_entry: &terminal_psi::CheckedProgramEntryTerminalReceipt,
        private_entry_ram: &[psi_entry_memory::CheckedPrivateEntryRamRecipe],
    ) -> Result<(), Diagnostic> {
        let (function, abi) = artifact.entry_facts()?;
        let admission = artifact.emission_facts().admission.entry_facts();
        verified_optimization_input::hosted_receiver::validate_entry_arguments(
            ReceiverArgumentInput {
                target: artifact.target,
                admission,
                function,
                abi,
            },
            source,
            checked_entry,
            private_entry_ram,
        )
    }

    pub(crate) fn validate_image(
        artifact: &crate::ObjectArtifact,
        object: &object_file::ObjectPlan,
        text: &[u8],
        relocations: &object_file::RelocationPlan,
        shim: crate::entries::hosted_unit_entry::EntryShim,
        output: &image::EmittedImageOutput,
    ) -> Result<(), Diagnostic> {
        let invalid = || invalid(artifact.target);
        let binding = artifact.hosted_receiver_binding().ok_or_else(invalid)?;
        validate_binding(artifact, binding)?;
        let expected = crate::hosted_program::prepare(artifact)?.ok_or_else(invalid)?;
        if shim != expected.shim {
            return Err(invalid());
        }
        if expected.object != *object
            || expected.text != text
            || expected.relocations != *relocations
        {
            return Err(invalid());
        }
        let (symbol, offset) = shim.symbol_offset();
        let byte_count = text.len().checked_sub(offset).ok_or_else(invalid)?;
        let end = offset.checked_add(byte_count).ok_or_else(invalid)?;
        if !crate::entries::hosted_unit_entry::unique_region(
            object, symbol, offset, byte_count, output,
        ) {
            return Err(invalid());
        }
        let shim_bytes = output
            .final_text_bytes
            .get(offset..end)
            .ok_or_else(invalid)?;
        let entry_selected = if artifact.target == target::NativeTarget::macos_arm64() {
            crate::entries::hosted_unit_entry::main_points_to(
                &output.bytes,
                offset,
                artifact.target,
            )
        } else if artifact.target == target::NativeTarget::windows_x64() {
            crate::entries::hosted_unit_entry::pe_entry_points_to(&output.bytes, offset, shim_bytes)
        } else {
            elf_entry_points_to(output, offset)
        };
        if !entry_selected {
            return Err(invalid());
        }
        let mut original_bss = artifact
            .object
            .layout
            .sections
            .iter()
            .filter(|(_, section)| section.kind == object_file::SectionKind::Bss);
        let original_bss_bytes = original_bss
            .next()
            .map_or(0, |(_, section)| section.size as u64);
        if original_bss.next().is_some() {
            return Err(invalid());
        }
        let context = artifact
            .hosted_entry
            .as_ref()
            .ok_or_else(invalid)?
            .context();
        let physical_recipes::hosted_program::HostedStackBacking::Receiver { layout, .. } =
            &context.backing
        else {
            return Err(invalid());
        };
        let partitions =
            super::hosted_entry_storage::partitions(artifact.target, layout, original_bss_bytes)?;
        crate::entries::hosted_entry_storage::validate_partitions(
            artifact.target,
            &context.demand,
            partitions,
            original_bss_bytes,
            output,
        )?;
        let selected_entry = output
            .final_image_layout
            .text_address
            .checked_add(artifact.entry_function().text_offset as u64)
            .ok_or_else(invalid)?;
        let shim_address = output
            .final_image_layout
            .text_address
            .checked_add(offset as u64)
            .ok_or_else(invalid)?;
        if artifact.target == target::NativeTarget::windows_x64() {
            return instructions::validate_windows_x86_64(
                shim_bytes,
                shim_address,
                selected_entry,
                output,
                partitions,
                binding.private_entry_ram(),
            );
        }
        if artifact.target.architecture == target::Architecture::Aarch64 {
            instructions::validate_aarch64(
                artifact.target,
                shim_bytes,
                shim_address,
                selected_entry,
                output.final_image_layout.bss_address,
                partitions,
                binding.private_entry_ram(),
            )
        } else {
            instructions::validate_x86_64(
                shim_bytes,
                shim_address,
                selected_entry,
                output.final_image_layout.bss_address,
                partitions,
                binding.private_entry_ram(),
            )
        }
    }

    fn elf_entry_points_to(output: &image::EmittedImageOutput, offset: usize) -> bool {
        let expected_entry = output
            .final_image_layout
            .text_address
            .checked_add(offset as u64);
        let encoded_entry = output
            .bytes
            .get(24..32)
            .and_then(|bytes| <[u8; 8]>::try_from(bytes).ok())
            .map(u64::from_le_bytes);
        expected_entry.is_some() && encoded_entry == expected_entry
    }
}

pub(crate) mod hosted_unit_entry {
    // An ordinary hosted import has no finite foreign-stack bound. Keep the
    // loader's OS-backed stack for that closure instead of switching onto a
    // fixed BSS stack sized only for the checked Omega frames.
    mod loader_stack {
        use super::UnitEntryForm;
        use diagnostics::Diagnostic;

        fn invalid(reason: &str) -> Diagnostic {
            Diagnostic::error(format!("hosted loader-stack entry: {reason}"))
        }

        fn require_hosted_entry(artifact: &crate::ObjectArtifact) -> Result<(), Diagnostic> {
            if artifact.profile().is_none_or(|profile| {
                profile.program_entry_slot().schema != target::ProgramEntrySchema::HostedApplication
            }) || artifact.physical_entry_binding().is_some()
                || artifact.hosted_receiver_binding().is_some()
                || !artifact.has_free_unit_entry()?
            {
                return Err(invalid("requires the exact free Unit hosted program entry"));
            }
            let demand = crate::derive_stack_demand(artifact, artifact.entry)
                .map_err(|error| invalid(&error.to_string()))?;
            if !demand.requires_host_stack() || demand.ceiling_bytes().is_some() {
                return Err(invalid("closure has no hosted foreign-stack demand"));
            }
            Ok(())
        }

        pub(super) fn validate(
            form: &UnitEntryForm,
            target: target::NativeTarget,
            artifact: &crate::ObjectArtifact,
            object: &object_file::ObjectPlan,
            text: &[u8],
            relocations: &object_file::RelocationPlan,
            shim: super::EntryShim,
            output: &image::EmittedImageOutput,
        ) -> Result<(), Diagnostic> {
            require_hosted_entry(artifact)?;
            let start = artifact.text_bytes.len();
            let bytes = text
                .get(start..)
                .ok_or_else(|| invalid("entry span is outside its text"))?;
            if !crate::hosted_loader::matches_entry(
                target,
                start,
                artifact.entry_function().text_offset,
                bytes,
            ) {
                return Err(invalid("loader stack, call, return or image entry changed"));
            }
            let mut expected_object = artifact.object.clone();
            let mut expected_text = artifact.text_bytes.clone();
            let (symbol, offset) = super::install_entry_shim(
                &mut expected_object,
                &mut expected_text,
                bytes,
                form.name,
                &|| invalid("entry text section is missing"),
            )?;
            if target != artifact.target
                || shim != form.surface.shim(symbol, offset)
                || *object != expected_object
                || text != expected_text
                || *relocations != artifact.relocations
                || !super::unique_region(object, symbol, offset, bytes.len(), output)
                || output.final_text_bytes.get(offset..text.len()) != Some(bytes)
                || !(form.header_check)(output, offset, bytes)
            {
                return Err(invalid("loader stack, call, return or image entry changed"));
            }
            Ok(())
        }
    }

    mod owned_stack {
        use super::{EntryShim, UnitEntryForm};
        use crate::entries::hosted_entry_storage::HostedEntryPartitions;
        use diagnostics::Diagnostic;
        use object_file::{ObjectPlan, SectionKind};

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub(crate) struct HostedUnitStackPlan {
            pub(in crate::entries) demand: crate::StackDemand,
            pub(in crate::entries) partitions: HostedEntryPartitions,
        }

        fn invalid() -> Diagnostic {
            Diagnostic::error(
                "free Unit entry lost exact stack, call, or loader-continuation custody",
            )
        }

        fn call_overhead(target: target::NativeTarget) -> u64 {
            if target.architecture == target::Architecture::Aarch64 {
                0
            } else if target == target::NativeTarget::windows_x64() {
                40
            } else {
                8
            }
        }

        pub(super) fn validate(
            form: &UnitEntryForm,
            target: target::NativeTarget,
            artifact: &crate::ObjectArtifact,
            object: &ObjectPlan,
            text: &[u8],
            _relocations: &object_file::RelocationPlan,
            shim: EntryShim,
            output: &image::EmittedImageOutput,
        ) -> Result<(), Diagnostic> {
            let plan = shim.stack.as_ref().ok_or_else(invalid)?;
            let (symbol, offset) = shim.symbol_offset();
            if target != artifact.target
                || plan.demand.psi() != artifact.psi
                || plan.demand.target() != artifact.target()
                || plan.demand.entry() != artifact.entry()
                || artifact.physical_entry_binding().is_some()
                || artifact.hosted_receiver_binding().is_some()
                || offset != artifact.text_bytes.len()
                || object.layout.entry_symbol != symbol
            {
                return Err(invalid());
            }
            let byte_count = text.len().checked_sub(offset).ok_or_else(invalid)?;
            if !super::unique_region(object, symbol, offset, byte_count, output) {
                return Err(invalid());
            }
            let bytes = output
                .final_text_bytes
                .get(offset..offset + byte_count)
                .ok_or_else(invalid)?;
            if !(form.header_check)(output, offset, bytes) {
                return Err(Diagnostic::error(form.header_error));
            }
            let demand = &plan.demand;
            let original_bss = artifact
                .object
                .layout
                .sections
                .iter()
                .filter(|(_, section)| section.kind == SectionKind::Bss)
                .try_fold(None, |previous, (_, section)| {
                    if previous.is_some() {
                        Err(invalid())
                    } else {
                        Ok(Some(section.size as u64))
                    }
                })?
                .unwrap_or(0);
            let context = artifact
                .hosted_entry
                .as_ref()
                .ok_or_else(invalid)?
                .context();
            let physical_recipes::hosted_program::HostedStackBacking::Owned(layout) =
                &context.backing
            else {
                return Err(invalid());
            };
            let partitions =
                crate::entries::hosted_entry_storage::partitions(target, layout, original_bss)?;
            // The byte verifier checks the target calling convention independently
            // of the stack dimensions selected by the physical planner.
            let expected_stack_bytes = demand
                .ceiling_bytes()
                .and_then(|bytes| bytes.max(16).checked_add(call_overhead(target)))
                .and_then(|bytes| bytes.checked_next_multiple_of(16))
                .ok_or_else(invalid)?;
            if partitions != plan.partitions || partitions.stack_byte_count != expected_stack_bytes
            {
                return Err(invalid());
            }
            crate::entries::hosted_entry_storage::validate_partitions(
                target,
                demand,
                partitions,
                original_bss,
                output,
            )?;
            let address = output
                .final_image_layout
                .text_address
                .checked_add(offset as u64)
                .ok_or_else(invalid)?;
            let entry = output
                .final_image_layout
                .text_address
                .checked_add(artifact.entry_function().text_offset as u64)
                .ok_or_else(invalid)?;
            if target.architecture == target::Architecture::Aarch64 {
                crate::entries::hosted_receiver::validate_aarch64_entry(
                    target,
                    bytes,
                    address,
                    entry,
                    output.final_image_layout.bss_address,
                    partitions,
                    &[],
                )
            } else {
                validate_x86_64(
                    target,
                    bytes,
                    address,
                    entry,
                    output.final_image_layout.bss_address,
                    partitions,
                )
            }
        }

        fn validate_x86_64(
            target: target::NativeTarget,
            bytes: &[u8],
            address: u64,
            entry: u64,
            bss: u64,
            partitions: HostedEntryPartitions,
        ) -> Result<(), Diagnostic> {
            let continuation = bss
                .checked_add(partitions.saved_continuation_offset)
                .ok_or_else(invalid)?;
            let stack_top = bss
                .checked_add(partitions.stack_offset)
                .and_then(|v| v.checked_add(partitions.stack_byte_count))
                .ok_or_else(invalid)?;
            let mut cursor = 0;
            read_pointer(
                bytes,
                address,
                &mut cursor,
                &[0x48, 0x89, 0x25],
                continuation,
            )?;
            read_pointer(bytes, address, &mut cursor, &[0x48, 0x8d, 0x25], stack_top)?;
            if target == target::NativeTarget::windows_x64() {
                read_exact(bytes, &mut cursor, &[0x48, 0x83, 0xec, 0x20])?;
            }
            read_pointer(bytes, address, &mut cursor, &[0xe8], entry)?;
            read_pointer(
                bytes,
                address,
                &mut cursor,
                &[0x48, 0x8b, 0x25],
                continuation,
            )?;
            let tail = if target == target::NativeTarget::linux_x64() {
                crate::hosted_loader::LINUX_X64_EXIT_ZERO
            } else {
                &[0x31, 0xc0, 0xc3]
            };
            read_exact(bytes, &mut cursor, tail)?;
            if cursor != bytes.len() {
                return Err(invalid());
            }
            Ok(())
        }

        fn read_exact(bytes: &[u8], cursor: &mut usize, expected: &[u8]) -> Result<(), Diagnostic> {
            let end = cursor.checked_add(expected.len()).ok_or_else(invalid)?;
            if bytes.get(*cursor..end) != Some(expected) {
                return Err(invalid());
            }
            *cursor = end;
            Ok(())
        }

        fn read_pointer(
            bytes: &[u8],
            address: u64,
            cursor: &mut usize,
            opcode: &[u8],
            expected: u64,
        ) -> Result<(), Diagnostic> {
            read_exact(bytes, cursor, opcode)?;
            let displacement =
                crate::codec_bytes::u32_at(bytes, *cursor).ok_or_else(invalid)? as i32;
            *cursor = cursor.checked_add(4).ok_or_else(invalid)?;
            if address
                .checked_add(*cursor as u64)
                .and_then(|v| v.checked_add_signed(i64::from(displacement)))
                != Some(expected)
            {
                return Err(invalid());
            }
            Ok(())
        }

        #[cfg(test)]
        mod tests {
            use super::*;
            use crate::entries::hosted_entry_storage::HostedEntryStorage;

            #[test]
            fn stack_switch_replay_rejects_changed_backing_call_restore_and_completion() {
                let mut symbols = arena::Arena::new();
                let scratch = symbols.insert(object_file::SymbolPlan::default());
                let stack_top = symbols.insert(object_file::SymbolPlan::default());
                let storage = HostedEntryStorage {
                    scratch,
                    stack_top,
                    private_ram: Vec::new(),
                    observation_headers: Vec::new(),
                };
                let address = 0x10000u64;
                let entry = 0x8000u64;
                let bss = 0x30000u64;
                let partitions = HostedEntryPartitions {
                    saved_continuation_offset: 0,
                    stack_offset: 16,
                    stack_byte_count: 9 * 1024 * 1024,
                    allocation_offset: 16 + 9 * 1024 * 1024,
                    end_offset: 16 + 9 * 1024 * 1024,
                };
                for target in [
                    target::NativeTarget::windows_x64(),
                    target::NativeTarget::linux_x64(),
                    target::NativeTarget::macos_x64(),
                ] {
                    use entry_planning::hosted_entry::plan_hosted_entry;
                    use machine_code::hosted_entry::{EntrySymbol, HostedEntryMode};
                    use native_encoding::hosted_entry::encode_hosted_entry;
                    let plan = plan_hosted_entry(target, HostedEntryMode::OwnedStack, &[]).unwrap();
                    let encoded = encode_hosted_entry(plan).unwrap();
                    let mut bytes = encoded.bytes;
                    let mut pointers = Vec::new();
                    for fixup in encoded.fixups {
                        match fixup.destination {
                            EntrySymbol::Body => {
                                let delta = i32::try_from(
                                    entry as i128 - address as i128 - fixup.offset as i128 - 4,
                                )
                                .unwrap();
                                bytes[fixup.offset..fixup.offset + 4]
                                    .copy_from_slice(&delta.to_le_bytes());
                            }
                            EntrySymbol::Scratch => pointers.push((fixup.offset, storage.scratch)),
                            EntrySymbol::StackTop => {
                                pointers.push((fixup.offset, storage.stack_top))
                            }
                            _ => panic!("unexpected unit entry relocation"),
                        }
                    }
                    for (field, symbol) in pointers {
                        let destination = if symbol == scratch {
                            bss
                        } else {
                            bss + partitions.allocation_offset
                        };
                        let displacement = i32::try_from(
                            destination as i128 - (address + field as u64 + 4) as i128,
                        )
                        .unwrap();
                        bytes[field..field + 4].copy_from_slice(&displacement.to_le_bytes());
                    }
                    validate_x86_64(target, &bytes, address, entry, bss, partitions).unwrap();
                    for bit in 0..bytes.len() * 8 {
                        let mut changed = bytes.clone();
                        changed[bit / 8] ^= 1 << (bit % 8);
                        assert!(
                            validate_x86_64(target, &changed, address, entry, bss, partitions)
                                .is_err(),
                            "{target:?} bit {bit}"
                        );
                    }
                    assert!(
                        validate_x86_64(target, &bytes, address, entry, bss + 16, partitions)
                            .is_err()
                    );
                    assert!(
                        validate_x86_64(target, &bytes, address, entry + 4, bss, partitions)
                            .is_err()
                    );
                    assert!(
                        validate_x86_64(
                            target,
                            &bytes[..bytes.len() - 1],
                            address,
                            entry,
                            bss,
                            partitions
                        )
                        .is_err()
                    );
                }
            }
        }
    }

    use diagnostics::Diagnostic;
    use object_file::{
        ObjectPlan, ObjectSymbolHandle, SectionKind, SymbolKind, SymbolPlan, SymbolSection,
    };

    #[derive(Clone, PartialEq, Eq)]
    pub(crate) struct EntryShim {
        pub(crate) surface: EntrySurface,
        pub(crate) symbol: ObjectSymbolHandle,
        pub(crate) offset: usize,
        pub(crate) stack: Option<owned_stack::HostedUnitStackPlan>,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum EntrySurface {
        Physical,
        ProvisionedUnit,
        DarwinReceiver,
        LinuxReceiver,
        LinuxArm64Receiver,
        WindowsReceiver,
        DarwinUnit,
        DarwinX86_64Unit,
        WindowsUnit,
        LinuxUnit,
        LinuxArm64Unit,
    }

    impl EntrySurface {
        pub(crate) const fn shim(self, symbol: ObjectSymbolHandle, offset: usize) -> EntryShim {
            EntryShim {
                surface: self,
                symbol,
                offset,
                stack: None,
            }
        }
    }

    impl EntryShim {
        pub(crate) const fn symbol_offset(&self) -> (ObjectSymbolHandle, usize) {
            (self.symbol, self.offset)
        }
        pub(crate) fn with_stack(
            mut self,
            demand: crate::StackDemand,
            partitions: crate::entries::hosted_entry_storage::HostedEntryPartitions,
        ) -> Self {
            self.surface = EntrySurface::ProvisionedUnit;
            self.stack = Some(owned_stack::HostedUnitStackPlan { demand, partitions });
            self
        }
    }

    pub(crate) struct PreparedEntry {
        pub object: ObjectPlan,
        pub text: Vec<u8>,
        pub relocations: object_file::RelocationPlan,
        pub shim: EntryShim,
    }

    pub(crate) struct PreparedParts<'a> {
        pub object: &'a ObjectPlan,
        pub text_bytes: &'a [u8],
        pub relocations: &'a object_file::RelocationPlan,
        pub entry_shim: Option<EntryShim>,
    }

    pub(crate) fn prepared_parts<'a>(
        artifact: &'a crate::ObjectArtifact,
        prepared_entry: Option<&'a PreparedEntry>,
    ) -> PreparedParts<'a> {
        prepared_entry.map_or(
            PreparedParts {
                object: artifact.object(),
                text_bytes: artifact.text_bytes(),
                relocations: artifact.relocations(),
                entry_shim: None,
            },
            |prepared| PreparedParts {
                object: &prepared.object,
                text_bytes: prepared.text.as_slice(),
                relocations: &prepared.relocations,
                entry_shim: Some(prepared.shim.clone()),
            },
        )
    }

    pub(crate) struct UnitEntryForm {
        pub(crate) name: &'static str,
        pub(crate) surface: EntrySurface,
        header_check: fn(&image::EmittedImageOutput, usize, &[u8]) -> bool,
        header_error: &'static str,
    }

    pub(crate) fn unit_entry_form(target: target::NativeTarget) -> Option<UnitEntryForm> {
        Some(if target == target::NativeTarget::macos_arm64() {
            UnitEntryForm {
                name: "omega_darwin_unit_entry",
                surface: EntrySurface::DarwinUnit,
                header_check: |output, offset, _expected| {
                    main_points_to(&output.bytes, offset, target::NativeTarget::macos_arm64())
                },
                header_error: "Mach-O LC_MAIN does not select the exact Unit entry adapter",
            }
        } else if target == target::NativeTarget::macos_x64() {
            UnitEntryForm {
                name: "omega_darwin_x86_64_unit_entry",
                surface: EntrySurface::DarwinX86_64Unit,
                header_check: |output, offset, _expected| {
                    main_points_to(&output.bytes, offset, target::NativeTarget::macos_x64())
                },
                header_error: "Mach-O LC_MAIN does not select the exact Darwin x86-64 Unit entry adapter",
            }
        } else if target == target::NativeTarget::windows_x64() {
            UnitEntryForm {
                name: "omega_windows_unit_entry",
                surface: EntrySurface::WindowsUnit,
                header_check: |output, offset, expected| {
                    pe_entry_points_to(&output.bytes, offset, expected)
                },
                header_error: "PE AddressOfEntryPoint does not select the exact Unit entry adapter",
            }
        } else if target == target::NativeTarget::linux_x64() {
            UnitEntryForm {
                name: "omega_linux_x86_64_unit_entry",
                surface: EntrySurface::LinuxUnit,
                header_check: |output, offset, expected| {
                    elf_entry_points_to(
                        &output.bytes,
                        62,
                        output.final_image_layout.text_address,
                        offset,
                        expected,
                    )
                },
                header_error: "ELF e_entry does not select the exact Linux x86-64 Unit entry adapter",
            }
        } else if target == target::NativeTarget::linux_arm64() {
            UnitEntryForm {
                name: "omega_linux_arm64_unit_entry",
                surface: EntrySurface::LinuxArm64Unit,
                header_check: |output, offset, expected| {
                    elf_entry_points_to(
                        &output.bytes,
                        183,
                        output.final_image_layout.text_address,
                        offset,
                        expected,
                    )
                },
                header_error: "ELF e_entry does not select the exact Linux ARM64 Unit entry adapter",
            }
        } else {
            return None;
        })
    }

    pub(crate) fn prepare(
        artifact: &crate::ObjectArtifact,
    ) -> Result<Option<PreparedEntry>, Diagnostic> {
        crate::object::object_artifact::installed_stack::require_installed_root_provision(artifact)
            .map_err(|error| Diagnostic::error(error.to_string()))?;
        if artifact.physical_entry_binding().is_some() {
            return crate::entries::physical_entry::prepare(artifact).map(Some);
        }
        crate::hosted_program::prepare(artifact)
    }

    pub(crate) fn install_entry_shim(
        object: &mut ObjectPlan,
        text: &mut Vec<u8>,
        bytes: &[u8],
        name: &str,
        missing_text_section: &dyn Fn() -> Diagnostic,
    ) -> Result<(ObjectSymbolHandle, usize), Diagnostic> {
        let offset = text.len();
        text.extend_from_slice(bytes);
        let section = object
            .layout
            .sections
            .iter()
            .find(|(_, row)| row.kind == SectionKind::Text)
            .map(|(handle, _)| handle)
            .ok_or_else(missing_text_section)?;
        object.layout.sections.get_mut(section).size = text.len();
        let symbol = object.layout.symbols.insert(SymbolPlan {
            name: name.into(),
            section: SymbolSection::Section(SectionKind::Text),
            offset,
            size: bytes.len(),
            kind: SymbolKind::Function,
            import_library: String::new(),
        });
        object.layout.entry_symbol = symbol;
        Ok((symbol, offset))
    }

    pub(crate) fn unique_region(
        object: &ObjectPlan,
        symbol: ObjectSymbolHandle,
        offset: usize,
        byte_count: usize,
        output: &image::EmittedImageOutput,
    ) -> bool {
        let name = object_file::object_symbol_name(object, symbol);
        output
            .executable_regions
            .regions
            .iter()
            .filter(|region| {
                region.origin == image::FinalExecutableRegionOrigin::CompilerFunction
                    && region.symbol == name
                    && region.section_offset == offset
                    && region.byte_count == byte_count
            })
            .count()
            == 1
    }

    fn validate_unit_entry(
        form: &UnitEntryForm,
        target: target::NativeTarget,
        artifact: &crate::ObjectArtifact,
        object: &ObjectPlan,
        text: &[u8],
        relocations: &object_file::RelocationPlan,
        shim: EntryShim,
        output: &image::EmittedImageOutput,
    ) -> Result<(), Diagnostic> {
        if shim.surface == EntrySurface::ProvisionedUnit {
            owned_stack::validate(
                form,
                target,
                artifact,
                object,
                text,
                relocations,
                shim,
                output,
            )
        } else {
            loader_stack::validate(
                form,
                target,
                artifact,
                object,
                text,
                relocations,
                shim,
                output,
            )
        }
    }

    pub(crate) fn validate_unit_shim(
        shim: EntryShim,
        artifact: &crate::ObjectArtifact,
        object: &ObjectPlan,
        text: &[u8],
        relocations: &object_file::RelocationPlan,
        output: &image::EmittedImageOutput,
    ) -> Result<(), Diagnostic> {
        let target = match shim.surface {
            EntrySurface::ProvisionedUnit => artifact.target,
            EntrySurface::DarwinUnit => target::NativeTarget::macos_arm64(),
            EntrySurface::DarwinX86_64Unit => target::NativeTarget::macos_x64(),
            EntrySurface::WindowsUnit => target::NativeTarget::windows_x64(),
            EntrySurface::LinuxUnit => target::NativeTarget::linux_x64(),
            EntrySurface::LinuxArm64Unit => target::NativeTarget::linux_arm64(),
            _ => {
                return Err(Diagnostic::error(
                    "Unit entry validation requires a free-Unit adapter shim",
                ));
            }
        };
        let form = unit_entry_form(target)
            .ok_or_else(|| Diagnostic::error("Unit entry has no mechanics row"))?;
        validate_unit_entry(
            &form,
            target,
            artifact,
            object,
            text,
            relocations,
            shim,
            output,
        )
    }

    pub(crate) fn pe_entry_points_to(
        bytes: &[u8],
        shim_offset: usize,
        expected_shim: &[u8],
    ) -> bool {
        fn word16(bytes: &[u8], offset: usize) -> Option<u16> {
            crate::codec_bytes::u16_at(bytes, offset)
        }
        fn word32(bytes: &[u8], offset: usize) -> Option<u32> {
            crate::codec_bytes::u32_at(bytes, offset)
        }
        if bytes.get(0..2) != Some(b"MZ") {
            return false;
        }
        let Some(pe) = word32(bytes, 0x3c).and_then(|pe| usize::try_from(pe).ok()) else {
            return false;
        };
        let Some(signature_end) = pe.checked_add(4) else {
            return false;
        };
        if bytes.get(pe..signature_end) != Some(b"PE\0\0") {
            return false;
        }
        let Some(coff) = pe.checked_add(4) else {
            return false;
        };
        let Some(sections) = word16(bytes, coff + 2) else {
            return false;
        };
        let Some(optional_size) = word16(bytes, coff + 16) else {
            return false;
        };
        let Some(optional) = coff.checked_add(20) else {
            return false;
        };
        if word16(bytes, optional) != Some(0x20b) {
            return false;
        }
        let Some(entry_rva) = word32(bytes, optional + 16) else {
            return false;
        };
        let Some(first_section) = optional.checked_add(usize::from(optional_size)) else {
            return false;
        };
        let mut text = None;
        for index in 0..usize::from(sections) {
            let Some(header) = first_section
                .checked_add(index.saturating_mul(40))
                .filter(|header| header.checked_add(40).is_some_and(|end| end <= bytes.len()))
            else {
                return false;
            };
            if bytes.get(header..header + 8) != Some(b".text\0\0\0") {
                continue;
            }
            if text.is_some() {
                return false;
            }
            let Some(virtual_address) = word32(bytes, header + 12) else {
                return false;
            };
            let Some(raw_size) = word32(bytes, header + 16) else {
                return false;
            };
            let Some(raw_pointer) = word32(bytes, header + 20) else {
                return false;
            };
            text = Some((virtual_address, raw_size, raw_pointer));
        }
        let Some((virtual_address, raw_size, raw_pointer)) = text else {
            return false;
        };
        if u64::from(entry_rva) != u64::from(virtual_address).saturating_add(shim_offset as u64) {
            return false;
        }
        let Some(raw_start) = (raw_pointer as usize).checked_add(shim_offset) else {
            return false;
        };
        let Some(raw_end) = raw_start.checked_add(expected_shim.len()) else {
            return false;
        };
        if raw_end - (raw_pointer as usize) > raw_size as usize {
            return false;
        }
        bytes.get(raw_start..raw_end) == Some(expected_shim)
    }

    pub(crate) fn elf_entry_points_to(
        bytes: &[u8],
        machine: u16,
        text_address: u64,
        shim_offset: usize,
        expected_shim: &[u8],
    ) -> bool {
        fn word16(bytes: &[u8], offset: usize) -> Option<u16> {
            crate::codec_bytes::u16_at(bytes, offset)
        }
        fn word64(bytes: &[u8], offset: usize) -> Option<u64> {
            crate::codec_bytes::u64_at(bytes, offset)
        }
        if bytes.get(0..4) != Some(&[0x7f, b'E', b'L', b'F'])
            || bytes.get(4) != Some(&2)
            || bytes.get(5) != Some(&1)
            || word16(bytes, 16) != Some(2)
            || word16(bytes, 18) != Some(machine)
        {
            return false;
        }
        let Some(entry) = word64(bytes, 24) else {
            return false;
        };
        if entry != text_address.saturating_add(shim_offset as u64) {
            return false;
        }
        let Some(program_headers) = word64(bytes, 32).and_then(|v| usize::try_from(v).ok()) else {
            return false;
        };
        let Some(header_size) = word16(bytes, 54).map(usize::from) else {
            return false;
        };
        let Some(header_count) = word16(bytes, 56).map(usize::from) else {
            return false;
        };
        let shim_file_offset = (0..header_count).find_map(|index| {
            let header = program_headers.checked_add(index.checked_mul(header_size)?)?;
            let kind = bytes
                .get(header..header.checked_add(4)?)
                .and_then(|v| TryInto::<[u8; 4]>::try_into(v).ok())
                .map(u32::from_le_bytes)?;
            let flags = bytes
                .get(header + 4..header.checked_add(8)?)
                .and_then(|v| TryInto::<[u8; 4]>::try_into(v).ok())
                .map(u32::from_le_bytes)?;
            let segment_offset = word64(bytes, header + 8)?;
            let segment_address = word64(bytes, header + 16)?;
            let segment_file_size = word64(bytes, header + 32)?;
            if kind != 1 || flags & 1 == 0 {
                return None;
            }
            if entry < segment_address || entry >= segment_address.checked_add(segment_file_size)? {
                return None;
            }
            let file_offset = segment_offset.checked_add(entry.checked_sub(segment_address)?)?;
            usize::try_from(file_offset).ok()
        });
        let Some(shim_file_offset) = shim_file_offset else {
            return false;
        };
        let Some(shim_file_end) = shim_file_offset.checked_add(expected_shim.len()) else {
            return false;
        };
        bytes.get(shim_file_offset..shim_file_end) == Some(expected_shim)
    }

    pub(crate) fn main_points_to(
        bytes: &[u8],
        shim_offset: usize,
        target: target::NativeTarget,
    ) -> bool {
        fn word(bytes: &[u8], offset: usize) -> Option<u32> {
            crate::codec_bytes::u32_at(bytes, offset)
        }
        let cpu_type = if target == target::NativeTarget::macos_arm64() {
            0x0100_000c
        } else if target == target::NativeTarget::macos_x64() {
            0x0100_0007
        } else {
            return false;
        };
        if word(bytes, 0) != Some(0xfeed_facf) || word(bytes, 4) != Some(cpu_type) {
            return false;
        }
        let Some(count) = word(bytes, 16) else {
            return false;
        };
        let Some(commands_end) = word(bytes, 20)
            .and_then(|size| 32usize.checked_add(size as usize))
            .filter(|end| *end <= bytes.len())
        else {
            return false;
        };
        let mut cursor = 32usize;
        let mut entry = None;
        let mut text = None;
        for _ in 0..count {
            let Some(command) = word(bytes, cursor) else {
                return false;
            };
            let Some(size) = word(bytes, cursor + 4).and_then(|size| usize::try_from(size).ok())
            else {
                return false;
            };
            let Some(end) = cursor
                .checked_add(size)
                .filter(|end| size >= 8 && size.is_multiple_of(8) && *end <= commands_end)
            else {
                return false;
            };
            if command == 0x8000_0028 {
                if entry.is_some() || size != 24 {
                    return false;
                }
                entry = crate::codec_bytes::u64_at(bytes, cursor + 8);
            }
            if command == 0x19 {
                if size < 72 {
                    return false;
                }
                let Some(sections) = word(bytes, cursor + 64) else {
                    return false;
                };
                if (sections as usize)
                    .checked_mul(80)
                    .and_then(|size| size.checked_add(72))
                    != Some(size)
                {
                    return false;
                }
                for section in 0..sections as usize {
                    let Some(section) = section
                        .checked_mul(80)
                        .and_then(|delta| cursor.checked_add(72)?.checked_add(delta))
                        .filter(|section| {
                            section
                                .checked_add(80)
                                .is_some_and(|end_section| end_section <= end)
                        })
                    else {
                        return false;
                    };
                    if bytes.get(section..section + 16) == Some(b"__text\0\0\0\0\0\0\0\0\0\0") {
                        if text.is_some() {
                            return false;
                        }
                        text = word(bytes, section + 48).map(u64::from);
                    }
                }
            }
            cursor = end;
        }
        cursor == commands_end
            && entry.is_some()
            && entry == text.and_then(|text| text.checked_add(shim_offset as u64))
    }

    #[cfg(test)]
    mod tests {
        fn placed_hosted_code(
            target: target::NativeTarget,
            mode: machine_code::hosted_entry::HostedEntryMode,
            address: u64,
            entry: u64,
            scratch: u64,
            stack_top: u64,
        ) -> Vec<u8> {
            use entry_planning::hosted_entry::plan_hosted_entry;
            use machine_code::hosted_entry::{EntryFixupKind, EntrySymbol};
            use native_encoding::hosted_entry::encode_hosted_entry;
            let code = encode_hosted_entry(plan_hosted_entry(target, mode, &[]).unwrap()).unwrap();
            let mut bytes = code.bytes;
            for fixup in code.fixups {
                let destination = match fixup.destination {
                    EntrySymbol::Body => entry,
                    EntrySymbol::Scratch => scratch,
                    EntrySymbol::StackTop => stack_top,
                    _ => panic!("unexpected unit entry address"),
                };
                let origin = address + fixup.offset as u64;
                let field = &mut bytes[fixup.offset..fixup.offset + 4];
                let mut word = u32::from_le_bytes(field.try_into().unwrap());
                match fixup.kind {
                    EntryFixupKind::X86Relative32 => {
                        word =
                            i32::try_from(destination as i128 - origin as i128 - 4).unwrap() as u32
                    }
                    EntryFixupKind::Aarch64Branch26 => {
                        word |= ((destination as i128 - origin as i128) / 4) as u32 & 0x03ff_ffff
                    }
                    EntryFixupKind::Aarch64Page21 => {
                        let pages =
                            ((destination & !4095) as i128 - (origin & !4095) as i128) / 4096;
                        let value = pages as u32 & 0x1f_ffff;
                        word |= (value & 3) << 29 | (value >> 2) << 5;
                    }
                    EntryFixupKind::Aarch64PageOffset12 => {
                        word |= (destination as u32 & 4095) << 10
                    }
                }
                field.copy_from_slice(&word.to_le_bytes());
            }
            bytes
        }

        #[test]
        fn planned_loader_entries_preserve_all_hosted_target_encodings() {
            use machine_code::hosted_entry::HostedEntryMode;
            for target in [
                target::NativeTarget::linux_x64(),
                target::NativeTarget::linux_arm64(),
                target::NativeTarget::macos_x64(),
                target::NativeTarget::macos_arm64(),
                target::NativeTarget::windows_x64(),
            ] {
                for (address, entry) in [(0x4000, 0x1000), (0x2000, 0x7800)] {
                    let bytes = placed_hosted_code(
                        target,
                        HostedEntryMode::LoaderStack,
                        address,
                        entry,
                        0,
                        0,
                    );
                    assert!(
                        crate::hosted_loader::matches_entry(
                            target,
                            address as usize,
                            entry as usize,
                            &bytes
                        ),
                        "{target:?}"
                    );
                    assert!(!crate::hosted_loader::matches_entry(
                        target,
                        address as usize,
                        entry as usize + 4,
                        &bytes
                    ));
                    for index in 0..bytes.len() {
                        let mut modified = bytes.clone();
                        modified[index] ^= 1;
                        assert!(
                            !crate::hosted_loader::matches_entry(
                                target,
                                address as usize,
                                entry as usize,
                                &modified
                            ),
                            "{target:?} changed byte at {index}"
                        );
                    }
                }
            }
        }

        #[test]
        fn planned_aarch64_owned_entries_pass_independent_byte_and_address_checks() {
            use machine_code::hosted_entry::HostedEntryMode;
            let partitions = crate::entries::hosted_entry_storage::HostedEntryPartitions {
                saved_continuation_offset: 0,
                stack_offset: 16,
                stack_byte_count: 9 * 1024 * 1024,
                allocation_offset: 16 + 9 * 1024 * 1024,
                end_offset: 16 + 9 * 1024 * 1024,
            };
            for target in [
                target::NativeTarget::linux_arm64(),
                target::NativeTarget::macos_arm64(),
            ] {
                for address in [0x1000, 0x1ffc] {
                    let entry = 0x6000;
                    let bss = 0x30000;
                    let bytes = placed_hosted_code(
                        target,
                        HostedEntryMode::OwnedStack,
                        address,
                        entry,
                        bss,
                        bss + partitions.allocation_offset,
                    );
                    let validate = |bytes: &[u8], destination| {
                        crate::entries::hosted_receiver::instructions::validate_aarch64(
                            target,
                            bytes,
                            address,
                            destination,
                            bss,
                            partitions,
                            &[],
                        )
                    };
                    validate(&bytes, entry).unwrap();
                    assert!(validate(&bytes, entry + 4).is_err());
                    for bit in 0..bytes.len() * 8 {
                        let mut corrupt = bytes.clone();
                        corrupt[bit / 8] ^= 1 << (bit % 8);
                        assert!(validate(&corrupt, entry).is_err(), "{target:?} bit {bit}");
                    }
                }
            }
        }

        use super::{elf_entry_points_to, main_points_to, pe_entry_points_to};
        #[test]
        fn darwin_main_requires_exact_adapter_offset_and_target_header() {
            let mut bytes = vec![0u8; 208];
            for (offset, word) in [
                (0, 0xfeed_facfu32),
                (4, 0x0100_000c),
                (16, 2),
                (20, 176),
                (32, 0x19),
                (36, 152),
                (96, 1),
                (152, 4096),
                (184, 0x8000_0028),
                (188, 24),
            ] {
                bytes[offset..offset + 4].copy_from_slice(&word.to_le_bytes());
            }
            bytes[104..120].copy_from_slice(b"__text\0\0\0\0\0\0\0\0\0\0");
            bytes[192..192 + 8].copy_from_slice(&4160u64.to_le_bytes());
            let arm64 = target::NativeTarget::macos_arm64();
            assert!(main_points_to(&bytes, 64, arm64));
            assert!(!main_points_to(&bytes, 68, arm64));
            assert!(!main_points_to(
                &bytes,
                64,
                target::NativeTarget::macos_x64()
            ));
            assert!(!main_points_to(
                &bytes,
                64,
                target::NativeTarget::linux_x64()
            ));
            let mut intel = bytes.clone();
            intel[4..4 + 4].copy_from_slice(&0x0100_0007u32.to_le_bytes());
            assert!(main_points_to(
                &intel,
                64,
                target::NativeTarget::macos_x64()
            ));
            assert!(!main_points_to(&intel, 64, arm64));
            for offset in [0, 4, 16, 20, 36, 96, 104, 152, 184, 188, 192] {
                let mut hostile = bytes.clone();
                hostile[offset] ^= 1;
                assert!(!main_points_to(&hostile, 64, arm64), "offset {offset}");
            }
            assert!(!main_points_to(&bytes[..207], 64, arm64));
        }
        #[test]
        fn windows_pe_entry_selects_the_exact_adapter_bytes() {
            let shim = vec![0x90; 16];
            let mut bytes = vec![0u8; 0x400];
            bytes[0..2].copy_from_slice(b"MZ");
            bytes[0x3c..0x3c + 4].copy_from_slice(&0x80u32.to_le_bytes());
            bytes[0x80..0x84].copy_from_slice(b"PE\0\0");
            let coff = 0x84;
            bytes[coff + 2..coff + 2 + 2].copy_from_slice(&1u16.to_le_bytes());
            bytes[coff + 16..coff + 16 + 2].copy_from_slice(&240u16.to_le_bytes());
            let optional = coff + 20;
            bytes[optional..optional + 2].copy_from_slice(&0x20bu16.to_le_bytes());
            bytes[optional + 16..optional + 16 + 4].copy_from_slice(&0x1020u32.to_le_bytes());
            let header = optional + 240;
            bytes[header..header + 8].copy_from_slice(b".text\0\0\0");
            bytes[header + 12..header + 12 + 4].copy_from_slice(&0x1000u32.to_le_bytes());
            bytes[header + 16..header + 16 + 4].copy_from_slice(&0x200u32.to_le_bytes());
            bytes[header + 20..header + 20 + 4].copy_from_slice(&0x200u32.to_le_bytes());
            bytes[0x220..0x230].copy_from_slice(&shim);
            assert!(pe_entry_points_to(&bytes, 0x20, &shim));
            assert!(!pe_entry_points_to(&bytes, 0x24, &shim));
            assert!(!pe_entry_points_to(&bytes, 0x20, &[0x91; 16]));
            for offset in [
                0,
                0x3c,
                0x80,
                coff + 16,
                optional,
                optional + 16,
                header,
                header + 12,
                header + 20,
                0x220,
            ] {
                let mut hostile = bytes.clone();
                hostile[offset] ^= 1;
                assert!(
                    !pe_entry_points_to(&hostile, 0x20, &shim),
                    "offset {offset:#x}"
                );
            }
            assert!(!pe_entry_points_to(&bytes[..0x22f], 0x20, &shim));
        }
        #[test]
        fn linux_elf_entry_selects_the_exact_adapter_bytes() {
            let shim = vec![0x90; 19];
            let text_address = 0x401000u64;
            let mut bytes = vec![0u8; 0x2000];
            bytes[0..4].copy_from_slice(&[0x7f, b'E', b'L', b'F']);
            bytes[4] = 2;
            bytes[5] = 1;
            bytes[16..16 + 2].copy_from_slice(&2u16.to_le_bytes());
            bytes[18..18 + 2].copy_from_slice(&62u16.to_le_bytes());
            bytes[24..32].copy_from_slice(&(text_address + 0x140).to_le_bytes());
            bytes[32..32 + 8].copy_from_slice(&64u64.to_le_bytes());
            bytes[54..54 + 2].copy_from_slice(&56u16.to_le_bytes());
            bytes[56..56 + 2].copy_from_slice(&2u16.to_le_bytes());
            bytes[64..64 + 4].copy_from_slice(&1u32.to_le_bytes());
            bytes[68..68 + 4].copy_from_slice(&5u32.to_le_bytes());
            bytes[72..72 + 8].copy_from_slice(&0u64.to_le_bytes());
            bytes[80..80 + 8].copy_from_slice(&0x400000u64.to_le_bytes());
            bytes[96..96 + 8].copy_from_slice(&0x1200u64.to_le_bytes());
            bytes[120..120 + 4].copy_from_slice(&1u32.to_le_bytes());
            bytes[124..124 + 4].copy_from_slice(&6u32.to_le_bytes());
            bytes[128..128 + 8].copy_from_slice(&0x2000u64.to_le_bytes());
            bytes[136..136 + 8].copy_from_slice(&0x402000u64.to_le_bytes());
            bytes[152..152 + 8].copy_from_slice(&0x100u64.to_le_bytes());
            bytes[0x1140..0x1140 + 19].copy_from_slice(&shim);
            assert!(elf_entry_points_to(&bytes, 62, text_address, 0x140, &shim));
            assert!(!elf_entry_points_to(&bytes, 62, text_address, 0x144, &shim));
            assert!(!elf_entry_points_to(
                &bytes,
                183,
                text_address,
                0x140,
                &shim
            ));
            assert!(!elf_entry_points_to(
                &bytes,
                62,
                text_address,
                0x140,
                &[0x91; 19]
            ));
            for offset in [0, 4, 16, 18, 24, 64, 68, 0x1140] {
                let mut hostile = bytes.clone();
                hostile[offset] ^= 1;
                assert!(
                    !elf_entry_points_to(&hostile, 62, text_address, 0x140, &shim),
                    "offset {offset:#x}"
                );
            }
            assert!(!elf_entry_points_to(
                &bytes[..0x1140 + 18],
                62,
                text_address,
                0x140,
                &shim
            ));
        }
    }
}

pub(crate) mod physical_entry {
    use crate::NativeEntryBridge;
    use crate::entries::callable_entry_transport::native_entry_request;
    use crate::object::object_artifact::stack_demand::derive_body_stack_demand;
    use calling_conventions::BoundaryEntryPlan;
    use diagnostics::Diagnostic;
    use object_file::{
        ObjectPlan, ObjectSymbolHandle, RelocationKind, RelocationOrigin, SectionKind, SectionPlan,
        SymbolKind, SymbolPlan, SymbolSection,
    };
    use semantic_vocabulary::MachineId;

    use crate::ObjectArtifact;
    use crate::entries::hosted_unit_entry::{EntrySurface, PreparedEntry};

    mod requirements {
        use super::{PhysicalEntryPartitions, StackSwitch, invalid, original_bss};
        use crate::NativeEntryBridge;
        use crate::{ObjectArtifact, StackDemand};
        use calling_conventions::BoundaryEntryPlan;
        use diagnostics::Diagnostic;
        use program_entry_contracts::ProgramEntryPhysicalContractPlan;
        use semantic_vocabulary::{MachineId, OperationId};

        mod cpu {
            use crate::ObjectArtifact;
            use diagnostics::Diagnostic;
            use semantic_vocabulary::{MachineId, OperationId};

            use physical_recipes::physical_entry::InstalledPhysicalCpuRequirement;

            pub(super) fn derive(
                artifact: &ObjectArtifact,
                scopes: &[(MachineId, OperationId)],
            ) -> Result<Option<InstalledPhysicalCpuRequirement>, Diagnostic> {
                if scopes.is_empty() {
                    return Ok(None);
                }
                let invalid = super::super::invalid;
                if artifact.target().architecture != target::Architecture::X86_64 {
                    return Err(invalid(
                        "installed physical CPU requirements have no supported transition",
                    ));
                }
                let mut observed = vec![false; scopes.len()];
                for private in artifact.private_functions() {
                    let Some(bridge) = &private.entry_bridge else {
                        continue;
                    };
                    let Some(plan) = bridge.installed_interrupt() else {
                        continue;
                    };
                    let execution = private.body.installed_entry_execution().ok_or_else(|| {
                        invalid("installed CPU transition has no admitted parent execution")
                    })?;
                    let operation = (execution.scope().machine(), execution.scope().operation());
                    let Ok(index) = scopes.binary_search(&operation) else {
                        continue;
                    };
                    if bridge.request().target != artifact.target()
                        || private.body.psi() != artifact.psi()
                        || private.body.target() != artifact.target()
                        || bridge.request().inbound.call.entry_control
                            != calling_conventions::EntryControl::InterruptReturn
                        || !plan.has_canonical_geometry()
                    {
                        return Err(invalid(
                            "installed CPU transition changed its physical owner or protocol",
                        ));
                    }
                    match plan.protocol {
                        runtime_abi::static_interrupts::InstalledInterruptProtocol::Acknowledgement
                        | runtime_abi::static_interrupts::InstalledInterruptProtocol::FatalContext { .. } => {
                            observed[index] = true;
                        }
                    }
                }
                if observed.iter().any(|present| !present) {
                    return Err(invalid(
                        "installed CPU requirement has no actual entry transition",
                    ));
                }
                Ok(Some(
                    InstalledPhysicalCpuRequirement::X86LongModeCpl0WithoutCet,
                ))
            }
        }

        pub use physical_recipes::physical_entry::InstalledPhysicalCpuRequirement;

        pub use physical_recipes::physical_stack_switch::PhysicalIncomingStackRequirement;

        pub use physical_recipes::physical_entry::PhysicalEntryStackRequirement;

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct PhysicalEntryRequirements {
            psi: terminal_psi::TerminalPsiIdentity,
            target: target::NativeTarget,
            semantic_entry: MachineId,
            adapter: MachineId,
            contract: ProgramEntryPhysicalContractPlan,
            stack: PhysicalEntryStackRequirement,
            retained_storage: PhysicalEntryPartitions,
            installed_scopes: Vec<(MachineId, OperationId)>,
            installed_cpu: Option<InstalledPhysicalCpuRequirement>,
        }

        impl PhysicalEntryRequirements {
            pub const fn psi(&self) -> terminal_psi::TerminalPsiIdentity {
                self.psi
            }
            pub const fn target(&self) -> target::NativeTarget {
                self.target
            }
            pub const fn semantic_entry(&self) -> MachineId {
                self.semantic_entry
            }
            pub const fn adapter(&self) -> MachineId {
                self.adapter
            }
            pub fn contract(&self) -> &ProgramEntryPhysicalContractPlan {
                &self.contract
            }
            pub const fn stack(&self) -> PhysicalEntryStackRequirement {
                self.stack
            }
            pub const fn retained_storage(&self) -> PhysicalEntryPartitions {
                self.retained_storage
            }
            pub fn installed_scopes(&self) -> &[(MachineId, OperationId)] {
                &self.installed_scopes
            }
            pub const fn installed_cpu(&self) -> Option<InstalledPhysicalCpuRequirement> {
                self.installed_cpu
            }
        }

        pub(super) fn partitions(
            existing_bss_bytes: u64,
            plan: &BoundaryEntryPlan,
            demand: &StackDemand,
            bridge: &NativeEntryBridge,
            switch: &StackSwitch,
            arrival: &target::TargetEntryStackGuarantee,
            installed_reserve: u64,
        ) -> Result<PhysicalEntryPartitions, Diagnostic> {
            let stack_demand = demand
                .ceiling_bytes()
                .ok_or_else(|| {
                    invalid("private entry requires an admitted finite foreign-stack bound")
                })?
                .checked_add(u64::from(bridge.frame_bytes()))
                .and_then(|bytes| bytes.checked_add(u64::from(bridge.call_stack_bias())))
                .and_then(|bytes| bytes.checked_add(switch.stack_overhead_bytes()))
                .and_then(|bytes| bytes.checked_add(arrival.admission_reserve_bytes()))
                .and_then(|bytes| bytes.checked_add(installed_reserve))
                .ok_or_else(|| invalid("private entry stack demand overflows"))?;
            PhysicalEntryPartitions::derive(
                existing_bss_bytes,
                u64::from(plan.call.stack_alignment),
                stack_demand,
                arrival.guaranteed_available_bytes(),
            )
        }

        pub(super) fn place(
            plan: physical_recipes::physical_entry::PlannedRequirements<
                program_entry_contracts::ProgramEntryPhysicalContractPlan,
            >,
            existing_bss: u64,
        ) -> Result<PhysicalEntryRequirements, Diagnostic> {
            let retained_storage = PhysicalEntryPartitions::derive(
                existing_bss,
                plan.alignment,
                plan.stack_bytes,
                plan.minimum_stack_bytes,
            )?;
            Ok(PhysicalEntryRequirements {
                psi: plan.psi,
                target: plan.target,
                semantic_entry: plan.semantic_entry,
                adapter: plan.adapter,
                contract: plan.contract,
                stack: plan.stack,
                retained_storage,
                installed_scopes: plan.installed_scopes,
                installed_cpu: plan.installed_cpu,
            })
        }

        pub(super) fn derive(
            artifact: &ObjectArtifact,
            semantic_entry: MachineId,
            contract: &ProgramEntryPhysicalContractPlan,
            demand: &StackDemand,
            bridge: &NativeEntryBridge,
            switch: &StackSwitch,
            installed_reserve: u64,
        ) -> Result<PhysicalEntryRequirements, Diagnostic> {
            if contract.target_slot().owner != target::TargetProfile::UefiX64
                || artifact.profile != Some(contract.target_slot().owner)
                || contract.target_slot().owner.native_target() != artifact.target()
            {
                return Err(invalid(
                    "physical memory requirement changed its selected target contract",
                ));
            }
            let arrival = contract
                .guaranteed_entry_stack()
                .ok_or_else(|| invalid("physical memory requirement lost its arrival contract"))?;
            if !arrival.admits_demand(
                switch.incoming.bytes_below_entry_rsp,
                switch.incoming.caller_alignment,
            ) || switch.incoming.caller_alignment
                != u64::from(contract.boundary_entry_plan().call.stack_alignment)
            {
                return Err(invalid(
                    "physical switch incoming demand changed its target contract",
                ));
            }
            let stack = PhysicalEntryStackRequirement::CheckedPrivateImage {
                incoming: switch.incoming,
                minimum_private_stack_bytes: arrival.guaranteed_available_bytes(),
            };
            let retained_storage = partitions(
                original_bss(artifact)?.map_or(0, |(_, bytes, _)| bytes as u64),
                contract.boundary_entry_plan(),
                demand,
                bridge,
                switch,
                arrival,
                installed_reserve,
            )?;
            let source = artifact.composition_source.as_deref().unwrap_or(artifact);
            let mut installed_scopes = Vec::new();
            for requirement in source
                .emission_input
                .installation_authority()
                .installed_stack_requirements()
            {
                let Some(arrival) = &requirement.arrival else {
                    continue;
                };
                if arrival.physical_entry.semantic_entry != semantic_entry
                    || arrival.physical_entry.adapter != artifact.entry()
                    || requirement.psi != artifact.psi()
                    || requirement.target != artifact.target()
                {
                    return Err(invalid(
                        "physical memory requirement changed its installed occurrence",
                    ));
                }
                installed_scopes.push((requirement.installation_machine, requirement.operation));
            }
            installed_scopes.sort_unstable();
            installed_scopes.dedup();
            let installed_cpu = cpu::derive(artifact, &installed_scopes)?;
            Ok(PhysicalEntryRequirements {
                psi: artifact.psi(),
                target: artifact.target(),
                semantic_entry,
                adapter: artifact.entry(),
                contract: contract.clone(),
                stack,
                retained_storage,
                installed_scopes,
                installed_cpu,
            })
        }
    }

    use physical_recipes::physical_stack_switch::{Destination, StackSwitch};
    pub use requirements::{
        InstalledPhysicalCpuRequirement, PhysicalEntryRequirements, PhysicalEntryStackRequirement,
        PhysicalIncomingStackRequirement,
    };

    #[cfg(test)]
    mod tests {
        use arena::Arena;
        use calling_conventions::{
            CallSignature, CallingPolicy, ValueShape, evaluate_ordinary_boundary_entry_plan,
        };
        use entry_planning::native_entry_bridge::{
            NativeEntryBridgeRequest, NativeEntryParameter, plan_native_entry_bridge,
        };

        #[test]
        fn retained_suffix_includes_alignment_after_nonempty_program_bss() {
            for existing in [0, 1, 15, 16, 17, 255, 4097] {
                let partitions =
                    super::PhysicalEntryPartitions::derive(existing, 16, 131_073, 131_072)
                        .expect("bounded partition geometry");
                assert_eq!(partitions.retained_start_offset, existing);
                assert!(partitions.retained_start_offset <= partitions.saved_state_offset);
                assert!(partitions.saved_state_offset + 24 <= partitions.stack_offset);
                assert_eq!(partitions.stack_byte_count, 131_088);
                assert_eq!(partitions.end_offset, partitions.stack_offset + 131_088);
                for aligned in [
                    partitions.saved_state_offset,
                    partitions.stack_offset,
                    partitions.end_offset,
                ] {
                    assert!(aligned.is_multiple_of(16));
                }
            }
            assert!(super::PhysicalEntryPartitions::derive(u64::MAX, 16, 128, 131_072).is_err());
            assert!(super::PhysicalEntryPartitions::derive(17, 3, 128, 131_072).is_err());
            assert!(super::PhysicalEntryPartitions::derive(17, 16, u64::MAX, 131_072).is_err());
        }

        #[test]
        fn private_stack_realizes_target_floor_and_larger_checked_demand() {
            let target =
                target::TargetSemantics::close_guaranteed_entry_stack::<target::UefiX86_64>(
                    target::TargetProfile::UefiX64,
                )
                .unwrap();
            let floor = target.guaranteed_available_bytes();
            for (demand, expected) in [
                (0, floor),
                (4096, floor),
                (floor, floor),
                (floor + 1, floor + 16),
            ] {
                let partitions =
                    super::PhysicalEntryPartitions::derive(17, 16, demand, floor).unwrap();
                assert_eq!(partitions.stack_byte_count, expected);
                assert_eq!(partitions.end_offset - partitions.stack_offset, expected);
                assert!(partitions.saved_state_offset + 24 <= partitions.stack_offset);
            }
            assert!(super::PhysicalEntryPartitions::derive(17, 16, 0, u64::MAX).is_err());
        }

        #[test]
        fn private_stack_prefix_copies_abi_stack_arguments_and_preserves_return() {
            use iced_x86::{Decoder, DecoderOptions, Mnemonic, Register};
            for parameter_count in [2, 6] {
                let signature = CallSignature {
                    parameters: vec![ValueShape::integer(8, 8); parameter_count],
                    result: Some(ValueShape::integer(8, 8)),
                };
                let plan =
                    evaluate_ordinary_boundary_entry_plan(CallingPolicy::MicrosoftX64, &signature)
                        .unwrap();
                let mut parameters = Arena::with_capacity(parameter_count);
                for ordinal in 0..parameter_count {
                    parameters.insert(NativeEntryParameter {
                        inbound_ordinal: ordinal as u32,
                        scalar_codec: 0,
                    });
                }
                let physical = plan_native_entry_bridge(NativeEntryBridgeRequest {
                    target: target::NativeTarget::uefi_x64(),
                    internal: plan.plan().call.clone(),
                    inbound: plan.plan().clone(),
                    body_footprint: Some(calling_conventions::StateEffectsEvidence::empty()),
                    parameters,
                })
                .unwrap();
                let switch = entry_planning::physical_stack_switch::plan(physical).unwrap();
                let (bridge, wrapper) =
                    native_encoding::physical_stack_switch::encode(switch).unwrap();
                crate::published_entry_bytes::verify_stack_switch_bytes(&wrapper, &wrapper.bytes)
                    .unwrap();
                let mut relocated = wrapper.bytes.clone();
                let field = wrapper.fixups.iter().next().unwrap().1.field;
                relocated[field..field + 4].copy_from_slice(&512_i32.to_le_bytes());
                crate::published_entry_bytes::verify_stack_switch_bytes(&wrapper, &relocated)
                    .unwrap();
                relocated[0] ^= 1;
                assert!(
                    crate::published_entry_bytes::verify_stack_switch_bytes(&wrapper, &relocated)
                        .unwrap_err()
                        .message
                        .contains("outside its declared relocation fields")
                );
                assert_eq!(wrapper.incoming.bytes_below_entry_rsp, 0);
                assert_eq!(wrapper.incoming.caller_alignment, 16);
                assert_eq!(wrapper.incoming.caller_rsp_bias, 8);
                assert_eq!(
                    wrapper.incoming.readable_bytes_from_entry_rsp,
                    if parameter_count == 6 { 56 } else { 8 },
                );
                let instructions = Decoder::new(64, &wrapper.bytes, DecoderOptions::NONE)
                    .into_iter()
                    .take_while(|instruction| instruction.mnemonic() != Mnemonic::Ret)
                    .collect::<Vec<_>>();
                assert!(
                    instructions
                        .iter()
                        .all(|instruction| !instruction.is_invalid())
                );
                let call = instructions
                    .iter()
                    .position(|instruction| instruction.mnemonic() == Mnemonic::Call)
                    .unwrap();
                assert_eq!(instructions[call - 2].op0_register(), Register::RAX);
                assert_eq!(instructions[call - 1].op0_register(), Register::R10);
                assert_eq!(instructions[call + 1].mnemonic(), Mnemonic::Mov);
                assert_eq!(instructions[call + 1].op0_register(), Register::RSP);
                assert_eq!(instructions.len(), call + 2);
                let bridge_offset = instructions[call].near_branch_target() as usize;
                assert_eq!(&wrapper.bytes[bridge_offset..], bridge.bytes());
                let copies = instructions
                    .iter()
                    .filter(|instruction| instruction.memory_base() == Register::R10)
                    .map(|instruction| instruction.memory_displacement64())
                    .collect::<Vec<_>>();
                assert_eq!(
                    copies,
                    if parameter_count == 6 {
                        vec![40, 48]
                    } else {
                        vec![]
                    }
                );
                assert!(
                    copies
                        .iter()
                        .all(|offset| offset + 8 <= wrapper.incoming.readable_bytes_from_entry_rsp)
                );
                let switch = instructions
                    .iter()
                    .position(|instruction| {
                        instruction.mnemonic() == Mnemonic::Lea
                            && instruction.op0_register() == Register::RSP
                    })
                    .unwrap();
                assert!(instructions[..switch].iter().all(|instruction| !matches!(
                    instruction.mnemonic(),
                    Mnemonic::Push | Mnemonic::Call
                )
                    && instruction.memory_base() != Register::RSP));
                assert_eq!(
                    wrapper.outgoing_frame_bytes,
                    if parameter_count == 6 { 48 } else { 32 }
                );
                assert_eq!(
                    wrapper.stack_overhead_bytes(),
                    u64::from(wrapper.outgoing_frame_bytes) + 8
                );
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct PhysicalEntryBinding {
        entry: MachineId,
        semantic_entry: MachineId,
        installed_reserve_bytes: u64,
        plan: BoundaryEntryPlan,
        bridge: NativeEntryBridge,
        body_demand: crate::StackDemand,
        arrival_stack: target::TargetEntryStackGuarantee,
        stack_switch: StackSwitch,
        requirements: PhysicalEntryRequirements,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct PhysicalEntryPartitions {
        pub retained_start_offset: u64,
        pub saved_state_offset: u64,
        pub stack_offset: u64,
        pub stack_byte_count: u64,
        pub end_offset: u64,
    }

    impl PhysicalEntryBinding {
        pub fn requirements(&self) -> &PhysicalEntryRequirements {
            &self.requirements
        }

        pub const fn entry(&self) -> MachineId {
            self.entry
        }

        pub const fn semantic_entry(&self) -> MachineId {
            self.semantic_entry
        }

        pub const fn installed_reserve_bytes(&self) -> u64 {
            self.installed_reserve_bytes
        }

        pub const fn plan(&self) -> &BoundaryEntryPlan {
            &self.plan
        }

        pub const fn arrival_stack(&self) -> &target::TargetEntryStackGuarantee {
            &self.arrival_stack
        }

        pub fn stack_overhead_bytes(&self) -> u64 {
            u64::from(self.bridge.frame_bytes())
                + u64::from(self.bridge.call_stack_bias())
                + self.stack_switch.stack_overhead_bytes()
        }

        pub fn partitions(
            &self,
            existing_bss_bytes: u64,
        ) -> Result<PhysicalEntryPartitions, Diagnostic> {
            requirements::partitions(
                existing_bss_bytes,
                &self.plan,
                &self.body_demand,
                &self.bridge,
                &self.stack_switch,
                &self.arrival_stack,
                self.installed_reserve_bytes,
            )
        }
    }

    impl PhysicalEntryPartitions {
        fn derive(
            existing_bss_bytes: u64,
            alignment: u64,
            stack_demand: u64,
            minimum_stack_bytes: u64,
        ) -> Result<Self, Diagnostic> {
            let saved_state_offset = align(existing_bss_bytes, alignment)?;
            let stack_offset = align(
                saved_state_offset
                    .checked_add(24)
                    .ok_or_else(|| invalid("saved entry state extent overflows"))?,
                alignment,
            )?;
            let stack_byte_count = align(stack_demand.max(minimum_stack_bytes), alignment)?;
            let end_offset = stack_offset
                .checked_add(stack_byte_count)
                .ok_or_else(|| invalid("private entry stack extent overflows"))?;
            Ok(Self {
                retained_start_offset: existing_bss_bytes,
                saved_state_offset,
                stack_offset,
                stack_byte_count,
                end_offset,
            })
        }
    }

    pub type EncodedPhysicalEntry = object_file::program_entry_object::PhysicalProgramObject<
        physical_recipes::physical_entry::PhysicalEntryContext<
            verified_optimization_input::AdmittedPhysicalEntry,
            program_entry_contracts::ProgramEntryPhysicalContractPlan,
        >,
    >;

    pub fn bind_encoded_physical_entry(
        artifact: &mut ObjectArtifact,
        encoded: EncodedPhysicalEntry,
    ) -> Result<(), Diagnostic> {
        let (plan, bridge, stack_switch) = encoded.into_parts();
        let selection = plan.selection.as_ref();
        if artifact.physical_entry.is_some() || artifact.hosted_entry.is_some() {
            return Err(invalid("object already has an arrival binding"));
        }
        if artifact.psi() != selection.psi
            || artifact.entry() != selection.adapter
            || artifact.target() != plan.target
            || artifact.profile != Some(target::TargetProfile::UefiX64)
        {
            return Err(invalid(
                "physical entry plan belongs to another body or target",
            ));
        }
        // Only storage placement remains: stack size, effects, calls and frame
        // geometry were fixed before encoding the executable program.
        let requirements = requirements::place(
            plan.requirements,
            original_bss(artifact)?.map_or(0, |(_, size, _)| size as u64),
        )?;
        artifact.physical_entry = Some(PhysicalEntryBinding {
            entry: selection.adapter,
            semantic_entry: selection.semantic_entry,
            installed_reserve_bytes: plan.installed_reserve_bytes,
            plan: plan.plan,
            bridge,
            body_demand: plan.body_demand,
            arrival_stack: plan.arrival_stack,
            stack_switch,
            requirements,
        });
        Ok(())
    }

    pub(crate) fn validate_binding(artifact: &ObjectArtifact) -> Result<(), Diagnostic> {
        let Some(binding) = &artifact.physical_entry else {
            return Ok(());
        };
        if binding.entry != artifact.entry || artifact.hosted_entry.is_some() {
            return Err(invalid("selected ordinary body or arrival kind changed"));
        }
        let ordinals = binding
            .bridge
            .request()
            .parameters
            .iter()
            .map(|(_, parameter)| parameter.inbound_ordinal)
            .collect::<Vec<_>>();
        let request = native_entry_request(artifact, &binding.plan, &ordinals)
            .map_err(|error| invalid(&error.to_string()))?;
        if request != *binding.bridge.request() {
            return Err(invalid(
                "physical plan differs from the retained body transport",
            ));
        }
        crate::published_entry_bytes::verify_entry_bridge_bytes(
            &binding.bridge,
            binding.bridge.bytes(),
        )
        .map_err(|error| invalid(&error.to_string()))?;
        let demand = derive_body_stack_demand(artifact, artifact.entry)
            .map_err(|error| invalid(&format!("ordinary body stack demand: {error}")))?;
        let installed_reserve = crate::object::object_artifact::installed_stack::root_reserve(
            artifact,
            installation_conditions::InstalledPhysicalStackRoot {
                semantic_entry: binding.semantic_entry,
                adapter: binding.entry,
            },
        )
        .map_err(|error| invalid(&error.to_string()))?;
        let expected_memory = requirements::derive(
            artifact,
            binding.semantic_entry,
            binding.requirements.contract(),
            &demand,
            &binding.bridge,
            &binding.stack_switch,
            installed_reserve,
        )?;
        if binding.requirements != expected_memory
            || binding.requirements.contract().boundary_entry_plan() != &binding.plan
            || binding.requirements.contract().guaranteed_entry_stack()
                != Some(&binding.arrival_stack)
            || installed_reserve != binding.installed_reserve_bytes
            || demand != binding.body_demand
        {
            return Err(invalid("private stack differs from its exact invocation"));
        }
        binding.stack_switch.validate_bridge(&binding.bridge)?;
        crate::published_entry_bytes::verify_stack_switch_bytes(
            &binding.stack_switch,
            &binding.stack_switch.bytes,
        )
    }

    pub(crate) fn prepare(artifact: &ObjectArtifact) -> Result<PreparedEntry, Diagnostic> {
        let binding = artifact
            .physical_entry
            .as_ref()
            .ok_or_else(|| invalid("physical entry binding is missing"))?;
        let (mut object, _partitions, saved_state, stack_top) = provision_stack(artifact, binding)?;
        let mut text = artifact.text_bytes.clone();
        let (symbol, offset) = crate::entries::hosted_unit_entry::install_entry_shim(
            &mut object,
            &mut text,
            &binding.stack_switch.bytes,
            "omega_physical_entry",
            &|| invalid("object has no text section"),
        )?;
        let mut relocations = artifact.relocations.clone();
        for (_, fixup) in binding.stack_switch.fixups.iter() {
            let (destination, addend) = match fixup.destination {
                Destination::SavedStack => (saved_state, 0),
                Destination::SavedAccumulator => (saved_state, 8),
                Destination::SavedScratch => (saved_state, 16),
                Destination::StackTop => (stack_top, 0),
                Destination::Body => (artifact.entry_function().symbol, 0),
            };
            relocations.push_record(object_file::RelocationRecord {
                origin: RelocationOrigin::Materialization {
                    object_symbol_handle: symbol,
                },
                section: SectionKind::Text,
                offset: offset
                    .checked_add(fixup.field)
                    .ok_or_else(|| invalid("physical call offset overflows"))?,
                byte_width: 4,
                symbol_handle: destination,
                addend,
                kind: RelocationKind::X86_64Relative32,
            });
        }
        Ok(PreparedEntry {
            object,
            text,
            relocations,
            shim: EntrySurface::Physical.shim(symbol, offset),
        })
    }

    pub(crate) fn generated_state_effects(
        artifact: &ObjectArtifact,
    ) -> Option<(
        &physical_recipes::physical_stack_switch::StackSwitch,
        &calling_conventions::StateEffectsEvidence,
    )> {
        let binding = artifact.physical_entry.as_ref()?;
        Some((
            &binding.stack_switch,
            &binding.stack_switch.complete_effects,
        ))
    }

    pub(crate) fn validate_image(
        artifact: &ObjectArtifact,
        object: &ObjectPlan,
        text: &[u8],
        relocations: &object_file::RelocationPlan,
        symbol: ObjectSymbolHandle,
        offset: usize,
        output: &image::EmittedImageOutput,
    ) -> Result<(), Diagnostic> {
        let binding = artifact
            .physical_entry
            .as_ref()
            .ok_or_else(|| invalid("image has no retained physical binding"))?;
        let end = offset
            .checked_add(binding.stack_switch.bytes.len())
            .ok_or_else(|| invalid("physical entry extent overflows"))?;
        if binding.entry != artifact.entry
            || object.layout.entry_symbol != symbol
            || offset != artifact.text_bytes.len()
            || text.get(offset..end) != Some(binding.stack_switch.bytes.as_slice())
            || !crate::entries::hosted_unit_entry::unique_region(
                object,
                symbol,
                offset,
                binding.stack_switch.bytes.len(),
                output,
            )
        {
            return Err(invalid("physical entry lost its exact executable region"));
        }
        let prepared = prepare(artifact)?;
        if prepared.object != *object
            || prepared.text != text
            || prepared.relocations != *relocations
        {
            return Err(invalid("physical stack residences or relocations changed"));
        }
        let original_bss = original_bss(artifact)?.map_or(0, |(_, size, _)| size as u64);
        let partitions = binding.partitions(original_bss)?;
        let bss = output.final_image_layout.bss_address;
        let alignment = u64::from(binding.plan.call.stack_alignment);
        if partitions.end_offset != output.bss_bytes as u64
            || [
                partitions.saved_state_offset,
                partitions.stack_offset,
                partitions.end_offset,
            ]
            .iter()
            .any(|offset| {
                bss.checked_add(*offset)
                    .is_none_or(|address| !address.is_multiple_of(alignment))
            })
        {
            return Err(invalid(
                "private stack is not the exact aligned mapped BSS residence",
            ));
        }
        if artifact.retained_entry_storage.is_valid() {
            let marker = object.layout.symbols.get(artifact.retained_entry_storage);
            if marker.section != SymbolSection::Section(SectionKind::Bss)
                || marker.offset as u64 != partitions.retained_start_offset
                || marker.size != 0
            {
                return Err(invalid(
                    "retained entry boundary does not exclude private alignment",
                ));
            }
            let destination = bss
                .checked_add(partitions.retained_start_offset)
                .ok_or_else(|| invalid("retained entry boundary overflows"))?;
            for (_, relocation) in relocations
                .records()
                .filter(|(_, row)| row.symbol_handle == artifact.retained_entry_storage)
            {
                let field_end = relocation
                    .offset
                    .checked_add(4)
                    .ok_or_else(|| invalid("image observation field overflows"))?;
                let bytes: [u8; 4] = output
                    .final_text_bytes
                    .get(relocation.offset..field_end)
                    .and_then(|bytes| bytes.try_into().ok())
                    .ok_or_else(|| invalid("image observation field is truncated"))?;
                let origin = output
                    .final_image_layout
                    .text_address
                    .checked_add(field_end as u64)
                    .ok_or_else(|| invalid("image observation origin overflows"))?;
                if relocation.kind != RelocationKind::X86_64Relative32
                    || relocation.section != SectionKind::Text
                    || relocation.addend != 0
                    || relocation.byte_width != 4
                    || i128::from(origin) + i128::from(i32::from_le_bytes(bytes))
                        != i128::from(destination)
                {
                    return Err(invalid(
                        "image observation does not resolve to the retained suffix",
                    ));
                }
            }
        }
        let mut expected = binding.stack_switch.bytes.clone();
        let text_address = output.final_image_layout.text_address;
        for (_, fixup) in binding.stack_switch.fixups.iter() {
            let (base, destination_offset) = match fixup.destination {
                Destination::SavedStack => (bss, partitions.saved_state_offset),
                Destination::SavedAccumulator => (bss, partitions.saved_state_offset + 8),
                Destination::SavedScratch => (bss, partitions.saved_state_offset + 16),
                Destination::StackTop => (bss, partitions.end_offset),
                Destination::Body => (text_address, artifact.entry_function().text_offset as u64),
            };
            let destination = base
                .checked_add(destination_offset)
                .ok_or_else(|| invalid("physical entry relocation destination overflows"))?;
            let origin = text_address
                .checked_add(offset as u64)
                .and_then(|address| address.checked_add(fixup.field as u64 + 4))
                .ok_or_else(|| invalid("physical entry relocation origin overflows"))?;
            let distance = i32::try_from(i128::from(destination) - i128::from(origin))
                .map_err(|_| invalid("physical entry residence is outside rel32 range"))?;
            expected[fixup.field..fixup.field + 4].copy_from_slice(&distance.to_le_bytes());
        }
        if output.final_text_bytes.get(offset..end) != Some(expected.as_slice())
            || !crate::entries::hosted_unit_entry::pe_entry_points_to(
                &output.bytes,
                offset,
                &expected,
            )
        {
            return Err(invalid(
                "PE entry or physical call differs from the selected body",
            ));
        }
        Ok(())
    }

    fn invalid(reason: &str) -> Diagnostic {
        Diagnostic::error(format!("physical image entry: {reason}"))
    }

    fn align(bytes: u64, alignment: u64) -> Result<u64, Diagnostic> {
        if !alignment.is_power_of_two() {
            return Err(invalid("private stack alignment is not a power of two"));
        }
        bytes
            .checked_add(alignment - 1)
            .map(|bytes| bytes & !(alignment - 1))
            .ok_or_else(|| invalid("private stack alignment overflows"))
    }

    fn original_bss(
        artifact: &ObjectArtifact,
    ) -> Result<Option<(arena::Handle<SectionPlan>, usize, usize)>, Diagnostic> {
        let mut sections = artifact
            .object
            .layout
            .sections
            .iter()
            .filter(|(_, section)| section.kind == SectionKind::Bss);
        let existing = sections
            .next()
            .map(|(handle, section)| (handle, section.size, section.alignment));
        if sections.next().is_some() {
            return Err(invalid("private stack requires a unique BSS section"));
        }
        Ok(existing)
    }

    fn provision_stack(
        artifact: &ObjectArtifact,
        binding: &PhysicalEntryBinding,
    ) -> Result<
        (
            ObjectPlan,
            PhysicalEntryPartitions,
            ObjectSymbolHandle,
            ObjectSymbolHandle,
        ),
        Diagnostic,
    > {
        let existing = original_bss(artifact)?;
        let partitions = binding.partitions(existing.map_or(0, |(_, size, _)| size as u64))?;
        let size = usize::try_from(partitions.end_offset)
            .map_err(|_| invalid("private stack exceeds host object coordinates"))?;
        let alignment = usize::from(binding.plan.call.stack_alignment);
        let mut object = artifact.object.clone();
        if let Some((handle, _, previous_alignment)) = existing {
            let section = object.layout.sections.get_mut(handle);
            section.size = size;
            section.alignment = previous_alignment.max(alignment);
        } else {
            object.layout.sections.insert(SectionPlan {
                kind: SectionKind::Bss,
                size,
                alignment,
            });
        }
        if artifact.retained_entry_storage.is_valid() {
            let marker = object
                .layout
                .symbols
                .get_mut(artifact.retained_entry_storage);
            marker.section = SymbolSection::Section(SectionKind::Bss);
            marker.offset = usize::try_from(partitions.retained_start_offset)
                .map_err(|_| invalid("retained entry boundary exceeds host coordinates"))?;
        }
        let mut symbol_at = |name: &str, offset: u64, size| -> Result<_, Diagnostic> {
            Ok(object.layout.symbols.insert(SymbolPlan {
                name: name.into(),
                section: SymbolSection::Section(SectionKind::Bss),
                offset: usize::try_from(offset)
                    .map_err(|_| invalid("entry storage offset exceeds host coordinates"))?,
                size,
                kind: SymbolKind::Object,
                import_library: String::new(),
            }))
        };
        let saved = symbol_at(
            "omega_physical_saved_state",
            partitions.saved_state_offset,
            24,
        )?;
        let stack_top = symbol_at("omega_physical_stack_top", partitions.end_offset, 0)?;
        Ok((object, partitions, saved, stack_top))
    }
}

pub(crate) mod private_entry_origin {
    use crate::{
        FunctionFragmentObjectArtifactError as Error, ObjectArtifact, ObjectCompilerPrivateFunction,
    };
    use function_identity::MachineFunctionIdentity;
    use provider_admission::AdmittedCallbackEntry;
    use runtime_abi::static_interrupts::{InstalledInterruptEntryPlan, InstalledInterruptFrame};

    type EntryOrigin<'a> = (
        MachineFunctionIdentity,
        &'a AdmittedCallbackEntry,
        bool,
        Option<InstalledInterruptEntryPlan>,
        Option<&'a installation_conditions::BootstrapEnvironmentAssumption>,
        Option<&'a provider_admission::AdmittedInstalledInterruptScope>,
        OriginOwners<'a>,
    );

    #[derive(Clone, Copy)]
    struct OriginOwners<'a> {
        entry: &'a ObjectArtifact,
        plan: Option<&'a ObjectArtifact>,
        assumption: Option<&'a ObjectArtifact>,
        scope: Option<&'a ObjectArtifact>,
    }

    pub(crate) struct EntryOrigins<'a> {
        entries: Vec<EntryOrigin<'a>>,
    }
    impl<'a> EntryOrigins<'a> {
        pub(crate) fn new(artifact: &'a ObjectArtifact) -> Result<Self, Error> {
            if artifact.installed_entry_execution().is_some() {
                return Err(Error::Mismatch(
                    "an installed private execution domain cannot be an image root",
                ));
            }
            let mut entries = Vec::new();
            let source = artifact.composition_source.as_deref().unwrap_or(artifact);
            collect(source, &mut entries)?;
            for private in &artifact.private_functions {
                collect(&private.body, &mut entries)?;
            }
            entries.sort_unstable_by_key(|row| row.0.to_bytes());
            let mut merged: Vec<EntryOrigin<'a>> = Vec::with_capacity(entries.len());
            for row in entries {
                if let Some(prior) = merged.last_mut().filter(|prior| prior.0 == row.0) {
                    if prior.1 != row.1
                        || prior.2 != row.2
                        || matches!((prior.3, row.3), (Some(left), Some(right)) if left != right)
                        || matches!((prior.4, row.4), (Some(left), Some(right)) if left != right)
                        || matches!((prior.5, row.5), (Some(left), Some(right)) if left != right)
                    {
                        return Err(Error::PrivateEntryRecipeConflict {
                            identity: row.0,
                            detail: conflicting_recipes(prior, &row),
                        });
                    }
                    if prior.5.is_none() {
                        prior.5 = row.5;
                        prior.6.scope = row.6.scope;
                    }
                    if prior.4.is_none() {
                        prior.4 = row.4;
                        prior.6.assumption = row.6.assumption;
                    }
                    if prior.3.is_none() {
                        prior.3 = row.3;
                        prior.6.plan = row.6.plan;
                    }
                } else {
                    merged.push(row);
                }
            }
            let entries = merged;
            Ok(Self { entries })
        }
        pub(crate) fn verify(&self, private: &ObjectCompilerPrivateFunction) -> Result<(), Error> {
            let index = self
                .entries
                .binary_search_by_key(&private.identity.to_bytes(), |row| row.0.to_bytes())
                .map_err(|_| {
                    Error::Mismatch("private entry has no original admitted callback recipe")
                })?;
            let (_, entry, installed, plan, assumption, parent_scope, _) = self.entries[index];
            let execution = private.body.installed_entry_execution();
            if installed != execution.is_some()
                || execution.is_some_and(|execution| {
                    execution.entry().callback_function != private.identity
                        || execution.entry().entry() != entry
                        || Some(execution.scope()) != parent_scope
                        || execution.scope().bootstrap_assumption() != assumption
                        || execution.scope().operation() != execution.entry().operation
                })
            {
                return Err(Error::Mismatch(
                    "private entry changed its retained privileged execution domain",
                ));
            }
            if installed != assumption.is_some() {
                return Err(Error::Mismatch(
                    "installed entry lost its parent bootstrap assumption",
                ));
            }
            if private.identity.caller() != entry.psi()
                || private.source_psi != entry.psi()
                || private.body.psi() != entry.psi()
                || private.body.target() != entry.target()
                || private.body.entry() != entry.machine()
                || private.function.machine != entry.machine()
                || private.inbound != *entry.inbound()
                || private.inbound_ordinals != entry.inbound_ordinals()
                || private
                    .entry_bridge
                    .as_ref()
                    .and_then(|bridge| bridge.installed_interrupt())
                    != plan
                || installed != plan.is_some()
            {
                return Err(Error::Mismatch(
                    "private entry metadata differs from its original admitted callback recipe",
                ));
            }
            Ok(())
        }
    }
    fn collect<'a>(
        body: &'a ObjectArtifact,
        entries: &mut Vec<EntryOrigin<'a>>,
    ) -> Result<(), Error> {
        let facts = body.emission_input.installation_authority();
        let ordinary = facts.native_callbacks();
        let installed = facts.installed_interrupts();
        if ordinary.psi() != body.psi() || installed.psi() != body.psi() {
            return Err(Error::Mismatch(
                "private entry cohort belongs to another program",
            ));
        }
        for row in ordinary.callbacks() {
            entries.push((
                row.callback_function,
                row.entry(),
                false,
                None,
                None,
                None,
                OriginOwners {
                    entry: body,
                    plan: None,
                    assumption: None,
                    scope: None,
                },
            ));
        }
        for row in installed.entries() {
            let mut scopes = installed.scopes().iter().filter(|scope| {
                scope.machine() == row.machine && scope.operation() == row.operation
            });
            let scope = scopes.next();
            if scopes.next().is_some() {
                return Err(Error::Mismatch(
                    "private entry has duplicate installation scopes",
                ));
            }
            let scope = if let Some(execution) = body.installed_entry_execution()
                && execution.entry().callback_function == row.callback_function
            {
                if execution.entry() != row
                    || execution.scope().operation() != row.operation
                    || row.entry().psi() != body.psi()
                    || row.entry().target() != body.target()
                    || row.entry().machine() != body.entry()
                    || scope.is_none()
                {
                    return Err(Error::Mismatch(
                        "private handler changed its original installation or admitted entry",
                    ));
                }
                execution
                    .require_current(installed)
                    .map_err(Error::Mismatch)?;
                Some(execution.scope())
            } else {
                scope
            };
            let assumption = scope.and_then(|scope| scope.bootstrap_assumption());
            let plan = if let Some(scope) = scope.filter(|scope| scope.context_anchor().is_some()) {
                let assumption = assumption.ok_or(Error::Mismatch(
                    "installation lost its explicit bootstrap environment assumption",
                ))?;
                if assumption.psi != body.psi()
                    || assumption.installation != scope.operation()
                    || assumption.profile.native_target() != body.target()
                {
                    return Err(Error::Mismatch(
                        "installation bootstrap assumption changed program, target or scope",
                    ));
                }
                assumption.validate_description().map_err(Error::Mismatch)?;
                for condition in scope.installation_assumptions() {
                    condition.validate_description().map_err(Error::Mismatch)?;
                    if condition.psi() != body.psi()
                        || condition.installation() != scope.operation()
                        || condition.profile().native_target() != body.target()
                        || condition.entry() != assumption.entry
                    {
                        return Err(Error::Mismatch(
                            "installation delivery condition changed its exact parent interval",
                        ));
                    }
                }
                if !scope.fatal_entries().is_empty()
                    && scope.interrupt_delivery_assumption().is_none()
                {
                    return Err(Error::Mismatch(
                        "fatal private entry lost its installed delivery condition",
                    ));
                }
                let anchor = scope.context_anchor().ok_or(Error::Mismatch(
                    "installation lost its retained context anchor",
                ))?;
                if anchor.application() != scope.acknowledgement_application()
                    || anchor.function().caller() != body.psi()
                {
                    return Err(Error::Mismatch(
                        "private entry context anchor changed its program or designated callback",
                    ));
                }
                if row.placement.application_index == anchor.application() {
                    if row.callback_function != anchor.function() {
                        return Err(Error::Mismatch(
                            "timer entry changed its anchor provider identity",
                        ));
                    }
                    Some(InstalledInterruptEntryPlan::new(
                        InstalledInterruptFrame::NoErrorCode,
                    ))
                } else {
                    let mut matching = scope.fatal_entries().iter().filter(|fatal| {
                        fatal.application() == row.placement.application_index
                            && fatal.machine() == row.entry().machine()
                    });
                    let (Some(fatal), None) = (matching.next(), matching.next()) else {
                        return Err(Error::Mismatch(
                            "private entry is absent from the admitted fatal roster",
                        ));
                    };
                    Some(
                        InstalledInterruptEntryPlan::fatal_context(
                            anchor.gate_byte_offset(),
                            fatal.gate_byte_offset(),
                            anchor.table_bytes(),
                        )
                        .ok_or(Error::Mismatch(
                            "fatal entry changed its admitted gate geometry",
                        ))?,
                    )
                }
            } else {
                None
            };
            entries.push((
                row.callback_function,
                row.entry(),
                true,
                plan,
                assumption,
                scope.filter(|scope| scope.context_anchor().is_some()),
                OriginOwners {
                    entry: body,
                    plan: plan.map(|_| body),
                    assumption: assumption.map(|_| body),
                    scope: scope
                        .filter(|scope| scope.context_anchor().is_some())
                        .map(|_| body),
                },
            ));
        }
        Ok(())
    }

    fn conflicting_recipes(left: &EntryOrigin<'_>, right: &EntryOrigin<'_>) -> String {
        let mut differences = Vec::new();
        if left.1 != right.1 {
            differences.push(format!(
                "callback entry: left={:?}; right={:?}",
                left.1, right.1
            ));
        }
        if left.2 != right.2 {
            differences.push(format!(
                "installed role: left={}; right={}",
                left.2, right.2
            ));
        }
        if matches!((left.3, right.3), (Some(a), Some(b)) if a != b) {
            differences.push(format!(
                "installed plan: left={:?}; right={:?}",
                left.3, right.3
            ));
        }
        if matches!((left.4, right.4), (Some(a), Some(b)) if a != b) {
            differences.push(format!(
                "bootstrap assumption: left={:?}; right={:?}",
                left.4, right.4
            ));
        }
        if matches!((left.5, right.5), (Some(a), Some(b)) if a != b) {
            differences.push(format!(
                "parent scope: left={:?}; right={:?}",
                left.5, right.5
            ));
        }
        let owner = |body: &ObjectArtifact| {
            format!(
                "root={:?}, psi={:?}, target={:?}, profile={:?}, installed_execution={:?}",
                body.entry(),
                body.psi(),
                body.target(),
                body.profile(),
                body.installed_entry_execution().map(|execution| (
                    execution.entry().callback_function,
                    execution.scope().operation(),
                )),
            )
        };
        differences.push(format!(
            "merged optional owners: left plan={:?}, assumption={:?}, scope={:?}; right plan={:?}, assumption={:?}, scope={:?}",
            left.6.plan.map(owner), left.6.assumption.map(owner), left.6.scope.map(owner),
            right.6.plan.map(owner), right.6.assumption.map(owner), right.6.scope.map(owner),
        ));
        format!(
            "left source [{}]; right source [{}]; {}",
            owner(left.6.entry),
            owner(right.6.entry),
            differences.join("; "),
        )
    }
}

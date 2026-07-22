use omega_abstract_operations::SelectedInstructionKind;
use omega_calling_conventions::{
    BoundaryEntryPlan, CallSignature, EntryControl, IndirectPointerLocation, MachineStateSet,
    PlanDiagnostic, RegisterSet, StateFootprintEvidence, ValidatedBoundaryEntryPlan, ValueLocation,
    ValueShape, validate_boundary_entry_plan, validate_state_footprint,
};

/// The observable exit half of one validated boundary plan. Result fragments
/// remain ordered exactly as canonical validation produced them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DerivedBoundaryExit {
    pub control: EntryControl,
    pub result_locations: Vec<ValueLocation>,
}

/// Target-specific inbound storage writes together with the exact registers
/// those generated fragments overwrite. This is a checkable fragment of the
/// eventual whole-artifact footprint certificate; it intentionally does not
/// claim to cover the handler body, veneers, thunks, or exit lowering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DerivedBoundaryEntryStorage {
    pub writes: Vec<SelectedInstructionKind>,
    pub footprint: StateFootprintEvidence,
}

/// Derive and validate the fixed scratch footprint of the special
/// `run(args: &[u8])` descriptor write. The ISA modules that emit the bytes own
/// the scratch identities; this layer only turns them into boundary evidence
/// and checks the retained state ceiling.
pub fn derive_boundary_entry_slice_descriptor_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    let registers = match boundary.plan().call.policy.architecture() {
        omega_target::Architecture::X86_64 => {
            omega_isa_x86_64::entry_arguments_slice_descriptor_write_clobbers()
        }
        omega_target::Architecture::Aarch64 => {
            omega_isa_aarch64::entry_arguments_slice_descriptor_write_clobbers()
        }
    };
    let evidence = StateFootprintEvidence::new(registers, MachineStateSet::empty());
    validate_state_footprint(boundary, &evidence)?;
    Ok(evidence)
}

/// Derive the exact register footprint of selected direct-result
/// materialization instructions and validate it under the complete entry
/// plan's state ceiling. Indirect result memory copies and the final return
/// sequence are intentionally separate fragments.
pub fn derive_boundary_exit_result_register_footprint<'instruction>(
    boundary: &ValidatedBoundaryEntryPlan,
    instructions: impl IntoIterator<Item = &'instruction SelectedInstructionKind>,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    let architecture = boundary.plan().call.policy.architecture();
    let mut registers = Vec::new();
    for instruction in instructions {
        let clobbers = match instruction {
            SelectedInstructionKind::WriteReturnRegisterInteger { register, .. } => {
                match architecture {
                    omega_target::Architecture::X86_64 => {
                        omega_isa_x86_64::return_register_integer_write_clobbers(*register)
                    }
                    omega_target::Architecture::Aarch64 => {
                        omega_isa_aarch64::return_register_integer_write_clobbers(*register)
                    }
                }
            }
            SelectedInstructionKind::CopyRuntimeStorageToReturnRegister {
                register,
                byte_offset,
                byte_size,
                ..
            } => match architecture {
                omega_target::Architecture::X86_64 => {
                    omega_isa_x86_64::runtime_storage_copy_to_return_register_clobbers(*register)
                }
                omega_target::Architecture::Aarch64 => {
                    omega_isa_aarch64::runtime_storage_copy_to_return_register_clobbers(
                        *register,
                        *byte_offset,
                        *byte_size,
                    )
                }
            },
            _ => continue,
        };
        registers.extend_from_slice(clobbers.as_slice());
    }
    let evidence =
        StateFootprintEvidence::new(RegisterSet::new(registers), MachineStateSet::empty());
    validate_state_footprint(boundary, &evidence)?;
    Ok(evidence)
}

/// Derive the exact scratch footprint of selected copies into an indirect
/// result destination captured in `pointer_byte_offset`. Structural matching
/// keeps ordinary body `CopyPlaces` operations outside this boundary fragment.
pub fn derive_boundary_exit_indirect_result_copy_footprint<'instruction>(
    boundary: &ValidatedBoundaryEntryPlan,
    pointer_byte_offset: usize,
    instructions: impl IntoIterator<Item = &'instruction SelectedInstructionKind>,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    let expected_byte_size = match boundary
        .plan()
        .call
        .result
        .as_ref()
        .map(|result| result.locations.as_slice())
    {
        Some([ValueLocation::Indirect { byte_size, .. }]) => usize::from(*byte_size),
        _ => 0,
    };
    let architecture = boundary.plan().call.policy.architecture();
    let mut registers = Vec::new();
    for instruction in instructions {
        let SelectedInstructionKind::CopyPlaces {
            source,
            target,
            byte_count,
        } = instruction
        else {
            continue;
        };
        let crate::CopyPlacesShape::ToPointee {
            source_offset,
            pointer_byte_offset: actual_pointer_byte_offset,
            field_byte_offset,
        } = crate::classify_copy_places_shape(source, target)
        else {
            continue;
        };
        if expected_byte_size == 0
            || *byte_count != expected_byte_size
            || actual_pointer_byte_offset != pointer_byte_offset
            || field_byte_offset != 0
        {
            continue;
        }
        let clobbers = match architecture {
            omega_target::Architecture::X86_64 => {
                omega_isa_x86_64::copy_places_to_pointee_clobbers(*byte_count)
            }
            omega_target::Architecture::Aarch64 => {
                omega_isa_aarch64::runtime_storage_copy_to_runtime_pointee_clobbers(
                    source_offset,
                    actual_pointer_byte_offset,
                    field_byte_offset,
                    *byte_count,
                )
            }
        };
        registers.extend_from_slice(clobbers.as_slice());
    }
    let evidence =
        StateFootprintEvidence::new(RegisterSet::new(registers), MachineStateSet::empty());
    validate_state_footprint(boundary, &evidence)?;
    Ok(evidence)
}

/// Derive result placement and exit control for a compiler-owned entry stub.
/// This consumes the complete plan so result lowering cannot accidentally
/// accept placements from a carrier whose state obligations are invalid.
pub fn derive_boundary_exit(
    boundary: &BoundaryEntryPlan,
    parameters: &[ValueShape],
    result: Option<ValueShape>,
) -> Result<DerivedBoundaryExit, PlanDiagnostic> {
    let boundary = validate_boundary_entry_plan(
        boundary.clone(),
        &CallSignature {
            parameters: parameters.to_vec(),
            result,
        },
    )?;
    Ok(DerivedBoundaryExit {
        control: boundary.plan().call.entry_control,
        result_locations: boundary
            .plan()
            .call
            .result
            .as_ref()
            .map(|placement| placement.locations.clone())
            .unwrap_or_default(),
    })
}

/// Derive the inbound argument-unmarshal half of a compiler-owned entry stub
/// from one already-evaluated boundary plan. `parameter_destinations` names
/// runtime-frame storage in signature order; an indirect result additionally
/// reserves one pointer-sized frame slot so terminal lowering can write back
/// through the caller's destination.
///
/// The complete boundary plan is revalidated here, not merely its placements.
/// Save/restore and state-ceiling lowering may therefore build on this same
/// seam without accepting a call-valid but state-invalid carrier.
pub fn derive_boundary_entry_storage_writes(
    boundary: &BoundaryEntryPlan,
    parameter_destinations: &[(usize, ValueShape)],
    result: Option<ValueShape>,
    indirect_result_pointer_byte_offset: Option<usize>,
) -> Result<Vec<SelectedInstructionKind>, PlanDiagnostic> {
    Ok(derive_boundary_entry_storage(
        boundary,
        parameter_destinations,
        result,
        indirect_result_pointer_byte_offset,
    )?
    .writes)
}

/// Derive and state-check the inbound storage fragment of a compiler-owned
/// entry stub. Scratch clobbers come from the same ISA modules as the concrete
/// encoders, and a selected input register may not overlap scratch destroyed
/// before that input is captured.
pub fn derive_boundary_entry_storage(
    boundary: &BoundaryEntryPlan,
    parameter_destinations: &[(usize, ValueShape)],
    result: Option<ValueShape>,
    indirect_result_pointer_byte_offset: Option<usize>,
) -> Result<DerivedBoundaryEntryStorage, PlanDiagnostic> {
    let signature = CallSignature {
        parameters: parameter_destinations
            .iter()
            .map(|(_, shape)| *shape)
            .collect(),
        result,
    };
    let boundary = validate_boundary_entry_plan(boundary.clone(), &signature)?;
    let call = &boundary.plan().call;
    let mut writes = Vec::new();

    if let Some(result) = &call.result {
        let indirect = match result.locations.as_slice() {
            [ValueLocation::Indirect { pointer, .. }] => Some(*pointer),
            _ => None,
        };
        match (indirect, indirect_result_pointer_byte_offset) {
            (Some(pointer), Some(byte_offset)) => {
                writes.push(pointer_storage_write(pointer, byte_offset))
            }
            (Some(_), None) => {
                return Err(PlanDiagnostic(
                    "indirect boundary result needs a destination-pointer storage slot".into(),
                ));
            }
            (None, Some(_)) => {
                return Err(PlanDiagnostic(
                    "direct boundary result must not reserve an indirect destination-pointer slot"
                        .into(),
                ));
            }
            (None, None) => {}
        }
    } else if indirect_result_pointer_byte_offset.is_some() {
        return Err(PlanDiagnostic(
            "void boundary result must not reserve an indirect destination-pointer slot".into(),
        ));
    }

    for ((destination_offset, _), placement) in parameter_destinations.iter().zip(&call.parameters)
    {
        for location in &placement.locations {
            writes.push(match *location {
                ValueLocation::Register {
                    register,
                    value_byte_offset,
                    byte_size,
                } => SelectedInstructionKind::WriteEntryArgumentRegister {
                    register,
                    byte_offset: *destination_offset + usize::from(value_byte_offset),
                    byte_size: usize::from(byte_size),
                },
                ValueLocation::Stack {
                    stack_byte_offset,
                    value_byte_offset,
                    byte_size,
                    ..
                } => SelectedInstructionKind::WriteEntryStackArgument {
                    stack_byte_offset,
                    byte_offset: *destination_offset + usize::from(value_byte_offset),
                    byte_size: usize::from(byte_size),
                },
                ValueLocation::Indirect {
                    pointer, byte_size, ..
                } => SelectedInstructionKind::WriteEntryIndirectArgument {
                    pointer,
                    byte_offset: *destination_offset,
                    byte_size: usize::from(byte_size),
                },
            });
        }
    }

    let mut prior_clobbers = Vec::new();
    for write in &writes {
        let clobbers =
            entry_storage_write_clobbers(boundary.plan().call.policy.architecture(), write)?;
        if let Some(source) = entry_storage_write_register_source(write)
            && (clobbers.contains(source) || prior_clobbers.contains(&source))
        {
            return Err(PlanDiagnostic(format!(
                "entry storage lowering would clobber selected input register {source:?} before capturing it"
            )));
        }
        prior_clobbers.extend_from_slice(clobbers.as_slice());
    }
    let footprint =
        StateFootprintEvidence::new(RegisterSet::new(prior_clobbers), MachineStateSet::empty());
    validate_state_footprint(&boundary, &footprint)?;

    Ok(DerivedBoundaryEntryStorage { writes, footprint })
}

fn entry_storage_write_register_source(
    write: &SelectedInstructionKind,
) -> Option<omega_calling_conventions::MachineRegister> {
    match write {
        SelectedInstructionKind::WriteEntryArgumentRegister { register, .. } => Some(*register),
        SelectedInstructionKind::WriteEntryIndirectArgument {
            pointer: IndirectPointerLocation::Register(register),
            ..
        } => Some(*register),
        _ => None,
    }
}

fn entry_storage_write_clobbers(
    architecture: omega_target::Architecture,
    write: &SelectedInstructionKind,
) -> Result<RegisterSet, PlanDiagnostic> {
    Ok(match (architecture, write) {
        (
            omega_target::Architecture::X86_64,
            SelectedInstructionKind::WriteEntryArgumentRegister { .. },
        ) => omega_isa_x86_64::entry_argument_register_write_clobbers(),
        (
            omega_target::Architecture::X86_64,
            SelectedInstructionKind::WriteEntryStackArgument { .. },
        ) => omega_isa_x86_64::entry_stack_argument_write_clobbers(),
        (
            omega_target::Architecture::X86_64,
            SelectedInstructionKind::WriteEntryIndirectArgument { .. },
        ) => omega_isa_x86_64::entry_indirect_argument_write_clobbers(),
        (
            omega_target::Architecture::Aarch64,
            SelectedInstructionKind::WriteEntryArgumentRegister { .. },
        ) => omega_isa_aarch64::entry_argument_register_write_clobbers(),
        (
            omega_target::Architecture::Aarch64,
            SelectedInstructionKind::WriteEntryStackArgument { .. },
        ) => omega_isa_aarch64::entry_stack_argument_write_clobbers(),
        (
            omega_target::Architecture::Aarch64,
            SelectedInstructionKind::WriteEntryIndirectArgument { pointer, .. },
        ) => omega_isa_aarch64::entry_indirect_argument_write_clobbers(*pointer),
        _ => {
            return Err(PlanDiagnostic(
                "entry storage derivation produced an instruction without target footprint evidence"
                    .into(),
            ));
        }
    })
}

fn pointer_storage_write(
    pointer: IndirectPointerLocation,
    byte_offset: usize,
) -> SelectedInstructionKind {
    match pointer {
        IndirectPointerLocation::Register(register) => {
            SelectedInstructionKind::WriteEntryArgumentRegister {
                register,
                byte_offset,
                byte_size: 8,
            }
        }
        IndirectPointerLocation::Stack {
            stack_byte_offset, ..
        } => SelectedInstructionKind::WriteEntryStackArgument {
            stack_byte_offset,
            byte_offset,
            byte_size: 8,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_calling_conventions::{
        CallingPolicy, MachineRegime, MachineRegister, MachineState, ValueShape,
        evaluate_ordinary_boundary_entry_plan,
    };

    #[test]
    fn inbound_writes_consume_the_exact_selected_register() {
        let signature = CallSignature {
            parameters: vec![ValueShape::integer(8, 8)],
            result: None,
        };
        let mut boundary =
            evaluate_ordinary_boundary_entry_plan(CallingPolicy::SystemVAMD64, &signature)
                .expect("SysV boundary")
                .plan()
                .clone();
        let ValueLocation::Register { register, .. } =
            &mut boundary.call.parameters[0].locations[0]
        else {
            panic!("register parameter");
        };
        *register = MachineRegister::X86R10;

        let writes = derive_boundary_entry_storage_writes(
            &boundary,
            &[(24, ValueShape::integer(8, 8))],
            None,
            None,
        )
        .expect("selected inbound writes");

        assert_eq!(
            writes,
            vec![SelectedInstructionKind::WriteEntryArgumentRegister {
                register: MachineRegister::X86R10,
                byte_offset: 24,
                byte_size: 8,
            }]
        );
    }

    #[test]
    fn inbound_writes_capture_an_indirect_result_pointer() {
        let result = ValueShape::integer(24, 8);
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters: Vec::new(),
                result: Some(result),
            },
        )
        .expect("SysV memory result");

        let writes =
            derive_boundary_entry_storage_writes(boundary.plan(), &[], Some(result), Some(96))
                .expect("hidden result pointer write");

        assert_eq!(
            writes,
            vec![SelectedInstructionKind::WriteEntryArgumentRegister {
                register: MachineRegister::X86Rdi,
                byte_offset: 96,
                byte_size: 8,
            }]
        );
    }

    #[test]
    fn inbound_writes_reject_a_state_invalid_plan() {
        let signature = CallSignature {
            parameters: vec![ValueShape::integer(8, 8)],
            result: None,
        };
        let mut boundary =
            evaluate_ordinary_boundary_entry_plan(CallingPolicy::SystemVAMD64, &signature)
                .expect("SysV boundary")
                .plan()
                .clone();
        boundary.state.initial_regime = MachineRegime::Aarch64A64 { exception_level: 0 };

        let error = derive_boundary_entry_storage_writes(
            &boundary,
            &[(0, ValueShape::integer(8, 8))],
            None,
            None,
        )
        .expect_err("architecture-mismatched state must fail closed");

        assert!(error.0.contains("different architectures"));
    }

    #[test]
    fn inbound_storage_carries_exact_x86_fragment_clobbers() {
        let parameters = vec![ValueShape::integer(8, 8); 7];
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters: parameters.clone(),
                result: None,
            },
        )
        .expect("SysV boundary with one stack argument");
        let destinations = parameters
            .into_iter()
            .enumerate()
            .map(|(index, shape)| (index * 8, shape))
            .collect::<Vec<_>>();

        let derived = derive_boundary_entry_storage(boundary.plan(), &destinations, None, None)
            .expect("state-checked inbound storage");

        assert_eq!(
            derived.footprint.registers().as_slice(),
            &[MachineRegister::X86R10, MachineRegister::X86R15]
        );
        assert_eq!(
            derived.footprint.machine_state(),
            MachineStateSet::new([MachineState::GeneralRegisters])
        );
    }

    #[test]
    fn inbound_storage_rejects_a_selected_register_destroyed_by_scratch() {
        let signature = CallSignature {
            parameters: vec![ValueShape::integer(8, 8)],
            result: None,
        };
        let mut boundary =
            evaluate_ordinary_boundary_entry_plan(CallingPolicy::SystemVAMD64, &signature)
                .expect("SysV boundary")
                .plan()
                .clone();
        let ValueLocation::Register { register, .. } =
            &mut boundary.call.parameters[0].locations[0]
        else {
            panic!("register parameter");
        };
        *register = MachineRegister::X86R15;

        let error =
            derive_boundary_entry_storage(&boundary, &[(0, ValueShape::integer(8, 8))], None, None)
                .expect_err("frame-base scratch cannot also carry an input");

        assert!(error.0.contains("before capturing it"));
        assert!(error.0.contains("X86R15"));
    }

    #[test]
    fn inbound_storage_tracks_aarch64_indirect_copy_scratch() {
        let parameter = ValueShape::integer(24, 8);
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::Aapcs64,
            &CallSignature {
                parameters: vec![parameter],
                result: None,
            },
        )
        .expect("AAPCS64 indirect boundary");

        let derived = derive_boundary_entry_storage(boundary.plan(), &[(0, parameter)], None, None)
            .expect("state-checked indirect copy");

        assert_eq!(
            derived.footprint.registers().as_slice(),
            &[MachineRegister::Aarch64X(16), MachineRegister::Aarch64X(17),]
        );
    }

    #[test]
    fn bytes_handoff_descriptor_footprint_comes_from_the_x86_encoder() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::MicrosoftX64,
            &CallSignature {
                parameters: vec![ValueShape::integer(8, 8); 4],
                result: None,
            },
        )
        .expect("Microsoft x64 bytes handoff");

        let evidence = derive_boundary_entry_slice_descriptor_footprint(&boundary)
            .expect("descriptor footprint");

        assert_eq!(
            evidence.registers().as_slice(),
            &[MachineRegister::X86Rax, MachineRegister::X86R15]
        );
    }

    #[test]
    fn bytes_handoff_descriptor_footprint_comes_from_the_aarch64_encoder() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::Aapcs64,
            &CallSignature {
                parameters: vec![ValueShape::integer(8, 8); 4],
                result: None,
            },
        )
        .expect("AAPCS64 bytes handoff");

        let evidence = derive_boundary_entry_slice_descriptor_footprint(&boundary)
            .expect("descriptor footprint");

        assert_eq!(
            evidence.registers().as_slice(),
            &[MachineRegister::Aarch64X(16), MachineRegister::Aarch64X(17),]
        );
    }

    #[test]
    fn exit_result_register_footprint_unions_x86_immediate_and_runtime_loads() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters: Vec::new(),
                result: Some(ValueShape::integer(8, 8)),
            },
        )
        .expect("SysV result boundary");
        let instructions = [
            SelectedInstructionKind::WriteReturnRegisterInteger {
                register: MachineRegister::X86Rax,
                byte_size: 8,
                value: 1,
            },
            SelectedInstructionKind::CopyRuntimeStorageToReturnRegister {
                register: MachineRegister::X86Xmm(0),
                region: omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 24,
                byte_size: 8,
            },
        ];

        let evidence = derive_boundary_exit_result_register_footprint(&boundary, &instructions)
            .expect("x86 result evidence");

        assert_eq!(
            evidence.registers().as_slice(),
            &[
                MachineRegister::X86Rax,
                MachineRegister::X86R15,
                MachineRegister::X86Xmm(0),
            ]
        );
    }

    #[test]
    fn exit_result_register_footprint_tracks_aarch64_large_offset_scratch() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::Aapcs64,
            &CallSignature {
                parameters: Vec::new(),
                result: Some(ValueShape::float(8)),
            },
        )
        .expect("AAPCS64 result boundary");
        let instructions = [
            SelectedInstructionKind::CopyRuntimeStorageToReturnRegister {
                register: MachineRegister::Aarch64V(0),
                region: omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 4097,
                byte_size: 8,
            },
        ];

        let evidence = derive_boundary_exit_result_register_footprint(&boundary, &instructions)
            .expect("AArch64 result evidence");

        assert_eq!(
            evidence.registers().as_slice(),
            &[
                MachineRegister::Aarch64X(16),
                MachineRegister::Aarch64X(19),
                MachineRegister::Aarch64X(26),
                MachineRegister::Aarch64V(0),
            ]
        );
    }

    fn indirect_result_copy_instruction(
        source_offset: usize,
        pointer_offset: usize,
        byte_count: usize,
    ) -> SelectedInstructionKind {
        let target = omega_abstract_operations::Place::at(
            omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
            pointer_offset,
        )
        .with_step(omega_abstract_operations::PlaceStep::Deref)
        .expect("pointee target");
        SelectedInstructionKind::CopyPlaces {
            source: omega_abstract_operations::Place::at(
                omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
                source_offset,
            ),
            target,
            byte_count,
        }
    }

    #[test]
    fn indirect_result_copy_footprint_tracks_x86_shared_base_scratch() {
        let result = ValueShape::integer(24, 8);
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters: Vec::new(),
                result: Some(result),
            },
        )
        .expect("SysV indirect result");
        let instructions = [
            indirect_result_copy_instruction(64, 32, 24),
            indirect_result_copy_instruction(96, 40, 24),
        ];

        let evidence =
            derive_boundary_exit_indirect_result_copy_footprint(&boundary, 32, &instructions)
                .expect("x86 indirect-result evidence");

        assert_eq!(
            evidence.registers().as_slice(),
            &[
                MachineRegister::X86Rax,
                MachineRegister::X86R14,
                MachineRegister::X86R15,
            ]
        );
    }

    #[test]
    fn indirect_result_copy_footprint_tracks_aarch64_pointee_scratch() {
        let result = ValueShape::integer(24, 8);
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::Aapcs64,
            &CallSignature {
                parameters: Vec::new(),
                result: Some(result),
            },
        )
        .expect("AAPCS64 indirect result");
        let instructions = [indirect_result_copy_instruction(64, 32, 24)];

        let evidence =
            derive_boundary_exit_indirect_result_copy_footprint(&boundary, 32, &instructions)
                .expect("AArch64 indirect-result evidence");

        assert_eq!(
            evidence.registers().as_slice(),
            &[
                MachineRegister::Aarch64X(16),
                MachineRegister::Aarch64X(17),
                MachineRegister::Aarch64X(20),
            ]
        );
    }

    #[test]
    fn boundary_exit_consumes_the_exact_selected_result_register() {
        let result = ValueShape::integer(8, 8);
        let mut boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters: Vec::new(),
                result: Some(result),
            },
        )
        .expect("SysV boundary")
        .plan()
        .clone();
        let ValueLocation::Register { register, .. } =
            &mut boundary.call.result.as_mut().expect("result").locations[0]
        else {
            panic!("register result");
        };
        *register = MachineRegister::X86R10;

        let exit = derive_boundary_exit(&boundary, &[], Some(result)).expect("boundary exit");

        assert_eq!(
            exit.control,
            omega_calling_conventions::EntryControl::CallReturn
        );
        assert_eq!(
            exit.result_locations,
            vec![ValueLocation::Register {
                register: MachineRegister::X86R10,
                value_byte_offset: 0,
                byte_size: 8,
            }]
        );
    }
}

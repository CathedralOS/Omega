use omega_abstract_operations::SelectedInstructionKind;
use omega_calling_conventions::{
    BoundaryEntryPlan, CallSignature, EntryControl, IndirectPointerLocation, MachineState,
    MachineStateSet, PlanDiagnostic, RegisterSet, StateFootprintEvidence,
    ValidatedBoundaryEntryPlan, ValueLocation, ValuePlacement, ValueShape,
    validate_boundary_entry_plan, validate_call_return_mechanics_footprint,
    validate_runtime_value_guard_footprint, validate_state_footprint,
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
    pub parameters: Vec<DerivedBoundaryEntryParameterStorage>,
    pub footprint: StateFootprintEvidence,
}

/// Exact relationship between one semantic parameter position, its normalized
/// ABI placement, and the generated prologue writes that capture it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DerivedBoundaryEntryParameterStorage {
    pub parameter_index: usize,
    pub destination_byte_offset: usize,
    pub shape: ValueShape,
    pub placement: ValuePlacement,
    pub write_range: std::ops::Range<usize>,
}

impl DerivedBoundaryEntryStorage {
    pub fn parameter(
        &self,
        parameter_index: usize,
    ) -> Option<&DerivedBoundaryEntryParameterStorage> {
        self.parameters
            .iter()
            .find(|parameter| parameter.parameter_index == parameter_index)
    }
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

/// Derive the exact fixed prologue/epilogue register and machine-state writes
/// for an ordinary call-return boundary. Stack/control effects are prescribed
/// by `EntryControl::CallReturn`, so their validator is deliberately distinct
/// from handler-body transitive state validation.
pub fn derive_boundary_call_return_mechanics_footprint<'instruction>(
    boundary: &ValidatedBoundaryEntryPlan,
    instructions: impl IntoIterator<Item = &'instruction SelectedInstructionKind>,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    if boundary.plan().call.entry_control != EntryControl::CallReturn {
        return Err(PlanDiagnostic(
            "ordinary function entry/return lowering requires CallReturn entry control".into(),
        ));
    }
    let architecture = boundary.plan().call.policy.architecture();
    let mut registers = Vec::new();
    let mut additional_state = MachineStateSet::empty();
    let mut enter_count = 0usize;
    let mut return_count = 0usize;
    for instruction in instructions {
        let (writes, state) = match instruction {
            SelectedInstructionKind::EnterFunction => {
                enter_count += 1;
                match architecture {
                    omega_target::Architecture::X86_64 => (
                        omega_isa_x86_64::function_enter_register_writes(),
                        omega_isa_x86_64::function_enter_additional_machine_state(),
                    ),
                    omega_target::Architecture::Aarch64 => (
                        omega_isa_aarch64::function_enter_register_writes(),
                        omega_isa_aarch64::function_enter_additional_machine_state(),
                    ),
                }
            }
            SelectedInstructionKind::LeaveFunction => {
                return_count += 1;
                match architecture {
                    omega_target::Architecture::X86_64 => (
                        omega_isa_x86_64::return_register_writes(),
                        omega_isa_x86_64::return_additional_machine_state(),
                    ),
                    omega_target::Architecture::Aarch64 => (
                        omega_isa_aarch64::return_register_writes(),
                        omega_isa_aarch64::return_additional_machine_state(),
                    ),
                }
            }
            _ => continue,
        };
        registers.extend_from_slice(writes.as_slice());
        additional_state = additional_state.union(state);
    }
    if enter_count != 1 || return_count != 1 {
        return Err(PlanDiagnostic(format!(
            "ordinary boundary mechanics require exactly one function entry and return (found {enter_count} entries and {return_count} returns)"
        )));
    }
    let evidence = StateFootprintEvidence::new(RegisterSet::new(registers), additional_state);
    validate_call_return_mechanics_footprint(boundary, &evidence)?;
    Ok(evidence)
}

/// Derive the compiler-generated runtime-dispatch scaffold separately from
/// authored/body operations. The scaffold owns the dispatch-state register;
/// case-entry comparisons additionally write condition flags. Guard operand
/// evaluation remains a later whole-body evidence slice.
pub fn derive_boundary_dispatch_scaffold_footprint<'instruction>(
    boundary: &ValidatedBoundaryEntryPlan,
    instructions: impl IntoIterator<Item = &'instruction SelectedInstructionKind>,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    let architecture = boundary.plan().call.policy.architecture();
    let mut registers = Vec::new();
    let mut additional_state = MachineStateSet::empty();
    let mut loop_enter_count = 0usize;
    let mut loop_leave_count = 0usize;
    for instruction in instructions {
        let (writes, state) = match instruction {
            SelectedInstructionKind::EnterDispatchLoop { .. } => {
                loop_enter_count += 1;
                match architecture {
                    omega_target::Architecture::X86_64 => (
                        omega_isa_x86_64::dispatch_loop_enter_register_writes(),
                        MachineStateSet::empty(),
                    ),
                    omega_target::Architecture::Aarch64 => (
                        omega_isa_aarch64::dispatch_loop_enter_register_writes(),
                        MachineStateSet::empty(),
                    ),
                }
            }
            SelectedInstructionKind::EnterDispatchCase { .. } => match architecture {
                omega_target::Architecture::X86_64 => (
                    omega_isa_x86_64::dispatch_case_enter_register_writes(),
                    omega_isa_x86_64::dispatch_case_enter_additional_machine_state(),
                ),
                omega_target::Architecture::Aarch64 => (
                    omega_isa_aarch64::dispatch_case_enter_register_writes(),
                    omega_isa_aarch64::dispatch_case_enter_additional_machine_state(),
                ),
            },
            SelectedInstructionKind::SetDispatchState { .. }
            | SelectedInstructionKind::TerminateDispatch => match architecture {
                omega_target::Architecture::X86_64 => (
                    omega_isa_x86_64::dispatch_state_write_register_writes(),
                    MachineStateSet::empty(),
                ),
                omega_target::Architecture::Aarch64 => (
                    omega_isa_aarch64::dispatch_state_write_register_writes(),
                    MachineStateSet::empty(),
                ),
            },
            SelectedInstructionKind::LeaveDispatchCase => match architecture {
                omega_target::Architecture::X86_64 => (
                    omega_isa_x86_64::dispatch_case_leave_register_writes(),
                    MachineStateSet::empty(),
                ),
                omega_target::Architecture::Aarch64 => (
                    omega_isa_aarch64::dispatch_case_leave_register_writes(),
                    MachineStateSet::empty(),
                ),
            },
            SelectedInstructionKind::LeaveDispatchLoop => {
                loop_leave_count += 1;
                (RegisterSet::default(), MachineStateSet::empty())
            }
            _ => continue,
        };
        registers.extend_from_slice(writes.as_slice());
        additional_state = additional_state.union(state);
    }
    if loop_enter_count != 1 || loop_leave_count != 1 {
        return Err(PlanDiagnostic(format!(
            "dispatch scaffold evidence requires exactly one loop entry and leave (found {loop_enter_count} entries and {loop_leave_count} leaves)"
        )));
    }
    let evidence = StateFootprintEvidence::new(RegisterSet::new(registers), additional_state);
    validate_state_footprint(boundary, &evidence)?;
    Ok(evidence)
}

/// Derive storage-backed static guard comparisons without sweeping the other
/// guard-lowering shapes into this fragment. The target encoders own the fixed
/// GPR/vector scratch identities and condition-flag effect.
pub fn derive_boundary_static_guard_footprint<'instruction>(
    boundary: &ValidatedBoundaryEntryPlan,
    instructions: impl IntoIterator<Item = &'instruction SelectedInstructionKind>,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    let architecture = boundary.plan().call.policy.architecture();
    let mut registers = Vec::new();
    let mut additional_state = MachineStateSet::empty();
    for instruction in instructions {
        let SelectedInstructionKind::EvaluateDispatchGuard {
            guard_lowering: omega_abstract_operations::StateGuardLowering::CompareStaticValue,
            operator,
            has_storage: true,
            is_float,
            ..
        } = instruction
        else {
            continue;
        };
        if !matches!(
            operator,
            omega_abstract_operations::StateGuardOperator::Equal
                | omega_abstract_operations::StateGuardOperator::NotEqual
                | omega_abstract_operations::StateGuardOperator::Greater
                | omega_abstract_operations::StateGuardOperator::GreaterOrEqual
                | omega_abstract_operations::StateGuardOperator::Less
                | omega_abstract_operations::StateGuardOperator::LessOrEqual
                | omega_abstract_operations::StateGuardOperator::GreaterUnsigned
                | omega_abstract_operations::StateGuardOperator::GreaterOrEqualUnsigned
                | omega_abstract_operations::StateGuardOperator::LessUnsigned
                | omega_abstract_operations::StateGuardOperator::LessOrEqualUnsigned
        ) {
            continue;
        }
        let (writes, state) = match architecture {
            omega_target::Architecture::X86_64 => (
                omega_isa_x86_64::dispatch_guard_compare_static_register_writes(*is_float),
                omega_isa_x86_64::dispatch_guard_compare_static_additional_machine_state(),
            ),
            omega_target::Architecture::Aarch64 => (
                omega_isa_aarch64::dispatch_guard_compare_static_register_writes(*is_float),
                omega_isa_aarch64::dispatch_guard_compare_static_additional_machine_state(),
            ),
        };
        registers.extend_from_slice(writes.as_slice());
        additional_state = additional_state.union(state);
    }
    let evidence = StateFootprintEvidence::new(RegisterSet::new(registers), additional_state);
    validate_state_footprint(boundary, &evidence)?;
    Ok(evidence)
}

/// Derive the fixed register/state effects of the two dedicated runtime-text
/// guard encoders. Computed text equality carried as a runtime value operand
/// and place-shaped comparisons remain separate later slices; this fragment
/// is limited to instruction kinds whose complete bytes are owned by the
/// literal and descriptor-vs-literal encoders.
pub fn derive_boundary_runtime_text_guard_footprint<'instruction>(
    boundary: &ValidatedBoundaryEntryPlan,
    instructions: impl IntoIterator<Item = &'instruction SelectedInstructionKind>,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    let architecture = boundary.plan().call.policy.architecture();
    let mut registers = Vec::new();
    let mut additional_state = MachineStateSet::empty();
    for instruction in instructions {
        let (writes, state) = match (architecture, instruction) {
            (
                omega_target::Architecture::X86_64,
                SelectedInstructionKind::CompareRuntimeTextLiteral { .. },
            ) => (
                omega_isa_x86_64::runtime_text_literal_compare_register_writes(),
                omega_isa_x86_64::runtime_text_literal_compare_additional_machine_state(),
            ),
            (
                omega_target::Architecture::X86_64,
                SelectedInstructionKind::CompareRuntimeTextStorage { .. },
            ) => (
                omega_isa_x86_64::runtime_text_storage_compare_register_writes(),
                omega_isa_x86_64::runtime_text_storage_compare_additional_machine_state(),
            ),
            (
                omega_target::Architecture::Aarch64,
                SelectedInstructionKind::CompareRuntimeTextLiteral { .. },
            ) => (
                omega_isa_aarch64::runtime_text_literal_compare_register_writes(),
                omega_isa_aarch64::runtime_text_literal_compare_additional_machine_state(),
            ),
            (
                omega_target::Architecture::Aarch64,
                SelectedInstructionKind::CompareRuntimeTextStorage { .. },
            ) => (
                omega_isa_aarch64::runtime_text_storage_compare_register_writes(),
                omega_isa_aarch64::runtime_text_storage_compare_additional_machine_state(),
            ),
            _ => continue,
        };
        registers.extend_from_slice(writes.as_slice());
        additional_state = additional_state.union(state);
    }
    let evidence = StateFootprintEvidence::new(RegisterSet::new(registers), additional_state);
    validate_state_footprint(boundary, &evidence)?;
    Ok(evidence)
}

/// Derive the complete effects of place-pair and place-vs-immediate guards.
/// x86 place walks and AArch64's currently admitted direct-place shapes both
/// obtain their scratch identities from the encoder modules that emit them.
pub fn derive_boundary_place_guard_footprint<'instruction>(
    boundary: &ValidatedBoundaryEntryPlan,
    instructions: impl IntoIterator<Item = &'instruction SelectedInstructionKind>,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    let architecture = boundary.plan().call.policy.architecture();
    let mut registers = Vec::new();
    let mut additional_state = MachineStateSet::empty();
    for instruction in instructions {
        let footprint = match (architecture, instruction) {
            (
                omega_target::Architecture::X86_64,
                SelectedInstructionKind::ComparePlaces { is_float, .. },
            ) => Some((
                omega_isa_x86_64::place_compare_register_writes(*is_float),
                omega_isa_x86_64::place_compare_additional_machine_state(),
            )),
            (
                omega_target::Architecture::X86_64,
                SelectedInstructionKind::ComparePlaceValue { .. },
            ) => Some((
                omega_isa_x86_64::place_value_compare_register_writes(),
                omega_isa_x86_64::place_value_compare_additional_machine_state(),
            )),
            (
                omega_target::Architecture::Aarch64,
                SelectedInstructionKind::ComparePlaces {
                    left,
                    right,
                    byte_size,
                    is_float,
                    ..
                },
            ) => match (left.const_offset(), right.const_offset()) {
                (Some(left_offset), Some(right_offset)) => Some((
                    omega_isa_aarch64::runtime_storage_compare_register_writes(
                        left_offset,
                        right_offset,
                        *byte_size,
                        *is_float,
                    ),
                    omega_isa_aarch64::runtime_storage_compare_additional_machine_state(),
                )),
                _ => None,
            },
            (
                omega_target::Architecture::Aarch64,
                SelectedInstructionKind::ComparePlaceValue { place, .. },
            ) if place.const_offset().is_some() => Some((
                omega_isa_aarch64::runtime_storage_value_compare_register_writes(),
                omega_isa_aarch64::runtime_storage_value_compare_additional_machine_state(),
            )),
            _ => None,
        };
        let Some((writes, state)) = footprint else {
            continue;
        };
        registers.extend_from_slice(writes.as_slice());
        additional_state = additional_state.union(state);
    }
    let evidence = StateFootprintEvidence::new(RegisterSet::new(registers), additional_state);
    validate_state_footprint(boundary, &evidence)?;
    Ok(evidence)
}

/// Derive the recursive runtime-value guard evaluator's closed encoder-family
/// may-write ceiling. The operand arena is the same arena consumed by byte
/// emission; on x86 it also determines whether a nested `Binary` introduces
/// balanced push/pop stack scratch.
pub fn derive_boundary_runtime_value_guard_footprint<'instruction>(
    boundary: &ValidatedBoundaryEntryPlan,
    runtime_value_operands: &impl omega_target_operations::RuntimeValueOperandSource,
    instructions: impl IntoIterator<Item = &'instruction SelectedInstructionKind>,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    let architecture = boundary.plan().call.policy.architecture();
    let mut registers = Vec::new();
    let mut additional_state = MachineStateSet::empty();
    for instruction in instructions {
        let SelectedInstructionKind::CompareRuntimeValues { left, right, .. } = instruction else {
            continue;
        };
        let (writes, state) = match architecture {
            omega_target::Architecture::X86_64 => (
                omega_isa_x86_64::runtime_value_compare_register_write_ceiling(),
                omega_isa_x86_64::runtime_value_compare_additional_machine_state(
                    runtime_value_operands,
                    *left,
                    *right,
                ),
            ),
            omega_target::Architecture::Aarch64 => (
                omega_isa_aarch64::runtime_value_compare_register_write_ceiling(),
                omega_isa_aarch64::runtime_value_compare_additional_machine_state(
                    runtime_value_operands,
                    *left,
                    *right,
                ),
            ),
        };
        registers.extend_from_slice(writes.as_slice());
        additional_state = additional_state.union(state);
    }
    let evidence = StateFootprintEvidence::new(RegisterSet::new(registers), additional_state);
    validate_runtime_value_guard_footprint(boundary, &evidence)?;
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
            role: omega_abstract_operations::CopyPlacesRole::ExitIndirectResult,
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

/// Derive the target scratch footprint for the currently admitted ordinary
/// compiler-body place-copy subset. Direct storage pairs and direct-storage to
/// pointee copies are included; every other `CopyPlaces` shape remains outside
/// this partial evidence until its encoder publishes the corresponding
/// clobber contract.
pub fn derive_boundary_compiler_body_place_copy_footprint<'instruction>(
    boundary: &ValidatedBoundaryEntryPlan,
    instructions: impl IntoIterator<Item = &'instruction SelectedInstructionKind>,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    let architecture = boundary.plan().call.policy.architecture();
    let mut registers = Vec::new();
    for instruction in instructions {
        let SelectedInstructionKind::CopyPlaces {
            source,
            target,
            byte_count,
            role: omega_abstract_operations::CopyPlacesRole::Ordinary,
        } = instruction
        else {
            continue;
        };
        let clobbers = match (
            architecture,
            crate::classify_copy_places_shape(source, target),
        ) {
            (omega_target::Architecture::X86_64, crate::CopyPlacesShape::Direct { .. }) => {
                omega_isa_x86_64::copy_places_direct_clobbers(*byte_count)
            }
            (
                omega_target::Architecture::Aarch64,
                crate::CopyPlacesShape::Direct {
                    source_offset,
                    target_offset,
                },
            ) => omega_isa_aarch64::runtime_storage_copy_clobbers(
                source_offset,
                target_offset,
                *byte_count,
            ),
            (omega_target::Architecture::X86_64, crate::CopyPlacesShape::ToPointee { .. }) => {
                omega_isa_x86_64::copy_places_to_pointee_clobbers(*byte_count)
            }
            (
                omega_target::Architecture::Aarch64,
                crate::CopyPlacesShape::ToPointee {
                    source_offset,
                    pointer_byte_offset,
                    field_byte_offset,
                },
            ) => omega_isa_aarch64::runtime_storage_copy_to_runtime_pointee_clobbers(
                source_offset,
                pointer_byte_offset,
                field_byte_offset,
                *byte_count,
            ),
            (omega_target::Architecture::X86_64, crate::CopyPlacesShape::FromPointee { .. }) => {
                omega_isa_x86_64::copy_places_from_pointee_clobbers(*byte_count)
            }
            (
                omega_target::Architecture::Aarch64,
                crate::CopyPlacesShape::FromPointee {
                    pointer_byte_offset,
                    field_byte_offset,
                    target_offset,
                },
            ) => omega_isa_aarch64::runtime_storage_copy_from_runtime_pointee_clobbers(
                pointer_byte_offset,
                field_byte_offset,
                target_offset,
                *byte_count,
            ),
            (omega_target::Architecture::X86_64, crate::CopyPlacesShape::PointeePair { .. })
                if source.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
                    && target.region
                        == omega_target_operations::RuntimeStorageRegion::RuntimeFrame =>
            {
                omega_isa_x86_64::copy_places_pointee_pair_clobbers(*byte_count)
            }
            (
                omega_target::Architecture::Aarch64,
                crate::CopyPlacesShape::PointeePair {
                    source_field_byte_offset,
                    target_field_byte_offset,
                    ..
                },
            ) if source.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
                && target.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame =>
            {
                omega_isa_aarch64::runtime_storage_copy_pointee_pair_clobbers(
                    source_field_byte_offset,
                    target_field_byte_offset,
                    *byte_count,
                )
            }
            (omega_target::Architecture::X86_64, crate::CopyPlacesShape::FromIndexed { .. })
                if source.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame =>
            {
                omega_isa_x86_64::copy_places_from_indexed_clobbers(*byte_count)
            }
            (
                omega_target::Architecture::Aarch64,
                crate::CopyPlacesShape::FromIndexed { index_region, .. },
            )
                if source.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame =>
            {
                omega_isa_aarch64::runtime_storage_copy_from_runtime_frame_indexed_with_index_region_clobbers(
                    index_region,
                )
            }
            (omega_target::Architecture::X86_64, crate::CopyPlacesShape::ToIndexed { .. })
                if source.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
                    && target.region
                        == omega_target_operations::RuntimeStorageRegion::RuntimeFrame =>
            {
                omega_isa_x86_64::copy_places_to_indexed_clobbers(*byte_count)
            }
            (
                omega_target::Architecture::X86_64,
                crate::CopyPlacesShape::ToIndexedByRegion { .. },
            ) => omega_isa_x86_64::copy_places_clobbers(source, target, *byte_count),
            (omega_target::Architecture::Aarch64, crate::CopyPlacesShape::ToIndexed { .. })
                if source.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
                    && target.region
                        == omega_target_operations::RuntimeStorageRegion::RuntimeFrame =>
            {
                omega_isa_aarch64::runtime_storage_copy_to_runtime_frame_indexed_clobbers()
            }
            (
                omega_target::Architecture::Aarch64,
                crate::CopyPlacesShape::ToIndexedByRegion { index_region, .. },
            ) if target.region
                == omega_target_operations::RuntimeStorageRegion::RuntimeFrame =>
            {
                omega_isa_aarch64::runtime_storage_copy_to_runtime_frame_indexed_with_regions_clobbers(
                    source.region,
                    index_region,
                )
            }
            (omega_target::Architecture::X86_64, crate::CopyPlacesShape::IndexedToPointee { .. })
                if source.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
                    && target.region
                        == omega_target_operations::RuntimeStorageRegion::RuntimeFrame =>
            {
                omega_isa_x86_64::copy_places_indexed_to_pointee_clobbers(*byte_count)
            }
            (
                omega_target::Architecture::X86_64,
                crate::CopyPlacesShape::IndexedToPointeeByRegion { .. },
            ) => omega_isa_x86_64::copy_places_clobbers(source, target, *byte_count),
            (
                omega_target::Architecture::Aarch64,
                crate::CopyPlacesShape::IndexedToPointee { .. },
            ) if source.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
                && target.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame =>
            {
                omega_isa_aarch64::runtime_storage_copy_from_runtime_frame_indexed_to_runtime_pointee_clobbers()
            }
            (
                omega_target::Architecture::Aarch64,
                crate::CopyPlacesShape::IndexedToPointeeByRegion { index_region, .. },
            ) if source.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
                && target.region
                    == omega_target_operations::RuntimeStorageRegion::RuntimeFrame =>
            {
                omega_isa_aarch64::runtime_storage_copy_from_runtime_frame_indexed_to_runtime_pointee_with_index_region_clobbers(
                    index_region,
                )
            }
            (
                omega_target::Architecture::X86_64,
                crate::CopyPlacesShape::FromFrameBaseIndexed { .. },
            ) => omega_isa_x86_64::copy_places_from_frame_base_indexed_clobbers(*byte_count),
            (
                omega_target::Architecture::Aarch64,
                crate::CopyPlacesShape::FromFrameBaseIndexed { .. },
            ) => omega_isa_aarch64::runtime_storage_copy_from_runtime_frame_base_indexed_clobbers(),
            (
                omega_target::Architecture::Aarch64,
                crate::CopyPlacesShape::ToFrameBaseIndexed { index_region, .. },
            ) => omega_isa_aarch64::runtime_storage_copy_to_runtime_frame_base_indexed_clobbers(
                source.region,
                index_region,
            ),
            (
                omega_target::Architecture::X86_64,
                crate::CopyPlacesShape::ToFrameBaseIndexed { .. },
            ) => omega_isa_x86_64::copy_places_clobbers(source, target, *byte_count),
            (
                omega_target::Architecture::Aarch64,
                crate::CopyPlacesShape::FrameBaseIndexedToPointee { .. },
            ) => omega_isa_aarch64::runtime_storage_copy_from_runtime_frame_base_indexed_to_runtime_pointee_clobbers(),
            (
                omega_target::Architecture::Aarch64,
                crate::CopyPlacesShape::PointeeToFrameBaseIndexed { .. },
            ) => omega_isa_aarch64::runtime_storage_copy_from_runtime_pointee_to_runtime_frame_base_indexed_clobbers(),
            (
                omega_target::Architecture::X86_64,
                crate::CopyPlacesShape::FrameBaseIndexedToPointee { .. }
                | crate::CopyPlacesShape::PointeeToFrameBaseIndexed { .. },
            ) => omega_isa_x86_64::copy_places_clobbers(source, target, *byte_count),
            (
                omega_target::Architecture::X86_64,
                crate::CopyPlacesShape::FromMachineIndexed { .. },
            ) => omega_isa_x86_64::copy_places_from_machine_indexed_clobbers(*byte_count),
            (
                omega_target::Architecture::Aarch64,
                crate::CopyPlacesShape::FromMachineIndexed { .. },
            ) => omega_isa_aarch64::runtime_storage_copy_from_runtime_machine_indexed_clobbers(),
            (
                omega_target::Architecture::X86_64,
                crate::CopyPlacesShape::ToMachineIndexed { .. },
            ) => omega_isa_x86_64::copy_places_to_machine_indexed_clobbers(*byte_count),
            (
                omega_target::Architecture::Aarch64,
                crate::CopyPlacesShape::ToMachineIndexed { .. },
            ) => omega_isa_aarch64::runtime_storage_copy_to_runtime_machine_indexed_clobbers(),
            (
                omega_target::Architecture::X86_64,
                crate::CopyPlacesShape::FromFrameBaseDoubleIndexed { .. },
            ) => omega_isa_x86_64::copy_places_from_frame_base_double_indexed_clobbers(*byte_count),
            (
                omega_target::Architecture::Aarch64,
                crate::CopyPlacesShape::FromFrameBaseDoubleIndexed {
                    outer_index_region,
                    inner_index_region,
                    ..
                },
            ) => omega_isa_aarch64::runtime_storage_copy_from_runtime_frame_base_double_indexed_clobbers(
                outer_index_region,
                inner_index_region,
            ),
            (
                omega_target::Architecture::Aarch64,
                crate::CopyPlacesShape::FrameBaseDoubleIndexedToPointee { .. },
            ) => omega_isa_aarch64::runtime_storage_copy_from_runtime_frame_base_double_indexed_to_runtime_pointee_clobbers(),
            (
                omega_target::Architecture::Aarch64,
                crate::CopyPlacesShape::PointeeToFrameBaseDoubleIndexed { .. },
            ) => omega_isa_aarch64::runtime_storage_copy_from_runtime_pointee_to_runtime_frame_base_double_indexed_clobbers(),
            (
                omega_target::Architecture::Aarch64,
                crate::CopyPlacesShape::ToFrameBaseDoubleIndexed {
                    outer_index_region,
                    inner_index_region,
                    ..
                },
            ) => omega_isa_aarch64::runtime_storage_copy_to_runtime_frame_base_double_indexed_clobbers(
                source.region,
                outer_index_region,
                inner_index_region,
            ),
            (
                omega_target::Architecture::X86_64,
                crate::CopyPlacesShape::ToFrameBaseDoubleIndexed { .. },
            ) => omega_isa_x86_64::copy_places_clobbers(source, target, *byte_count),
            (
                omega_target::Architecture::X86_64,
                crate::CopyPlacesShape::FrameBaseDoubleIndexedToPointee { .. },
            ) => omega_isa_x86_64::copy_places_clobbers(source, target, *byte_count),
            (
                omega_target::Architecture::X86_64,
                crate::CopyPlacesShape::PointeeToFrameBaseDoubleIndexed { .. },
            ) => omega_isa_x86_64::copy_places_clobbers(source, target, *byte_count),
            (
                omega_target::Architecture::X86_64,
                crate::CopyPlacesShape::FromMachineDoubleIndexed { .. },
            ) => omega_isa_x86_64::copy_places_from_machine_double_indexed_clobbers(*byte_count),
            (
                omega_target::Architecture::Aarch64,
                crate::CopyPlacesShape::FromMachineDoubleIndexed {
                    outer_index_region,
                    inner_index_region,
                    ..
                },
            ) => omega_isa_aarch64::runtime_storage_copy_from_runtime_machine_double_indexed_clobbers(
                outer_index_region,
                inner_index_region,
            ),
            (
                omega_target::Architecture::X86_64,
                crate::CopyPlacesShape::ToMachineDoubleIndexed { .. },
            ) => omega_isa_x86_64::copy_places_to_machine_double_indexed_clobbers(*byte_count),
            (
                omega_target::Architecture::Aarch64,
                crate::CopyPlacesShape::ToMachineDoubleIndexed {
                    outer_index_region,
                    inner_index_region,
                    ..
                },
            ) => omega_isa_aarch64::runtime_storage_copy_to_runtime_machine_double_indexed_clobbers(
                source.region,
                outer_index_region,
                inner_index_region,
            ),
            (
                omega_target::Architecture::X86_64,
                crate::CopyPlacesShape::MachineIndexedPair { .. },
            ) => omega_isa_x86_64::copy_places_machine_indexed_pair_clobbers(*byte_count),
            (
                omega_target::Architecture::Aarch64,
                crate::CopyPlacesShape::MachineIndexedPair { .. },
            ) => omega_isa_aarch64::runtime_storage_copy_machine_indexed_to_machine_indexed_clobbers(),
            (
                omega_target::Architecture::X86_64,
                crate::CopyPlacesShape::FrameBaseIndexedPair { .. },
            ) => omega_isa_x86_64::copy_places_clobbers(source, target, *byte_count),
            (
                omega_target::Architecture::Aarch64,
                crate::CopyPlacesShape::FrameBaseIndexedPair {
                    source_index_region,
                    target_index_region,
                    ..
                },
            ) => omega_isa_aarch64::runtime_storage_copy_frame_base_indexed_to_frame_base_indexed_clobbers(
                source_index_region,
                target_index_region,
            ),
            (
                omega_target::Architecture::X86_64,
                crate::CopyPlacesShape::CrossRegionIndexedPair { .. },
            ) => omega_isa_x86_64::copy_places_clobbers(source, target, *byte_count),
            (
                omega_target::Architecture::Aarch64,
                crate::CopyPlacesShape::CrossRegionIndexedPair { .. },
            ) => omega_isa_aarch64::runtime_storage_copy_cross_region_indexed_pair_clobbers(),
            (
                omega_target::Architecture::X86_64,
                crate::CopyPlacesShape::CrossRegionDoubleIndexedPair { .. },
            ) => omega_isa_x86_64::copy_places_clobbers(source, target, *byte_count),
            (
                omega_target::Architecture::Aarch64,
                crate::CopyPlacesShape::CrossRegionDoubleIndexedPair { .. },
            ) => omega_isa_aarch64::runtime_storage_copy_cross_region_double_indexed_pair_clobbers(),
            (
                omega_target::Architecture::X86_64,
                crate::CopyPlacesShape::FrameBaseDoubleIndexedPair { .. },
            ) => omega_isa_x86_64::copy_places_clobbers(source, target, *byte_count),
            (
                omega_target::Architecture::Aarch64,
                crate::CopyPlacesShape::FrameBaseDoubleIndexedPair {
                    source_outer_index_region,
                    source_inner_index_region,
                    target_outer_index_region,
                    target_inner_index_region,
                    ..
                },
            ) => omega_isa_aarch64::runtime_storage_copy_frame_base_double_indexed_to_frame_base_double_indexed_clobbers(
                source_outer_index_region,
                source_inner_index_region,
                target_outer_index_region,
                target_inner_index_region,
            ),
            (
                omega_target::Architecture::X86_64,
                crate::CopyPlacesShape::MachineDoubleIndexedPair { .. },
            ) => omega_isa_x86_64::copy_places_clobbers(source, target, *byte_count),
            (
                omega_target::Architecture::Aarch64,
                crate::CopyPlacesShape::MachineDoubleIndexedPair {
                    source_outer_index_region,
                    source_inner_index_region,
                    target_outer_index_region,
                    target_inner_index_region,
                    ..
                },
            ) => omega_isa_aarch64::runtime_storage_copy_machine_double_indexed_to_machine_double_indexed_clobbers(
                source_outer_index_region,
                source_inner_index_region,
                target_outer_index_region,
                target_inner_index_region,
            ),
            (
                omega_target::Architecture::X86_64,
                crate::CopyPlacesShape::MachineIndexedToPointee { .. }
                | crate::CopyPlacesShape::PointeeToMachineIndexed { .. },
            ) => omega_isa_x86_64::copy_places_clobbers(source, target, *byte_count),
            (
                omega_target::Architecture::Aarch64,
                crate::CopyPlacesShape::MachineIndexedToPointee { .. },
            ) => omega_isa_aarch64::runtime_storage_copy_machine_indexed_to_runtime_pointee_clobbers(),
            (
                omega_target::Architecture::Aarch64,
                crate::CopyPlacesShape::PointeeToMachineIndexed { .. },
            ) => omega_isa_aarch64::runtime_storage_copy_runtime_pointee_to_machine_indexed_clobbers(),
            (
                omega_target::Architecture::X86_64,
                crate::CopyPlacesShape::MachineDoubleIndexedToPointee { .. }
                | crate::CopyPlacesShape::PointeeToMachineDoubleIndexed { .. },
            ) => omega_isa_x86_64::copy_places_clobbers(source, target, *byte_count),
            (
                omega_target::Architecture::Aarch64,
                crate::CopyPlacesShape::MachineDoubleIndexedToPointee { .. },
            ) => omega_isa_aarch64::runtime_storage_copy_machine_double_indexed_to_runtime_pointee_clobbers(),
            (
                omega_target::Architecture::Aarch64,
                crate::CopyPlacesShape::PointeeToMachineDoubleIndexed { .. },
            ) => omega_isa_aarch64::runtime_storage_copy_runtime_pointee_to_machine_double_indexed_clobbers(),
            (omega_target::Architecture::X86_64, crate::CopyPlacesShape::General) => {
                omega_isa_x86_64::copy_places_clobbers(source, target, *byte_count)
            }
            _ => continue,
        };
        registers.extend_from_slice(clobbers.as_slice());
    }
    let evidence =
        StateFootprintEvidence::new(RegisterSet::new(registers), MachineStateSet::empty());
    validate_state_footprint(boundary, &evidence)?;
    Ok(evidence)
}

/// Derive the exact scratch footprint of compiler-body immediate integer
/// writes whose final replay contracts have landed. Other place shapes remain
/// separate until their retained target encoder publishes and tests an exact
/// clobber contract.
pub fn derive_boundary_compiler_body_place_integer_write_footprint<'instruction>(
    boundary: &ValidatedBoundaryEntryPlan,
    instructions: impl IntoIterator<Item = &'instruction SelectedInstructionKind>,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    let architecture = boundary.plan().call.policy.architecture();
    let mut registers = Vec::new();
    for instruction in instructions {
        let SelectedInstructionKind::WritePlaceInteger { target, .. } = instruction else {
            continue;
        };
        let shape = crate::classify_write_place_shape(target);
        let frame_indexed = crate::classify_frame_base_indexed_integer_shape(target);
        let frame_double = crate::classify_frame_base_double_indexed_integer_shape(target);
        if architecture == omega_target::Architecture::Aarch64
            && let Some(frame_indexed) = frame_indexed
        {
            registers.extend_from_slice(
                omega_isa_aarch64::runtime_frame_base_indexed_integer_write_with_index_region_clobbers(
                    frame_indexed.index_region,
                )
                .as_slice(),
            );
            continue;
        }
        let clobbers = match (architecture, shape) {
            (
                omega_target::Architecture::X86_64,
                crate::WritePlaceShape::Direct { .. } | crate::WritePlaceShape::Pointee { .. },
            ) => omega_isa_x86_64::place_integer_write_clobbers(target),
            (
                omega_target::Architecture::Aarch64,
                crate::WritePlaceShape::Direct { byte_offset },
            ) => omega_isa_aarch64::runtime_machine_integer_write_clobbers(byte_offset),
            (
                omega_target::Architecture::Aarch64,
                crate::WritePlaceShape::Pointee {
                    pointer_byte_offset,
                    field_byte_offset,
                },
            ) => omega_isa_aarch64::runtime_pointee_integer_write_clobbers(
                pointer_byte_offset,
                field_byte_offset,
            ),
            (omega_target::Architecture::Aarch64, crate::WritePlaceShape::FrameIndexed { .. }) => {
                omega_isa_aarch64::runtime_frame_indexed_integer_write_clobbers(
                    omega_target_operations::RuntimeStorageRegion::RuntimeFrame,
                )
            }
            (
                omega_target::Architecture::Aarch64,
                crate::WritePlaceShape::FrameIndexedByRegion { index_region, .. },
            ) => omega_isa_aarch64::runtime_frame_indexed_integer_write_clobbers(index_region),
            (
                omega_target::Architecture::X86_64,
                crate::WritePlaceShape::FrameIndexed { .. }
                | crate::WritePlaceShape::FrameIndexedByRegion { .. },
            ) => omega_isa_x86_64::place_integer_write_clobbers(target),
            (
                omega_target::Architecture::Aarch64,
                crate::WritePlaceShape::FrameBaseIndexed { .. },
            ) => omega_isa_aarch64::runtime_frame_base_indexed_integer_write_clobbers(),
            (
                omega_target::Architecture::X86_64,
                crate::WritePlaceShape::FrameBaseIndexed { .. },
            ) => omega_isa_x86_64::place_integer_write_clobbers(target),
            (
                omega_target::Architecture::Aarch64,
                crate::WritePlaceShape::MachineIndexed { .. },
            ) => omega_isa_aarch64::runtime_machine_indexed_integer_write_clobbers(),
            (omega_target::Architecture::X86_64, crate::WritePlaceShape::MachineIndexed { .. }) => {
                omega_isa_x86_64::place_integer_write_clobbers(target)
            }
            (
                omega_target::Architecture::Aarch64,
                crate::WritePlaceShape::MachineDoubleIndexed {
                    outer_index_region,
                    inner_index_region,
                    ..
                },
            ) => omega_isa_aarch64::runtime_machine_double_indexed_integer_write_clobbers(
                outer_index_region,
                inner_index_region,
            ),
            (
                omega_target::Architecture::X86_64,
                crate::WritePlaceShape::MachineDoubleIndexed { .. },
            ) => omega_isa_x86_64::place_integer_write_clobbers(target),
            (omega_target::Architecture::X86_64, crate::WritePlaceShape::Unsupported) => {
                omega_isa_x86_64::place_integer_write_clobbers(target)
            }
            (omega_target::Architecture::Aarch64, crate::WritePlaceShape::Unsupported)
                if frame_double.is_some() =>
            {
                omega_isa_aarch64::runtime_frame_base_double_indexed_integer_write_clobbers()
            }
            _ => continue,
        };
        registers.extend_from_slice(clobbers.as_slice());
    }
    let evidence =
        StateFootprintEvidence::new(RegisterSet::new(registers), MachineStateSet::empty());
    validate_state_footprint(boundary, &evidence)?;
    Ok(evidence)
}

/// Derive the exact scratch and machine-state footprint of compiler-body
/// address writes. These operations materialize the address of one canonical
/// `Place` and store it into a runtime-frame pointer slot.
pub fn derive_boundary_compiler_body_place_address_write_footprint<'instruction>(
    boundary: &ValidatedBoundaryEntryPlan,
    instructions: impl IntoIterator<Item = &'instruction SelectedInstructionKind>,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    let architecture = boundary.plan().call.policy.architecture();
    let mut registers = Vec::new();
    let mut additional_state = MachineStateSet::empty();
    for instruction in instructions {
        let SelectedInstructionKind::WritePlaceAddress {
            source,
            target_offset,
        } = instruction
        else {
            continue;
        };
        let Ok(clobbers) =
            crate::write_place_address_register_writes(architecture, source, *target_offset)
        else {
            continue;
        };
        registers.extend_from_slice(clobbers.as_slice());
        additional_state = additional_state.union(
            crate::write_place_address_additional_machine_state(architecture),
        );
    }
    let evidence = StateFootprintEvidence::new(RegisterSet::new(registers), additional_state);
    validate_state_footprint(boundary, &evidence)?;
    Ok(evidence)
}

/// Derive the exact scratch footprint of per-target constant host results.
/// These rows materialize a value directly into runtime storage and never
/// cross a foreign-call boundary.
pub fn derive_boundary_compiler_body_constant_host_result_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    use omega_abstract_operations::{AbstractOperationKind, InstructionOperandKind};
    use omega_calling_conventions::PlatformCallData;

    let architecture = boundary.plan().call.policy.architecture();
    let mut registers = Vec::new();
    for instruction in instructions {
        let AbstractOperationKind::HostOperation {
            operation_ordinal,
            operands: operand_span,
        } = &instruction.kind
        else {
            continue;
        };
        let Some((_, host_call)) = input.host_calls.calls.iter().find(|(_, host_call)| {
            host_call.source_key == instruction.source_key
                && host_call.statement_index == instruction.source_statement
        }) else {
            continue;
        };
        if !matches!(host_call.data, PlatformCallData::ConstantResult { .. }) {
            continue;
        }
        let Some(operation) = input
            .host_calls
            .operations
            .span(host_call.operations)
            .and_then(|operations| operations.get(usize::from(*operation_ordinal)))
        else {
            continue;
        };
        if !operation.operation_key.lowers_to_constant_result() {
            continue;
        }
        let Some(omega_abstract_operations::InstructionOperand {
            kind:
                InstructionOperandKind::RuntimeScalarInteger {
                    byte_offset,
                    byte_count,
                    ..
                },
        }) = operands
            .span(*operand_span)
            .and_then(|operands| operands.first())
        else {
            continue;
        };
        let clobbers = match architecture {
            omega_target::Architecture::X86_64 => omega_isa_x86_64::constant_host_result_clobbers(),
            omega_target::Architecture::Aarch64 => {
                omega_isa_aarch64::constant_host_result_clobbers(*byte_offset, *byte_count)
            }
        };
        registers.extend_from_slice(clobbers.as_slice());
    }
    let evidence =
        StateFootprintEvidence::new(RegisterSet::new(registers), MachineStateSet::empty());
    validate_state_footprint(boundary, &evidence)?;
    Ok(evidence)
}

/// Derive the semantic leaf footprint of simple outbound syscalls. The target
/// encoder is constrained by the same retained `CallPlan`; the supervisor may
/// realize any ordinary clobber admitted by that plan.
pub fn derive_boundary_compiler_body_outbound_syscall_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    use omega_abstract_operations::{AbstractOperationKind, InstructionOperandKind};
    use omega_calling_conventions::{EntryControl, HostBindingMechanism};

    let mut registers = Vec::new();
    let mut has_syscall = false;
    for instruction in instructions {
        let AbstractOperationKind::HostOperation {
            operation_ordinal,
            operands: operand_span,
        } = &instruction.kind
        else {
            continue;
        };
        let Some((_, host_call)) = input.host_calls.calls.iter().find(|(_, host_call)| {
            host_call.source_key == instruction.source_key
                && host_call.statement_index == instruction.source_statement
        }) else {
            continue;
        };
        let Some(operation) = input
            .host_calls
            .operations
            .span(host_call.operations)
            .and_then(|operations| operations.get(usize::from(*operation_ordinal)))
        else {
            continue;
        };
        let Some((_, binding)) = input
            .host_abi
            .bindings
            .iter()
            .find(|(_, binding)| binding.operation_key == operation.operation_key)
        else {
            continue;
        };
        if !matches!(binding.mechanism, HostBindingMechanism::Syscall { .. })
            || operation.operation_key.uses_linux_timespec_result()
            || operation.operation_key.uses_linux_timespec_argument()
            || binding.call_plan().result.is_some()
            || !matches!(
                binding.call_plan().entry_control,
                EntryControl::SupervisorCall { .. }
            )
            || !operands.span(*operand_span).is_some_and(|operands| {
                !operands.is_empty()
                    && operands.iter().all(|operand| {
                        matches!(
                            operand.kind,
                            InstructionOperandKind::ImmediateInteger(_)
                                | InstructionOperandKind::ByteLength(_)
                        )
                    })
            })
        {
            continue;
        }
        has_syscall = true;
        registers.extend_from_slice(binding.call_plan().ordinary_clobbers.as_slice());
    }
    let evidence = StateFootprintEvidence::new(
        RegisterSet::new(registers),
        if has_syscall {
            MachineStateSet::new([
                MachineState::Flags,
                MachineState::InstructionPointer,
                MachineState::ControlState,
            ])
        } else {
            MachineStateSet::empty()
        },
    );
    omega_calling_conventions::validate_outbound_call_footprint(boundary, &evidence)?;
    Ok(evidence)
}

/// Derive the footprint of ordinary built-in imports whose complete source
/// boundary is a non-empty list of immediate integer arguments and no result.
/// The foreign-control envelope is part of the same instruction program, so
/// its stack/control-state writes and AArch64 x16 scratch are retained here in
/// addition to the selected call plan's ordinary foreign-call clobbers.
pub fn derive_boundary_compiler_body_outbound_immediate_import_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    derive_boundary_compiler_body_outbound_direct_import_footprint(
        boundary,
        input,
        operands,
        instructions,
        DirectImportArgumentClass::Immediate,
    )
}

/// Derive the companion no-result import class that loads one or more scalar
/// arguments from runtime storage. Exact storage relocations are retained at
/// machine emission; the semantic leaf is otherwise the same wrapped foreign
/// call ceiling as the immediate-only class.
pub fn derive_boundary_compiler_body_outbound_storage_import_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    derive_boundary_compiler_body_outbound_direct_import_footprint(
        boundary,
        input,
        operands,
        instructions,
        DirectImportArgumentClass::Storage,
    )
}

/// Derive integer-result built-in imports whose actual arguments are all
/// immediate integers. The leading runtime scalar is the post-call result
/// store, not a wire argument.
pub fn derive_boundary_compiler_body_outbound_immediate_import_result_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    derive_boundary_compiler_body_outbound_direct_import_footprint(
        boundary,
        input,
        operands,
        instructions,
        DirectImportArgumentClass::ImmediateResult,
    )
}

/// Derive built-in imports with one or more runtime float parameters and a
/// direct scalar result. Integer-returning rounding and float-returning math
/// operations share the same storage/control envelope.
pub fn derive_boundary_compiler_body_outbound_float_import_result_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    derive_boundary_compiler_body_outbound_direct_import_footprint(
        boundary,
        input,
        operands,
        instructions,
        DirectImportArgumentClass::FloatResult,
    )
}

/// Derive the built-in errno accessor shape whose imported pointer result is
/// dereferenced once before its integer value is stored. The operation has no
/// wire arguments; its leading runtime scalar is solely the result store.
pub fn derive_boundary_compiler_body_outbound_dereferenced_import_result_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    derive_boundary_compiler_body_outbound_direct_import_footprint(
        boundary,
        input,
        operands,
        instructions,
        DirectImportArgumentClass::DereferencedResult,
    )
}

/// Derive no-result built-in imports whose ordinary scalar parameter list
/// includes at least one compiler-owned static data address.
pub fn derive_boundary_compiler_body_outbound_data_import_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    derive_boundary_compiler_body_outbound_direct_import_footprint(
        boundary,
        input,
        operands,
        instructions,
        DirectImportArgumentClass::Data,
    )
}

/// Derive direct-integer-result built-in imports whose ordinary scalar
/// parameter list includes at least one compiler-owned static data address.
pub fn derive_boundary_compiler_body_outbound_data_import_result_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    derive_boundary_compiler_body_outbound_direct_import_footprint(
        boundary,
        input,
        operands,
        instructions,
        DirectImportArgumentClass::DataResult,
    )
}

/// Derive scalar source-authored imports whose retained canonical call plan is
/// the sole placement authority. This no-result subset accepts integer and
/// compiler-owned data-address parameters.
pub fn derive_boundary_compiler_body_outbound_authored_import_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    derive_boundary_compiler_body_outbound_direct_import_footprint(
        boundary,
        input,
        operands,
        instructions,
        DirectImportArgumentClass::Authored,
    )
}

/// Derive the direct-integer-result companion to scalar source-authored
/// imports. The leading runtime scalar is the result root, never an argument.
pub fn derive_boundary_compiler_body_outbound_authored_import_result_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    derive_boundary_compiler_body_outbound_direct_import_footprint(
        boundary,
        input,
        operands,
        instructions,
        DirectImportArgumentClass::AuthoredResult,
    )
}

/// Derive source-authored no-result imports with at least one runtime-float
/// parameter. Integer and static-data parameters may share the retained plan.
pub fn derive_boundary_compiler_body_outbound_authored_float_import_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    derive_boundary_compiler_body_outbound_direct_import_footprint(
        boundary,
        input,
        operands,
        instructions,
        DirectImportArgumentClass::AuthoredFloat,
    )
}

/// Derive source-authored scalar imports with a float result or at least one
/// runtime-float parameter and a direct integer/float result.
pub fn derive_boundary_compiler_body_outbound_authored_float_import_result_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    derive_boundary_compiler_body_outbound_direct_import_footprint(
        boundary,
        input,
        operands,
        instructions,
        DirectImportArgumentClass::AuthoredFloatResult,
    )
}

/// Derive source-authored no-result imports with at least one by-value
/// aggregate parameter. The retained plan owns direct, fragmented, stack, or
/// caller-copy placement for that one source operand.
pub fn derive_boundary_compiler_body_outbound_authored_aggregate_import_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    derive_boundary_compiler_body_outbound_direct_import_footprint(
        boundary,
        input,
        operands,
        instructions,
        DirectImportArgumentClass::AuthoredAggregate,
    )
}

/// Derive source-authored imports with at least one by-value aggregate
/// parameter and one direct integer/float result.
pub fn derive_boundary_compiler_body_outbound_authored_aggregate_import_result_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    derive_boundary_compiler_body_outbound_direct_import_footprint(
        boundary,
        input,
        operands,
        instructions,
        DirectImportArgumentClass::AuthoredAggregateResult,
    )
}

/// Derive source-authored imports whose result remains one aggregate storage
/// operand while the selected plan owns its direct fragments or hidden
/// destination pointer.
pub fn derive_boundary_compiler_body_outbound_authored_aggregate_result_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    derive_boundary_compiler_body_outbound_direct_import_footprint(
        boundary,
        input,
        operands,
        instructions,
        DirectImportArgumentClass::AuthoredAggregateReturning,
    )
}

/// Derive Darwin's concrete variadic `open(path, flags, mode)` adapter. The
/// retained call plan owns the fixed/anonymous boundary and outgoing mode slot.
pub fn derive_boundary_compiler_body_outbound_open_create_import_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    derive_boundary_compiler_body_outbound_direct_import_footprint(
        boundary,
        input,
        operands,
        instructions,
        DirectImportArgumentClass::OpenCreate,
    )
}

/// Derive the retained stdin byte adapter. This is a composite implementation
/// leaf, not a second language boundary: Linux owns one normalized read
/// syscall plan, Darwin one AAPCS64 read plan, and Win64 the complete
/// GetStdHandle + ReadFile pair.
pub fn derive_boundary_compiler_body_runtime_byte_read_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    instructions: &[omega_abstract_operations::AbstractOperation],
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    derive_boundary_compiler_body_runtime_byte_footprint(boundary, input, instructions, true)
}

/// Derive the retained stdout byte adapter under the same target-owned plan
/// rules as [`derive_boundary_compiler_body_runtime_byte_read_footprint`].
pub fn derive_boundary_compiler_body_runtime_byte_write_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    instructions: &[omega_abstract_operations::AbstractOperation],
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    derive_boundary_compiler_body_runtime_byte_footprint(boundary, input, instructions, false)
}

/// Derive the complete target-owned line-read adapter without exposing its
/// byte-at-a-time native subcalls as an outer Omega ABI.
pub fn derive_boundary_compiler_body_runtime_line_read_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    instructions: &[omega_abstract_operations::AbstractOperation],
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    use omega_abstract_operations::{AbstractOperationKind, RuntimeTextReadTarget};
    use omega_calling_conventions::{
        HostBindingMechanism, HostCapability, HostOperation, HostOperationKey, MachineRegister,
    };

    let operation_key = HostOperationKey::new(HostCapability::Stdin, HostOperation::Read);
    let binding = input
        .host_abi
        .bindings
        .iter()
        .find_map(|(_, binding)| (binding.operation_key == operation_key).then_some(binding));
    let mut registers = Vec::new();
    let mut has_adapter = false;
    let mut has_import = false;
    for instruction in instructions {
        let (target_offset, target) = match &instruction.kind {
            AbstractOperationKind::ReadRuntimeTextLine {
                target_offset,
                target,
                ..
            } => (*target_offset, *target),
            _ => continue,
        };
        let Some(binding) = binding else {
            continue;
        };
        if !matches!(
            binding.mechanism,
            HostBindingMechanism::Import { .. } | HostBindingMechanism::Syscall { .. }
        ) {
            continue;
        }
        has_adapter = true;
        let is_import = matches!(binding.mechanism, HostBindingMechanism::Import { .. });
        has_import |= is_import;
        registers.extend_from_slice(binding.call_plan().ordinary_clobbers.as_slice());
        match input.target.architecture {
            omega_target::Architecture::X86_64 => {
                registers.extend([MachineRegister::X86R14, MachineRegister::X86R15]);
                if is_import || target == RuntimeTextReadTarget::StringDescriptor {
                    registers.push(MachineRegister::X86R13);
                }
                if is_import {
                    registers.push(MachineRegister::X86Rsp);
                    let handle_key = HostOperationKey::new(
                        operation_key.capability,
                        HostOperation::GetStdHandle,
                    );
                    if let Some(handle_binding) =
                        input.host_abi.bindings.iter().find_map(|(_, candidate)| {
                            (candidate.operation_key == handle_key).then_some(candidate)
                        })
                    {
                        registers.extend_from_slice(
                            handle_binding.call_plan().ordinary_clobbers.as_slice(),
                        );
                    }
                }
            }
            omega_target::Architecture::Aarch64 => {
                registers.extend([
                    MachineRegister::Aarch64X(20),
                    MachineRegister::Aarch64X(21),
                    MachineRegister::Aarch64X(22),
                    MachineRegister::Aarch64X(24),
                ]);
                match target {
                    RuntimeTextReadTarget::StringDescriptor => {
                        registers.push(MachineRegister::Aarch64X(16));
                        let direct_descriptor_stores = (target_offset + 8).is_multiple_of(8)
                            && (target_offset + 8) / 8 <= 4095;
                        if !direct_descriptor_stores && target_offset > 4095 {
                            registers.push(MachineRegister::Aarch64X(9));
                        }
                    }
                    RuntimeTextReadTarget::BoundedByteBuffer => {
                        if target_offset + 8 > 4095 {
                            registers.push(MachineRegister::Aarch64X(19));
                        }
                    }
                    RuntimeTextReadTarget::FixedByteArray => {
                        if target_offset > 4095 {
                            registers.push(MachineRegister::Aarch64X(19));
                        }
                    }
                }
            }
        }
    }
    let mut machine_state = if has_adapter {
        MachineStateSet::new([
            MachineState::Flags,
            MachineState::InstructionPointer,
            MachineState::ControlState,
        ])
    } else {
        MachineStateSet::empty()
    };
    if has_import {
        machine_state = machine_state.union(MachineStateSet::new([MachineState::StackPointer]));
    }
    let evidence = StateFootprintEvidence::new(RegisterSet::new(registers), machine_state);
    omega_calling_conventions::validate_outbound_call_footprint(boundary, &evidence)?;
    Ok(evidence)
}

fn derive_boundary_compiler_body_runtime_byte_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    instructions: &[omega_abstract_operations::AbstractOperation],
    read: bool,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    use omega_abstract_operations::AbstractOperationKind;
    use omega_calling_conventions::{
        HostBindingMechanism, HostCapability, HostOperation, HostOperationKey, MachineRegister,
    };

    let operation_key = HostOperationKey::new(
        if read {
            HostCapability::Stdin
        } else {
            HostCapability::Stdout
        },
        if read {
            HostOperation::Read
        } else {
            HostOperation::Write
        },
    );
    let binding = input
        .host_abi
        .bindings
        .iter()
        .find_map(|(_, binding)| (binding.operation_key == operation_key).then_some(binding));
    let mut registers = Vec::new();
    let mut has_adapter = false;
    let mut has_import = false;
    for instruction in instructions {
        let source_offset = match (&instruction.kind, read) {
            (AbstractOperationKind::ReadRuntimeByte { .. }, true) => Some(0),
            (AbstractOperationKind::WriteRuntimeByte { source_offset, .. }, false) => {
                Some(*source_offset)
            }
            _ => None,
        };
        let Some(source_offset) = source_offset else {
            continue;
        };
        let Some(binding) = binding else {
            continue;
        };
        if !matches!(
            binding.mechanism,
            HostBindingMechanism::Import { .. } | HostBindingMechanism::Syscall { .. }
        ) {
            continue;
        }
        has_adapter = true;
        has_import |= matches!(binding.mechanism, HostBindingMechanism::Import { .. });
        registers.extend_from_slice(binding.call_plan().ordinary_clobbers.as_slice());
        match input.target.architecture {
            omega_target::Architecture::X86_64 => {
                registers.push(MachineRegister::X86R14);
                if matches!(binding.mechanism, HostBindingMechanism::Import { .. }) {
                    registers.push(MachineRegister::X86Rsp);
                    let get_std_handle_key = HostOperationKey::new(
                        operation_key.capability,
                        HostOperation::GetStdHandle,
                    );
                    if let Some(handle_binding) =
                        input.host_abi.bindings.iter().find_map(|(_, candidate)| {
                            (candidate.operation_key == get_std_handle_key).then_some(candidate)
                        })
                    {
                        registers.extend_from_slice(
                            handle_binding.call_plan().ordinary_clobbers.as_slice(),
                        );
                    }
                }
            }
            omega_target::Architecture::Aarch64 => {
                registers.push(MachineRegister::Aarch64X(20));
                if read || source_offset > 4095 {
                    registers.push(MachineRegister::Aarch64X(9));
                }
            }
        }
    }
    let mut machine_state = if has_adapter {
        MachineStateSet::new([
            MachineState::Flags,
            MachineState::InstructionPointer,
            MachineState::ControlState,
        ])
    } else {
        MachineStateSet::empty()
    };
    if has_import {
        machine_state = machine_state.union(MachineStateSet::new([MachineState::StackPointer]));
    }
    let evidence = StateFootprintEvidence::new(RegisterSet::new(registers), machine_state);
    omega_calling_conventions::validate_outbound_call_footprint(boundary, &evidence)?;
    Ok(evidence)
}

/// Derive integer-result built-in imports with one or more runtime-scalar
/// arguments. The leading runtime scalar remains the post-call result store;
/// only the trailing operands are wire arguments.
pub fn derive_boundary_compiler_body_outbound_storage_import_result_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    derive_boundary_compiler_body_outbound_direct_import_footprint(
        boundary,
        input,
        operands,
        instructions,
        DirectImportArgumentClass::StorageResult,
    )
}

#[derive(Clone, Copy)]
enum DirectImportArgumentClass {
    Immediate,
    Storage,
    ImmediateResult,
    FloatResult,
    DereferencedResult,
    Data,
    DataResult,
    Authored,
    AuthoredResult,
    AuthoredFloat,
    AuthoredFloatResult,
    AuthoredAggregate,
    AuthoredAggregateResult,
    AuthoredAggregateReturning,
    OpenCreate,
    StorageResult,
}

fn is_runtime_aggregate_operand(kind: &omega_abstract_operations::InstructionOperandKind) -> bool {
    matches!(
        kind,
        omega_abstract_operations::InstructionOperandKind::RuntimeHomogeneousFloatAggregate { .. }
            | omega_abstract_operations::InstructionOperandKind::RuntimeSystemVAggregate { .. }
            | omega_abstract_operations::InstructionOperandKind::RuntimeSmallAggregate { .. }
            | omega_abstract_operations::InstructionOperandKind::RuntimeLargeAggregate { .. }
    )
}

fn derive_boundary_compiler_body_outbound_direct_import_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
    argument_class: DirectImportArgumentClass,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    use omega_abstract_operations::{AbstractOperationKind, InstructionOperandKind};
    use omega_calling_conventions::{
        EntryControl, HostBindingMechanism, HostCapability, MachineRegister,
    };

    let mut registers = Vec::new();
    let mut has_import = false;
    for instruction in instructions {
        let AbstractOperationKind::HostOperation {
            operation_ordinal,
            operands: operand_span,
        } = &instruction.kind
        else {
            continue;
        };
        let Some((_, host_call)) = input.host_calls.calls.iter().find(|(_, host_call)| {
            host_call.source_key == instruction.source_key
                && host_call.statement_index == instruction.source_statement
        }) else {
            continue;
        };
        let Some(operation) = input
            .host_calls
            .operations
            .span(host_call.operations)
            .and_then(|operations| operations.get(usize::from(*operation_ordinal)))
        else {
            continue;
        };
        let Some((_, binding)) = input
            .host_abi
            .bindings
            .iter()
            .find(|(_, binding)| binding.operation_key == operation.operation_key)
        else {
            continue;
        };
        let Some(selected_operands) = operands.span(*operand_span) else {
            continue;
        };
        if !matches!(binding.mechanism, HostBindingMechanism::Import { .. })
            || matches!(
                operation.operation_key.capability,
                HostCapability::Custom(_) | HostCapability::Unknown
            ) != matches!(
                argument_class,
                DirectImportArgumentClass::Authored
                    | DirectImportArgumentClass::AuthoredResult
                    | DirectImportArgumentClass::AuthoredFloat
                    | DirectImportArgumentClass::AuthoredFloatResult
                    | DirectImportArgumentClass::AuthoredAggregate
                    | DirectImportArgumentClass::AuthoredAggregateResult
                    | DirectImportArgumentClass::AuthoredAggregateReturning
            )
            || operation.operation_key.dereferences_result()
                != matches!(
                    argument_class,
                    DirectImportArgumentClass::DereferencedResult
                )
            || (input.target.architecture == omega_target::Architecture::Aarch64
                && matches!(
                    (
                        operation.operation_key.capability,
                        operation.operation_key.operation,
                    ),
                    (
                        HostCapability::Filesystem,
                        omega_calling_conventions::HostOperation::OpenCreate
                    )
                )
                && !matches!(argument_class, DirectImportArgumentClass::OpenCreate))
            || !matches!(binding.call_plan().entry_control, EntryControl::CallReturn)
            || selected_operands.is_empty()
            || match argument_class {
                DirectImportArgumentClass::Immediate => {
                    binding.call_plan().result.is_some()
                        || binding.call_plan().parameters.len() != selected_operands.len()
                        || selected_operands.iter().any(|operand| {
                            !matches!(operand.kind, InstructionOperandKind::ImmediateInteger(_))
                        })
                }
                DirectImportArgumentClass::Storage => {
                    binding.call_plan().result.is_some()
                        || binding.call_plan().parameters.len() != selected_operands.len()
                        || !selected_operands.iter().all(|operand| {
                            matches!(
                                operand.kind,
                                InstructionOperandKind::ImmediateInteger(_)
                                    | InstructionOperandKind::RuntimeScalarInteger { .. }
                            )
                        })
                        || !selected_operands.iter().any(|operand| {
                            matches!(
                                operand.kind,
                                InstructionOperandKind::RuntimeScalarInteger { .. }
                            )
                        })
                }
                DirectImportArgumentClass::ImmediateResult => {
                    !binding.call_plan().result.as_ref().is_some_and(|result| {
                        matches!(
                            result.shape.class,
                            omega_calling_conventions::ValueClass::Integer
                        )
                    }) || binding.call_plan().parameters.len() + 1 != selected_operands.len()
                        || !matches!(
                            selected_operands.first().map(|operand| &operand.kind),
                            Some(InstructionOperandKind::RuntimeScalarInteger { .. })
                        )
                        || selected_operands[1..].iter().any(|operand| {
                            !matches!(operand.kind, InstructionOperandKind::ImmediateInteger(_))
                        })
                }
                DirectImportArgumentClass::FloatResult => {
                    operation.operation_key.capability != HostCapability::Math
                        || !binding.call_plan().result.as_ref().is_some_and(|result| {
                            matches!(
                                result.shape.class,
                                omega_calling_conventions::ValueClass::Integer
                                    | omega_calling_conventions::ValueClass::Float
                            )
                        })
                        || binding.call_plan().parameters.len() + 1 != selected_operands.len()
                        || !matches!(
                            selected_operands.first().map(|operand| &operand.kind),
                            Some(InstructionOperandKind::RuntimeScalarInteger { .. })
                        )
                        || selected_operands[1..].is_empty()
                        || !selected_operands[1..].iter().all(|operand| {
                            matches!(
                                operand.kind,
                                InstructionOperandKind::RuntimeScalarFloat { .. }
                            )
                        })
                }
                DirectImportArgumentClass::DereferencedResult => {
                    !binding.call_plan().result.as_ref().is_some_and(|result| {
                        matches!(
                            result.shape.class,
                            omega_calling_conventions::ValueClass::Integer
                        )
                    }) || !binding.call_plan().parameters.is_empty()
                        || selected_operands.len() != 1
                        || !matches!(
                            selected_operands.first().map(|operand| &operand.kind),
                            Some(InstructionOperandKind::RuntimeScalarInteger { .. })
                        )
                }
                DirectImportArgumentClass::Data => {
                    binding.call_plan().result.is_some()
                        || binding.call_plan().parameters.len() != selected_operands.len()
                        || !selected_operands.iter().any(|operand| {
                            matches!(operand.kind, InstructionOperandKind::DataAddress { .. })
                        })
                        || !selected_operands.iter().all(|operand| {
                            matches!(
                                operand.kind,
                                InstructionOperandKind::ImmediateInteger(_)
                                    | InstructionOperandKind::RuntimeScalarInteger { .. }
                                    | InstructionOperandKind::DataAddress { .. }
                            )
                        })
                }
                DirectImportArgumentClass::DataResult => {
                    !binding.call_plan().result.as_ref().is_some_and(|result| {
                        matches!(
                            result.shape.class,
                            omega_calling_conventions::ValueClass::Integer
                        )
                    }) || binding.call_plan().parameters.len() + 1 != selected_operands.len()
                        || !matches!(
                            selected_operands.first().map(|operand| &operand.kind),
                            Some(InstructionOperandKind::RuntimeScalarInteger { .. })
                        )
                        || !selected_operands[1..].iter().any(|operand| {
                            matches!(operand.kind, InstructionOperandKind::DataAddress { .. })
                        })
                        || !selected_operands[1..].iter().all(|operand| {
                            matches!(
                                operand.kind,
                                InstructionOperandKind::ImmediateInteger(_)
                                    | InstructionOperandKind::RuntimeScalarInteger { .. }
                                    | InstructionOperandKind::DataAddress { .. }
                            )
                        })
                }
                DirectImportArgumentClass::Authored => {
                    binding.call_plan().result.is_some()
                        || binding.call_plan().parameters.len() != selected_operands.len()
                        || !selected_operands.iter().all(|operand| {
                            matches!(
                                operand.kind,
                                InstructionOperandKind::ImmediateInteger(_)
                                    | InstructionOperandKind::RuntimeScalarInteger { .. }
                                    | InstructionOperandKind::DataAddress { .. }
                            )
                        })
                }
                DirectImportArgumentClass::AuthoredResult => {
                    !binding.call_plan().result.as_ref().is_some_and(|result| {
                        matches!(
                            result.shape.class,
                            omega_calling_conventions::ValueClass::Integer
                        )
                    }) || binding.call_plan().parameters.len() + 1 != selected_operands.len()
                        || !matches!(
                            selected_operands.first().map(|operand| &operand.kind),
                            Some(InstructionOperandKind::RuntimeScalarInteger { .. })
                        )
                        || !selected_operands[1..].iter().all(|operand| {
                            matches!(
                                operand.kind,
                                InstructionOperandKind::ImmediateInteger(_)
                                    | InstructionOperandKind::RuntimeScalarInteger { .. }
                                    | InstructionOperandKind::DataAddress { .. }
                            )
                        })
                }
                DirectImportArgumentClass::AuthoredFloat => {
                    binding.call_plan().result.is_some()
                        || binding.call_plan().parameters.len() != selected_operands.len()
                        || !selected_operands.iter().any(|operand| {
                            matches!(
                                operand.kind,
                                InstructionOperandKind::RuntimeScalarFloat { .. }
                            )
                        })
                        || !selected_operands.iter().all(|operand| {
                            matches!(
                                operand.kind,
                                InstructionOperandKind::ImmediateInteger(_)
                                    | InstructionOperandKind::RuntimeScalarInteger { .. }
                                    | InstructionOperandKind::RuntimeScalarFloat { .. }
                                    | InstructionOperandKind::DataAddress { .. }
                            )
                        })
                }
                DirectImportArgumentClass::AuthoredFloatResult => {
                    binding.call_plan().result.as_ref().map_or(true, |result| {
                        !matches!(
                            result.shape.class,
                            omega_calling_conventions::ValueClass::Integer
                                | omega_calling_conventions::ValueClass::Float
                        ) || binding.call_plan().parameters.len() + 1 != selected_operands.len()
                            || match result.shape.class {
                                omega_calling_conventions::ValueClass::Integer => !matches!(
                                    selected_operands.first().map(|operand| &operand.kind),
                                    Some(InstructionOperandKind::RuntimeScalarInteger { .. })
                                ),
                                omega_calling_conventions::ValueClass::Float => !matches!(
                                    selected_operands.first().map(|operand| &operand.kind),
                                    Some(InstructionOperandKind::RuntimeScalarFloat { .. })
                                ),
                                _ => true,
                            }
                            || (!matches!(
                                result.shape.class,
                                omega_calling_conventions::ValueClass::Float
                            ) && !selected_operands[1..].iter().any(|operand| {
                                matches!(
                                    operand.kind,
                                    InstructionOperandKind::RuntimeScalarFloat { .. }
                                )
                            }))
                            || !selected_operands[1..].iter().all(|operand| {
                                matches!(
                                    operand.kind,
                                    InstructionOperandKind::ImmediateInteger(_)
                                        | InstructionOperandKind::RuntimeScalarInteger { .. }
                                        | InstructionOperandKind::RuntimeScalarFloat { .. }
                                        | InstructionOperandKind::DataAddress { .. }
                                )
                            })
                    })
                }
                DirectImportArgumentClass::AuthoredAggregate => {
                    binding.call_plan().result.is_some()
                        || binding.call_plan().parameters.len() != selected_operands.len()
                        || !selected_operands
                            .iter()
                            .any(|operand| is_runtime_aggregate_operand(&operand.kind))
                        || !selected_operands.iter().all(|operand| {
                            matches!(
                                operand.kind,
                                InstructionOperandKind::ImmediateInteger(_)
                                    | InstructionOperandKind::RuntimeScalarInteger { .. }
                                    | InstructionOperandKind::RuntimeScalarFloat { .. }
                                    | InstructionOperandKind::RuntimeHomogeneousFloatAggregate { .. }
                                    | InstructionOperandKind::RuntimeSystemVAggregate { .. }
                                    | InstructionOperandKind::RuntimeSmallAggregate { .. }
                                    | InstructionOperandKind::RuntimeLargeAggregate { .. }
                                    | InstructionOperandKind::DataAddress { .. }
                            )
                        })
                }
                DirectImportArgumentClass::AuthoredAggregateResult => {
                    binding.call_plan().result.as_ref().map_or(true, |result| {
                        !matches!(
                            result.shape.class,
                            omega_calling_conventions::ValueClass::Integer
                                | omega_calling_conventions::ValueClass::Float
                        ) || binding.call_plan().parameters.len() + 1 != selected_operands.len()
                            || match result.shape.class {
                                omega_calling_conventions::ValueClass::Integer => !matches!(
                                    selected_operands.first().map(|operand| &operand.kind),
                                    Some(InstructionOperandKind::RuntimeScalarInteger { .. })
                                ),
                                omega_calling_conventions::ValueClass::Float => !matches!(
                                    selected_operands.first().map(|operand| &operand.kind),
                                    Some(InstructionOperandKind::RuntimeScalarFloat { .. })
                                ),
                                _ => true,
                            }
                            || !selected_operands[1..]
                                .iter()
                                .any(|operand| is_runtime_aggregate_operand(&operand.kind))
                            || !selected_operands[1..].iter().all(|operand| {
                                matches!(
                                    operand.kind,
                                    InstructionOperandKind::ImmediateInteger(_)
                                        | InstructionOperandKind::RuntimeScalarInteger { .. }
                                        | InstructionOperandKind::RuntimeScalarFloat { .. }
                                        | InstructionOperandKind::RuntimeHomogeneousFloatAggregate { .. }
                                        | InstructionOperandKind::RuntimeSystemVAggregate { .. }
                                        | InstructionOperandKind::RuntimeSmallAggregate { .. }
                                        | InstructionOperandKind::RuntimeLargeAggregate { .. }
                                        | InstructionOperandKind::DataAddress { .. }
                                )
                            })
                    })
                }
                DirectImportArgumentClass::AuthoredAggregateReturning => {
                    binding.call_plan().result.is_none()
                        || binding.call_plan().parameters.len() + 1 != selected_operands.len()
                        || !selected_operands
                            .first()
                            .is_some_and(|operand| is_runtime_aggregate_operand(&operand.kind))
                        || !selected_operands[1..].iter().all(|operand| {
                            matches!(
                                operand.kind,
                                InstructionOperandKind::ImmediateInteger(_)
                                    | InstructionOperandKind::RuntimeScalarInteger { .. }
                                    | InstructionOperandKind::RuntimeScalarFloat { .. }
                                    | InstructionOperandKind::RuntimeHomogeneousFloatAggregate { .. }
                                    | InstructionOperandKind::RuntimeSystemVAggregate { .. }
                                    | InstructionOperandKind::RuntimeSmallAggregate { .. }
                                    | InstructionOperandKind::RuntimeLargeAggregate { .. }
                                    | InstructionOperandKind::DataAddress { .. }
                            )
                        })
                }
                DirectImportArgumentClass::OpenCreate => {
                    input.target.architecture != omega_target::Architecture::Aarch64
                        || !matches!(
                            (
                                operation.operation_key.capability,
                                operation.operation_key.operation,
                            ),
                            (
                                HostCapability::Filesystem,
                                omega_calling_conventions::HostOperation::OpenCreate
                            )
                        )
                        || !binding.call_plan().result.as_ref().is_some_and(|result| {
                            matches!(
                                result.shape.class,
                                omega_calling_conventions::ValueClass::Integer
                            )
                        })
                        || binding.call_plan().parameters.len() != 3
                        || !matches!(
                            selected_operands,
                            [
                                omega_abstract_operations::InstructionOperand {
                                    kind: InstructionOperandKind::RuntimeScalarInteger { .. }
                                },
                                omega_abstract_operations::InstructionOperand {
                                    kind: InstructionOperandKind::DataAddress { .. }
                                        | InstructionOperandKind::RuntimeStringPointer { .. }
                                        | InstructionOperandKind::RuntimePointeeStringPointer { .. }
                                        | InstructionOperandKind::RuntimeStorageAddress { .. }
                                },
                                omega_abstract_operations::InstructionOperand {
                                    kind: InstructionOperandKind::ImmediateInteger(_)
                                        | InstructionOperandKind::RuntimeScalarInteger { .. }
                                },
                                omega_abstract_operations::InstructionOperand {
                                    kind: InstructionOperandKind::ImmediateInteger(_)
                                },
                            ]
                        )
                }
                DirectImportArgumentClass::StorageResult => {
                    !binding.call_plan().result.as_ref().is_some_and(|result| {
                        matches!(
                            result.shape.class,
                            omega_calling_conventions::ValueClass::Integer
                        )
                    }) || binding.call_plan().parameters.len() + 1 != selected_operands.len()
                        || !matches!(
                            selected_operands.first().map(|operand| &operand.kind),
                            Some(InstructionOperandKind::RuntimeScalarInteger { .. })
                        )
                        || !selected_operands[1..].iter().all(|operand| {
                            matches!(
                                operand.kind,
                                InstructionOperandKind::ImmediateInteger(_)
                                    | InstructionOperandKind::RuntimeScalarInteger { .. }
                            )
                        })
                        || !selected_operands[1..].iter().any(|operand| {
                            matches!(
                                operand.kind,
                                InstructionOperandKind::RuntimeScalarInteger { .. }
                            )
                        })
                }
            }
        {
            continue;
        }
        has_import = true;
        registers.extend_from_slice(binding.call_plan().ordinary_clobbers.as_slice());
        match input.target.architecture {
            omega_target::Architecture::X86_64 => registers.push(MachineRegister::X86Rsp),
            omega_target::Architecture::Aarch64 => {
                registers.push(MachineRegister::Aarch64X(16));
                if matches!(
                    argument_class,
                    DirectImportArgumentClass::ImmediateResult
                        | DirectImportArgumentClass::FloatResult
                        | DirectImportArgumentClass::DereferencedResult
                        | DirectImportArgumentClass::DataResult
                        | DirectImportArgumentClass::AuthoredResult
                        | DirectImportArgumentClass::AuthoredFloatResult
                        | DirectImportArgumentClass::AuthoredAggregateResult
                        | DirectImportArgumentClass::OpenCreate
                        | DirectImportArgumentClass::StorageResult
                ) {
                    let result_range = selected_operands.first().and_then(|operand| match &operand
                        .kind
                    {
                        InstructionOperandKind::RuntimeScalarInteger {
                            byte_offset,
                            byte_count,
                            ..
                        }
                        | InstructionOperandKind::RuntimeScalarFloat {
                            byte_offset,
                            byte_count,
                            ..
                        } => Some((*byte_offset, *byte_count)),
                        _ => None,
                    });
                    if let Some((byte_offset, byte_count)) = result_range {
                        registers.extend_from_slice(
                            omega_isa_aarch64::constant_host_result_clobbers(
                                byte_offset,
                                byte_count,
                            )
                            .as_slice(),
                        );
                    }
                }
            }
        }
    }
    let evidence = StateFootprintEvidence::new(
        RegisterSet::new(registers),
        if has_import {
            MachineStateSet::new([
                MachineState::Flags,
                MachineState::InstructionPointer,
                MachineState::StackPointer,
                MachineState::ControlState,
            ])
        } else {
            MachineStateSet::empty()
        },
    );
    omega_calling_conventions::validate_outbound_call_footprint(boundary, &evidence)?;
    Ok(evidence)
}

fn abstract_outbound_syscall_storage_argument_is_closed(
    architecture: omega_target::Architecture,
    operand: &omega_abstract_operations::InstructionOperand,
) -> bool {
    use omega_abstract_operations::InstructionOperandKind;

    match operand.kind {
        InstructionOperandKind::RuntimeStringPointer {
            byte_offset,
            is_bounded_buffer: true,
            ..
        } => {
            architecture == omega_target::Architecture::X86_64
                || byte_offset
                    .checked_add(8)
                    .is_some_and(|content_offset| content_offset <= 4095)
        }
        InstructionOperandKind::RuntimeStringPointer { .. }
        | InstructionOperandKind::RuntimeStringLength { .. }
        | InstructionOperandKind::RuntimePointeeStringPointer { .. }
        | InstructionOperandKind::RuntimePointeeStringLength { .. }
        | InstructionOperandKind::RuntimeScalarInteger { .. }
        | InstructionOperandKind::RuntimeStorageAddress { .. } => true,
        _ => false,
    }
}

fn abstract_outbound_syscall_data_argument_is_closed(
    operand: &omega_abstract_operations::InstructionOperand,
) -> bool {
    matches!(
        operand.kind,
        omega_abstract_operations::InstructionOperandKind::DataAddress { .. }
    )
}

/// Derive no-result outbound syscall leaves that marshal one or more values,
/// descriptor fields, or addresses from runtime storage. Their marshallers use
/// only the normalized syscall plan's ordinary-clobber set; exact storage
/// relocations are retained later beside the encoded instruction.
pub fn derive_boundary_compiler_body_outbound_syscall_storage_arguments_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    derive_boundary_compiler_body_outbound_syscall_relocatable_arguments_footprint(
        boundary,
        input,
        operands,
        instructions,
        false,
    )
}

/// Derive no-result outbound syscall leaves with at least one exact static
/// data-object address. Other parameters may be immediate or use the already
/// closed runtime-storage forms; the final validator retains both relocation
/// target classes independently.
pub fn derive_boundary_compiler_body_outbound_syscall_data_arguments_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    derive_boundary_compiler_body_outbound_syscall_relocatable_arguments_footprint(
        boundary,
        input,
        operands,
        instructions,
        true,
    )
}

fn derive_boundary_compiler_body_outbound_syscall_relocatable_arguments_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
    requires_data_argument: bool,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    use omega_abstract_operations::{AbstractOperationKind, InstructionOperandKind};
    use omega_calling_conventions::{EntryControl, HostBindingMechanism};

    let mut registers = Vec::new();
    let mut has_syscall = false;
    for instruction in instructions {
        let AbstractOperationKind::HostOperation {
            operation_ordinal,
            operands: operand_span,
        } = &instruction.kind
        else {
            continue;
        };
        let Some((_, host_call)) = input.host_calls.calls.iter().find(|(_, host_call)| {
            host_call.source_key == instruction.source_key
                && host_call.statement_index == instruction.source_statement
        }) else {
            continue;
        };
        let Some(operation) = input
            .host_calls
            .operations
            .span(host_call.operations)
            .and_then(|operations| operations.get(usize::from(*operation_ordinal)))
        else {
            continue;
        };
        let Some((_, binding)) = input
            .host_abi
            .bindings
            .iter()
            .find(|(_, binding)| binding.operation_key == operation.operation_key)
        else {
            continue;
        };
        let Some(arguments) = operands.span(*operand_span) else {
            continue;
        };
        let has_storage = arguments.iter().any(|operand| {
            abstract_outbound_syscall_storage_argument_is_closed(input.target.architecture, operand)
        });
        let has_data = arguments
            .iter()
            .any(abstract_outbound_syscall_data_argument_is_closed);
        if !matches!(binding.mechanism, HostBindingMechanism::Syscall { .. })
            || operation.operation_key.uses_linux_timespec_result()
            || operation.operation_key.uses_linux_timespec_argument()
            || binding.call_plan().result.is_some()
            || binding.call_plan().parameters.len() != arguments.len()
            || !matches!(
                binding.call_plan().entry_control,
                EntryControl::SupervisorCall { .. }
            )
            || if requires_data_argument {
                !has_data
            } else {
                !has_storage || has_data
            }
            || !arguments.iter().all(|operand| {
                matches!(
                    operand.kind,
                    InstructionOperandKind::ImmediateInteger(_)
                        | InstructionOperandKind::ByteLength(_)
                ) || abstract_outbound_syscall_storage_argument_is_closed(
                    input.target.architecture,
                    operand,
                ) || abstract_outbound_syscall_data_argument_is_closed(operand)
            })
        {
            continue;
        }
        has_syscall = true;
        registers.extend_from_slice(binding.call_plan().ordinary_clobbers.as_slice());
    }
    let evidence = StateFootprintEvidence::new(
        RegisterSet::new(registers),
        if has_syscall {
            MachineStateSet::new([
                MachineState::Flags,
                MachineState::InstructionPointer,
                MachineState::ControlState,
            ])
        } else {
            MachineStateSet::empty()
        },
    );
    omega_calling_conventions::validate_outbound_call_footprint(boundary, &evidence)?;
    Ok(evidence)
}

/// Derive the first result-bearing outbound syscall leaf. This deliberately
/// covers only a runtime-scalar destination followed by immediate/byte-length
/// parameters; relocatable parameters and composite adapters retain separate
/// footprint classes. AArch64's post-call store owns x16 and, for a large or
/// unscaled destination offset, x17 in addition to the syscall plan ceiling.
pub fn derive_boundary_compiler_body_outbound_syscall_result_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    derive_boundary_compiler_body_outbound_syscall_result_footprint_for_arguments(
        boundary,
        input,
        operands,
        instructions,
        OutboundSyscallResultArgumentClass::Immediate,
    )
}

/// Derive result-bearing outbound syscalls whose ordinary parameters include
/// one or more of the closed runtime-storage forms. The plan still owns the
/// syscall marshaller; AArch64's post-call destination materializer contributes
/// its offset-sensitive x16/x17 scratch separately.
pub fn derive_boundary_compiler_body_outbound_syscall_result_storage_arguments_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    derive_boundary_compiler_body_outbound_syscall_result_footprint_for_arguments(
        boundary,
        input,
        operands,
        instructions,
        OutboundSyscallResultArgumentClass::Storage,
    )
}

/// Derive result-bearing outbound syscall leaves with at least one exact
/// static data-object address and any otherwise-closed runtime-storage or
/// immediate parameters.
pub fn derive_boundary_compiler_body_outbound_syscall_result_data_arguments_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    derive_boundary_compiler_body_outbound_syscall_result_footprint_for_arguments(
        boundary,
        input,
        operands,
        instructions,
        OutboundSyscallResultArgumentClass::Data,
    )
}

#[derive(Clone, Copy)]
enum OutboundSyscallResultArgumentClass {
    Immediate,
    Storage,
    Data,
}

#[derive(Clone, Copy)]
enum OutboundSyscallTimespecClass {
    Argument,
    Result,
}

/// Derive the Linux nanosleep adapter leaf. The concrete two-pointer syscall
/// plan owns the supervisor boundary while the compiler-owned request builder
/// additionally mutates balanced stack state and target-specific arithmetic
/// scratch.
pub fn derive_boundary_compiler_body_outbound_syscall_timespec_argument_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    derive_boundary_compiler_body_outbound_syscall_timespec_footprint(
        boundary,
        input,
        operands,
        instructions,
        OutboundSyscallTimespecClass::Argument,
    )
}

/// Derive the Linux clock_gettime adapter leaf. Its private two-word result is
/// reduced to nanoseconds and stored into the semantic scalar destination.
pub fn derive_boundary_compiler_body_outbound_syscall_timespec_result_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    derive_boundary_compiler_body_outbound_syscall_timespec_footprint(
        boundary,
        input,
        operands,
        instructions,
        OutboundSyscallTimespecClass::Result,
    )
}

fn derive_boundary_compiler_body_outbound_syscall_timespec_footprint(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
    class: OutboundSyscallTimespecClass,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    use omega_abstract_operations::{AbstractOperationKind, InstructionOperandKind};
    use omega_calling_conventions::{EntryControl, HostBindingMechanism, MachineRegister};

    let mut registers = Vec::new();
    let mut has_syscall = false;
    for instruction in instructions {
        let AbstractOperationKind::HostOperation {
            operation_ordinal,
            operands: operand_span,
        } = &instruction.kind
        else {
            continue;
        };
        let Some((_, host_call)) = input.host_calls.calls.iter().find(|(_, host_call)| {
            host_call.source_key == instruction.source_key
                && host_call.statement_index == instruction.source_statement
        }) else {
            continue;
        };
        let Some(operation) = input
            .host_calls
            .operations
            .span(host_call.operations)
            .and_then(|operations| operations.get(usize::from(*operation_ordinal)))
        else {
            continue;
        };
        let Some((_, binding)) = input
            .host_abi
            .bindings
            .iter()
            .find(|(_, binding)| binding.operation_key == operation.operation_key)
        else {
            continue;
        };
        let Some(call_operands) = operands.span(*operand_span) else {
            continue;
        };
        let shape_matches = match (class, call_operands) {
            (
                OutboundSyscallTimespecClass::Result,
                [
                    omega_abstract_operations::InstructionOperand {
                        kind: InstructionOperandKind::RuntimeScalarInteger { byte_count: 8, .. },
                    },
                    omega_abstract_operations::InstructionOperand {
                        kind: InstructionOperandKind::ImmediateInteger(_),
                    },
                ],
            ) => true,
            (
                OutboundSyscallTimespecClass::Argument,
                [
                    omega_abstract_operations::InstructionOperand {
                        kind:
                            InstructionOperandKind::RuntimeScalarInteger {
                                byte_count: 4 | 8, ..
                            }
                            | InstructionOperandKind::ImmediateInteger(0..),
                    },
                ],
            ) => true,
            _ => false,
        };
        let operation_matches = match class {
            OutboundSyscallTimespecClass::Argument => {
                operation.operation_key.uses_linux_timespec_argument()
            }
            OutboundSyscallTimespecClass::Result => {
                operation.operation_key.uses_linux_timespec_result()
            }
        };
        if !operation_matches
            || !shape_matches
            || !matches!(binding.mechanism, HostBindingMechanism::Syscall { .. })
            || binding.call_plan().parameters.len() != 2
            || binding.call_plan().result.is_none()
            || !matches!(
                binding.call_plan().entry_control,
                EntryControl::SupervisorCall { .. }
            )
        {
            continue;
        }
        has_syscall = true;
        registers.extend_from_slice(binding.call_plan().ordinary_clobbers.as_slice());
        match (input.target.architecture, class, call_operands) {
            (omega_target::Architecture::X86_64, OutboundSyscallTimespecClass::Result, _) => {
                registers.push(MachineRegister::X86Rsp)
            }
            (omega_target::Architecture::X86_64, OutboundSyscallTimespecClass::Argument, _) => {
                registers.extend([MachineRegister::X86Rdx, MachineRegister::X86Rsp])
            }
            (
                omega_target::Architecture::Aarch64,
                OutboundSyscallTimespecClass::Result,
                [
                    omega_abstract_operations::InstructionOperand {
                        kind:
                            InstructionOperandKind::RuntimeScalarInteger {
                                byte_offset,
                                byte_count,
                                ..
                            },
                    },
                    _,
                ],
            ) => registers.extend_from_slice(
                omega_isa_aarch64::constant_host_result_clobbers(*byte_offset, *byte_count)
                    .as_slice(),
            ),
            _ => {}
        }
    }
    let evidence = StateFootprintEvidence::new(
        RegisterSet::new(registers),
        if has_syscall {
            MachineStateSet::new([
                MachineState::Flags,
                MachineState::InstructionPointer,
                MachineState::StackPointer,
                MachineState::ControlState,
            ])
        } else {
            MachineStateSet::empty()
        },
    );
    omega_calling_conventions::validate_outbound_call_footprint(boundary, &evidence)?;
    Ok(evidence)
}

fn derive_boundary_compiler_body_outbound_syscall_result_footprint_for_arguments(
    boundary: &ValidatedBoundaryEntryPlan,
    input: &crate::InstructionSelectionInput<'_>,
    operands: &psi_arena::Arena<omega_abstract_operations::InstructionOperand>,
    instructions: &[omega_abstract_operations::AbstractOperation],
    argument_class: OutboundSyscallResultArgumentClass,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    use omega_abstract_operations::{AbstractOperationKind, InstructionOperandKind};
    use omega_calling_conventions::{EntryControl, HostBindingMechanism};

    let mut registers = Vec::new();
    let mut has_syscall = false;
    for instruction in instructions {
        let AbstractOperationKind::HostOperation {
            operation_ordinal,
            operands: operand_span,
        } = &instruction.kind
        else {
            continue;
        };
        let Some((_, host_call)) = input.host_calls.calls.iter().find(|(_, host_call)| {
            host_call.source_key == instruction.source_key
                && host_call.statement_index == instruction.source_statement
        }) else {
            continue;
        };
        let Some(operation) = input
            .host_calls
            .operations
            .span(host_call.operations)
            .and_then(|operations| operations.get(usize::from(*operation_ordinal)))
        else {
            continue;
        };
        let Some((_, binding)) = input
            .host_abi
            .bindings
            .iter()
            .find(|(_, binding)| binding.operation_key == operation.operation_key)
        else {
            continue;
        };
        let Some(call_operands) = operands.span(*operand_span) else {
            continue;
        };
        let Some((result, arguments)) = call_operands.split_first() else {
            continue;
        };
        let InstructionOperandKind::RuntimeScalarInteger {
            byte_offset,
            byte_count,
            ..
        } = &result.kind
        else {
            continue;
        };
        let has_storage_argument = arguments.iter().any(|operand| {
            abstract_outbound_syscall_storage_argument_is_closed(input.target.architecture, operand)
        });
        let has_data_argument = arguments
            .iter()
            .any(abstract_outbound_syscall_data_argument_is_closed);
        if !matches!(binding.mechanism, HostBindingMechanism::Syscall { .. })
            || operation.operation_key.uses_linux_timespec_result()
            || operation.operation_key.uses_linux_timespec_argument()
            || binding.call_plan().result.is_none()
            || binding.call_plan().parameters.len() != arguments.len()
            || !matches!(
                binding.call_plan().entry_control,
                EntryControl::SupervisorCall { .. }
            )
            || !match argument_class {
                OutboundSyscallResultArgumentClass::Immediate => {
                    !has_storage_argument && !has_data_argument
                }
                OutboundSyscallResultArgumentClass::Storage => {
                    has_storage_argument && !has_data_argument
                }
                OutboundSyscallResultArgumentClass::Data => has_data_argument,
            }
            || !arguments.iter().all(|operand| {
                matches!(
                    operand.kind,
                    InstructionOperandKind::ImmediateInteger(_)
                        | InstructionOperandKind::ByteLength(_)
                ) || abstract_outbound_syscall_storage_argument_is_closed(
                    input.target.architecture,
                    operand,
                ) || abstract_outbound_syscall_data_argument_is_closed(operand)
            })
        {
            continue;
        }
        has_syscall = true;
        registers.extend_from_slice(binding.call_plan().ordinary_clobbers.as_slice());
        if input.target.architecture == omega_target::Architecture::Aarch64 {
            registers.extend_from_slice(
                omega_isa_aarch64::constant_host_result_clobbers(*byte_offset, *byte_count)
                    .as_slice(),
            );
        }
    }
    let evidence = StateFootprintEvidence::new(
        RegisterSet::new(registers),
        if has_syscall {
            MachineStateSet::new([
                MachineState::Flags,
                MachineState::InstructionPointer,
                MachineState::ControlState,
            ])
        } else {
            MachineStateSet::empty()
        },
    );
    omega_calling_conventions::validate_outbound_call_footprint(boundary, &evidence)?;
    Ok(evidence)
}

/// Derive the closed encoder-family footprint for retained compiler-body binary
/// writes. The retained operand arena is the one byte emission consumes, so
/// nested evaluator stack/control-state needs cannot drift from this evidence.
pub fn derive_boundary_compiler_body_place_binary_write_footprint<'instruction>(
    boundary: &ValidatedBoundaryEntryPlan,
    runtime_value_operands: &impl omega_target_operations::RuntimeValueOperandSource,
    instructions: impl IntoIterator<Item = &'instruction SelectedInstructionKind>,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    let architecture = boundary.plan().call.policy.architecture();
    let mut registers = Vec::new();
    let mut additional_state = MachineStateSet::empty();
    for instruction in instructions {
        let SelectedInstructionKind::WritePlaceBinary {
            target,
            left,
            operator,
            right,
            ..
        } = instruction
        else {
            continue;
        };
        let supported = architecture == omega_target::Architecture::X86_64
            || matches!(
                crate::classify_write_place_shape(target),
                crate::WritePlaceShape::Direct { .. }
                    | crate::WritePlaceShape::Pointee { .. }
                    | crate::WritePlaceShape::FrameIndexed { .. }
                    | crate::WritePlaceShape::FrameIndexedByRegion { .. }
                    | crate::WritePlaceShape::FrameBaseIndexed { .. }
                    | crate::WritePlaceShape::MachineIndexed { .. }
                    | crate::WritePlaceShape::MachineDoubleIndexed { .. },
            )
            || crate::classify_frame_base_indexed_binary_shape(target).is_some()
            || crate::classify_frame_base_double_indexed_binary_shape(target).is_some();
        if !supported {
            continue;
        }
        let (writes, state) = match architecture {
            omega_target::Architecture::X86_64 => (
                omega_isa_x86_64::place_binary_write_register_write_ceiling(),
                omega_isa_x86_64::place_binary_write_additional_machine_state(
                    runtime_value_operands,
                    *left,
                    *operator,
                    *right,
                ),
            ),
            omega_target::Architecture::Aarch64 => (
                omega_isa_aarch64::place_binary_write_register_write_ceiling(),
                omega_isa_aarch64::place_binary_write_additional_machine_state(
                    runtime_value_operands,
                    *left,
                    *operator,
                    *right,
                ),
            ),
        };
        registers.extend_from_slice(writes.as_slice());
        additional_state = additional_state.union(state);
    }
    let evidence = StateFootprintEvidence::new(RegisterSet::new(registers), additional_state);
    validate_runtime_value_guard_footprint(boundary, &evidence)?;
    Ok(evidence)
}

/// Derive the closed encoder-family footprint for retained compiler-body
/// immediate bit-field writes.
pub fn derive_boundary_compiler_body_storage_bit_field_write_footprint<'instruction>(
    boundary: &ValidatedBoundaryEntryPlan,
    instructions: impl IntoIterator<Item = &'instruction SelectedInstructionKind>,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    let architecture = boundary.plan().call.policy.architecture();
    let mut registers = Vec::new();
    let mut additional_state = MachineStateSet::empty();
    for instruction in instructions {
        if !matches!(
            instruction,
            SelectedInstructionKind::WriteStorageBitField { .. }
        ) {
            continue;
        }
        let (writes, state) = match architecture {
            omega_target::Architecture::X86_64 => (
                omega_isa_x86_64::runtime_storage_bit_field_write_register_write_ceiling(),
                omega_isa_x86_64::runtime_storage_bit_field_write_additional_machine_state(),
            ),
            omega_target::Architecture::Aarch64 => (
                omega_isa_aarch64::runtime_storage_bit_field_write_register_write_ceiling(),
                omega_isa_aarch64::runtime_storage_bit_field_write_additional_machine_state(),
            ),
        };
        registers.extend_from_slice(writes.as_slice());
        additional_state = additional_state.union(state);
    }
    let evidence = StateFootprintEvidence::new(RegisterSet::new(registers), additional_state);
    validate_state_footprint(boundary, &evidence)?;
    Ok(evidence)
}

/// Derive the closed encoder-family footprint for retained compiler-body
/// immediate bounded-buffer literal writes.
pub fn derive_boundary_compiler_body_place_bounded_buffer_write_footprint<'instruction>(
    boundary: &ValidatedBoundaryEntryPlan,
    instructions: impl IntoIterator<Item = &'instruction SelectedInstructionKind>,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    let architecture = boundary.plan().call.policy.architecture();
    let mut registers = Vec::new();
    let mut additional_state = MachineStateSet::empty();
    for instruction in instructions {
        let (target, source, append_kind) = match instruction {
            SelectedInstructionKind::WritePlaceBoundedBuffer { target, .. } => (target, None, 0u8),
            SelectedInstructionKind::AppendPlaceBoundedBufferLiteral { target, .. } => {
                (target, None, 1)
            }
            SelectedInstructionKind::AppendPlaceBoundedBufferSource { target, source } => {
                (target, Some(source), 2)
            }
            _ => continue,
        };
        let target_shape = crate::classify_write_place_shape(target);
        let supported = architecture == omega_target::Architecture::X86_64
            || (!matches!(target_shape, crate::WritePlaceShape::Unsupported)
                && source.map_or(true, |source| {
                    matches!(
                        crate::classify_write_place_shape(source),
                        crate::WritePlaceShape::Direct { .. }
                            | crate::WritePlaceShape::Pointee { .. }
                    )
                }))
            || (append_kind == 0
                && crate::classify_frame_base_indexed_bounded_buffer_shape(target).is_some())
            || (append_kind == 0
                && crate::classify_frame_base_double_indexed_bounded_buffer_shape(target)
                    .is_some())
            || (append_kind == 1
                && crate::classify_frame_base_indexed_bounded_buffer_literal_append_shape(target)
                    .is_some())
            || (append_kind == 1
                && crate::classify_frame_base_double_indexed_bounded_buffer_literal_append_shape(
                    target,
                )
                .is_some())
            || (append_kind == 2
                && (crate::classify_frame_base_indexed_bounded_buffer_source_append_shape(target)
                    .is_some()
                    || crate::classify_frame_base_double_indexed_bounded_buffer_source_append_shape(
                        target,
                    )
                    .is_some())
                && source.is_some_and(|source| {
                    matches!(
                        crate::classify_write_place_shape(source),
                        crate::WritePlaceShape::Direct { .. }
                            | crate::WritePlaceShape::Pointee { .. }
                    )
                }));
        if !supported {
            continue;
        }
        let (writes, state) = match architecture {
            omega_target::Architecture::X86_64 if append_kind == 2 => (
                omega_isa_x86_64::place_bounded_buffer_source_append_register_writes(
                    target,
                    source.expect("source append retains a source"),
                ),
                omega_isa_x86_64::place_bounded_buffer_source_append_additional_machine_state(),
            ),
            omega_target::Architecture::X86_64 if append_kind == 1 => (
                omega_isa_x86_64::place_bounded_buffer_literal_append_register_writes(target),
                omega_isa_x86_64::place_bounded_buffer_literal_append_additional_machine_state(),
            ),
            omega_target::Architecture::X86_64 => (
                omega_isa_x86_64::place_bounded_buffer_write_register_writes(target),
                omega_isa_x86_64::place_bounded_buffer_write_additional_machine_state(target),
            ),
            omega_target::Architecture::Aarch64 if append_kind == 2 => (
                omega_isa_aarch64::place_bounded_buffer_source_append_register_write_ceiling(),
                omega_isa_aarch64::place_bounded_buffer_source_append_additional_machine_state(),
            ),
            omega_target::Architecture::Aarch64 if append_kind == 1 => (
                omega_isa_aarch64::place_bounded_buffer_literal_append_register_write_ceiling(),
                omega_isa_aarch64::place_bounded_buffer_literal_append_additional_machine_state(),
            ),
            omega_target::Architecture::Aarch64 => (
                omega_isa_aarch64::place_bounded_buffer_write_register_write_ceiling(),
                omega_isa_aarch64::place_bounded_buffer_write_additional_machine_state(),
            ),
        };
        registers.extend_from_slice(writes.as_slice());
        additional_state = additional_state.union(state);
    }
    let evidence = StateFootprintEvidence::new(RegisterSet::new(registers), additional_state);
    validate_state_footprint(boundary, &evidence)?;
    Ok(evidence)
}

/// Derive the closed encoder-family footprint for retained compiler-body
/// string-descriptor writes.
pub fn derive_boundary_compiler_body_place_string_write_footprint<'instruction>(
    boundary: &ValidatedBoundaryEntryPlan,
    instructions: impl IntoIterator<Item = &'instruction SelectedInstructionKind>,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    let architecture = boundary.plan().call.policy.architecture();
    let mut registers = Vec::new();
    let mut additional_state = MachineStateSet::empty();
    for instruction in instructions {
        let SelectedInstructionKind::WritePlaceString { target, .. } = instruction else {
            continue;
        };
        let supported = architecture == omega_target::Architecture::X86_64
            || matches!(
                crate::classify_write_place_shape(target),
                crate::WritePlaceShape::Direct { .. }
                    | crate::WritePlaceShape::Pointee { .. }
                    | crate::WritePlaceShape::FrameIndexed { .. }
                    | crate::WritePlaceShape::FrameIndexedByRegion { .. }
                    | crate::WritePlaceShape::FrameBaseIndexed { .. }
                    | crate::WritePlaceShape::MachineIndexed { .. }
                    | crate::WritePlaceShape::MachineDoubleIndexed { .. }
            )
            || crate::classify_frame_base_double_indexed_string_shape(target).is_some()
            || crate::classify_frame_base_indexed_string_shape(target).is_some();
        if !supported {
            continue;
        }
        let (writes, state) = match architecture {
            omega_target::Architecture::X86_64 => (
                omega_isa_x86_64::place_string_write_register_writes(target),
                omega_isa_x86_64::place_string_write_additional_machine_state(target),
            ),
            omega_target::Architecture::Aarch64 => (
                omega_isa_aarch64::place_string_write_register_write_ceiling(),
                omega_isa_aarch64::place_string_write_additional_machine_state(),
            ),
        };
        registers.extend_from_slice(writes.as_slice());
        additional_state = additional_state.union(state);
    }
    let evidence = StateFootprintEvidence::new(RegisterSet::new(registers), additional_state);
    validate_state_footprint(boundary, &evidence)?;
    Ok(evidence)
}

/// Derive the exact scratch footprint of compiler-generated compact-binary
/// framing-byte appends. Final-image validation replays the same closed
/// encoder while independently binding both relocated storage roots.
pub fn derive_boundary_compiler_body_wire_literal_byte_append_footprint<'instruction>(
    boundary: &ValidatedBoundaryEntryPlan,
    instructions: impl IntoIterator<Item = &'instruction SelectedInstructionKind>,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    let architecture = boundary.plan().call.policy.architecture();
    let mut registers = Vec::new();
    let mut additional_state = MachineStateSet::empty();
    for instruction in instructions {
        if !matches!(
            instruction,
            SelectedInstructionKind::AppendWireLiteralByte { .. }
        ) {
            continue;
        }
        match architecture {
            omega_target::Architecture::X86_64 => {
                registers.extend_from_slice(
                    omega_isa_x86_64::append_wire_literal_byte_clobbers().as_slice(),
                );
                additional_state = additional_state
                    .union(omega_isa_x86_64::append_wire_literal_byte_additional_machine_state());
            }
            omega_target::Architecture::Aarch64 => registers.extend_from_slice(
                omega_isa_aarch64::append_wire_literal_byte_clobbers().as_slice(),
            ),
        }
    }
    let evidence = StateFootprintEvidence::new(RegisterSet::new(registers), additional_state);
    validate_state_footprint(boundary, &evidence)?;
    Ok(evidence)
}

/// Derive the exact scratch footprint of compiler-generated compact-binary
/// scalar-varint appends. Final-image validation replays the closed encoder
/// while independently binding the source, output, and cursor storage roots.
pub fn derive_boundary_compiler_body_wire_scalar_varint_append_footprint<'instruction>(
    boundary: &ValidatedBoundaryEntryPlan,
    instructions: impl IntoIterator<Item = &'instruction SelectedInstructionKind>,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    let architecture = boundary.plan().call.policy.architecture();
    let mut registers = Vec::new();
    let mut additional_state = MachineStateSet::empty();
    for instruction in instructions {
        if !matches!(
            instruction,
            SelectedInstructionKind::AppendWireScalarVarint { .. }
        ) {
            continue;
        }
        match architecture {
            omega_target::Architecture::X86_64 => {
                registers.extend_from_slice(
                    omega_isa_x86_64::append_wire_scalar_varint_clobbers().as_slice(),
                );
                additional_state = additional_state
                    .union(omega_isa_x86_64::append_wire_scalar_varint_additional_machine_state());
            }
            omega_target::Architecture::Aarch64 => registers.extend_from_slice(
                omega_isa_aarch64::append_wire_scalar_varint_clobbers().as_slice(),
            ),
        }
    }
    let evidence = StateFootprintEvidence::new(RegisterSet::new(registers), additional_state);
    validate_state_footprint(boundary, &evidence)?;
    Ok(evidence)
}

/// Derive the exact scratch footprint of compiler-generated compact-binary
/// text appends, including the length-varint and capacity-bounded copy loops.
pub fn derive_boundary_compiler_body_wire_text_bytes_append_footprint<'instruction>(
    boundary: &ValidatedBoundaryEntryPlan,
    instructions: impl IntoIterator<Item = &'instruction SelectedInstructionKind>,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    let architecture = boundary.plan().call.policy.architecture();
    let mut registers = Vec::new();
    let mut additional_state = MachineStateSet::empty();
    for instruction in instructions {
        if !matches!(
            instruction,
            SelectedInstructionKind::AppendWireTextBytes { .. }
        ) {
            continue;
        }
        match architecture {
            omega_target::Architecture::X86_64 => {
                registers.extend_from_slice(
                    omega_isa_x86_64::append_wire_text_bytes_clobbers().as_slice(),
                );
                additional_state = additional_state
                    .union(omega_isa_x86_64::append_wire_text_bytes_additional_machine_state());
            }
            omega_target::Architecture::Aarch64 => {
                registers.extend_from_slice(
                    omega_isa_aarch64::append_wire_text_bytes_clobbers().as_slice(),
                );
                additional_state = additional_state
                    .union(omega_isa_aarch64::append_wire_text_bytes_additional_machine_state());
            }
        }
    }
    let evidence = StateFootprintEvidence::new(RegisterSet::new(registers), additional_state);
    validate_state_footprint(boundary, &evidence)?;
    Ok(evidence)
}

/// Derive the exact scratch footprint of compiler-generated compact-binary
/// borrowed scalar-slice appends, including both the measurement and emission
/// passes over the descriptor.
pub fn derive_boundary_compiler_body_wire_scalar_slice_append_footprint<'instruction>(
    boundary: &ValidatedBoundaryEntryPlan,
    instructions: impl IntoIterator<Item = &'instruction SelectedInstructionKind>,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    let architecture = boundary.plan().call.policy.architecture();
    let mut registers = Vec::new();
    let mut additional_state = MachineStateSet::empty();
    for instruction in instructions {
        if !matches!(
            instruction,
            SelectedInstructionKind::AppendWireScalarSlice { .. }
        ) {
            continue;
        }
        match architecture {
            omega_target::Architecture::X86_64 => {
                registers.extend_from_slice(
                    omega_isa_x86_64::append_wire_scalar_slice_clobbers().as_slice(),
                );
                additional_state = additional_state
                    .union(omega_isa_x86_64::append_wire_scalar_slice_additional_machine_state());
            }
            omega_target::Architecture::Aarch64 => {
                registers.extend_from_slice(
                    omega_isa_aarch64::append_wire_scalar_slice_clobbers().as_slice(),
                );
                additional_state = additional_state
                    .union(omega_isa_aarch64::append_wire_scalar_slice_additional_machine_state());
            }
        }
    }
    let evidence = StateFootprintEvidence::new(RegisterSet::new(registers), additional_state);
    validate_state_footprint(boundary, &evidence)?;
    Ok(evidence)
}

/// Derive the exact scratch footprint of compiler-generated compact-binary
/// repeated scalar appends, including the runtime-count guard around each
/// statically unrolled element.
pub fn derive_boundary_compiler_body_wire_repeated_scalar_varint_append_footprint<'instruction>(
    boundary: &ValidatedBoundaryEntryPlan,
    instructions: impl IntoIterator<Item = &'instruction SelectedInstructionKind>,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    let architecture = boundary.plan().call.policy.architecture();
    let mut registers = Vec::new();
    let mut additional_state = MachineStateSet::empty();
    for instruction in instructions {
        if !matches!(
            instruction,
            SelectedInstructionKind::AppendWireRepeatedScalarVarint { .. }
        ) {
            continue;
        }
        match architecture {
            omega_target::Architecture::X86_64 => {
                registers.extend_from_slice(
                    omega_isa_x86_64::append_wire_repeated_scalar_varint_clobbers().as_slice(),
                );
                additional_state = additional_state.union(
                    omega_isa_x86_64::append_wire_repeated_scalar_varint_additional_machine_state(),
                );
            }
            omega_target::Architecture::Aarch64 => {
                registers.extend_from_slice(
                    omega_isa_aarch64::append_wire_repeated_scalar_varint_clobbers().as_slice(),
                );
                additional_state = additional_state.union(
                    omega_isa_aarch64::append_wire_repeated_scalar_varint_additional_machine_state(
                    ),
                );
            }
        }
    }
    let evidence = StateFootprintEvidence::new(RegisterSet::new(registers), additional_state);
    validate_state_footprint(boundary, &evidence)?;
    Ok(evidence)
}

/// Derive the exact scratch footprint of compiler-generated compact-binary
/// framing-byte reads. The AArch64 cursor/verdict offset forms determine
/// whether the address scratch register participates in the sequence.
pub fn derive_boundary_compiler_body_wire_expected_byte_read_footprint<'instruction>(
    boundary: &ValidatedBoundaryEntryPlan,
    instructions: impl IntoIterator<Item = &'instruction SelectedInstructionKind>,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    let architecture = boundary.plan().call.policy.architecture();
    let mut registers = Vec::new();
    let mut additional_state = MachineStateSet::empty();
    for instruction in instructions {
        let SelectedInstructionKind::ReadWireExpectedByte {
            read_offset,
            ok_offset,
            ..
        } = instruction
        else {
            continue;
        };
        match architecture {
            omega_target::Architecture::X86_64 => {
                registers.extend_from_slice(
                    omega_isa_x86_64::read_wire_expected_byte_clobbers().as_slice(),
                );
                additional_state = additional_state
                    .union(omega_isa_x86_64::read_wire_expected_byte_additional_machine_state());
            }
            omega_target::Architecture::Aarch64 => {
                registers.extend_from_slice(
                    omega_isa_aarch64::read_wire_expected_byte_clobbers(*read_offset, *ok_offset)
                        .as_slice(),
                );
                additional_state = additional_state
                    .union(omega_isa_aarch64::read_wire_expected_byte_additional_machine_state());
            }
        }
    }
    let evidence = StateFootprintEvidence::new(RegisterSet::new(registers), additional_state);
    validate_state_footprint(boundary, &evidence)?;
    Ok(evidence)
}

/// Derive the exact scratch footprint of compiler-generated compact-binary
/// scalar-varint reads, including the arithmetic flags consumed by canonical,
/// range, and signed-decode branches.
pub fn derive_boundary_compiler_body_wire_scalar_varint_read_footprint<'instruction>(
    boundary: &ValidatedBoundaryEntryPlan,
    instructions: impl IntoIterator<Item = &'instruction SelectedInstructionKind>,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    let architecture = boundary.plan().call.policy.architecture();
    let mut registers = Vec::new();
    let mut additional_state = MachineStateSet::empty();
    for instruction in instructions {
        if !matches!(
            instruction,
            SelectedInstructionKind::ReadWireScalarVarint { .. }
        ) {
            continue;
        }
        match architecture {
            omega_target::Architecture::X86_64 => {
                registers.extend_from_slice(
                    omega_isa_x86_64::read_wire_scalar_varint_clobbers().as_slice(),
                );
                additional_state = additional_state
                    .union(omega_isa_x86_64::read_wire_scalar_varint_additional_machine_state());
            }
            omega_target::Architecture::Aarch64 => {
                registers.extend_from_slice(
                    omega_isa_aarch64::read_wire_scalar_varint_clobbers().as_slice(),
                );
                additional_state = additional_state
                    .union(omega_isa_aarch64::read_wire_scalar_varint_additional_machine_state());
            }
        }
    }
    let evidence = StateFootprintEvidence::new(RegisterSet::new(registers), additional_state);
    validate_state_footprint(boundary, &evidence)?;
    Ok(evidence)
}

/// Derive the exact scratch footprint of compiler-generated compact-binary
/// borrowed byte-slice reads, including length decoding, bounds checks,
/// predicate validation, and zero-copy descriptor construction.
pub fn derive_boundary_compiler_body_wire_byte_slice_read_footprint<'instruction>(
    boundary: &ValidatedBoundaryEntryPlan,
    instructions: impl IntoIterator<Item = &'instruction SelectedInstructionKind>,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    let architecture = boundary.plan().call.policy.architecture();
    let mut registers = Vec::new();
    let mut additional_state = MachineStateSet::empty();
    for instruction in instructions {
        if !matches!(
            instruction,
            SelectedInstructionKind::ReadWireByteSlice { .. }
        ) {
            continue;
        }
        match architecture {
            omega_target::Architecture::X86_64 => {
                registers.extend_from_slice(
                    omega_isa_x86_64::read_wire_byte_slice_clobbers().as_slice(),
                );
                additional_state = additional_state
                    .union(omega_isa_x86_64::read_wire_byte_slice_additional_machine_state());
            }
            omega_target::Architecture::Aarch64 => {
                registers.extend_from_slice(
                    omega_isa_aarch64::read_wire_byte_slice_clobbers().as_slice(),
                );
                additional_state = additional_state
                    .union(omega_isa_aarch64::read_wire_byte_slice_additional_machine_state());
            }
        }
    }
    let evidence = StateFootprintEvidence::new(RegisterSet::new(registers), additional_state);
    validate_state_footprint(boundary, &evidence)?;
    Ok(evidence)
}

/// Derive the exact scratch footprint of compiler-generated compact-binary
/// nested-open checks, which turn a decoded length into an absolute end bound.
pub fn derive_boundary_compiler_body_wire_nested_open_footprint<'instruction>(
    boundary: &ValidatedBoundaryEntryPlan,
    instructions: impl IntoIterator<Item = &'instruction SelectedInstructionKind>,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    let architecture = boundary.plan().call.policy.architecture();
    let mut registers = Vec::new();
    let mut additional_state = MachineStateSet::empty();
    for instruction in instructions {
        if !matches!(
            instruction,
            SelectedInstructionKind::ReadWireNestedOpen { .. }
        ) {
            continue;
        }
        match architecture {
            omega_target::Architecture::X86_64 => {
                registers.extend_from_slice(
                    omega_isa_x86_64::read_wire_nested_open_clobbers().as_slice(),
                );
                additional_state = additional_state
                    .union(omega_isa_x86_64::read_wire_nested_open_additional_machine_state());
            }
            omega_target::Architecture::Aarch64 => {
                registers.extend_from_slice(
                    omega_isa_aarch64::read_wire_nested_open_clobbers().as_slice(),
                );
                additional_state = additional_state
                    .union(omega_isa_aarch64::read_wire_nested_open_additional_machine_state());
            }
        }
    }
    let evidence = StateFootprintEvidence::new(RegisterSet::new(registers), additional_state);
    validate_state_footprint(boundary, &evidence)?;
    Ok(evidence)
}

/// Derive the exact scratch footprint of compiler-generated compact-binary
/// nested-close checks, which require the live cursor to equal the end bound.
pub fn derive_boundary_compiler_body_wire_nested_close_footprint<'instruction>(
    boundary: &ValidatedBoundaryEntryPlan,
    instructions: impl IntoIterator<Item = &'instruction SelectedInstructionKind>,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    let architecture = boundary.plan().call.policy.architecture();
    let mut registers = Vec::new();
    let mut additional_state = MachineStateSet::empty();
    for instruction in instructions {
        if !matches!(
            instruction,
            SelectedInstructionKind::ReadWireNestedClose { .. }
        ) {
            continue;
        }
        match architecture {
            omega_target::Architecture::X86_64 => {
                registers.extend_from_slice(
                    omega_isa_x86_64::read_wire_nested_close_clobbers().as_slice(),
                );
                additional_state = additional_state
                    .union(omega_isa_x86_64::read_wire_nested_close_additional_machine_state());
            }
            omega_target::Architecture::Aarch64 => {
                registers.extend_from_slice(
                    omega_isa_aarch64::read_wire_nested_close_clobbers().as_slice(),
                );
                additional_state = additional_state
                    .union(omega_isa_aarch64::read_wire_nested_close_additional_machine_state());
            }
        }
    }
    let evidence = StateFootprintEvidence::new(RegisterSet::new(registers), additional_state);
    validate_state_footprint(boundary, &evidence)?;
    Ok(evidence)
}

/// Derive the exact scratch footprint of compiler-generated compact-binary
/// guarded repeated-scalar reads, including the end-bound guard, canonical
/// decode, range check, target store, count bump, and sticky verdict merge.
pub fn derive_boundary_compiler_body_wire_repeated_scalar_varint_read_footprint<'instruction>(
    boundary: &ValidatedBoundaryEntryPlan,
    instructions: impl IntoIterator<Item = &'instruction SelectedInstructionKind>,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    let architecture = boundary.plan().call.policy.architecture();
    let mut registers = Vec::new();
    let mut additional_state = MachineStateSet::empty();
    for instruction in instructions {
        if !matches!(
            instruction,
            SelectedInstructionKind::ReadWireRepeatedScalarVarint { .. }
        ) {
            continue;
        }
        match architecture {
            omega_target::Architecture::X86_64 => {
                registers.extend_from_slice(
                    omega_isa_x86_64::read_wire_repeated_scalar_varint_clobbers().as_slice(),
                );
                additional_state = additional_state.union(
                    omega_isa_x86_64::read_wire_repeated_scalar_varint_additional_machine_state(),
                );
            }
            omega_target::Architecture::Aarch64 => {
                registers.extend_from_slice(
                    omega_isa_aarch64::read_wire_repeated_scalar_varint_clobbers().as_slice(),
                );
                additional_state = additional_state.union(
                    omega_isa_aarch64::read_wire_repeated_scalar_varint_additional_machine_state(),
                );
            }
        }
    }
    let evidence = StateFootprintEvidence::new(RegisterSet::new(registers), additional_state);
    validate_state_footprint(boundary, &evidence)?;
    Ok(evidence)
}

/// Derive the target-encoder footprint for compiler-body runtime-text
/// assembly. This is ordinary lowering evidence: Psi has already established
/// the text operation, while Omega retains the exact buffer/place recipe that
/// the final artifact validator can replay.
pub fn derive_boundary_compiler_body_text_assembly_write_footprint<'instruction>(
    boundary: &ValidatedBoundaryEntryPlan,
    instructions: impl IntoIterator<Item = &'instruction SelectedInstructionKind>,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    let architecture = boundary.plan().call.policy.architecture();
    let mut registers = Vec::new();
    let mut additional_state = MachineStateSet::empty();
    for instruction in instructions {
        if matches!(
            instruction,
            SelectedInstructionKind::AppendRuntimeTextStoredSuffix { .. }
        ) {
            let (writes, state) = match architecture {
                omega_target::Architecture::X86_64 => (
                    omega_isa_x86_64::runtime_text_stored_suffix_append_register_writes(),
                    omega_isa_x86_64::runtime_text_stored_suffix_append_additional_machine_state(),
                ),
                omega_target::Architecture::Aarch64 => (
                    omega_isa_aarch64::runtime_text_stored_suffix_append_register_writes(),
                    omega_isa_aarch64::runtime_text_stored_suffix_append_additional_machine_state(),
                ),
            };
            registers.extend_from_slice(writes.as_slice());
            additional_state = additional_state.union(state);
            continue;
        }
        let segmented_literal = match instruction {
            SelectedInstructionKind::WriteRuntimeTextLiteral { .. } => {
                architecture == omega_target::Architecture::Aarch64
            }
            SelectedInstructionKind::WriteRuntimeTextLiteralSegment { .. } => true,
            _ => false,
        };
        if segmented_literal {
            let (writes, state) = match architecture {
                omega_target::Architecture::X86_64 => (
                    omega_isa_x86_64::runtime_text_literal_segment_write_register_writes(),
                    omega_isa_x86_64::runtime_text_literal_segment_write_additional_machine_state(),
                ),
                omega_target::Architecture::Aarch64 => (
                    omega_isa_aarch64::runtime_text_literal_segment_write_register_writes(),
                    omega_isa_aarch64::runtime_text_literal_segment_write_additional_machine_state(
                    ),
                ),
            };
            registers.extend_from_slice(writes.as_slice());
            additional_state = additional_state.union(state);
            continue;
        }
        let (target, write_kind) = match instruction {
            SelectedInstructionKind::MaterializeTextBufferToPlace { target, .. } => (target, 0u8),
            SelectedInstructionKind::AppendTextLiteralToPlace { target, .. } => (target, 1u8),
            SelectedInstructionKind::AppendTextStoredToPlace { target, .. } => (target, 2u8),
            _ => continue,
        };
        let shape = crate::classify_write_place_shape(target);
        if architecture == omega_target::Architecture::Aarch64
            && crate::classify_frame_base_double_indexed_text_assembly_shape(target).is_some()
        {
            let (writes, state) = match write_kind {
                0 => (
                    omega_isa_aarch64::runtime_text_buffer_materialize_to_runtime_frame_base_double_indexed_register_writes(),
                    omega_isa_aarch64::runtime_text_buffer_materialize_additional_machine_state(),
                ),
                1 => (
                    omega_isa_aarch64::runtime_text_literal_append_to_runtime_frame_base_double_indexed_register_writes(),
                    omega_isa_aarch64::runtime_text_literal_append_additional_machine_state(),
                ),
                2 => (
                    omega_isa_aarch64::runtime_text_stored_place_append_to_runtime_frame_base_double_indexed_register_writes(),
                    omega_isa_aarch64::runtime_text_stored_place_append_additional_machine_state(),
                ),
                _ => unreachable!("closed text-assembly write kind"),
            };
            registers.extend_from_slice(writes.as_slice());
            additional_state = additional_state.union(state);
            continue;
        }
        let (writes, state) = match (architecture, shape, write_kind) {
            (
                omega_target::Architecture::X86_64,
                crate::WritePlaceShape::Direct { .. } | crate::WritePlaceShape::Pointee { .. },
                2,
            ) => (
                omega_isa_x86_64::runtime_text_stored_place_append_register_writes(),
                omega_isa_x86_64::runtime_text_stored_place_append_additional_machine_state(),
            ),
            (
                omega_target::Architecture::X86_64,
                crate::WritePlaceShape::FrameIndexed { .. },
                2,
            ) => (
                omega_isa_x86_64::runtime_text_stored_place_append_to_runtime_frame_indexed_register_writes(),
                omega_isa_x86_64::runtime_text_stored_place_append_additional_machine_state(),
            ),
            (
                omega_target::Architecture::X86_64,
                crate::WritePlaceShape::Direct { .. } | crate::WritePlaceShape::Pointee { .. },
                1,
            ) => (
                omega_isa_x86_64::runtime_text_literal_append_register_writes(),
                omega_isa_x86_64::runtime_text_literal_append_additional_machine_state(),
            ),
            (
                omega_target::Architecture::X86_64,
                crate::WritePlaceShape::FrameIndexed { .. },
                1,
            ) => (
                omega_isa_x86_64::runtime_text_literal_append_to_runtime_frame_indexed_register_writes(),
                omega_isa_x86_64::runtime_text_literal_append_additional_machine_state(),
            ),
            (
                omega_target::Architecture::X86_64,
                crate::WritePlaceShape::Direct { .. },
                0,
            ) => (
                omega_isa_x86_64::runtime_text_buffer_materialize_register_writes(),
                omega_isa_x86_64::runtime_text_buffer_materialize_additional_machine_state(),
            ),
            (
                omega_target::Architecture::X86_64,
                crate::WritePlaceShape::Pointee { .. },
                0,
            ) => (
                omega_isa_x86_64::runtime_text_buffer_materialize_to_runtime_pointee_register_writes(),
                omega_isa_x86_64::runtime_text_buffer_materialize_additional_machine_state(),
            ),
            (
                omega_target::Architecture::X86_64,
                crate::WritePlaceShape::FrameIndexed { .. },
                0,
            ) => (
                omega_isa_x86_64::runtime_text_buffer_materialize_to_runtime_frame_indexed_register_writes(),
                omega_isa_x86_64::runtime_text_buffer_materialize_additional_machine_state(),
            ),
            (omega_target::Architecture::X86_64, _, 2) => (
                omega_isa_x86_64::place_text_stored_append_register_writes(),
                omega_isa_x86_64::runtime_text_stored_place_append_additional_machine_state(),
            ),
            (omega_target::Architecture::X86_64, _, 1) => (
                omega_isa_x86_64::place_text_literal_append_register_writes(target),
                omega_isa_x86_64::runtime_text_literal_append_additional_machine_state(),
            ),
            (omega_target::Architecture::X86_64, _, 0) => (
                omega_isa_x86_64::place_text_buffer_materialize_register_writes(),
                omega_isa_x86_64::place_text_buffer_materialize_additional_machine_state(target),
            ),
            (
                omega_target::Architecture::Aarch64,
                crate::WritePlaceShape::Direct { .. } | crate::WritePlaceShape::Pointee { .. },
                2,
            ) => (
                omega_isa_aarch64::runtime_text_stored_place_append_register_writes(),
                omega_isa_aarch64::runtime_text_stored_place_append_additional_machine_state(),
            ),
            (
                omega_target::Architecture::Aarch64,
                crate::WritePlaceShape::FrameIndexed { .. }
                | crate::WritePlaceShape::FrameBaseIndexed { .. },
                2,
            ) => (
                omega_isa_aarch64::runtime_text_stored_place_append_to_runtime_frame_indexed_register_writes(),
                omega_isa_aarch64::runtime_text_stored_place_append_additional_machine_state(),
            ),
            (
                omega_target::Architecture::Aarch64,
                crate::WritePlaceShape::FrameBaseIndexed { .. },
                1,
            ) => (
                omega_isa_aarch64::runtime_text_literal_append_to_runtime_frame_base_indexed_register_writes(),
                omega_isa_aarch64::runtime_text_literal_append_additional_machine_state(),
            ),
            (
                omega_target::Architecture::Aarch64,
                crate::WritePlaceShape::Direct { .. }
                | crate::WritePlaceShape::Pointee { .. }
                | crate::WritePlaceShape::FrameIndexed { .. },
                1,
            ) => (
                omega_isa_aarch64::runtime_text_literal_append_register_writes(),
                omega_isa_aarch64::runtime_text_literal_append_additional_machine_state(),
            ),
            (
                omega_target::Architecture::Aarch64,
                crate::WritePlaceShape::Direct { .. },
                0,
            ) => (
                omega_isa_aarch64::runtime_text_buffer_materialize_register_writes(),
                omega_isa_aarch64::runtime_text_buffer_materialize_additional_machine_state(),
            ),
            (
                omega_target::Architecture::Aarch64,
                crate::WritePlaceShape::Pointee { .. },
                0,
            ) => (
                omega_isa_aarch64::runtime_text_buffer_materialize_to_runtime_pointee_register_writes(),
                omega_isa_aarch64::runtime_text_buffer_materialize_additional_machine_state(),
            ),
            (
                omega_target::Architecture::Aarch64,
                crate::WritePlaceShape::FrameIndexed { .. }
                | crate::WritePlaceShape::FrameIndexedByRegion { .. }
                | crate::WritePlaceShape::FrameBaseIndexed { .. },
                0,
            ) => (
                omega_isa_aarch64::runtime_text_buffer_materialize_to_runtime_frame_indexed_register_writes(),
                omega_isa_aarch64::runtime_text_buffer_materialize_additional_machine_state(),
            ),
            _ => continue,
        };
        registers.extend_from_slice(writes.as_slice());
        additional_state = additional_state.union(state);
    }
    let evidence = StateFootprintEvidence::new(RegisterSet::new(registers), additional_state);
    validate_state_footprint(boundary, &evidence)?;
    Ok(evidence)
}

/// Derive the closed encoder-family footprint for direct compiler-body
/// conversion writes from the same operand arena consumed by emission.
pub fn derive_boundary_compiler_body_storage_convert_write_footprint<'instruction>(
    boundary: &ValidatedBoundaryEntryPlan,
    runtime_value_operands: &impl omega_target_operations::RuntimeValueOperandSource,
    instructions: impl IntoIterator<Item = &'instruction SelectedInstructionKind>,
) -> Result<StateFootprintEvidence, PlanDiagnostic> {
    let architecture = boundary.plan().call.policy.architecture();
    let mut registers = Vec::new();
    let mut additional_state = MachineStateSet::empty();
    for instruction in instructions {
        let source = match instruction {
            SelectedInstructionKind::WriteRuntimeStorageConvert { source, .. } => *source,
            SelectedInstructionKind::WritePlaceConvert { target, source, .. }
                if architecture == omega_target::Architecture::X86_64
                    || matches!(
                        crate::classify_write_place_shape(target),
                        crate::WritePlaceShape::Direct { .. }
                            | crate::WritePlaceShape::Pointee { .. }
                            | crate::WritePlaceShape::FrameIndexed { .. }
                            | crate::WritePlaceShape::FrameIndexedByRegion { .. }
                            | crate::WritePlaceShape::FrameBaseIndexed { .. }
                            | crate::WritePlaceShape::MachineIndexed { .. }
                            | crate::WritePlaceShape::MachineDoubleIndexed { .. }
                    )
                    || crate::classify_frame_base_indexed_convert_shape(target).is_some()
                    || crate::classify_frame_base_double_indexed_convert_shape(target)
                        .is_some() =>
            {
                *source
            }
            _ => continue,
        };
        let (writes, state) = match architecture {
            omega_target::Architecture::X86_64 => (
                omega_isa_x86_64::storage_convert_write_register_write_ceiling(),
                omega_isa_x86_64::storage_convert_write_additional_machine_state(
                    runtime_value_operands,
                    source,
                ),
            ),
            omega_target::Architecture::Aarch64 => (
                omega_isa_aarch64::storage_convert_write_register_write_ceiling(),
                omega_isa_aarch64::storage_convert_write_additional_machine_state(
                    runtime_value_operands,
                    source,
                ),
            ),
        };
        registers.extend_from_slice(writes.as_slice());
        additional_state = additional_state.union(state);
    }
    let evidence = StateFootprintEvidence::new(RegisterSet::new(registers), additional_state);
    validate_runtime_value_guard_footprint(boundary, &evidence)?;
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
    let mut parameters = Vec::with_capacity(parameter_destinations.len());

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

    for (parameter_index, ((destination_offset, shape), placement)) in parameter_destinations
        .iter()
        .zip(&call.parameters)
        .enumerate()
    {
        let write_start = writes.len();
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
        parameters.push(DerivedBoundaryEntryParameterStorage {
            parameter_index,
            destination_byte_offset: *destination_offset,
            shape: *shape,
            placement: placement.clone(),
            write_range: write_start..writes.len(),
        });
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

    Ok(DerivedBoundaryEntryStorage {
        writes,
        parameters,
        footprint,
    })
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
    fn outbound_syscall_storage_arguments_close_over_runtime_address_shapes() {
        use omega_abstract_operations::{
            InstructionOperand, InstructionOperandKind, RuntimeStorageRegion,
        };

        let runtime_address = InstructionOperand {
            kind: InstructionOperandKind::RuntimeStorageAddress {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 64,
            },
        };
        let descriptor_length = InstructionOperand {
            kind: InstructionOperandKind::RuntimePointeeStringLength {
                region: RuntimeStorageRegion::Machine,
                byte_offset: 32,
            },
        };
        let bounded_at_aarch64_limit = InstructionOperand {
            kind: InstructionOperandKind::RuntimeStringPointer {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 4087,
                is_bounded_buffer: true,
            },
        };
        let bounded_beyond_aarch64_limit = InstructionOperand {
            kind: InstructionOperandKind::RuntimeStringPointer {
                region: RuntimeStorageRegion::RuntimeFrame,
                byte_offset: 4088,
                is_bounded_buffer: true,
            },
        };
        let data_address = InstructionOperand {
            kind: InstructionOperandKind::DataAddress {
                data: psi_arena::Handle::invalid(),
            },
        };

        for operand in [&runtime_address, &descriptor_length] {
            assert!(abstract_outbound_syscall_storage_argument_is_closed(
                omega_target::Architecture::X86_64,
                operand,
            ));
            assert!(abstract_outbound_syscall_storage_argument_is_closed(
                omega_target::Architecture::Aarch64,
                operand,
            ));
        }
        assert!(abstract_outbound_syscall_storage_argument_is_closed(
            omega_target::Architecture::Aarch64,
            &bounded_at_aarch64_limit,
        ));
        assert!(!abstract_outbound_syscall_storage_argument_is_closed(
            omega_target::Architecture::Aarch64,
            &bounded_beyond_aarch64_limit,
        ));
        assert!(abstract_outbound_syscall_storage_argument_is_closed(
            omega_target::Architecture::X86_64,
            &bounded_beyond_aarch64_limit,
        ));
        assert!(abstract_outbound_syscall_data_argument_is_closed(
            &data_address,
        ));
        assert!(!abstract_outbound_syscall_storage_argument_is_closed(
            omega_target::Architecture::X86_64,
            &data_address,
        ));
    }

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

        assert_eq!(derived.parameters.len(), 7);
        for (parameter_index, parameter) in derived.parameters.iter().enumerate() {
            assert_eq!(parameter.parameter_index, parameter_index);
            assert_eq!(parameter.destination_byte_offset, parameter_index * 8);
            assert_eq!(parameter.shape, ValueShape::integer(8, 8));
            assert_eq!(
                parameter.placement,
                boundary.plan().call.parameters[parameter_index]
            );
            assert_eq!(parameter.write_range, parameter_index..parameter_index + 1);
            assert_eq!(
                &derived.writes[parameter.write_range.clone()],
                &derived.writes[parameter_index..parameter_index + 1]
            );
        }
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
    fn call_return_mechanics_track_x86_stack_and_control_writes() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("SysV call-return boundary");
        let instructions = [
            SelectedInstructionKind::EnterFunction,
            SelectedInstructionKind::LeaveFunction,
        ];

        let evidence = derive_boundary_call_return_mechanics_footprint(&boundary, &instructions)
            .expect("x86 call-return mechanics");

        assert_eq!(
            evidence.registers().as_slice(),
            &[
                MachineRegister::X86Rbx,
                MachineRegister::X86Rsp,
                MachineRegister::X86Rbp,
                MachineRegister::X86Rsi,
                MachineRegister::X86Rdi,
                MachineRegister::X86R12,
                MachineRegister::X86R13,
                MachineRegister::X86R14,
                MachineRegister::X86R15,
            ]
        );
        assert!(evidence.machine_state().contains_all(MachineStateSet::new([
            MachineState::GeneralRegisters,
            MachineState::InstructionPointer,
            MachineState::StackPointer,
            MachineState::ControlState,
        ])));
    }

    #[test]
    fn call_return_mechanics_track_aarch64_frame_restore_and_control_writes() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::Aapcs64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("AAPCS64 call-return boundary");
        let instructions = [
            SelectedInstructionKind::EnterFunction,
            SelectedInstructionKind::LeaveFunction,
        ];

        let evidence = derive_boundary_call_return_mechanics_footprint(&boundary, &instructions)
            .expect("AArch64 call-return mechanics");

        assert_eq!(
            evidence.registers().as_slice(),
            &[MachineRegister::Aarch64X(16)]
                .into_iter()
                .chain((19..=30).map(MachineRegister::Aarch64X))
                .collect::<Vec<_>>()
        );
        assert!(evidence.machine_state().contains_all(MachineStateSet::new([
            MachineState::GeneralRegisters,
            MachineState::InstructionPointer,
            MachineState::StackPointer,
            MachineState::ControlState,
        ])));
    }

    #[test]
    fn call_return_mechanics_reject_an_incomplete_selected_pair() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("SysV call-return boundary");

        let error = derive_boundary_call_return_mechanics_footprint(
            &boundary,
            &[SelectedInstructionKind::EnterFunction],
        )
        .expect_err("missing return must reject");

        assert!(error.0.contains("exactly one function entry and return"));
    }

    fn dispatch_scaffold_instructions() -> [SelectedInstructionKind; 5] {
        [
            SelectedInstructionKind::EnterDispatchLoop {
                entry_dispatch_index: 0,
                terminal_dispatch_index: 2,
            },
            SelectedInstructionKind::EnterDispatchCase { dispatch_index: 0 },
            SelectedInstructionKind::SetDispatchState { dispatch_index: 1 },
            SelectedInstructionKind::LeaveDispatchCase,
            SelectedInstructionKind::LeaveDispatchLoop,
        ]
    }

    #[test]
    fn dispatch_scaffold_tracks_x86_state_register_and_flags() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("SysV dispatch boundary");

        let evidence = derive_boundary_dispatch_scaffold_footprint(
            &boundary,
            &dispatch_scaffold_instructions(),
        )
        .expect("x86 dispatch scaffold");

        assert_eq!(evidence.registers().as_slice(), &[MachineRegister::X86R12]);
        assert!(
            evidence
                .machine_state()
                .contains_all(MachineStateSet::new([MachineState::Flags]))
        );
    }

    #[test]
    fn dispatch_scaffold_tracks_aarch64_state_register_and_flags() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::Aapcs64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("AAPCS64 dispatch boundary");

        let evidence = derive_boundary_dispatch_scaffold_footprint(
            &boundary,
            &dispatch_scaffold_instructions(),
        )
        .expect("AArch64 dispatch scaffold");

        assert_eq!(
            evidence.registers().as_slice(),
            &[MachineRegister::Aarch64X(28)]
        );
        assert!(
            evidence
                .machine_state()
                .contains_all(MachineStateSet::new([MachineState::Flags]))
        );
    }

    #[test]
    fn dispatch_scaffold_rejects_an_incomplete_loop_pair() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("SysV dispatch boundary");

        let error = derive_boundary_dispatch_scaffold_footprint(
            &boundary,
            &[SelectedInstructionKind::EnterDispatchLoop {
                entry_dispatch_index: 0,
                terminal_dispatch_index: 1,
            }],
        )
        .expect_err("missing loop leave must reject");

        assert!(error.0.contains("exactly one loop entry and leave"));
    }

    fn static_guard_instruction(is_float: bool, has_storage: bool) -> SelectedInstructionKind {
        SelectedInstructionKind::EvaluateDispatchGuard {
            guard_lowering: omega_abstract_operations::StateGuardLowering::CompareStaticValue,
            operator: omega_abstract_operations::StateGuardOperator::Equal,
            storage_region: omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
            byte_offset: 65_537,
            byte_size: 8,
            expected_value: 1,
            has_storage,
            is_float,
        }
    }

    #[test]
    fn static_guard_footprint_tracks_x86_integer_and_float_scratch() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("SysV guard boundary");
        let instructions = [
            static_guard_instruction(false, true),
            static_guard_instruction(true, true),
            static_guard_instruction(true, false),
        ];

        let evidence = derive_boundary_static_guard_footprint(&boundary, &instructions)
            .expect("x86 static guard evidence");

        assert_eq!(
            evidence.registers().as_slice(),
            &[
                MachineRegister::X86R10,
                MachineRegister::X86R11,
                MachineRegister::X86R15,
                MachineRegister::X86Xmm(0),
                MachineRegister::X86Xmm(1),
            ]
        );
        assert!(
            evidence
                .machine_state()
                .contains_all(MachineStateSet::new([MachineState::Flags]))
        );
    }

    #[test]
    fn static_guard_footprint_tracks_aarch64_integer_and_float_scratch() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::Aapcs64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("AAPCS64 guard boundary");
        let instructions = [
            static_guard_instruction(false, true),
            static_guard_instruction(true, true),
            static_guard_instruction(true, false),
        ];

        let evidence = derive_boundary_static_guard_footprint(&boundary, &instructions)
            .expect("AArch64 static guard evidence");

        assert_eq!(
            evidence.registers().as_slice(),
            &[
                MachineRegister::Aarch64X(16),
                MachineRegister::Aarch64X(17),
                MachineRegister::Aarch64X(26),
                MachineRegister::Aarch64V(0),
                MachineRegister::Aarch64V(1),
            ]
        );
        assert!(
            evidence
                .machine_state()
                .contains_all(MachineStateSet::new([MachineState::Flags]))
        );
    }

    #[test]
    fn storage_free_static_guard_contributes_no_footprint() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("SysV guard boundary");

        let evidence = derive_boundary_static_guard_footprint(
            &boundary,
            &[static_guard_instruction(true, false)],
        )
        .expect("storage-free static guard evidence");

        assert!(evidence.registers().as_slice().is_empty());
        assert!(evidence.machine_state().is_empty());
    }

    fn runtime_text_guard_instructions() -> [SelectedInstructionKind; 2] {
        [
            SelectedInstructionKind::CompareRuntimeTextLiteral {
                buffer: omega_abstract_operations::AbstractDataObjectHandle::invalid(),
                literal: std::sync::Arc::from("omega"),
            },
            SelectedInstructionKind::CompareRuntimeTextStorage {
                buffer: omega_abstract_operations::AbstractDataObjectHandle::invalid(),
                source_region: omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
                source_offset: 65_537,
                operator: omega_abstract_operations::StateGuardOperator::Equal,
            },
        ]
    }

    #[test]
    fn runtime_text_guards_track_x86_literal_and_descriptor_loop_scratch() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("SysV text-guard boundary");

        let evidence = derive_boundary_runtime_text_guard_footprint(
            &boundary,
            &runtime_text_guard_instructions(),
        )
        .expect("x86 runtime-text guard evidence");

        assert_eq!(
            evidence.registers().as_slice(),
            &[
                MachineRegister::X86Rax,
                MachineRegister::X86Rcx,
                MachineRegister::X86R8,
                MachineRegister::X86R9,
                MachineRegister::X86R14,
                MachineRegister::X86R15,
            ]
        );
        assert!(
            evidence
                .machine_state()
                .contains_all(MachineStateSet::new([MachineState::Flags]))
        );
    }

    #[test]
    fn runtime_text_guards_track_aarch64_literal_and_descriptor_loop_scratch() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::Aapcs64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("AAPCS64 text-guard boundary");

        let evidence = derive_boundary_runtime_text_guard_footprint(
            &boundary,
            &runtime_text_guard_instructions(),
        )
        .expect("AArch64 runtime-text guard evidence");

        assert_eq!(
            evidence.registers().as_slice(),
            &[14, 15, 16, 17, 19, 20, 21, 26].map(MachineRegister::Aarch64X)
        );
        assert!(
            evidence
                .machine_state()
                .contains_all(MachineStateSet::new([MachineState::Flags]))
        );
    }

    fn place_guard_instructions() -> [SelectedInstructionKind; 2] {
        [
            SelectedInstructionKind::ComparePlaces {
                left: omega_abstract_operations::Place::at(
                    omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
                    65_537,
                ),
                right: omega_abstract_operations::Place::at(
                    omega_abstract_operations::RuntimeStorageRegion::Machine,
                    131_073,
                ),
                byte_size: 8,
                operator: omega_abstract_operations::StateGuardOperator::Equal,
                is_float: true,
            },
            SelectedInstructionKind::ComparePlaceValue {
                place: omega_abstract_operations::Place::at(
                    omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
                    40,
                ),
                byte_size: 8,
                expected_value: 7,
                operator: omega_abstract_operations::StateGuardOperator::Equal,
            },
        ]
    }

    #[test]
    fn place_guards_track_x86_walk_bases_values_and_float_scratch() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("SysV place-guard boundary");

        let evidence =
            derive_boundary_place_guard_footprint(&boundary, &place_guard_instructions())
                .expect("x86 place-guard evidence");

        assert_eq!(
            evidence.registers().as_slice(),
            &[
                MachineRegister::X86R10,
                MachineRegister::X86R11,
                MachineRegister::X86R14,
                MachineRegister::X86R15,
                MachineRegister::X86Xmm(0),
                MachineRegister::X86Xmm(1),
            ]
        );
        assert!(
            evidence
                .machine_state()
                .contains_all(MachineStateSet::new([MachineState::Flags]))
        );
    }

    #[test]
    fn place_guards_track_aarch64_large_offset_and_float_scratch() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::Aapcs64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("AAPCS64 place-guard boundary");

        let evidence =
            derive_boundary_place_guard_footprint(&boundary, &place_guard_instructions())
                .expect("AArch64 place-guard evidence");

        assert_eq!(
            evidence.registers().as_slice(),
            &[
                MachineRegister::Aarch64X(16),
                MachineRegister::Aarch64X(17),
                MachineRegister::Aarch64X(19),
                MachineRegister::Aarch64X(20),
                MachineRegister::Aarch64X(21),
                MachineRegister::Aarch64X(26),
                MachineRegister::Aarch64V(0),
                MachineRegister::Aarch64V(1),
            ]
        );
        assert!(
            evidence
                .machine_state()
                .contains_all(MachineStateSet::new([MachineState::Flags]))
        );
    }

    fn runtime_value_guard_fixture() -> (
        psi_arena::Arena<omega_abstract_operations::AbstractValueOperand>,
        SelectedInstructionKind,
    ) {
        let mut operands = psi_arena::Arena::new();
        let left = operands.insert(omega_abstract_operations::ValueOperand::Storage {
            region: omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
            byte_offset: 40,
            byte_size: 8,
        });
        let right = operands.insert(omega_abstract_operations::ValueOperand::Immediate(2));
        let binary = operands.insert(omega_abstract_operations::ValueOperand::Binary {
            left,
            operator: omega_abstract_operations::StateGuardOperator::AddTowardPositive,
            right,
            is_float: true,
            byte_width: 8,
            arithmetic_domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
            operands_signed: false,
        });
        (
            operands,
            SelectedInstructionKind::CompareRuntimeValues {
                left: binary,
                right,
                byte_size: 8,
                operator: omega_abstract_operations::StateGuardOperator::Equal,
            },
        )
    }

    #[test]
    fn runtime_value_guards_track_x86_family_ceiling_and_nested_stack_scratch() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("SysV runtime-value guard boundary");
        let (operands, instruction) = runtime_value_guard_fixture();

        let evidence =
            derive_boundary_runtime_value_guard_footprint(&boundary, &operands, &[instruction])
                .expect("x86 runtime-value guard evidence");

        assert_eq!(
            evidence.registers().as_slice(),
            &[
                MachineRegister::X86Rax,
                MachineRegister::X86Rcx,
                MachineRegister::X86Rdx,
                MachineRegister::X86R8,
                MachineRegister::X86R9,
                MachineRegister::X86R10,
                MachineRegister::X86R11,
                MachineRegister::X86R15,
                MachineRegister::X86Xmm(0),
                MachineRegister::X86Xmm(1),
            ]
        );
        assert!(evidence.machine_state().contains_all(MachineStateSet::new([
            MachineState::Flags,
            MachineState::StackPointer,
            MachineState::ControlState,
        ])));
    }

    #[test]
    fn runtime_value_guards_track_aarch64_recursive_scratch_pool_ceiling() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::Aapcs64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("AAPCS64 runtime-value guard boundary");
        let (operands, instruction) = runtime_value_guard_fixture();

        let evidence =
            derive_boundary_runtime_value_guard_footprint(&boundary, &operands, &[instruction])
                .expect("AArch64 runtime-value guard evidence");

        assert_eq!(
            evidence.registers().as_slice(),
            &[9, 10, 11, 12, 13, 14, 15, 17, 19, 20, 21, 26]
                .map(MachineRegister::Aarch64X)
                .into_iter()
                .chain([MachineRegister::Aarch64V(0), MachineRegister::Aarch64V(1),])
                .collect::<Vec<_>>()
        );
        assert!(evidence.machine_state().contains_all(MachineStateSet::new([
            MachineState::Flags,
            MachineState::ControlState,
        ])));
        assert!(
            !evidence
                .machine_state()
                .contains_all(MachineStateSet::new([MachineState::StackPointer,]))
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
            role: omega_abstract_operations::CopyPlacesRole::ExitIndirectResult,
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
    fn ordinary_pointee_copy_does_not_acquire_indirect_result_footprint() {
        let result = ValueShape::integer(24, 8);
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters: Vec::new(),
                result: Some(result),
            },
        )
        .expect("SysV indirect result");
        let mut instruction = indirect_result_copy_instruction(64, 32, 24);
        let SelectedInstructionKind::CopyPlaces { role, .. } = &mut instruction else {
            unreachable!("helper returns a place copy")
        };
        *role = omega_abstract_operations::CopyPlacesRole::Ordinary;

        let evidence =
            derive_boundary_exit_indirect_result_copy_footprint(&boundary, 32, [&instruction])
                .expect("ordinary copy remains valid outside boundary evidence");

        assert!(evidence.registers().as_slice().is_empty());
        assert!(evidence.machine_state().is_empty());
    }

    #[test]
    fn compiler_body_pointee_copy_footprint_requires_ordinary_role() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters: Vec::new(),
                result: Some(ValueShape::integer(24, 8)),
            },
        )
        .expect("SysV boundary");
        let mut ordinary = indirect_result_copy_instruction(64, 32, 24);
        let SelectedInstructionKind::CopyPlaces { role, .. } = &mut ordinary else {
            unreachable!("helper returns a place copy")
        };
        *role = omega_abstract_operations::CopyPlacesRole::Ordinary;
        let exit = indirect_result_copy_instruction(64, 32, 24);

        let evidence =
            derive_boundary_compiler_body_place_copy_footprint(&boundary, [&ordinary, &exit])
                .expect("ordinary pointee-copy evidence");

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
    fn compiler_body_direct_copy_footprint_uses_exact_encoder_clobbers() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::Aapcs64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("AAPCS64 boundary");
        let instruction = SelectedInstructionKind::CopyPlaces {
            source: omega_abstract_operations::Place::at(
                omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
                4096,
            ),
            target: omega_abstract_operations::Place::at(
                omega_abstract_operations::RuntimeStorageRegion::Machine,
                32,
            ),
            byte_count: 8,
            role: omega_abstract_operations::CopyPlacesRole::Ordinary,
        };

        let evidence =
            derive_boundary_compiler_body_place_copy_footprint(&boundary, [&instruction])
                .expect("ordinary direct-copy evidence");

        assert_eq!(
            evidence.registers().as_slice(),
            &[
                MachineRegister::Aarch64X(16),
                MachineRegister::Aarch64X(17),
                MachineRegister::Aarch64X(19),
                MachineRegister::Aarch64X(26),
            ]
        );
    }

    #[test]
    fn compiler_body_from_pointee_footprint_uses_exact_encoder_clobbers() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::Aapcs64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("AAPCS64 boundary");
        let source = omega_abstract_operations::Place::at(
            omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
            32,
        )
        .with_step(omega_abstract_operations::PlaceStep::Deref)
        .and_then(|place| place.with_step(omega_abstract_operations::PlaceStep::ConstOffset(4096)))
        .expect("from-pointee source");
        let instruction = SelectedInstructionKind::CopyPlaces {
            source,
            target: omega_abstract_operations::Place::at(
                omega_abstract_operations::RuntimeStorageRegion::Machine,
                64,
            ),
            byte_count: 8,
            role: omega_abstract_operations::CopyPlacesRole::Ordinary,
        };

        let evidence =
            derive_boundary_compiler_body_place_copy_footprint(&boundary, [&instruction])
                .expect("ordinary from-pointee evidence");

        assert_eq!(
            evidence.registers().as_slice(),
            &[
                MachineRegister::Aarch64X(16),
                MachineRegister::Aarch64X(17),
                MachineRegister::Aarch64X(19),
                MachineRegister::Aarch64X(20),
            ]
        );
    }

    #[test]
    fn compiler_body_pointee_pair_footprint_uses_exact_encoder_clobbers() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::Aapcs64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("AAPCS64 boundary");
        let pointee = |pointer_offset, field_offset| {
            omega_abstract_operations::Place::at(
                omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
                pointer_offset,
            )
            .with_step(omega_abstract_operations::PlaceStep::Deref)
            .and_then(|place| {
                place.with_step(omega_abstract_operations::PlaceStep::ConstOffset(
                    field_offset,
                ))
            })
            .expect("frame-held pointee")
        };
        let instruction = SelectedInstructionKind::CopyPlaces {
            source: pointee(32, 4096),
            target: pointee(40, 0),
            byte_count: 8,
            role: omega_abstract_operations::CopyPlacesRole::Ordinary,
        };

        let evidence =
            derive_boundary_compiler_body_place_copy_footprint(&boundary, [&instruction])
                .expect("ordinary pointee-pair evidence");

        assert_eq!(
            evidence.registers().as_slice(),
            &[
                MachineRegister::Aarch64X(16),
                MachineRegister::Aarch64X(17),
                MachineRegister::Aarch64X(19),
                MachineRegister::Aarch64X(20),
            ]
        );
    }

    #[test]
    fn compiler_body_from_indexed_footprint_uses_exact_encoder_clobbers() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::Aapcs64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("AAPCS64 boundary");
        let source = omega_abstract_operations::Place::at(
            omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
            32,
        )
        .with_step(omega_abstract_operations::PlaceStep::Deref)
        .and_then(|place| {
            place.with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
                index_region: omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
                index_offset: 40,
                index_byte_size: 8,
                element_byte_size: 24,
            })
        })
        .and_then(|place| place.with_step(omega_abstract_operations::PlaceStep::ConstOffset(16)))
        .expect("single indexed source");
        let instruction = SelectedInstructionKind::CopyPlaces {
            source,
            target: omega_abstract_operations::Place::at(
                omega_abstract_operations::RuntimeStorageRegion::Machine,
                64,
            ),
            byte_count: 8,
            role: omega_abstract_operations::CopyPlacesRole::Ordinary,
        };

        let evidence =
            derive_boundary_compiler_body_place_copy_footprint(&boundary, [&instruction])
                .expect("ordinary from-indexed evidence");

        assert_eq!(
            evidence.registers().as_slice(),
            &[
                MachineRegister::Aarch64X(16),
                MachineRegister::Aarch64X(17),
                MachineRegister::Aarch64X(19),
                MachineRegister::Aarch64X(20),
                MachineRegister::Aarch64X(21),
                MachineRegister::Aarch64X(26),
            ]
        );
    }

    #[test]
    fn compiler_body_to_indexed_footprint_uses_exact_encoder_clobbers() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::Aapcs64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("AAPCS64 boundary");
        let target = omega_abstract_operations::Place::at(
            omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
            32,
        )
        .with_step(omega_abstract_operations::PlaceStep::Deref)
        .and_then(|place| {
            place.with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
                index_region: omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
                index_offset: 40,
                index_byte_size: 8,
                element_byte_size: 24,
            })
        })
        .and_then(|place| place.with_step(omega_abstract_operations::PlaceStep::ConstOffset(16)))
        .expect("single indexed target");
        let instruction = SelectedInstructionKind::CopyPlaces {
            source: omega_abstract_operations::Place::at(
                omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
                64,
            ),
            target,
            byte_count: 8,
            role: omega_abstract_operations::CopyPlacesRole::Ordinary,
        };

        let evidence =
            derive_boundary_compiler_body_place_copy_footprint(&boundary, [&instruction])
                .expect("ordinary to-indexed evidence");

        assert_eq!(
            evidence.registers().as_slice(),
            &[
                MachineRegister::Aarch64X(16),
                MachineRegister::Aarch64X(17),
                MachineRegister::Aarch64X(19),
                MachineRegister::Aarch64X(20),
                MachineRegister::Aarch64X(21),
                MachineRegister::Aarch64X(26),
            ]
        );
    }

    #[test]
    fn compiler_body_indexed_to_pointee_footprint_uses_exact_encoder_clobbers() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::Aapcs64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("AAPCS64 boundary");
        let source = omega_abstract_operations::Place::at(
            omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
            32,
        )
        .with_step(omega_abstract_operations::PlaceStep::Deref)
        .and_then(|place| {
            place.with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
                index_region: omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
                index_offset: 40,
                index_byte_size: 8,
                element_byte_size: 24,
            })
        })
        .and_then(|place| place.with_step(omega_abstract_operations::PlaceStep::ConstOffset(16)))
        .expect("single indexed source");
        let target = omega_abstract_operations::Place::at(
            omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
            64,
        )
        .with_step(omega_abstract_operations::PlaceStep::Deref)
        .and_then(|place| place.with_step(omega_abstract_operations::PlaceStep::ConstOffset(8)))
        .expect("pointee target");
        let instruction = SelectedInstructionKind::CopyPlaces {
            source,
            target,
            byte_count: 8,
            role: omega_abstract_operations::CopyPlacesRole::Ordinary,
        };

        let evidence =
            derive_boundary_compiler_body_place_copy_footprint(&boundary, [&instruction])
                .expect("ordinary indexed-to-pointee evidence");

        assert_eq!(
            evidence.registers().as_slice(),
            &[
                MachineRegister::Aarch64X(16),
                MachineRegister::Aarch64X(17),
                MachineRegister::Aarch64X(19),
                MachineRegister::Aarch64X(20),
                MachineRegister::Aarch64X(21),
                MachineRegister::Aarch64X(26),
            ]
        );
    }

    #[test]
    fn compiler_body_frame_base_indexed_footprint_uses_exact_encoder_clobbers() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::Aapcs64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("AAPCS64 boundary");
        let source = omega_abstract_operations::Place::at(
            omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
            32,
        )
        .with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
            index_region: omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
            index_offset: 40,
            index_byte_size: 8,
            element_byte_size: 24,
        })
        .and_then(|place| place.with_step(omega_abstract_operations::PlaceStep::ConstOffset(16)))
        .expect("frame-base-indexed source");
        let instruction = SelectedInstructionKind::CopyPlaces {
            source,
            target: omega_abstract_operations::Place::at(
                omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
                64,
            ),
            byte_count: 8,
            role: omega_abstract_operations::CopyPlacesRole::Ordinary,
        };

        let evidence =
            derive_boundary_compiler_body_place_copy_footprint(&boundary, [&instruction])
                .expect("ordinary frame-base-indexed evidence");

        assert_eq!(
            evidence.registers().as_slice(),
            &[
                MachineRegister::Aarch64X(16),
                MachineRegister::Aarch64X(17),
                MachineRegister::Aarch64X(20),
                MachineRegister::Aarch64X(24),
                MachineRegister::Aarch64X(26),
            ]
        );
    }

    #[test]
    fn compiler_body_machine_indexed_footprint_uses_exact_encoder_clobbers() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::Aapcs64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("AAPCS64 boundary");
        let source = omega_abstract_operations::Place::at(
            omega_abstract_operations::RuntimeStorageRegion::Machine,
            32,
        )
        .with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
            index_region: omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
            index_offset: 40,
            index_byte_size: 8,
            element_byte_size: 24,
        })
        .and_then(|place| place.with_step(omega_abstract_operations::PlaceStep::ConstOffset(16)))
        .expect("machine-indexed source");
        let instruction = SelectedInstructionKind::CopyPlaces {
            source,
            target: omega_abstract_operations::Place::at(
                omega_abstract_operations::RuntimeStorageRegion::Machine,
                64,
            ),
            byte_count: 8,
            role: omega_abstract_operations::CopyPlacesRole::Ordinary,
        };

        let evidence =
            derive_boundary_compiler_body_place_copy_footprint(&boundary, [&instruction])
                .expect("ordinary machine-indexed evidence");

        assert_eq!(
            evidence.registers().as_slice(),
            &[
                MachineRegister::Aarch64X(16),
                MachineRegister::Aarch64X(17),
                MachineRegister::Aarch64X(19),
                MachineRegister::Aarch64X(20),
                MachineRegister::Aarch64X(26),
            ]
        );
    }

    #[test]
    fn compiler_body_to_machine_indexed_footprint_uses_exact_encoder_clobbers() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::Aapcs64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("AAPCS64 boundary");
        let target = omega_abstract_operations::Place::at(
            omega_abstract_operations::RuntimeStorageRegion::Machine,
            32,
        )
        .with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
            index_region: omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
            index_offset: 40,
            index_byte_size: 8,
            element_byte_size: 24,
        })
        .and_then(|place| place.with_step(omega_abstract_operations::PlaceStep::ConstOffset(16)))
        .expect("machine-indexed target");
        let instruction = SelectedInstructionKind::CopyPlaces {
            source: omega_abstract_operations::Place::at(
                omega_abstract_operations::RuntimeStorageRegion::Machine,
                64,
            ),
            target,
            byte_count: 8,
            role: omega_abstract_operations::CopyPlacesRole::Ordinary,
        };

        let evidence =
            derive_boundary_compiler_body_place_copy_footprint(&boundary, [&instruction])
                .expect("ordinary to-machine-indexed evidence");

        assert_eq!(
            evidence.registers().as_slice(),
            &[
                MachineRegister::Aarch64X(16),
                MachineRegister::Aarch64X(17),
                MachineRegister::Aarch64X(19),
                MachineRegister::Aarch64X(20),
                MachineRegister::Aarch64X(26),
            ]
        );
    }

    #[test]
    fn compiler_body_frame_double_indexed_footprint_uses_both_index_scratches() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("System V boundary");
        let source = omega_abstract_operations::Place::at(
            omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
            32,
        )
        .with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
            index_region: omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
            index_offset: 40,
            index_byte_size: 8,
            element_byte_size: 24,
        })
        .and_then(|place| {
            place.with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
                index_region: omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
                index_offset: 48,
                index_byte_size: 8,
                element_byte_size: 8,
            })
        })
        .expect("frame double-indexed source");
        let instruction = SelectedInstructionKind::CopyPlaces {
            source,
            target: omega_abstract_operations::Place::at(
                omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
                64,
            ),
            byte_count: 8,
            role: omega_abstract_operations::CopyPlacesRole::Ordinary,
        };
        let evidence =
            derive_boundary_compiler_body_place_copy_footprint(&boundary, [&instruction])
                .expect("ordinary frame-double-indexed evidence");
        assert_eq!(
            evidence.registers().as_slice(),
            &[
                MachineRegister::X86Rax,
                MachineRegister::X86R10,
                MachineRegister::X86R11,
                MachineRegister::X86R14,
                MachineRegister::X86R15,
            ]
        );
    }

    #[test]
    fn compiler_body_machine_indexed_pair_reuses_one_x86_index_scratch() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("System V boundary");
        let indexed = |base_offset, index_offset| {
            omega_abstract_operations::Place::at(
                omega_abstract_operations::RuntimeStorageRegion::Machine,
                base_offset,
            )
            .with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
                index_region: omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
                index_offset,
                index_byte_size: 8,
                element_byte_size: 4,
            })
            .expect("machine indexed place")
        };
        let instruction = SelectedInstructionKind::CopyPlaces {
            source: indexed(32, 40),
            target: indexed(32, 48),
            byte_count: 4,
            role: omega_abstract_operations::CopyPlacesRole::Ordinary,
        };
        let evidence =
            derive_boundary_compiler_body_place_copy_footprint(&boundary, [&instruction])
                .expect("ordinary machine-indexed-pair evidence");
        assert_eq!(
            evidence.registers().as_slice(),
            &[
                MachineRegister::X86Rax,
                MachineRegister::X86R11,
                MachineRegister::X86R14,
                MachineRegister::X86R15,
            ]
        );
    }

    #[test]
    fn compiler_body_general_x86_copy_uses_materializer_clobbers() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("System V boundary");
        let target = omega_abstract_operations::Place::at(
            omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
            32,
        )
        .with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
            index_region: omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
            index_offset: 64,
            index_byte_size: 8,
            element_byte_size: 24,
        })
        .and_then(|place| {
            place.with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
                index_region: omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
                index_offset: 72,
                index_byte_size: 8,
                element_byte_size: 8,
            })
        })
        .expect("frame double-indexed target");
        let source = omega_abstract_operations::Place::at(
            omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
            80,
        )
        .with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
            index_region: omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
            index_offset: 88,
            index_byte_size: 8,
            element_byte_size: 8,
        })
        .expect("indexed source keeps the pair in the general class");
        let instruction = SelectedInstructionKind::CopyPlaces {
            source,
            target,
            byte_count: 8,
            role: omega_abstract_operations::CopyPlacesRole::Ordinary,
        };
        assert!(matches!(
            crate::classify_copy_places_shape(
                match &instruction {
                    SelectedInstructionKind::CopyPlaces { source, .. } => source,
                    _ => unreachable!(),
                },
                match &instruction {
                    SelectedInstructionKind::CopyPlaces { target, .. } => target,
                    _ => unreachable!(),
                },
            ),
            crate::CopyPlacesShape::General
        ));
        let evidence =
            derive_boundary_compiler_body_place_copy_footprint(&boundary, [&instruction])
                .expect("ordinary general place-copy evidence");
        assert_eq!(
            evidence.registers().as_slice(),
            &[
                MachineRegister::X86Rax,
                MachineRegister::X86R10,
                MachineRegister::X86R11,
                MachineRegister::X86R14,
                MachineRegister::X86R15,
            ]
        );
    }

    #[test]
    fn compiler_body_direct_integer_write_tracks_large_aarch64_offset_scratch() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::Aapcs64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("AAPCS64 boundary");
        let instruction = SelectedInstructionKind::WritePlaceInteger {
            target: omega_abstract_operations::Place::at(
                omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
                5000,
            ),
            value: 7,
            byte_size: 4,
        };
        let evidence =
            derive_boundary_compiler_body_place_integer_write_footprint(&boundary, [&instruction])
                .expect("ordinary direct integer-write evidence");
        assert_eq!(
            evidence.registers().as_slice(),
            &[
                MachineRegister::Aarch64X(16),
                MachineRegister::Aarch64X(17),
                MachineRegister::Aarch64X(19),
            ]
        );
    }

    #[test]
    fn compiler_body_pointee_integer_write_tracks_large_aarch64_offset_scratch() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::Aapcs64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("AAPCS64 boundary");
        let target = omega_abstract_operations::Place::at(
            omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
            5000,
        )
        .with_step(omega_abstract_operations::PlaceStep::Deref)
        .and_then(|place| place.with_step(omega_abstract_operations::PlaceStep::ConstOffset(16)))
        .expect("frame-held pointee target");
        let instruction = SelectedInstructionKind::WritePlaceInteger {
            target,
            value: 7,
            byte_size: 4,
        };
        let evidence =
            derive_boundary_compiler_body_place_integer_write_footprint(&boundary, [&instruction])
                .expect("ordinary pointee integer-write evidence");
        assert_eq!(
            evidence.registers().as_slice(),
            &[
                MachineRegister::Aarch64X(16),
                MachineRegister::Aarch64X(17),
                MachineRegister::Aarch64X(19),
            ]
        );
    }

    #[test]
    fn compiler_body_cross_region_frame_indexed_integer_write_tracks_aarch64_base() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::Aapcs64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("AAPCS64 boundary");
        let target = omega_abstract_operations::Place::at(
            omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
            16,
        )
        .with_step(omega_abstract_operations::PlaceStep::Deref)
        .and_then(|place| {
            place.with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
                index_region: omega_abstract_operations::RuntimeStorageRegion::Machine,
                index_offset: 64,
                index_byte_size: 8,
                element_byte_size: 24,
            })
        })
        .and_then(|place| place.with_step(omega_abstract_operations::PlaceStep::ConstOffset(8)))
        .expect("cross-region frame-indexed target");
        let instruction = SelectedInstructionKind::WritePlaceInteger {
            target,
            value: 7,
            byte_size: 4,
        };
        let evidence =
            derive_boundary_compiler_body_place_integer_write_footprint(&boundary, [&instruction])
                .expect("ordinary frame-indexed integer-write evidence");
        assert_eq!(
            evidence.registers().as_slice(),
            &[
                MachineRegister::Aarch64X(15),
                MachineRegister::Aarch64X(16),
                MachineRegister::Aarch64X(17),
                MachineRegister::Aarch64X(19),
                MachineRegister::Aarch64X(20),
                MachineRegister::Aarch64X(21),
                MachineRegister::Aarch64X(26),
            ]
        );
    }

    #[test]
    fn compiler_body_cross_region_frame_base_indexed_integer_write_tracks_aarch64_base() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::Aapcs64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("AAPCS64 boundary");
        let target = omega_abstract_operations::Place::at(
            omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
            16,
        )
        .with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
            index_region: omega_abstract_operations::RuntimeStorageRegion::Machine,
            index_offset: 64,
            index_byte_size: 8,
            element_byte_size: 24,
        })
        .and_then(|place| place.with_step(omega_abstract_operations::PlaceStep::ConstOffset(8)))
        .expect("cross-region inline-frame target");
        let instruction = SelectedInstructionKind::WritePlaceInteger {
            target,
            value: 7,
            byte_size: 4,
        };
        let evidence =
            derive_boundary_compiler_body_place_integer_write_footprint(&boundary, [&instruction])
                .expect("ordinary inline-frame integer-write evidence");
        assert_eq!(
            evidence.registers().as_slice(),
            &[
                MachineRegister::Aarch64X(15),
                MachineRegister::Aarch64X(16),
                MachineRegister::Aarch64X(17),
                MachineRegister::Aarch64X(19),
                MachineRegister::Aarch64X(20),
                MachineRegister::Aarch64X(26),
            ]
        );
    }

    #[test]
    fn compiler_body_x86_place_address_tracks_walk_indices_and_flags() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("SysV boundary");
        let source = omega_abstract_operations::Place::at(
            omega_abstract_operations::RuntimeStorageRegion::Machine,
            16,
        )
        .with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
            index_region: omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
            index_offset: 32,
            index_byte_size: 8,
            element_byte_size: 24,
        })
        .and_then(|place| {
            place.with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
                index_region: omega_abstract_operations::RuntimeStorageRegion::Machine,
                index_offset: 48,
                index_byte_size: 8,
                element_byte_size: 8,
            })
        })
        .expect("double-indexed source");
        let instruction = SelectedInstructionKind::WritePlaceAddress {
            source,
            target_offset: 64,
        };

        let evidence =
            derive_boundary_compiler_body_place_address_write_footprint(&boundary, [&instruction])
                .expect("x86 place-address evidence");

        assert_eq!(
            evidence.registers().as_slice(),
            &[
                MachineRegister::X86R10,
                MachineRegister::X86R11,
                MachineRegister::X86R14,
                MachineRegister::X86R15,
            ]
        );
        assert_eq!(
            evidence.machine_state(),
            MachineStateSet::new([MachineState::GeneralRegisters, MachineState::Flags])
        );
    }

    #[test]
    fn compiler_body_aarch64_place_address_tracks_machine_index_and_store_scratch() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::Aapcs64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("AAPCS64 boundary");
        let source = omega_abstract_operations::Place::at(
            omega_abstract_operations::RuntimeStorageRegion::Machine,
            16,
        )
        .with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
            index_region: omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
            index_offset: 32,
            index_byte_size: 8,
            element_byte_size: 24,
        })
        .expect("machine-indexed source");
        let instruction = SelectedInstructionKind::WritePlaceAddress {
            source,
            target_offset: 3,
        };

        let evidence =
            derive_boundary_compiler_body_place_address_write_footprint(&boundary, [&instruction])
                .expect("aarch64 place-address evidence");

        assert_eq!(
            evidence.registers().as_slice(),
            &[
                MachineRegister::Aarch64X(9),
                MachineRegister::Aarch64X(16),
                MachineRegister::Aarch64X(17),
                MachineRegister::Aarch64X(19),
                MachineRegister::Aarch64X(20),
                MachineRegister::Aarch64X(26),
            ]
        );
        assert_eq!(
            evidence.machine_state(),
            MachineStateSet::new([MachineState::GeneralRegisters])
        );
    }

    #[test]
    fn compiler_body_aarch64_place_address_tracks_frame_double_index_scratch() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::Aapcs64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("AAPCS64 boundary");
        let source = omega_abstract_operations::Place::at(
            omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
            16,
        )
        .with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
            index_region: omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
            index_offset: 32,
            index_byte_size: 8,
            element_byte_size: 24,
        })
        .and_then(|place| {
            place.with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
                index_region: omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
                index_offset: 40,
                index_byte_size: 8,
                element_byte_size: 8,
            })
        })
        .expect("frame-double-indexed source");
        let instruction = SelectedInstructionKind::WritePlaceAddress {
            source,
            target_offset: 64,
        };

        let evidence =
            derive_boundary_compiler_body_place_address_write_footprint(&boundary, [&instruction])
                .expect("aarch64 frame-double-indexed place-address evidence");

        assert_eq!(
            evidence.registers().as_slice(),
            &[
                MachineRegister::Aarch64X(14),
                MachineRegister::Aarch64X(16),
                MachineRegister::Aarch64X(17),
                MachineRegister::Aarch64X(20),
                MachineRegister::Aarch64X(26),
            ]
        );
        assert_eq!(
            evidence.machine_state(),
            MachineStateSet::new([MachineState::GeneralRegisters])
        );
    }

    #[test]
    fn compiler_body_aarch64_place_address_tracks_machine_double_index_scratch() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::Aapcs64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("AAPCS64 boundary");
        let source = omega_abstract_operations::Place::at(
            omega_abstract_operations::RuntimeStorageRegion::Machine,
            16,
        )
        .with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
            index_region: omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
            index_offset: 32,
            index_byte_size: 8,
            element_byte_size: 24,
        })
        .and_then(|place| {
            place.with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
                index_region: omega_abstract_operations::RuntimeStorageRegion::Machine,
                index_offset: 40,
                index_byte_size: 8,
                element_byte_size: 8,
            })
        })
        .expect("machine-double-indexed source");
        let instruction = SelectedInstructionKind::WritePlaceAddress {
            source,
            target_offset: 64,
        };

        let evidence =
            derive_boundary_compiler_body_place_address_write_footprint(&boundary, [&instruction])
                .expect("aarch64 machine-double-indexed place-address evidence");

        assert_eq!(
            evidence.registers().as_slice(),
            &[
                MachineRegister::Aarch64X(14),
                MachineRegister::Aarch64X(15),
                MachineRegister::Aarch64X(16),
                MachineRegister::Aarch64X(17),
                MachineRegister::Aarch64X(26),
            ]
        );
        assert_eq!(
            evidence.machine_state(),
            MachineStateSet::new([MachineState::GeneralRegisters])
        );
    }

    #[test]
    fn compiler_body_general_x86_integer_write_uses_materializer_clobbers() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("SysV boundary");
        let target = omega_abstract_operations::Place::at(
            omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
            16,
        )
        .with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
            index_region: omega_abstract_operations::RuntimeStorageRegion::Machine,
            index_offset: 64,
            index_byte_size: 8,
            element_byte_size: 24,
        })
        .expect("cross-region inline frame target");
        assert_eq!(
            crate::classify_write_place_shape(&target),
            crate::WritePlaceShape::Unsupported
        );
        let instruction = SelectedInstructionKind::WritePlaceInteger {
            target,
            value: 7,
            byte_size: 4,
        };
        let evidence =
            derive_boundary_compiler_body_place_integer_write_footprint(&boundary, [&instruction])
                .expect("ordinary general integer-write evidence");
        assert_eq!(
            evidence.registers().as_slice(),
            &[
                MachineRegister::X86Rax,
                MachineRegister::X86R11,
                MachineRegister::X86R15,
            ]
        );
    }

    #[test]
    fn compiler_body_general_x86_binary_write_uses_materializer_ceiling() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("SysV boundary");
        let target = omega_abstract_operations::Place::at(
            omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
            16,
        )
        .with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
            index_region: omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
            index_offset: 64,
            index_byte_size: 8,
            element_byte_size: 24,
        })
        .and_then(|place| {
            place.with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
                index_region: omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
                index_offset: 72,
                index_byte_size: 8,
                element_byte_size: 8,
            })
        })
        .expect("frame double-indexed target");
        assert_eq!(
            crate::classify_write_place_shape(&target),
            crate::WritePlaceShape::Unsupported
        );

        let mut operands = psi_arena::Arena::new();
        let left = operands.insert(omega_abstract_operations::ValueOperand::Immediate(2));
        let right = operands.insert(omega_abstract_operations::ValueOperand::Immediate(3));
        let instruction = SelectedInstructionKind::WritePlaceBinary {
            target,
            byte_size: 4,
            left,
            operator: omega_abstract_operations::StateGuardOperator::Add,
            right,
            is_float: false,
            domain: psi_numerics::arithmetic::ArithmeticDomain::Exact,
            target_signed: true,
        };
        let evidence = derive_boundary_compiler_body_place_binary_write_footprint(
            &boundary,
            &operands,
            [&instruction],
        )
        .expect("ordinary general binary-write evidence");
        assert_eq!(
            evidence.registers(),
            &omega_isa_x86_64::place_binary_write_register_write_ceiling()
        );
        assert_eq!(
            evidence.machine_state(),
            MachineStateSet::new([
                MachineState::GeneralRegisters,
                MachineState::VectorRegisters,
                MachineState::Flags,
                MachineState::StackPointer,
            ])
        );
    }

    #[test]
    fn compiler_body_general_x86_text_assembly_uses_materializer_ceiling() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters: Vec::new(),
                result: None,
            },
        )
        .expect("SysV boundary");
        let target = omega_abstract_operations::Place::at(
            omega_abstract_operations::RuntimeStorageRegion::RuntimeFrame,
            16,
        )
        .with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
            index_region: omega_abstract_operations::RuntimeStorageRegion::Machine,
            index_offset: 64,
            index_byte_size: 8,
            element_byte_size: 24,
        })
        .and_then(|place| {
            place.with_step(omega_abstract_operations::PlaceStep::ScaledIndex {
                index_region: omega_abstract_operations::RuntimeStorageRegion::Machine,
                index_offset: 72,
                index_byte_size: 8,
                element_byte_size: 8,
            })
        })
        .expect("cross-region frame double-indexed target");
        assert_eq!(
            crate::classify_write_place_shape(&target),
            crate::WritePlaceShape::Unsupported
        );
        let instruction = SelectedInstructionKind::MaterializeTextBufferToPlace {
            buffer: psi_arena::Handle::invalid(),
            target,
        };
        let evidence =
            derive_boundary_compiler_body_text_assembly_write_footprint(&boundary, [&instruction])
                .expect("ordinary general text-assembly evidence");
        assert_eq!(
            evidence.registers(),
            &omega_isa_x86_64::place_text_buffer_materialize_register_writes()
        );
        assert!(evidence.machine_state().contains_all(MachineStateSet::new([
            MachineState::GeneralRegisters,
            MachineState::Flags,
        ])));
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

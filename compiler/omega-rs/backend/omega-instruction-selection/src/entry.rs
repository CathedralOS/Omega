use omega_abstract_operations::SelectedInstructionKind;
use omega_calling_conventions::{
    BoundaryEntryPlan, CallSignature, EntryControl, IndirectPointerLocation, MachineStateSet,
    PlanDiagnostic, RegisterSet, StateFootprintEvidence, ValidatedBoundaryEntryPlan, ValueLocation,
    ValuePlacement, ValueShape, validate_boundary_entry_plan,
    validate_call_return_mechanics_footprint, validate_runtime_value_guard_footprint,
    validate_state_footprint,
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
            (omega_target::Architecture::Aarch64, crate::CopyPlacesShape::FromIndexed { .. })
                if source.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame =>
            {
                omega_isa_aarch64::runtime_storage_copy_from_runtime_frame_indexed_clobbers()
            }
            (omega_target::Architecture::X86_64, crate::CopyPlacesShape::ToIndexed { .. })
                if source.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
                    && target.region
                        == omega_target_operations::RuntimeStorageRegion::RuntimeFrame =>
            {
                omega_isa_x86_64::copy_places_to_indexed_clobbers(*byte_count)
            }
            (omega_target::Architecture::Aarch64, crate::CopyPlacesShape::ToIndexed { .. })
                if source.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
                    && target.region
                        == omega_target_operations::RuntimeStorageRegion::RuntimeFrame =>
            {
                omega_isa_aarch64::runtime_storage_copy_to_runtime_frame_indexed_clobbers()
            }
            (omega_target::Architecture::X86_64, crate::CopyPlacesShape::IndexedToPointee { .. })
                if source.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
                    && target.region
                        == omega_target_operations::RuntimeStorageRegion::RuntimeFrame =>
            {
                omega_isa_x86_64::copy_places_indexed_to_pointee_clobbers(*byte_count)
            }
            (
                omega_target::Architecture::Aarch64,
                crate::CopyPlacesShape::IndexedToPointee { .. },
            ) if source.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame
                && target.region == omega_target_operations::RuntimeStorageRegion::RuntimeFrame =>
            {
                omega_isa_aarch64::runtime_storage_copy_from_runtime_frame_indexed_to_runtime_pointee_clobbers()
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
                crate::CopyPlacesShape::FromFrameBaseDoubleIndexed { .. },
            ) => omega_isa_aarch64::runtime_storage_copy_from_runtime_frame_base_double_indexed_clobbers(),
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
            _ => continue,
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

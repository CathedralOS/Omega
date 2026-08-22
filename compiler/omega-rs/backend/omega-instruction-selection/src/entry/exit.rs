//! Boundary exit placement and result-footprint derivation.

use omega_abstract_operations::SelectedInstructionKind;
use omega_calling_conventions::{
    BoundaryEntryPlan, CallSignature, EntryControl, MachineStateSet, PlanDiagnostic, RegisterSet,
    StateFootprintEvidence, ValidatedBoundaryEntryPlan, ValueLocation, ValueShape,
    validate_boundary_entry_plan, validate_state_footprint,
};

/// The observable exit half of one validated boundary plan. Result fragments
/// remain ordered exactly as canonical validation produced them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DerivedBoundaryExit {
    pub control: EntryControl,
    pub result_locations: Vec<ValueLocation>,
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

//! Inbound boundary argument storage and footprint derivation.

use omega_abstract_operations::SelectedInstructionKind;
use omega_calling_conventions::{
    BoundaryEntryPlan, CallSignature, IndirectPointerLocation, MachineStateSet, PlanDiagnostic,
    RegisterSet, StateFootprintEvidence, ValidatedBoundaryEntryPlan, ValueLocation, ValuePlacement,
    ValueShape, validate_boundary_entry_plan, validate_state_footprint,
};

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

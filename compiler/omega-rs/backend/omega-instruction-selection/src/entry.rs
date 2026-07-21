use omega_abstract_operations::SelectedInstructionKind;
use omega_calling_conventions::{
    BoundaryEntryPlan, CallSignature, EntryControl, IndirectPointerLocation, PlanDiagnostic,
    ValueLocation, ValueShape, validate_boundary_entry_plan,
};

/// The observable exit half of one validated boundary plan. Result fragments
/// remain ordered exactly as canonical validation produced them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DerivedBoundaryExit {
    pub control: EntryControl,
    pub result_locations: Vec<ValueLocation>,
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

    Ok(writes)
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
        CallingPolicy, MachineRegime, MachineRegister, ValueShape,
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

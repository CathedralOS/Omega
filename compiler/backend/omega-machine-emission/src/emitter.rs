use crate::MachineEmissionContext;
use crate::branch_distances;
use crate::encoding::encode_machine_instruction_bytes;
use crate::layout::{self, layout_machine_instructions};
use omega_assigned_target_operations::{
    SelectedInstructionKind, StateGuardLowering, StateGuardOperator, TargetOperationKind,
};
use omega_core::arena::{Arena, HandleSpan};
use omega_core::diagnostics::Diagnostic;
use omega_machine_bytes::{EncodedMachineFunction, EncodedMachineInstruction, EncodedMachinePlan};
use omega_machine_instructions::{MachineInstruction, MachineInstructionPlan};
use omega_target::NativeTarget;

#[derive(Debug)]
pub struct MachineEmissionInput<'plan, 'machine> {
    pub target: NativeTarget,
    pub assigned_target_operations:
        &'plan omega_assigned_target_operations::AssignedTargetOperationPlan,
    pub machine_instructions: &'machine MachineInstructionPlan,
    pub host_abi: &'plan omega_calling_conventions::HostAbiPlan,
    pub terminal_dispatch_index: u32,
}

pub fn emit_machine_bytes(
    input: MachineEmissionInput<'_, '_>,
) -> Result<EncodedMachinePlan, Diagnostic> {
    let mut encoded_bytes = EncodedMachinePlan::with_capacity(
        input.target,
        input.machine_instructions.code.functions.len(),
        input.machine_instructions.code.instructions.len(),
        0,
    );

    for (_, function) in input.machine_instructions.code.functions.iter() {
        let byte_offset = encoded_bytes.code.bytes.len();
        emit_function_bytes(
            MachineEmissionContext {
                target: input.target,
                assigned_target_operations: input.assigned_target_operations,
                host_abi: input.host_abi,
                terminal_dispatch_index: input.terminal_dispatch_index,
            },
            input.machine_instructions,
            &mut encoded_bytes,
            function.instructions,
        )?;
        let byte_count = encoded_bytes.code.bytes.len() - byte_offset;
        encoded_bytes.code.functions.insert(EncodedMachineFunction {
            source_key: function.source_key,
            byte_offset,
            byte_count,
        });
    }

    encoded_bytes.code.byte_count = encoded_bytes.code.bytes.len();
    encoded_bytes.semantics.values = input.machine_instructions.semantics.values.clone();
    encoded_bytes.semantics.boundary_edges =
        input.machine_instructions.semantics.boundary_edges.clone();
    encoded_bytes.semantics.ownership = input.machine_instructions.semantics.ownership.clone();

    Ok(encoded_bytes)
}

fn emit_function_bytes(
    emission_context: MachineEmissionContext<'_>,
    machine_instructions: &MachineInstructionPlan,
    encoded_plan: &mut EncodedMachinePlan,
    machine_instructions_span: HandleSpan<MachineInstruction>,
) -> Result<(), Diagnostic> {
    let Some(machine_instructions) = machine_instructions
        .code
        .instructions
        .span(machine_instructions_span)
    else {
        return Ok(());
    };
    let laid_out_instructions =
        layout_machine_instructions(emission_context, machine_instructions)?;
    encoded_plan.code.bytes.reserve(
        laid_out_instructions
            .iter()
            .map(|instruction| instruction.byte_width)
            .sum(),
    );

    for (machine_instruction_index, machine_instruction) in machine_instructions.iter().enumerate()
    {
        let laid_out_instruction = &laid_out_instructions[machine_instruction_index];
        if laid_out_instruction.byte_width == 0 {
            encoded_plan
                .code
                .instructions
                .insert(EncodedMachineInstruction {
                    selected_instruction_index: machine_instruction.selected_instruction_index,
                    bytes: HandleSpan::empty(),
                });
            continue;
        }

        let byte_span = insert_encoded_machine_instruction(
            &mut encoded_plan.code.bytes,
            emission_context,
            &laid_out_instructions,
            machine_instruction_index,
            &machine_instruction.source_kind,
        )?;
        if byte_span.len() != laid_out_instruction.byte_width {
            let operand_note = match &machine_instruction.source_kind {
                SelectedInstructionKind::WriteRuntimeStorageBinary { left, right, .. }
                | SelectedInstructionKind::WriteRuntimePointeeBinary { left, right, .. }
                | SelectedInstructionKind::WriteRuntimeFrameIndexedBinary { left, right, .. } => {
                    format!(
                        "; operands: left={:?}, right={:?}",
                        emission_context
                            .assigned_target_operations
                            .runtime_value_operand(*left)
                            .expect("assigned left runtime value operand should exist"),
                        emission_context
                            .assigned_target_operations
                            .runtime_value_operand(*right)
                            .expect("assigned right runtime value operand should exist"),
                    )
                }
                _ => String::new(),
            };
            return Err(Diagnostic::error(format!(
                "encoded instruction width mismatch for selected #{} ({:?} from {:?}): layout planned {} byte(s), encoder emitted {} byte(s){}",
                machine_instruction.selected_instruction_index,
                machine_instruction.source_kind,
                machine_instruction.kind,
                laid_out_instruction.byte_width,
                byte_span.len(),
                operand_note,
            )));
        }
        encoded_plan
            .code
            .instructions
            .insert(EncodedMachineInstruction {
                selected_instruction_index: machine_instruction.selected_instruction_index,
                bytes: byte_span,
            });
    }

    Ok(())
}

fn insert_encoded_machine_instruction(
    encoded_bytes: &mut Arena<u8>,
    emission_context: MachineEmissionContext<'_>,
    laid_out_instructions: &[layout::LaidOutMachineInstruction],
    machine_instruction_index: usize,
    kind: &TargetOperationKind,
) -> Result<HandleSpan<u8>, Diagnostic> {
    encoded_bytes.try_insert_many_with(|inserter| {
        if insert_fixed_machine_instruction_bytes(
            inserter,
            emission_context,
            laid_out_instructions,
            machine_instruction_index,
            kind,
        )? {
            return Ok(());
        }

        for byte in encode_machine_instruction_bytes(
            emission_context,
            laid_out_instructions,
            machine_instruction_index,
            kind,
        )? {
            inserter.insert(byte);
        }

        Ok(())
    })
}

fn insert_fixed_machine_instruction_bytes(
    inserter: &mut omega_core::arena::ArenaSpanInserter<'_, u8>,
    emission_context: MachineEmissionContext<'_>,
    laid_out_instructions: &[layout::LaidOutMachineInstruction],
    machine_instruction_index: usize,
    kind: &TargetOperationKind,
) -> Result<bool, Diagnostic> {
    match kind {
        SelectedInstructionKind::EnterFunction => {
            let (bytes, byte_count) = omega_instruction_selection::encode_function_enter_bytes(
                emission_context.target.architecture,
            )?;
            for byte in bytes.into_iter().take(byte_count) {
                inserter.insert(byte);
            }
            Ok(true)
        }
        SelectedInstructionKind::EnterDispatchLoop {
            entry_dispatch_index,
            ..
        } => {
            let bytes = omega_instruction_selection::encode_dispatch_loop_enter_bytes(
                emission_context.target.architecture,
                *entry_dispatch_index,
            )?;
            for byte in bytes {
                inserter.insert(byte);
            }
            Ok(true)
        }
        SelectedInstructionKind::EnterDispatchCase { dispatch_index, .. } => {
            let bytes = omega_instruction_selection::encode_dispatch_case_enter_bytes(
                emission_context.target.architecture,
                *dispatch_index,
                branch_distances::byte_distance_to_case_end(
                    laid_out_instructions,
                    machine_instruction_index,
                )?,
            )?;
            for byte in bytes {
                inserter.insert(byte);
            }
            Ok(true)
        }
        SelectedInstructionKind::EvaluateDispatchGuard {
            guard_lowering: StateGuardLowering::CompareStaticValue,
            operator:
                operator @ (StateGuardOperator::Equal
                | StateGuardOperator::NotEqual
                | StateGuardOperator::Greater
                | StateGuardOperator::GreaterOrEqual
                | StateGuardOperator::Less
                | StateGuardOperator::LessOrEqual),
            byte_offset,
            byte_size,
            expected_value,
            has_storage: true,
            ..
        } => {
            let bytes = omega_instruction_selection::encode_dispatch_guard_compare_static_bytes(
                emission_context.target.architecture,
                *byte_offset,
                *byte_size,
                *expected_value,
                branch_distances::byte_distance_to_next_dispatch_action_end(
                    laid_out_instructions,
                    machine_instruction_index,
                )?,
                *operator,
            )?;
            for byte in bytes {
                inserter.insert(byte);
            }
            Ok(true)
        }
        SelectedInstructionKind::CompareRuntimeStorage {
            left_offset,
            right_offset,
            byte_size,
            operator,
            ..
        } => {
            let bytes = omega_instruction_selection::encode_runtime_storage_compare_bytes(
                emission_context.target.architecture,
                *left_offset,
                *right_offset,
                *byte_size,
                branch_distances::byte_distance_to_next_runtime_write_end(
                    emission_context,
                    laid_out_instructions,
                    machine_instruction_index,
                )?,
                *operator,
            )?;
            for byte in bytes {
                inserter.insert(byte);
            }
            Ok(true)
        }
        SelectedInstructionKind::CompareRuntimeStorageValue {
            byte_offset,
            byte_size,
            expected_value,
            operator,
            ..
        } => {
            let bytes = omega_instruction_selection::encode_runtime_storage_value_compare_bytes(
                emission_context.target.architecture,
                *byte_offset,
                *byte_size,
                *expected_value,
                branch_distances::byte_distance_to_next_runtime_write_end(
                    emission_context,
                    laid_out_instructions,
                    machine_instruction_index,
                )?,
                *operator,
            )?;
            for byte in bytes {
                inserter.insert(byte);
            }
            Ok(true)
        }
        SelectedInstructionKind::CompareRuntimeTextStorage {
            source_offset,
            operator,
            ..
        } => {
            let storage_compare_width =
                omega_instruction_selection::runtime_text_storage_compare_width(
                    emission_context.target.architecture,
                );
            let bytes = omega_instruction_selection::encode_runtime_text_storage_compare_bytes(
                emission_context.target.architecture,
                *source_offset,
                branch_distances::byte_distance_to_next_guarded_effect_end(
                    emission_context,
                    laid_out_instructions,
                    machine_instruction_index,
                    40,
                )?,
                branch_distances::byte_distance_to_next_guarded_effect_end(
                    emission_context,
                    laid_out_instructions,
                    machine_instruction_index,
                    storage_compare_width.saturating_sub(4),
                )?,
                *operator == StateGuardOperator::NotEqual,
            )?;
            for byte in bytes {
                inserter.insert(byte);
            }
            Ok(true)
        }
        SelectedInstructionKind::SetDispatchState { dispatch_index } => {
            insert_dispatch_state_write_bytes(
                inserter,
                emission_context,
                laid_out_instructions,
                machine_instruction_index,
                *dispatch_index,
            )?;
            Ok(true)
        }
        SelectedInstructionKind::WriteReturnRegisterInteger { byte_size, value } => {
            let (bytes, byte_count) =
                omega_instruction_selection::encode_return_register_integer_write_bytes(
                    emission_context.target.architecture,
                    *byte_size,
                    *value,
                )?;
            for byte in bytes.into_iter().take(byte_count) {
                inserter.insert(byte);
            }
            Ok(true)
        }
        SelectedInstructionKind::TerminateDispatch => {
            let bytes = omega_instruction_selection::encode_dispatch_state_write_bytes(
                emission_context.target.architecture,
                emission_context.terminal_dispatch_index,
                branch_distances::byte_distance_to_dispatch_loop_leave(
                    emission_context,
                    laid_out_instructions,
                    machine_instruction_index,
                )?,
            )?;
            for byte in bytes {
                inserter.insert(byte);
            }
            Ok(true)
        }
        SelectedInstructionKind::LeaveDispatchCase => {
            let bytes = omega_instruction_selection::encode_dispatch_case_leave_bytes(
                emission_context.target.architecture,
                branch_distances::byte_distance_to_dispatch_loop_start(
                    laid_out_instructions,
                    machine_instruction_index,
                )?,
            )?;
            for byte in bytes {
                inserter.insert(byte);
            }
            Ok(true)
        }
        SelectedInstructionKind::LeaveFunction => {
            let (bytes, byte_count) = omega_instruction_selection::encode_return_bytes(
                emission_context.target.architecture,
            )?;
            for byte in bytes.into_iter().take(byte_count) {
                inserter.insert(byte);
            }
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn insert_dispatch_state_write_bytes(
    inserter: &mut omega_core::arena::ArenaSpanInserter<'_, u8>,
    emission_context: MachineEmissionContext<'_>,
    laid_out_instructions: &[layout::LaidOutMachineInstruction],
    machine_instruction_index: usize,
    dispatch_index: u32,
) -> Result<(), Diagnostic> {
    let bytes = omega_instruction_selection::encode_dispatch_state_write_bytes(
        emission_context.target.architecture,
        dispatch_index,
        branch_distances::byte_distance_to_case_leave(
            laid_out_instructions,
            machine_instruction_index,
        )?,
    )?;
    for byte in bytes {
        inserter.insert(byte);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{MachineEmissionInput, emit_machine_bytes};
    use omega_assigned_target_operations::{AssignedTargetOperationPlan, SelectedInstructionKind};
    use omega_calling_conventions::build_host_abi_plan;
    use omega_core::arena::HandleSpan;
    use omega_machine_instructions::{
        MachineInstruction, MachineInstructionFunction, MachineInstructionKind,
        MachineInstructionPlan,
    };
    use omega_target::NativeTarget;

    #[test]
    fn copies_machine_semantic_summaries_to_encoded_plan() {
        let target = NativeTarget::host();
        let assigned_target_operations = AssignedTargetOperationPlan::default();
        let host_abi = build_host_abi_plan(target);
        let mut machine_instructions = MachineInstructionPlan::with_capacity(target, 1, 1);
        let instructions =
            machine_instructions
                .code
                .instructions
                .insert_many([MachineInstruction {
                    selected_instruction_index: 7,
                    source_kind: SelectedInstructionKind::EnterFunction,
                    kind: MachineInstructionKind::NoOp,
                }]);
        machine_instructions
            .code
            .functions
            .insert(MachineInstructionFunction {
                source_key: Default::default(),
                instructions,
            });
        machine_instructions
            .semantics
            .values
            .values
            .insert(Default::default());
        machine_instructions
            .semantics
            .boundary_edges
            .source_edges
            .insert(Default::default());
        machine_instructions
            .semantics
            .boundary_edges
            .edges
            .insert(Default::default());
        machine_instructions
            .semantics
            .ownership
            .moves
            .insert(Default::default());

        let encoded = emit_machine_bytes(MachineEmissionInput {
            target,
            assigned_target_operations: &assigned_target_operations,
            machine_instructions: &machine_instructions,
            host_abi: &host_abi,
            terminal_dispatch_index: 0,
        })
        .expect("machine emission should preserve semantic summaries");

        assert_eq!(
            encoded.semantics.values.values.len(),
            machine_instructions.semantics.values.values.len()
        );
        assert_eq!(
            encoded.semantics.boundary_edges.source_edges.len(),
            machine_instructions
                .semantics
                .boundary_edges
                .source_edges
                .len()
        );
        assert_eq!(
            encoded.semantics.boundary_edges.edges.len(),
            machine_instructions.semantics.boundary_edges.edges.len()
        );
        assert_eq!(
            encoded.semantics.ownership.moves.len(),
            machine_instructions.semantics.ownership.moves.len()
        );
        assert_eq!(encoded.code.instructions.len(), 1);
        assert!(encoded.code.byte_count > 0);
        assert_ne!(instructions, HandleSpan::empty());
    }
}

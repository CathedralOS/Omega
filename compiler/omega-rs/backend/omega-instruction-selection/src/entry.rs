mod assembly;
mod constant_results;
mod control;
mod direct_imports;
mod exit;
mod guards;
mod inbound;
mod indirect_calls;
mod place_copies;
mod place_writes;
mod runtime_io;
mod runtime_values;
mod syscalls;
mod text;
mod wire;

pub use assembly::derive_boundary_checked_assembly_footprint;
pub use constant_results::derive_boundary_compiler_body_constant_host_result_footprint;
pub use control::{
    derive_boundary_call_return_mechanics_footprint, derive_boundary_dispatch_scaffold_footprint,
};
pub use direct_imports::{
    derive_boundary_compiler_body_outbound_authored_aggregate_import_footprint,
    derive_boundary_compiler_body_outbound_authored_aggregate_import_result_footprint,
    derive_boundary_compiler_body_outbound_authored_aggregate_result_footprint,
    derive_boundary_compiler_body_outbound_authored_float_import_footprint,
    derive_boundary_compiler_body_outbound_authored_float_import_result_footprint,
    derive_boundary_compiler_body_outbound_authored_import_footprint,
    derive_boundary_compiler_body_outbound_authored_import_result_footprint,
    derive_boundary_compiler_body_outbound_data_import_footprint,
    derive_boundary_compiler_body_outbound_data_import_result_footprint,
    derive_boundary_compiler_body_outbound_dereferenced_import_result_footprint,
    derive_boundary_compiler_body_outbound_float_import_result_footprint,
    derive_boundary_compiler_body_outbound_immediate_import_footprint,
    derive_boundary_compiler_body_outbound_immediate_import_result_footprint,
    derive_boundary_compiler_body_outbound_open_create_import_footprint,
    derive_boundary_compiler_body_outbound_storage_import_footprint,
    derive_boundary_compiler_body_outbound_storage_import_result_footprint,
};
pub use exit::{
    DerivedBoundaryExit, derive_boundary_exit, derive_boundary_exit_indirect_result_copy_footprint,
    derive_boundary_exit_result_register_footprint,
};
pub use guards::{
    derive_boundary_place_guard_footprint, derive_boundary_runtime_text_guard_footprint,
    derive_boundary_runtime_value_guard_footprint, derive_boundary_static_guard_footprint,
};
pub use inbound::{
    DerivedBoundaryEntryParameterStorage, DerivedBoundaryEntryStorage,
    derive_boundary_entry_slice_descriptor_footprint, derive_boundary_entry_storage,
    derive_boundary_entry_storage_writes,
};
pub use indirect_calls::derive_boundary_compiler_body_outbound_indirect_call_footprint;
pub use place_copies::derive_boundary_compiler_body_place_copy_footprint;
pub use place_writes::{
    derive_boundary_compiler_body_place_address_write_footprint,
    derive_boundary_compiler_body_place_binary_write_footprint,
    derive_boundary_compiler_body_place_integer_write_footprint,
    derive_boundary_compiler_body_storage_bit_field_write_footprint,
};
pub use runtime_io::{
    derive_boundary_compiler_body_runtime_byte_read_footprint,
    derive_boundary_compiler_body_runtime_byte_write_footprint,
    derive_boundary_compiler_body_runtime_line_read_footprint,
};
pub use runtime_values::{
    derive_boundary_compiler_body_atomic_footprint,
    derive_boundary_compiler_body_storage_convert_write_footprint,
};
pub use syscalls::{
    derive_boundary_compiler_body_outbound_syscall_data_arguments_footprint,
    derive_boundary_compiler_body_outbound_syscall_footprint,
    derive_boundary_compiler_body_outbound_syscall_result_data_arguments_footprint,
    derive_boundary_compiler_body_outbound_syscall_result_footprint,
    derive_boundary_compiler_body_outbound_syscall_result_storage_arguments_footprint,
    derive_boundary_compiler_body_outbound_syscall_storage_arguments_footprint,
    derive_boundary_compiler_body_outbound_syscall_timespec_argument_footprint,
    derive_boundary_compiler_body_outbound_syscall_timespec_result_footprint,
};
pub use text::{
    derive_boundary_compiler_body_place_bounded_buffer_write_footprint,
    derive_boundary_compiler_body_place_string_write_footprint,
    derive_boundary_compiler_body_text_assembly_write_footprint,
};
pub use wire::{
    derive_boundary_compiler_body_wire_byte_slice_read_footprint,
    derive_boundary_compiler_body_wire_expected_byte_read_footprint,
    derive_boundary_compiler_body_wire_literal_byte_append_footprint,
    derive_boundary_compiler_body_wire_nested_close_footprint,
    derive_boundary_compiler_body_wire_nested_open_footprint,
    derive_boundary_compiler_body_wire_repeated_scalar_varint_append_footprint,
    derive_boundary_compiler_body_wire_repeated_scalar_varint_read_footprint,
    derive_boundary_compiler_body_wire_scalar_slice_append_footprint,
    derive_boundary_compiler_body_wire_scalar_varint_append_footprint,
    derive_boundary_compiler_body_wire_scalar_varint_read_footprint,
    derive_boundary_compiler_body_wire_text_bytes_append_footprint,
};

#[cfg(test)]
mod tests {
    use super::*;
    use omega_abstract_operations::SelectedInstructionKind;
    use omega_calling_conventions::{
        CallSignature, CallingPolicy, MachineRegime, MachineRegister, MachineState,
        MachineStateSet, ValueLocation, ValueShape, evaluate_ordinary_boundary_entry_plan,
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
                literal: std::sync::Arc::from(&b"omega"[..]),
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

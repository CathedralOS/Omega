use crate::code::build_encoded_machine_code;
use crate::semantics::build_encoded_machine_semantic_summary;
use omega_machine_bytes::EncodedMachinePlan;
use omega_machine_instructions::MachineInstructionPlan;
use omega_target::NativeTarget;
use psi_diagnostics::Diagnostic;

#[derive(Debug)]
pub struct MachineEmissionInput<'plan, 'machine> {
    pub target: NativeTarget,
    pub assigned_target_operations:
        &'plan omega_assigned_target_operations::AssignedTargetOperationPlan,
    pub machine_instructions: &'machine MachineInstructionPlan,
    pub host_abi: &'plan omega_calling_conventions::HostAbiPlan,
    pub data: &'plan omega_target_operations::TargetDataPlan,
    pub terminal_dispatch_index: u32,
}

pub fn emit_machine_bytes(
    input: MachineEmissionInput<'_, '_>,
) -> Result<EncodedMachinePlan, Diagnostic> {
    Ok(EncodedMachinePlan::with_roots(
        input.target,
        build_encoded_machine_code(&input)?,
        build_encoded_machine_semantic_summary(&input),
    ))
}

#[cfg(test)]
mod tests {
    use super::{MachineEmissionInput, emit_machine_bytes};
    use omega_assigned_target_operations::{AssignedTargetOperationPlan, SelectedInstructionKind};
    use omega_calling_conventions::build_host_abi_plan;
    use omega_calling_conventions::{
        MachineRegister, MachineStateSet, RegisterSet, StateFootprintEvidence,
    };
    use omega_control_flow::{MachineFunctionIdentity, StateKey};
    use omega_machine_instructions::{
        AbstractBoundaryPolicyCheck, AbstractBoundaryPolicyVerdict, BoundaryFootprintFragment,
        BoundaryFootprintFragmentOrigin, MachineInstruction, MachineInstructionFunction,
        MachineInstructionKind, MachineInstructionPlan,
    };
    use omega_target::NativeTarget;
    use psi_arena::HandleSpan;
    use psi_symbols::SymbolHandle;

    #[test]
    fn emits_exact_identity_internal_call_placeholders_for_both_architectures() {
        let caller_key = StateKey {
            machine: SymbolHandle::from_arena_index(1),
            state: SymbolHandle::from_arena_index(2),
            segment_index: 0,
        };
        let target_identity = MachineFunctionIdentity::source(StateKey {
            state: SymbolHandle::from_arena_index(3),
            ..caller_key
        });
        for (target, expected_bytes) in [
            (NativeTarget::linux_x64(), vec![0xe8, 0, 0, 0, 0]),
            (NativeTarget::linux_arm64(), vec![0, 0, 0, 0x94]),
        ] {
            let assigned_target_operations = AssignedTargetOperationPlan::default();
            let host_abi = build_host_abi_plan(target);
            let data = omega_target_operations::TargetDataPlan::default();
            let mut machine_instructions = MachineInstructionPlan::with_capacity(target, 1, 1);
            let instructions =
                machine_instructions
                    .code
                    .instructions
                    .insert_many([MachineInstruction {
                        selected_instruction_index: 0,
                        source_kind: SelectedInstructionKind::CallInternalFunction {
                            target: target_identity,
                        },
                        kind: MachineInstructionKind::InternalFunctionCall,
                    }]);
            machine_instructions
                .code
                .functions
                .insert(MachineInstructionFunction {
                    symbol: "caller".into(),
                    identity: MachineFunctionIdentity::source(caller_key),
                    instructions,
                });

            let encoded = emit_machine_bytes(MachineEmissionInput {
                target,
                assigned_target_operations: &assigned_target_operations,
                machine_instructions: &machine_instructions,
                host_abi: &host_abi,
                data: &data,
                terminal_dispatch_index: 0,
            })
            .expect("internal call placeholder emission");

            assert_eq!(encoded.code.bytes.storage_slice(), expected_bytes);
            let instruction = encoded
                .code
                .instructions
                .iter()
                .next()
                .map(|(_, instruction)| instruction)
                .expect("encoded internal call");
            assert_eq!(
                instruction.compiler_validation_kind,
                Some(
                    omega_machine_bytes::CompilerInstructionValidationKind::InternalFunctionCall {
                        target: target_identity,
                    }
                )
            );
        }
    }

    #[test]
    fn emits_exact_balanced_outgoing_stack_frame_without_relocations() {
        let target = NativeTarget::uefi_x64();
        let assigned_target_operations = AssignedTargetOperationPlan::default();
        let host_abi = build_host_abi_plan(target);
        let data = omega_target_operations::TargetDataPlan::default();
        let mut machine_instructions = MachineInstructionPlan::with_capacity(target, 1, 8);
        let instructions = machine_instructions.code.instructions.insert_many([
            MachineInstruction {
                selected_instruction_index: 0,
                source_kind: SelectedInstructionKind::ReserveOutgoingStackFrame { byte_count: 72 },
                kind: MachineInstructionKind::OutgoingStackFrameReserve,
            },
            MachineInstruction {
                selected_instruction_index: 1,
                source_kind: SelectedInstructionKind::WriteOutgoingStackU64 {
                    stack_byte_offset: 32,
                    value: 0x1000,
                },
                kind: MachineInstructionKind::OutgoingStackU64Write,
            },
            MachineInstruction {
                selected_instruction_index: 2,
                source_kind: SelectedInstructionKind::WriteOutgoingStackU64 {
                    stack_byte_offset: 40,
                    value: 0x800,
                },
                kind: MachineInstructionKind::OutgoingStackU64Write,
            },
            MachineInstruction {
                selected_instruction_index: 3,
                source_kind: SelectedInstructionKind::WriteOutgoingStackU64 {
                    stack_byte_offset: 48,
                    value: 0x8000,
                },
                kind: MachineInstructionKind::OutgoingStackU64Write,
            },
            MachineInstruction {
                selected_instruction_index: 4,
                source_kind: SelectedInstructionKind::WriteOutgoingStackU64 {
                    stack_byte_offset: 56,
                    value: 0x2000,
                },
                kind: MachineInstructionKind::OutgoingStackU64Write,
            },
            MachineInstruction {
                selected_instruction_index: 5,
                source_kind: SelectedInstructionKind::LoadOutgoingStackAddress {
                    register: MachineRegister::X86Rcx,
                    stack_byte_offset: 32,
                },
                kind: MachineInstructionKind::OutgoingStackAddressLoad,
            },
            MachineInstruction {
                selected_instruction_index: 6,
                source_kind: SelectedInstructionKind::LoadOutgoingStackAddress {
                    register: MachineRegister::X86Rdx,
                    stack_byte_offset: 48,
                },
                kind: MachineInstructionKind::OutgoingStackAddressLoad,
            },
            MachineInstruction {
                selected_instruction_index: 7,
                source_kind: SelectedInstructionKind::ReleaseOutgoingStackFrame { byte_count: 72 },
                kind: MachineInstructionKind::OutgoingStackFrameRelease,
            },
        ]);
        machine_instructions
            .code
            .functions
            .insert(MachineInstructionFunction {
                symbol: "synthetic_wrapper".into(),
                identity: MachineFunctionIdentity::default(),
                instructions,
            });
        let encoded = emit_machine_bytes(MachineEmissionInput {
            target,
            assigned_target_operations: &assigned_target_operations,
            machine_instructions: &machine_instructions,
            host_abi: &host_abi,
            data: &data,
            terminal_dispatch_index: 0,
        })
        .expect("outgoing stack-address emission");
        let mut expected =
            omega_isa_x86_64::encode_outgoing_stack_frame_reserve_bytes(72).expect("reserve bytes");
        for (offset, value) in [(32, 0x1000), (40, 0x800), (48, 0x8000), (56, 0x2000)] {
            expected.extend(
                omega_isa_x86_64::encode_outgoing_stack_u64_write_bytes(offset, value)
                    .expect("word bytes"),
            );
        }
        expected.extend(
            omega_isa_x86_64::encode_outgoing_stack_address_load_bytes(MachineRegister::X86Rcx, 32)
                .expect("RCX address"),
        );
        expected.extend(
            omega_isa_x86_64::encode_outgoing_stack_address_load_bytes(MachineRegister::X86Rdx, 48)
                .expect("RDX address"),
        );
        expected.extend(
            omega_isa_x86_64::encode_outgoing_stack_frame_release_bytes(72).expect("release bytes"),
        );
        assert_eq!(encoded.code.bytes.storage_slice(), expected);
        let kinds = encoded
            .code
            .instructions
            .iter()
            .map(|(_, instruction)| instruction.compiler_validation_kind.clone())
            .collect::<Vec<_>>();
        assert_eq!(
            kinds,
            [
                Some(
                    omega_machine_bytes::CompilerInstructionValidationKind::OutgoingStackFrameReserve {
                        byte_count: 72,
                    },
                ),
                Some(
                    omega_machine_bytes::CompilerInstructionValidationKind::OutgoingStackU64Write {
                        stack_byte_offset: 32,
                        value: 0x1000,
                    },
                ),
                Some(
                    omega_machine_bytes::CompilerInstructionValidationKind::OutgoingStackU64Write {
                        stack_byte_offset: 40,
                        value: 0x800,
                    },
                ),
                Some(
                    omega_machine_bytes::CompilerInstructionValidationKind::OutgoingStackU64Write {
                        stack_byte_offset: 48,
                        value: 0x8000,
                    },
                ),
                Some(
                    omega_machine_bytes::CompilerInstructionValidationKind::OutgoingStackU64Write {
                        stack_byte_offset: 56,
                        value: 0x2000,
                    },
                ),
                Some(
                    omega_machine_bytes::CompilerInstructionValidationKind::OutgoingStackAddressLoad {
                        register: MachineRegister::X86Rcx,
                        stack_byte_offset: 32,
                    },
                ),
                Some(
                    omega_machine_bytes::CompilerInstructionValidationKind::OutgoingStackAddressLoad {
                        register: MachineRegister::X86Rdx,
                        stack_byte_offset: 48,
                    },
                ),
                Some(
                    omega_machine_bytes::CompilerInstructionValidationKind::OutgoingStackFrameRelease {
                        byte_count: 72,
                    },
                ),
            ]
        );

        let aarch64_target = NativeTarget::linux_arm64();
        let aarch64_abi = build_host_abi_plan(aarch64_target);
        let diagnostic = emit_machine_bytes(MachineEmissionInput {
            target: aarch64_target,
            assigned_target_operations: &assigned_target_operations,
            machine_instructions: &machine_instructions,
            host_abi: &aarch64_abi,
            data: &data,
            terminal_dispatch_index: 0,
        })
        .expect_err("the RSP-relative operation must remain x86-64-only");
        assert!(diagnostic.message.contains("only on x86-64"));

        let mut aarch64_write = MachineInstructionPlan::with_capacity(aarch64_target, 1, 1);
        let instructions = aarch64_write
            .code
            .instructions
            .insert_many([MachineInstruction {
                selected_instruction_index: 0,
                source_kind: SelectedInstructionKind::WriteOutgoingStackU64 {
                    stack_byte_offset: 32,
                    value: u64::MAX,
                },
                kind: MachineInstructionKind::OutgoingStackU64Write,
            }]);
        aarch64_write
            .code
            .functions
            .insert(MachineInstructionFunction {
                symbol: "synthetic_aarch64_wrapper".into(),
                identity: MachineFunctionIdentity::default(),
                instructions,
            });
        let diagnostic = emit_machine_bytes(MachineEmissionInput {
            target: aarch64_target,
            assigned_target_operations: &assigned_target_operations,
            machine_instructions: &aarch64_write,
            host_abi: &aarch64_abi,
            data: &data,
            terminal_dispatch_index: 0,
        })
        .expect_err("outgoing u64 writes must remain x86-64-only");
        assert!(diagnostic.message.contains("only on x86-64"));
    }

    #[test]
    fn emits_exact_launch_value_copies_for_receiver_free_caller_frame() {
        let target = NativeTarget::uefi_x64();
        let assigned_target_operations = AssignedTargetOperationPlan::default();
        let host_abi = build_host_abi_plan(target);
        let data = omega_target_operations::TargetDataPlan::default();
        let mut machine_instructions = MachineInstructionPlan::with_capacity(target, 1, 8);
        let sources = [
            (MachineRegister::X86Rcx, 0, 32),
            (MachineRegister::X86Rcx, 8, 40),
            (MachineRegister::X86Rdx, 0, 48),
            (MachineRegister::X86Rdx, 8, 56),
        ];
        let mut rows = vec![MachineInstruction {
            selected_instruction_index: 0,
            source_kind: SelectedInstructionKind::ReserveOutgoingStackFrame { byte_count: 72 },
            kind: MachineInstructionKind::OutgoingStackFrameReserve,
        }];
        rows.extend(sources.into_iter().enumerate().map(
            |(index, (source_register, source_byte_offset, stack_byte_offset))| {
                MachineInstruction {
                    selected_instruction_index: index as u32 + 1,
                    source_kind: SelectedInstructionKind::CopyEntryIndirectU64ToOutgoingStack {
                        source_register,
                        source_byte_offset,
                        stack_byte_offset,
                    },
                    kind: MachineInstructionKind::EntryIndirectU64ToOutgoingStackCopy,
                }
            },
        ));
        rows.extend([
            MachineInstruction {
                selected_instruction_index: 5,
                source_kind: SelectedInstructionKind::LoadOutgoingStackAddress {
                    register: MachineRegister::X86Rcx,
                    stack_byte_offset: 32,
                },
                kind: MachineInstructionKind::OutgoingStackAddressLoad,
            },
            MachineInstruction {
                selected_instruction_index: 6,
                source_kind: SelectedInstructionKind::LoadOutgoingStackAddress {
                    register: MachineRegister::X86Rdx,
                    stack_byte_offset: 48,
                },
                kind: MachineInstructionKind::OutgoingStackAddressLoad,
            },
            MachineInstruction {
                selected_instruction_index: 7,
                source_kind: SelectedInstructionKind::ReleaseOutgoingStackFrame { byte_count: 72 },
                kind: MachineInstructionKind::OutgoingStackFrameRelease,
            },
        ]);
        let instructions = machine_instructions.code.instructions.insert_many(rows);
        machine_instructions
            .code
            .functions
            .insert(MachineInstructionFunction {
                symbol: "synthetic_launch_copy".into(),
                identity: MachineFunctionIdentity::default(),
                instructions,
            });

        let encoded = emit_machine_bytes(MachineEmissionInput {
            target,
            assigned_target_operations: &assigned_target_operations,
            machine_instructions: &machine_instructions,
            host_abi: &host_abi,
            data: &data,
            terminal_dispatch_index: 0,
        })
        .expect("exact launch-value copy emission");
        let mut expected =
            omega_isa_x86_64::encode_outgoing_stack_frame_reserve_bytes(72).expect("reserve");
        for (source_register, source_byte_offset, stack_byte_offset) in sources {
            expected.extend(
                omega_isa_x86_64::encode_entry_indirect_u64_to_outgoing_stack_copy_bytes(
                    source_register,
                    source_byte_offset,
                    stack_byte_offset,
                )
                .expect("launch copy"),
            );
        }
        expected.extend(
            omega_isa_x86_64::encode_outgoing_stack_address_load_bytes(MachineRegister::X86Rcx, 32)
                .expect("image address"),
        );
        expected.extend(
            omega_isa_x86_64::encode_outgoing_stack_address_load_bytes(MachineRegister::X86Rdx, 48)
                .expect("storage address"),
        );
        expected.extend(
            omega_isa_x86_64::encode_outgoing_stack_frame_release_bytes(72).expect("release"),
        );
        assert_eq!(encoded.code.bytes.storage_slice(), expected);
        let kinds = encoded
            .code
            .instructions
            .iter()
            .map(|(_, instruction)| instruction.compiler_validation_kind.clone())
            .collect::<Vec<_>>();
        assert_eq!(
            kinds[1],
            Some(
                omega_machine_bytes::CompilerInstructionValidationKind::EntryIndirectU64ToOutgoingStackCopy {
                    source_register: MachineRegister::X86Rcx,
                    source_byte_offset: 0,
                    stack_byte_offset: 32,
                }
            )
        );
        assert_eq!(kinds.len(), 8);
    }

    #[test]
    fn copies_machine_semantic_summaries_and_private_identities_to_encoded_plan() {
        let target = NativeTarget::host();
        let continuation = StateKey {
            machine: SymbolHandle::from_arena_index(1),
            state: SymbolHandle::from_arena_index(2),
            segment_index: 0,
        };
        let wrapper_identity = MachineFunctionIdentity::program_storage_entry_wrapper(continuation)
            .expect("valid continuation should admit wrapper identity");
        let callback_identity = MachineFunctionIdentity::callback_thunk(continuation, 7)
            .expect("valid continuation should admit callback identity");
        let mut assigned_target_operations = AssignedTargetOperationPlan::default();
        assigned_target_operations
            .code
            .runtime_value_operands
            .insert(omega_assigned_target_operations::AssignedValueOperand {
                kind: omega_target_operations::RuntimeValueOperand::Immediate(42),
                home: omega_assigned_target_operations::AssignedValueHomeKind::Immediate,
            });
        let host_abi = build_host_abi_plan(target);
        let data = omega_target_operations::TargetDataPlan::default();
        let mut machine_instructions = MachineInstructionPlan::with_capacity(target, 1, 2);
        // Exercise both halves of the fixed ordinary frame so the semantic
        // summary test also crosses architecture-specific entry/return bytes.
        let instructions = machine_instructions.code.instructions.insert_many([
            MachineInstruction {
                selected_instruction_index: 7,
                source_kind: SelectedInstructionKind::EnterFunction,
                kind: MachineInstructionKind::NoOp,
            },
            MachineInstruction {
                selected_instruction_index: 8,
                source_kind: SelectedInstructionKind::LeaveFunction,
                kind: MachineInstructionKind::NoOp,
            },
        ]);
        machine_instructions
            .code
            .functions
            .insert(MachineInstructionFunction {
                symbol: std::sync::Arc::from("test_entry"),
                identity: MachineFunctionIdentity::source(continuation),
                instructions,
            });
        machine_instructions
            .code
            .functions
            .insert(MachineInstructionFunction {
                symbol: std::sync::Arc::from("__omega_program_storage_entry"),
                identity: wrapper_identity,
                instructions: HandleSpan::empty(),
            });
        machine_instructions
            .code
            .functions
            .insert(MachineInstructionFunction {
                symbol: std::sync::Arc::from("__omega_callback_test"),
                identity: callback_identity,
                instructions: HandleSpan::empty(),
            });
        machine_instructions
            .semantics
            .values
            .values
            .insert(Default::default());
        machine_instructions
            .semantics
            .boundaries
            .source_edges
            .insert(Default::default());
        machine_instructions
            .semantics
            .boundaries
            .edges
            .insert(Default::default());
        machine_instructions
            .semantics
            .boundaries
            .policy_checks
            .insert(AbstractBoundaryPolicyCheck {
                boundary_policy: "omega::host::targets::linux".into(),
                verdict: AbstractBoundaryPolicyVerdict::MissingHostBinding,
                ..Default::default()
            });
        machine_instructions
            .semantics
            .boundaries
            .footprints
            .boundary_contract_fingerprint = Some(0x5678);
        machine_instructions
            .semantics
            .boundaries
            .footprints
            .fragments
            .push(BoundaryFootprintFragment {
                origin: BoundaryFootprintFragmentOrigin::DispatchScaffold,
                evidence: StateFootprintEvidence::new(
                    RegisterSet::new([MachineRegister::X86R12]),
                    MachineStateSet::empty(),
                ),
            });
        machine_instructions
            .semantics
            .ownership
            .permissions
            .insert(Default::default());

        let encoded = emit_machine_bytes(MachineEmissionInput {
            target,
            assigned_target_operations: &assigned_target_operations,
            machine_instructions: &machine_instructions,
            host_abi: &host_abi,
            data: &data,
            terminal_dispatch_index: 0,
        })
        .expect("machine emission should preserve semantic summaries");

        assert_eq!(
            encoded.semantics.values.values.len(),
            machine_instructions.semantics.values.values.len()
        );
        let encoded_function = encoded
            .code
            .functions
            .iter()
            .find(|(_, function)| function.symbol.as_ref() == "test_entry")
            .map(|(_, function)| function)
            .expect("encoded function");
        assert_eq!(encoded_function.symbol.as_ref(), "test_entry");
        assert_eq!(
            encoded_function.identity,
            MachineFunctionIdentity::source(continuation)
        );
        let encoded_wrapper = encoded
            .code
            .functions
            .iter()
            .find(|(_, function)| function.symbol.as_ref() == "__omega_program_storage_entry")
            .map(|(_, function)| function)
            .expect("synthetic wrapper identity carrier");
        assert_eq!(encoded_wrapper.identity, wrapper_identity);
        assert_eq!(encoded_wrapper.byte_count, 0);
        assert_eq!(
            encoded_wrapper
                .identity
                .program_storage_entry_continuation(),
            Some(continuation)
        );
        let encoded_callback = encoded
            .code
            .functions
            .iter()
            .find(|(_, function)| function.symbol.as_ref() == "__omega_callback_test")
            .map(|(_, function)| function)
            .expect("callback thunk identity carrier");
        assert_eq!(encoded_callback.identity, callback_identity);
        assert_eq!(
            encoded_callback.identity.callback_thunk_placement_index(),
            Some(7)
        );
        assert_eq!(encoded_callback.byte_count, 0);
        assert_eq!(encoded_function.instructions.len(), 2);
        assert_eq!(
            encoded
                .code
                .runtime_value_operands
                .iter()
                .next()
                .map(|(_, operand)| operand),
            Some(&omega_target_operations::RuntimeValueOperand::Immediate(42))
        );
        assert_eq!(
            encoded
                .code
                .instructions
                .span(encoded_function.instructions)
                .expect("encoded function instruction rows")
                .len(),
            2
        );
        assert_eq!(
            encoded.semantics.boundaries.source_edges.len(),
            machine_instructions.semantics.boundaries.source_edges.len()
        );
        assert_eq!(
            encoded.semantics.boundaries.edges.len(),
            machine_instructions.semantics.boundaries.edges.len()
        );
        assert_eq!(
            encoded.semantics.boundaries.policy_checks.len(),
            machine_instructions
                .semantics
                .boundaries
                .policy_checks
                .len()
        );
        let check = encoded
            .semantics
            .boundaries
            .policy_checks
            .iter()
            .next()
            .map(|(_, check)| check)
            .expect("encoded boundary policy check");
        assert_eq!(
            check.verdict,
            AbstractBoundaryPolicyVerdict::MissingHostBinding
        );
        assert_eq!(
            encoded.semantics.ownership.permissions.len(),
            machine_instructions.semantics.ownership.permissions.len()
        );
        assert_eq!(
            encoded.semantics.boundaries.footprints,
            machine_instructions.semantics.boundaries.footprints
        );
        assert_eq!(encoded.code.instructions.len(), 2);
        assert!(encoded.code.byte_count > 0);
        assert_ne!(instructions, HandleSpan::empty());
    }
}

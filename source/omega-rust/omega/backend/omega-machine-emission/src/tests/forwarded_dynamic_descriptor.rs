use omega_abstract_operations_to_target_operations::lower_to_target_operations;
use omega_psi_to_abstract_operations::lower_artifact_sections;
use omega_target::{Architecture, NativeTarget};
use omega_target_operations_to_assigned_target_operations::assign_registers;
use psi_proof_admission::AdmissionProfile;
use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use psi_terminal_codec::{encode_module, encode_proof_bundle};
use psi_tokens_to_syntax_trees::parse_syntax_trees;
use psi_typed_trees_to_checked_trees::lower_typed_trees;

fn assigned_plan(target: NativeTarget) -> omega_assigned_target_operations::AssignedOperationPlan {
    let source = r#"
        trait Measure {
            machine measure(&self) -> i32;
            machine alternate(&self) -> i32;
        }

        data Item { value: i32; }

        Primary: Item satisfies Measure {
            machine measure(&self) -> i32 {
                transition { _ -> self.value }
            }

            machine alternate(&self) -> i32 {
                transition { _ -> self.value }
            }
        }

        data Main {
            decoy: Item;
            selected: Item;
        }

        machine Main::run(&mut self) {
            let mut erased: &dyn Measure = &self.decoy as &dyn Item::Primary;
            erased = &self.selected as &dyn Item::Primary;
            let result: i32 = forward(erased);
        }

        machine forward(erased: &dyn Measure) -> i32 {
            let result: i32 = erased.measure();
            transition { _ -> result }
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize source");
    let syntax = parse_syntax_trees(&tokens).expect("parse source");
    let resolved = lower_syntax_trees(&syntax).expect("resolve source");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type source");
    let checked = lower_typed_trees(typed).expect("check source");
    let terminal = psi_checked_trees_to_terminal::lower_machine(&checked, "Main::run")
        .expect("lower forwarded descriptor source");
    let semantic = encode_module(&terminal.semantic_module).expect("encode semantics");
    let proof = encode_proof_bundle(&terminal.proof_bundle).expect("encode proof");
    let abstract_plan = lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default())
        .expect("admit Terminal artifact");
    let target_plan = lower_to_target_operations(&abstract_plan, target)
        .expect("lower caller and helper to target operations");
    assign_registers(&target_plan).expect("assign forwarded descriptor ABI")
}

#[test]
fn emits_forwarded_descriptor_materializations_and_direct_helper_call() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let assigned = assigned_plan(target);
        let emitted = crate::emit_machine_code(&assigned)
            .expect("emit caller and descriptor-dispatch helper");
        let caller = emitted
            .functions
            .iter()
            .find(|function| function.machine == emitted.entry)
            .expect("entry caller");
        let [call] = caller.forwarded_dynamic_descriptor_calls.as_slice() else {
            panic!("one caller-side descriptor call expected: {caller:#?}")
        };
        let [argument] = call.dynamic_arguments.as_slice() else {
            panic!("one descriptor argument expected")
        };
        assert_eq!(call.call_plan.parameters.len(), 2);
        assert_eq!(argument.instance.destination, call.call_plan.parameters[0]);
        assert_eq!(argument.adapters.len(), 2);
        for (row_index, adapter) in argument.adapters.iter().enumerate() {
            assert_eq!(adapter.identity.row_index, row_index as u32);
            assert_eq!(
                adapter.identity.application,
                match &argument.custody.source {
                    omega_target_operations::AbstractDynamicDescriptorSource::Rebound {
                        application,
                        ..
                    } => application.commitment,
                    _ => panic!("caller-local descriptor expected"),
                }
            );
            assert_eq!(adapter.erased_call_plan.parameters.len(), 1);
            assert_eq!(adapter.realization_call_plan.parameters.len(), 1);
            assert_eq!(
                adapter.direct_call_byte_count,
                if target.architecture == Architecture::X86_64 {
                    5
                } else {
                    4
                }
            );
            assert_eq!(
                adapter.return_byte_count,
                if target.architecture == Architecture::X86_64 {
                    1
                } else {
                    4
                }
            );
            match target.architecture {
                Architecture::X86_64 => {
                    assert_eq!(adapter.bytes[adapter.direct_call_offset], 0xe8);
                    assert_eq!(adapter.bytes[adapter.return_offset], 0xc3);
                }
                Architecture::Aarch64 => {
                    assert_eq!(
                        u32::from_le_bytes(
                            adapter.bytes
                                [adapter.direct_call_offset..adapter.direct_call_offset + 4]
                                .try_into()
                                .unwrap()
                        ),
                        0x9400_0000,
                    );
                    assert_eq!(
                        u32::from_le_bytes(
                            adapter.bytes[adapter.return_offset..adapter.return_offset + 4]
                                .try_into()
                                .unwrap()
                        ),
                        0xd65f_03c0,
                    );
                }
            }
        }
        assert_eq!(
            call.direct_call_byte_count,
            if target.architecture == Architecture::X86_64 {
                5
            } else {
                4
            }
        );
        assert!(call.direct_call_offset > argument.table_address.code_offset);
        assert!(caller
            .internal_calls
            .iter()
            .any(|relocation| relocation.target == call.callee));
        match (
            target.architecture,
            argument.table_address.encoding,
            argument.instance_destination,
            argument.table_destination,
        ) {
            (
                Architecture::X86_64,
                omega_machine_code::DynamicTableAddressEncoding::X86_64Relative32 { .. },
                omega_target_operations::MachineRegister::X86Rdi,
                omega_target_operations::MachineRegister::X86Rsi,
            ) => assert_eq!(caller.bytes[call.direct_call_offset], 0xe8),
            (
                Architecture::Aarch64,
                omega_machine_code::DynamicTableAddressEncoding::Aarch64PageAddress { .. },
                omega_target_operations::MachineRegister::Aarch64X(0),
                omega_target_operations::MachineRegister::Aarch64X(1),
            ) => assert_eq!(
                u32::from_le_bytes(
                    caller.bytes[call.direct_call_offset..call.direct_call_offset + 4]
                        .try_into()
                        .unwrap()
                ) & 0xfc00_0000,
                0x9400_0000
            ),
            other => panic!("unexpected target-specific descriptor evidence: {other:?}"),
        }

        let helper = emitted
            .functions
            .iter()
            .find(|function| function.machine == call.callee)
            .expect("forwarded helper");
        assert_eq!(helper.dynamic_parameter_scalar_calls.len(), 1);
    }
}

use super::*;

use omega_abstract_operations::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation,
    AbstractOperationPlan, AbstractParameter, AbstractResult,
};
use omega_machine_code::InternalUnitScalarArgumentSourceRecord;
use omega_target::{Architecture, NativeTarget, ObjectFormat};
use psi_core::{
    BlockId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, OperationId, ScalarType,
    StructuralTypeId, ValueId,
};
use psi_terminal::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

fn attached_scalar_chain() -> AbstractOperationPlan {
    let attached_machine = MachineId::new(1).unwrap();
    let scalar_machine = MachineId::new(2).unwrap();
    let integer_type = IntegerType::new(IntegerSign::Signed, 32).unwrap();
    let scalar_type = ScalarType::Integer(integer_type);
    let parameter = AbstractParameter {
        value: ValueId::new(20).unwrap(),
        scalar_type,
    };

    AbstractOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([0x5a; 32]),
        },
        entry: attached_machine,
        structural_types: Vec::new(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![
            AbstractFunction {
                machine: attached_machine,
                attachment: Some(StructuralTypeId::new(1).unwrap()),
                entry: BlockId::new(1).unwrap(),
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                result: AbstractFunctionResult::Unit,
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![AbstractBlockEntry {
                    block: BlockId::new(1).unwrap(),
                    parameters: Vec::new(),
                    operation_offset: 0,
                }],
                operations: vec![
                    AbstractOperation::IntegerConstant {
                        psi_operation: OperationId::new(10).unwrap(),
                        result: ValueId::new(10).unwrap(),
                        scalar_type,
                        value: IntegerValue::Signed(-17),
                    },
                    AbstractOperation::Call {
                        psi_operation: OperationId::new(11).unwrap(),
                        result: ValueId::new(11).unwrap(),
                        scalar_type,
                        callee: scalar_machine,
                        arguments: vec![ValueId::new(10).unwrap()],
                    },
                    AbstractOperation::Call {
                        psi_operation: OperationId::new(12).unwrap(),
                        result: ValueId::new(12).unwrap(),
                        scalar_type,
                        callee: scalar_machine,
                        arguments: vec![ValueId::new(11).unwrap()],
                    },
                    AbstractOperation::ReturnUnit {
                        psi_edge: EdgeId::new(1).unwrap(),
                        cleanup_actions: Vec::new(),
                    },
                ],
            },
            AbstractFunction {
                machine: scalar_machine,
                attachment: None,
                entry: BlockId::new(2).unwrap(),
                parameters: vec![parameter],
                structural_parameters: Vec::new(),
                result: AbstractFunctionResult::Scalar(AbstractResult {
                    value: ValueId::new(21).unwrap(),
                    scalar_type,
                }),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: Vec::new(),
                operations: vec![AbstractOperation::Return {
                    psi_edge: EdgeId::new(2).unwrap(),
                    result: ValueId::new(21).unwrap(),
                    value: parameter.value,
                    scalar_type,
                    cleanup_actions: Vec::new(),
                }],
            },
        ],
    }
}

#[test]
fn attached_unit_scalar_chain_emits_real_calls_and_durable_homes_on_every_native_target() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let selected = omega_abstract_operations_to_target_operations::lower_to_target_operations(
            &attached_scalar_chain(),
            target,
        )
        .expect("select attached scalar chain");
        let assigned =
            omega_target_operations_to_assigned_target_operations::assign_registers(&selected)
                .expect("assign attached scalar chain");
        let emitted = emit_machine_code(&assigned).expect("emit attached scalar chain");
        let caller = &emitted.functions[0];
        let callee = &emitted.functions[1];

        assert!(caller.fixed_integer_scalar_abi.is_none());
        assert_eq!(
            callee.fixed_integer_scalar_abi,
            selected.functions[1].fixed_integer_scalar_abi
        );
        assert_eq!(caller.unit_integer_constants.len(), 1);
        assert_eq!(caller.unit_scalar_homes.len(), 2);
        assert_eq!(caller.unit_scalar_homes[0].byte_offset, 0);
        assert_eq!(caller.unit_scalar_homes[1].byte_offset, 8);
        assert_eq!(caller.internal_unit_scalar_calls.len(), 2);
        assert_eq!(caller.internal_calls.len(), 2);

        let first = &caller.internal_unit_scalar_calls[0];
        let second = &caller.internal_unit_scalar_calls[1];
        assert!(matches!(
            first.arguments[0].source,
            InternalUnitScalarArgumentSourceRecord::IntegerImmediate {
                value: IntegerValue::Signed(-17),
                ..
            }
        ));
        assert_eq!(
            second.arguments[0].source,
            InternalUnitScalarArgumentSourceRecord::Home(first.result.home)
        );
        assert!(first.code_offset < second.code_offset);
        assert!(first.result.code_offset < second.arguments[0].code_offset);

        for (call, relocation) in caller
            .internal_unit_scalar_calls
            .iter()
            .zip(&caller.internal_calls)
        {
            assert_eq!(call.target, relocation.target);
            assert!(relocation.scalar_stack.is_none());
            assert!(relocation.unit_stack.is_some());
            match target.architecture {
                Architecture::X86_64 => {
                    assert_eq!(caller.bytes[relocation.offset - 1], 0xe8);
                    assert_eq!(
                        &caller.bytes[relocation.offset..relocation.offset + 4],
                        &[0; 4]
                    );
                    let expected = if target.object_format == ObjectFormat::Coff {
                        40
                    } else {
                        8
                    };
                    assert_eq!(
                        relocation
                            .unit_stack
                            .as_ref()
                            .unwrap()
                            .outbound
                            .as_ref()
                            .unwrap()
                            .byte_size,
                        expected
                    );
                }
                Architecture::Aarch64 => {
                    assert_eq!(
                        &caller.bytes[relocation.offset..relocation.offset + 4],
                        &0x9400_0000_u32.to_le_bytes()
                    );
                    assert!(relocation.unit_stack.as_ref().unwrap().outbound.is_none());
                }
            }
        }

        let stack = caller.unit_stack.as_ref().expect("attached Unit frame");
        assert_eq!(
            stack.frame.as_ref().unwrap().byte_size,
            match target.architecture {
                Architecture::X86_64 => 16,
                Architecture::Aarch64 => 32,
            }
        );
        assert_eq!(
            stack.aarch64_return_link.is_some(),
            target.architecture == Architecture::Aarch64
        );
    }
}

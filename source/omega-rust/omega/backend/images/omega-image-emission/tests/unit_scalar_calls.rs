use omega_abstract_operations::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation,
    AbstractOperationPlan, AbstractParameter, AbstractResult,
};
use omega_image_emission::{ObjectError, build_object_artifact};
use omega_target::NativeTarget;
use psi_core::{
    BlockId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId, OperationId, ScalarType,
    StructuralTypeId, ValueId,
};
use psi_terminal::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

fn emitted_scalar_chain(target: NativeTarget) -> omega_machine_code::MachineCodePlan {
    let attached_machine = MachineId::new(1).unwrap();
    let scalar_machine = MachineId::new(2).unwrap();
    let integer_type = IntegerType::new(IntegerSign::Signed, 32).unwrap();
    let scalar_type = ScalarType::Integer(integer_type);
    let parameter = AbstractParameter {
        value: ValueId::new(20).unwrap(),
        scalar_type,
    };
    let source = AbstractOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([0x6b; 32]),
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
    };
    let selected =
        omega_abstract_operations_to_target_operations::lower_to_target_operations(&source, target)
            .unwrap();
    let assigned =
        omega_target_operations_to_assigned_target_operations::assign_registers(&selected).unwrap();
    omega_machine_emission::emit_machine_code(&assigned).unwrap()
}

#[test]
fn object_replays_unit_scalar_call_bytes_and_semantics_on_every_native_target() {
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        build_object_artifact(&emitted_scalar_chain(target)).expect("independent scalar replay");
    }
}

#[test]
fn object_rejects_scalar_argument_result_home_constant_and_abi_tampering() {
    let expected = Err(ObjectError::InvalidInternalUnitScalarCallEvidence(
        MachineId::new(1).unwrap(),
    ));

    let mut argument_bytes = emitted_scalar_chain(NativeTarget::linux_x64());
    let offset = argument_bytes.functions[0].internal_unit_scalar_calls[0].arguments[0].code_offset;
    argument_bytes.functions[0].bytes[offset] ^= 1;
    assert_eq!(build_object_artifact(&argument_bytes), expected);

    let mut result_bytes = emitted_scalar_chain(NativeTarget::linux_x64());
    let offset = result_bytes.functions[0].internal_unit_scalar_calls[0]
        .result
        .code_offset;
    result_bytes.functions[0].bytes[offset] ^= 1;
    assert_eq!(build_object_artifact(&result_bytes), expected);

    let mut home = emitted_scalar_chain(NativeTarget::linux_x64());
    home.functions[0].unit_scalar_homes[0].byte_offset = 8;
    assert_eq!(build_object_artifact(&home), expected);

    let mut constant = emitted_scalar_chain(NativeTarget::linux_x64());
    constant.functions[0].unit_integer_constants[0].value = IntegerValue::Signed(-16);
    assert_eq!(build_object_artifact(&constant), expected);

    let mut abi = emitted_scalar_chain(NativeTarget::linux_x64());
    abi.functions[1]
        .fixed_integer_scalar_abi
        .as_mut()
        .unwrap()
        .parameters[0]
        .scalar_type = IntegerType::new(IntegerSign::Unsigned, 32).unwrap();
    assert_eq!(build_object_artifact(&abi), expected);

    let mut use_before_definition = emitted_scalar_chain(NativeTarget::linux_x64());
    let later_home = use_before_definition.functions[0].unit_scalar_homes[1];
    use_before_definition.functions[0].internal_unit_scalar_calls[1].arguments[0].source =
        omega_machine_code::InternalUnitScalarArgumentSourceRecord::Home(later_home);
    assert_eq!(build_object_artifact(&use_before_definition), expected);

    let mut operation_order = emitted_scalar_chain(NativeTarget::linux_x64());
    operation_order.functions[0].internal_unit_scalar_calls[1].operation_ordinal = 1;
    assert_eq!(build_object_artifact(&operation_order), expected);
}

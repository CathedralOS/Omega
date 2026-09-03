//! Runtime scalar-home source fixture kept separate from literal argument cases.

pub(super) fn runtime_argument_abstract_plan(
    integer_type: psi_core::IntegerType,
    argument_count: usize,
) -> omega_abstract_operations::AbstractOperationPlan {
    let machine = psi_core::MachineId::new(820).unwrap();
    let scalar_machine = psi_core::MachineId::new(821).unwrap();
    let boundary = psi_core::BoundaryMachineId::new(822).unwrap();
    let input = psi_core::ValueId::new(820).unwrap();
    let runtime = psi_core::ValueId::new(821).unwrap();
    let callee_parameter = psi_core::ValueId::new(822).unwrap();
    let callee_result = psi_core::ValueId::new(823).unwrap();
    let scalar_type = psi_core::ScalarType::Integer(integer_type);
    omega_abstract_operations::AbstractOperationPlan {
        psi: psi_terminal::TerminalPsiIdentity {
            vocabulary_marker: psi_terminal::VocabularyMarker::CURRENT,
            program_fingerprint: psi_terminal::SemanticFingerprint::from_bytes([0x82; 32]),
        },
        entry: machine,
        structural_types: Vec::new(),
        boundary_machines: vec![psi_terminal::BoundaryMachineDeclaration {
            id: boundary,
            identity: "omega::test::Foreign::leaf()".into(),
            attachment: None,
            scalar_parameters: vec![scalar_type; argument_count],
            structural_parameters: Vec::new(),
            result: psi_terminal::BoundaryMachineResult::Unit,
            requires: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            published_service_ceiling: Vec::new(),
        }],
        provider_candidates: Vec::new(),
        functions: vec![
            omega_abstract_operations::AbstractFunction {
                machine,
                attachment: Some(psi_core::StructuralTypeId::new(820).unwrap()),
                entry: psi_core::BlockId::new(820).unwrap(),
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                result: omega_abstract_operations::AbstractFunctionResult::Unit,
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![omega_abstract_operations::AbstractBlockEntry {
                    block: psi_core::BlockId::new(820).unwrap(),
                    parameters: Vec::new(),
                    operation_offset: 0,
                }],
                operations: vec![
                    omega_abstract_operations::AbstractOperation::IntegerConstant {
                        psi_operation: psi_core::OperationId::new(820).unwrap(),
                        result: input,
                        scalar_type,
                        value: psi_core::IntegerValue::Signed(37),
                    },
                    omega_abstract_operations::AbstractOperation::Call {
                        psi_operation: psi_core::OperationId::new(821).unwrap(),
                        result: runtime,
                        scalar_type,
                        callee: scalar_machine,
                        arguments: vec![input],
                        requirement_obligations: Vec::new(),
                        crash_continuations: Vec::new(),
                    },
                    omega_abstract_operations::AbstractOperation::BoundaryCall {
                        psi_operation: psi_core::OperationId::new(822).unwrap(),
                        result: None,
                        boundary,
                        arguments: vec![runtime; argument_count],
                        structural_arguments: Vec::new(),
                        completion_claim_sources: Vec::new(),
                        completion_receipts: Vec::new(),
                    },
                    omega_abstract_operations::AbstractOperation::ReturnUnit {
                        psi_edge: psi_core::EdgeId::new(820).unwrap(),
                        cleanup_actions: Vec::new(),
                    },
                ],
            },
            omega_abstract_operations::AbstractFunction {
                machine: scalar_machine,
                attachment: None,
                entry: psi_core::BlockId::new(821).unwrap(),
                parameters: vec![omega_abstract_operations::AbstractParameter {
                    value: callee_parameter,
                    scalar_type,
                }],
                structural_parameters: Vec::new(),
                result: omega_abstract_operations::AbstractFunctionResult::Scalar(
                    omega_abstract_operations::AbstractResult {
                        value: callee_result,
                        scalar_type,
                    },
                ),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: Vec::new(),
                operations: vec![omega_abstract_operations::AbstractOperation::Return {
                    psi_edge: psi_core::EdgeId::new(821).unwrap(),
                    result: callee_result,
                    value: callee_parameter,
                    scalar_type,
                    cleanup_actions: Vec::new(),
                }],
            },
        ],
    }
}

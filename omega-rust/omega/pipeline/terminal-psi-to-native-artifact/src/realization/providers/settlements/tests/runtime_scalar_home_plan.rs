//! Runtime scalar-home source fixture kept separate from literal argument cases.

pub(super) fn runtime_argument_abstract_plan(
    integer_type: semantic_vocabulary::IntegerType,
    argument_count: usize,
) -> abstract_operations::AbstractOperationPlan {
    let machine = semantic_vocabulary::MachineId::new(820).unwrap();
    let scalar_machine = semantic_vocabulary::MachineId::new(821).unwrap();
    let boundary = semantic_vocabulary::BoundaryMachineId::new(822).unwrap();
    let input = semantic_vocabulary::ValueId::new(820).unwrap();
    let runtime = semantic_vocabulary::ValueId::new(821).unwrap();
    let callee_parameter = semantic_vocabulary::ValueId::new(822).unwrap();
    let callee_result = semantic_vocabulary::ValueId::new(823).unwrap();
    let scalar_type = semantic_vocabulary::ScalarType::Integer(integer_type);
    abstract_operations::AbstractOperationPlan {
        psi: terminal_psi::TerminalPsiIdentity {
            vocabulary_marker: terminal_psi::VocabularyMarker::CURRENT,
            program_fingerprint: terminal_psi::SemanticFingerprint::from_bytes([0x82; 32]),
        },
        entry: machine,
        structural_types: Vec::new(),
        boundary_machines: vec![terminal_psi::BoundaryMachineDeclaration {
            id: boundary,
            identity: "omega::test::Foreign::leaf()".into(),
            attachment: None,
            scalar_parameters: vec![scalar_type; argument_count],
            structural_parameters: Vec::new(),
            result: terminal_psi::BoundaryMachineResult::Unit,
            requires: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            published_service_ceiling: Vec::new(),
        }],
        provider_candidates: Vec::new(),
        functions: vec![
            abstract_operations::AbstractFunction {
                machine,
                attachment: Some(semantic_vocabulary::StructuralTypeId::new(820).unwrap()),
                entry: semantic_vocabulary::BlockId::new(820).unwrap(),
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                result: abstract_operations::AbstractFunctionResult::Unit,
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![abstract_operations::AbstractBlockEntry {
                    block: semantic_vocabulary::BlockId::new(820).unwrap(),
                    parameters: Vec::new(),
                    operation_offset: 0,
                }],
                operations: vec![
                    abstract_operations::AbstractOperation::IntegerConstant {
                        psi_operation: semantic_vocabulary::OperationId::new(820).unwrap(),
                        result: input,
                        scalar_type,
                        value: semantic_vocabulary::IntegerValue::Signed(37),
                    },
                    abstract_operations::AbstractOperation::Call {
                        psi_operation: semantic_vocabulary::OperationId::new(821).unwrap(),
                        result: runtime,
                        scalar_type,
                        callee: scalar_machine,
                        arguments: vec![input],
                        requirement_obligations: Vec::new(),
                        crash_continuations: Vec::new(),
                    },
                    abstract_operations::AbstractOperation::BoundaryCall {
                        psi_operation: semantic_vocabulary::OperationId::new(822).unwrap(),
                        result: abstract_operations::AbstractBoundaryResult::Unit,
                        boundary,
                        arguments: vec![runtime; argument_count],
                        structural_arguments: Vec::new(),
                        completion_claim_sources: Vec::new(),
                        completion_receipts: Vec::new(),
                    },
                    abstract_operations::AbstractOperation::ReturnUnit {
                        psi_edge: semantic_vocabulary::EdgeId::new(820).unwrap(),
                        cleanup_actions: Vec::new(),
                    },
                ],
            },
            abstract_operations::AbstractFunction {
                machine: scalar_machine,
                attachment: None,
                entry: semantic_vocabulary::BlockId::new(821).unwrap(),
                parameters: vec![abstract_operations::AbstractParameter {
                    value: callee_parameter,
                    scalar_type,
                }],
                structural_parameters: Vec::new(),
                result: abstract_operations::AbstractFunctionResult::Scalar(
                    abstract_operations::AbstractResult {
                        value: callee_result,
                        scalar_type,
                    },
                ),
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: Vec::new(),
                operations: vec![abstract_operations::AbstractOperation::Return {
                    psi_edge: semantic_vocabulary::EdgeId::new(821).unwrap(),
                    result: callee_result,
                    value: callee_parameter,
                    scalar_type,
                    cleanup_actions: Vec::new(),
                }],
            },
        ],
    }
}

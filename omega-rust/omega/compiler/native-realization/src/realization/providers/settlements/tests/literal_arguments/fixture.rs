//! Literal argument cases and their authenticated abstract-call fixture.

use super::LiteralCase;

pub(super) fn literal_cases(
    count: usize,
    tail_type: semantic_vocabulary::IntegerType,
    tail_immediate: semantic_vocabulary::IntegerValue,
    tail_identity: &'static str,
) -> Vec<LiteralCase> {
    let mut types = vec![
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 16)
            .unwrap(),
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Signed, 64)
            .unwrap(),
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 32)
            .unwrap(),
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 8)
            .unwrap(),
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 32)
            .unwrap(),
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64)
            .unwrap(),
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 16)
            .unwrap(),
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64)
            .unwrap(),
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Signed, 8).unwrap(),
        semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64)
            .unwrap(),
    ];
    let mut immediates = vec![
        semantic_vocabulary::IntegerValue::Unsigned(513),
        semantic_vocabulary::IntegerValue::Signed(-29),
        semantic_vocabulary::IntegerValue::Unsigned(0x1234_5678),
        semantic_vocabulary::IntegerValue::Unsigned(0xa5),
        semantic_vocabulary::IntegerValue::Unsigned(0x89ab_cdef),
        semantic_vocabulary::IntegerValue::Unsigned(0x0123_4567_89ab_cdef),
        semantic_vocabulary::IntegerValue::Unsigned(0x4321),
        semantic_vocabulary::IntegerValue::Unsigned(0x0123_4567_89ab_cdef),
        semantic_vocabulary::IntegerValue::Signed(-7),
        semantic_vocabulary::IntegerValue::Unsigned(0xfedc_ba98_7654_3210),
    ];
    let mut type_identities = vec![
        "u16", "i64", "u32", "u8", "u32", "u64", "u16", "u64", "i8", "u64",
    ];
    assert!(matches!(count, 6 | 8 | 10));
    types[count - 1] = tail_type;
    immediates[count - 1] = tail_immediate;
    type_identities[count - 1] = tail_identity;
    types
        .into_iter()
        .zip(immediates)
        .zip(type_identities)
        .take(count)
        .enumerate()
        .map(
            |(index, ((scalar_type, immediate), type_identity))| LiteralCase {
                operation: semantic_vocabulary::OperationId::new(
                    810 + u64::try_from(index).unwrap(),
                )
                .unwrap(),
                value: semantic_vocabulary::ValueId::new(810 + u64::try_from(index).unwrap())
                    .unwrap(),
                scalar_type,
                immediate,
                type_identity,
            },
        )
        .collect()
}

pub(super) fn abstract_plan(cases: &[LiteralCase]) -> abstract_operations::AbstractOperationPlan {
    let requirement = "omega::test::Foreign::leaf()";
    let machine = semantic_vocabulary::MachineId::new(810).unwrap();
    let block = semantic_vocabulary::BlockId::new(810).unwrap();
    let boundary = semantic_vocabulary::BoundaryMachineId::new(810).unwrap();
    let call_operation = semantic_vocabulary::OperationId::new(830).unwrap();
    let return_edge = semantic_vocabulary::EdgeId::new(810).unwrap();
    let mut operations = cases
        .iter()
        .map(
            |case| abstract_operations::AbstractOperation::IntegerConstant {
                psi_operation: case.operation,
                result: case.value,
                scalar_type: semantic_vocabulary::ScalarType::Integer(case.scalar_type),
                value: case.immediate,
            },
        )
        .collect::<Vec<_>>();
    operations.push(abstract_operations::AbstractOperation::BoundaryCall {
        psi_operation: call_operation,
        result: abstract_operations::AbstractBoundaryResult::Unit,
        boundary,
        arguments: cases.iter().map(|case| case.value).collect(),
        structural_arguments: Vec::new(),
        completion_claim_sources: Vec::new(),
        completion_receipts: Vec::new(),
    });
    operations.push(abstract_operations::AbstractOperation::ReturnUnit {
        psi_edge: return_edge,
        cleanup_actions: Vec::new(),
    });
    abstract_operations::AbstractOperationPlan {
        psi: terminal_psi::TerminalPsiIdentity {
            vocabulary_marker: terminal_psi::VocabularyMarker::CURRENT,
            program_fingerprint: terminal_psi::SemanticFingerprint::from_bytes([0x81; 32]),
        },
        entry: machine,
        structural_types: Vec::new(),
        boundary_machines: vec![terminal_psi::BoundaryMachineDeclaration {
            id: boundary,
            identity: requirement.into(),
            attachment: None,
            scalar_parameters: cases
                .iter()
                .map(|case| semantic_vocabulary::ScalarType::Integer(case.scalar_type))
                .collect(),
            structural_parameters: Vec::new(),
            result: terminal_psi::BoundaryMachineResult::Unit,
            requires: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            published_service_ceiling: Vec::new(),
        }],
        provider_candidates: Vec::new(),
        functions: vec![abstract_operations::AbstractFunction {
            machine,
            attachment: None,
            entry: block,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            result: abstract_operations::AbstractFunctionResult::Unit,
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            block_entries: vec![abstract_operations::AbstractBlockEntry {
                block,
                parameters: Vec::new(),
                operation_offset: 0,
            }],
            operations,
        }],
    }
}

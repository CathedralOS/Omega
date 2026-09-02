//! Literal argument cases and their authenticated abstract-call fixture.

use super::LiteralCase;

pub(super) fn literal_cases(
    count: usize,
    tail_type: psi_core::IntegerType,
    tail_immediate: psi_core::IntegerValue,
    tail_identity: &'static str,
) -> Vec<LiteralCase> {
    let mut types = vec![
        psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 16).unwrap(),
        psi_core::IntegerType::new(psi_core::IntegerSign::Signed, 64).unwrap(),
        psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 32).unwrap(),
        psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 8).unwrap(),
        psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 32).unwrap(),
        psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 64).unwrap(),
        psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 16).unwrap(),
        psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 64).unwrap(),
        psi_core::IntegerType::new(psi_core::IntegerSign::Signed, 8).unwrap(),
        psi_core::IntegerType::new(psi_core::IntegerSign::Unsigned, 64).unwrap(),
    ];
    let mut immediates = vec![
        psi_core::IntegerValue::Unsigned(513),
        psi_core::IntegerValue::Signed(-29),
        psi_core::IntegerValue::Unsigned(0x1234_5678),
        psi_core::IntegerValue::Unsigned(0xa5),
        psi_core::IntegerValue::Unsigned(0x89ab_cdef),
        psi_core::IntegerValue::Unsigned(0x0123_4567_89ab_cdef),
        psi_core::IntegerValue::Unsigned(0x4321),
        psi_core::IntegerValue::Unsigned(0x0123_4567_89ab_cdef),
        psi_core::IntegerValue::Signed(-7),
        psi_core::IntegerValue::Unsigned(0xfedc_ba98_7654_3210),
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
                operation: psi_core::OperationId::new(810 + u64::try_from(index).unwrap()).unwrap(),
                value: psi_core::ValueId::new(810 + u64::try_from(index).unwrap()).unwrap(),
                scalar_type,
                immediate,
                type_identity,
            },
        )
        .collect()
}

pub(super) fn abstract_plan(
    cases: &[LiteralCase],
) -> omega_abstract_operations::AbstractOperationPlan {
    let requirement = "omega::test::Foreign::leaf()";
    let machine = psi_core::MachineId::new(810).unwrap();
    let block = psi_core::BlockId::new(810).unwrap();
    let boundary = psi_core::BoundaryMachineId::new(810).unwrap();
    let call_operation = psi_core::OperationId::new(830).unwrap();
    let return_edge = psi_core::EdgeId::new(810).unwrap();
    let mut operations = cases
        .iter()
        .map(
            |case| omega_abstract_operations::AbstractOperation::IntegerConstant {
                psi_operation: case.operation,
                result: case.value,
                scalar_type: psi_core::ScalarType::Integer(case.scalar_type),
                value: case.immediate,
            },
        )
        .collect::<Vec<_>>();
    operations.push(omega_abstract_operations::AbstractOperation::BoundaryCall {
        psi_operation: call_operation,
        result: None,
        boundary,
        arguments: cases.iter().map(|case| case.value).collect(),
        structural_arguments: Vec::new(),
        completion_claim_sources: Vec::new(),
        completion_receipts: Vec::new(),
    });
    operations.push(omega_abstract_operations::AbstractOperation::ReturnUnit {
        psi_edge: return_edge,
        cleanup_actions: Vec::new(),
    });
    omega_abstract_operations::AbstractOperationPlan {
        psi: psi_terminal::TerminalPsiIdentity {
            vocabulary_marker: psi_terminal::VocabularyMarker::CURRENT,
            program_fingerprint: psi_terminal::SemanticFingerprint::from_bytes([0x81; 32]),
        },
        entry: machine,
        structural_types: Vec::new(),
        boundary_machines: vec![psi_terminal::BoundaryMachineDeclaration {
            id: boundary,
            identity: requirement.into(),
            attachment: None,
            scalar_parameters: cases
                .iter()
                .map(|case| psi_core::ScalarType::Integer(case.scalar_type))
                .collect(),
            structural_parameters: Vec::new(),
            result: None,
            requires: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            published_service_ceiling: Vec::new(),
        }],
        provider_candidates: Vec::new(),
        functions: vec![omega_abstract_operations::AbstractFunction {
            machine,
            attachment: None,
            entry: block,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            result: omega_abstract_operations::AbstractFunctionResult::Unit,
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            block_entries: vec![omega_abstract_operations::AbstractBlockEntry {
                block,
                parameters: Vec::new(),
                operation_offset: 0,
            }],
            operations,
        }],
    }
}

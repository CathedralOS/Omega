//! Scalar elimination must retain array leaves and the complete construction.

use super::*;
use terminal_psi::{
    StructuralMultiplicity, StructuralOperationResult, StructuralTypeDeclaration,
    StructuralTypeShape,
};

#[test]
fn dead_literal_elimination_preserves_repeated_array_leaves_and_construction() {
    let machine = id(1, MachineId::new);
    let block = id(2, BlockId::new);
    let primitive = id(3, StructuralTypeId::new);
    let array_type = id(4, StructuralTypeId::new);
    let leaf = id(5, ValueId::new);
    let constructor = O::EstablishScalarArray {
        psi_operation: id(6, OperationId::new),
        result: StructuralOperationResult {
            place: id(7, PlaceId::new),
            structural_type: array_type,
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            claims: Vec::new(),
        },
        elements: vec![leaf, leaf],
    };
    let plan = AbstractOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([85; 32]),
        },
        entry: machine,
        structural_types: vec![
            StructuralTypeDeclaration {
                id: primitive,
                identity: "test::boolean".into(),
                shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
            },
            StructuralTypeDeclaration {
                id: array_type,
                identity: "test::array".into(),
                shape: StructuralTypeShape::FixedArray {
                    element: primitive,
                    length: 2,
                },
            },
        ]
        .into(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![AbstractFunction {
            machine,
            attachment: None,
            entry: block,
            parameters: Vec::new(),
            structural_parameters: Vec::new(),
            result: AbstractFunctionResult::Unit,
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            block_entries: vec![AbstractBlockEntry {
                block,
                parameters: Vec::new(),
                structural_parameters: Vec::new(),
                operation_offset: 0,
            }],
            operations: vec![
                O::BooleanConstant {
                    psi_operation: id(8, OperationId::new),
                    result: leaf,
                    value: true,
                },
                constructor.clone(),
                O::BooleanConstant {
                    psi_operation: id(9, OperationId::new),
                    result: id(10, ValueId::new),
                    value: false,
                },
                O::ReturnUnit {
                    psi_edge: id(11, EdgeId::new),
                    cleanup_actions: Vec::new(),
                },
            ],
        }],
    };
    let unit = reconstruct_psi_optimization_unit_seed(&plan, FuelScheduleIdentity::new(1).unwrap())
        .unwrap();
    validate_psi_optimization_unit(&unit).unwrap();
    let AnalysisProduct::EffectSummaries(effects) =
        compute_analysis(&unit, AnalysisKind::EffectSummaries).unwrap()
    else {
        panic!("effect summaries")
    };
    assert_eq!(effects.nodes[1].class, crate::EffectClass::StructuralState);
    let contract = DeadScalarLiteralEliminationRule::contract();
    let mut manager = crate::AnalysisManager::new(&unit);
    let products = manager
        .require_all(&unit, contract.required_analyses())
        .unwrap()
        .into_iter()
        .cloned()
        .collect::<Vec<_>>();
    let candidates = DeadScalarLiteralEliminationRule
        .propose(&unit, RuleAnalysisView::new(&products))
        .unwrap();
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].node_decision_point().unwrap().node, 2);
    let accepted = validate_dead_scalar_node_candidate(&unit, &candidates[0]).unwrap();
    validate_psi_optimization_unit(accepted.unit()).unwrap();
    let nodes = &accepted.unit().functions[0].blocks[0].nodes;
    assert_eq!(nodes.len(), 3);
    assert_eq!(nodes[0], unit.functions[0].blocks[0].nodes[0]);
    assert_eq!(nodes[1].operation, constructor);
    assert_eq!(
        nodes[1]
            .uses
            .iter()
            .map(|operand| operand.value)
            .collect::<Vec<_>>(),
        vec![leaf, leaf]
    );
}

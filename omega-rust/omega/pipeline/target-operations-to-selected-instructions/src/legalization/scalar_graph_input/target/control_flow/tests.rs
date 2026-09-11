//! Correspondence-only hostile controls; accepted bounds remain whole-unit custody.
use super::*;
use abstract_operations::{AbstractBlockEntry, AbstractParameter};
use semantic_vocabulary::{
    EdgeId, FuelScheduleIdentity, MachineId, OperationId, PlaceId, StructuralTypeId,
};
use terminal_psi::{
    ByteSequenceCarrier, SemanticFingerprint, StructuralAccess, StructuralMultiplicity,
    StructuralOperationResult, StructuralParameterDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalPsiIdentity, VocabularyMarker,
};

#[test]
fn subslice_row_rejoins_exact_producer_place_and_obligation() {
    let integer = u64_type();
    let scalar_type = ScalarType::Integer(integer);
    let place = PlaceId::new(1).unwrap();
    let derived = PlaceId::new(2).unwrap();
    let structural_type = StructuralTypeId::new(1).unwrap();
    let machine = MachineId::new(1).unwrap();
    let block = BlockId::new(1).unwrap();
    let values = [1, 2, 3].map(|value| ValueId::new(value).unwrap());
    let plan = AbstractOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([71; 32]),
        },
        entry: machine,
        structural_types: vec![StructuralTypeDeclaration {
            id: structural_type,
            identity: "view".into(),
            shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView),
        }]
        .into(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![AbstractFunction {
            machine,
            attachment: None,
            entry: block,
            parameters: values[..2]
                .iter()
                .map(|value| AbstractParameter {
                    value: *value,
                    scalar_type,
                })
                .collect(),
            structural_parameters: vec![StructuralParameterDeclaration {
                place,
                position: 0,
                is_self: false,
                structural_type,
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: StructuralAccess::SharedBorrow,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            }],
            result: AbstractFunctionResult::Unit,
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            block_entries: vec![AbstractBlockEntry {
                structural_parameters: Vec::new(),
                block,
                parameters: Vec::new(),
                operation_offset: 0,
            }],
            operations: vec![
                AbstractOperation::ByteSequenceLength {
                    psi_operation: OperationId::new(1).unwrap(),
                    result: abstract_operations::AbstractResult {
                        value: values[2],
                        scalar_type,
                    },
                    source: place,
                },
                AbstractOperation::ByteSequenceSubslice {
                    psi_operation: OperationId::new(2).unwrap(),
                    result: StructuralOperationResult {
                        place: derived,
                        structural_type,
                        multiplicity: StructuralMultiplicity::Unrestricted,
                        qualifications: Vec::new(),
                        projected_qualifications: Vec::new(),
                        claims: Vec::new(),
                    },
                    source: place,
                    start: values[0],
                    end: values[1],
                    length: values[2],
                    obligation: semantic_vocabulary::ObligationId::new(1).unwrap(),
                },
                AbstractOperation::ReturnUnit {
                    psi_edge: EdgeId::new(1).unwrap(),
                    cleanup_actions: Vec::new(),
                },
            ],
        }],
    };
    // This test joins descriptions only: the parameters do not assert bounds.
    // Production legalization must additionally import the verifier's catalog.
    let target = abstract_operations_to_target_operations::lower_to_target_operations(
        &plan,
        ::target::NativeTarget::linux_x64(),
    )
    .unwrap();
    let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &plan,
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    let check = |function: &TargetFunction| {
        super::super::validate_target(
            function,
            &plan.functions[0],
            &unit.functions[0],
            &target,
            &plan,
            &unit,
        )
    };
    check(&target.functions[0]).unwrap();
    let mut invented = target.functions[0].clone();
    let TargetOperation::ControlGraph(graph) = &mut invented.operation else {
        panic!("graph");
    };
    let TargetUnitOperation::ByteSequenceSubslice { view, .. } = &mut graph.blocks[0].operations[1]
    else {
        panic!("subslice");
    };
    let target_operations::TargetByteView::Subslice { source, .. } = view else {
        panic!("view");
    };
    **source = target_operations::TargetByteView::BlockParameter {
        block: unit.functions[0].entry,
        place,
        structural_type,
    };
    assert!(
        check(&invented).is_err(),
        "machine parameter cannot masquerade as a block descriptor"
    );
    for mutation in 0..5 {
        let mut changed = target.functions[0].clone();
        let TargetOperation::ControlGraph(graph) = &mut changed.operation else {
            panic!("graph");
        };
        let TargetUnitOperation::ByteSequenceSubslice { result, view } =
            &mut graph.blocks[0].operations[1]
        else {
            panic!("subslice");
        };
        let target_operations::TargetByteView::Subslice {
            psi_operation,
            place,
            obligation,
            length,
            ..
        } = view
        else {
            panic!("view");
        };
        match mutation {
            0 => *psi_operation = OperationId::new(99).unwrap(),
            1 => result.place = PlaceId::new(99).unwrap(),
            2 => *place = PlaceId::new(99).unwrap(),
            3 => *obligation = semantic_vocabulary::ObligationId::new(99).unwrap(),
            _ => *length = values[1],
        }
        assert!(check(&changed).is_err(), "mutation {mutation}");
    }
}

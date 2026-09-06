//! Explicit type, authored move, and expected cleanup fixtures.

use abstract_operations::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation,
    AbstractOperationPlan,
};
use optimization_unit::{PsiOptimizationUnit, reconstruct_psi_optimization_unit_seed};
use semantic_vocabulary::{
    BlockId, EdgeId, FuelScheduleIdentity, MachineId, OperationId, PlaceId, StructuralFieldId,
    StructuralTypeId,
};
use terminal_psi::{
    BindingRelevance, SemanticFingerprint, StructuralAccess, StructuralAffineDiscard,
    StructuralArgument, StructuralFieldDeclaration, StructuralFieldType, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralPathSegment, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalAffineCleanupAction, TerminalPsiIdentity, VocabularyMarker,
};

use crate::tests::id;

pub(super) fn field(identity: &str) -> StructuralPathSegment {
    StructuralPathSegment::Field(identity.into())
}

pub(super) fn index(value: u64) -> StructuralPathSegment {
    StructuralPathSegment::FixedIndex(value)
}

pub(super) fn record(identity: u64, fields: &[(&str, u64)]) -> StructuralTypeDeclaration {
    StructuralTypeDeclaration {
        id: id(identity, StructuralTypeId::new),
        identity: format!("validation::general-record-{identity}"),
        shape: StructuralTypeShape::Record {
            fields: fields
                .iter()
                .enumerate()
                .map(|(position, (name, child))| StructuralFieldDeclaration {
                    id: id(position as u64 + 1, StructuralFieldId::new),
                    identity: (*name).into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Structural(id(*child, StructuralTypeId::new)),
                })
                .collect(),
        },
    }
}

pub(super) fn array(identity: u64, element: u64, length: u64) -> StructuralTypeDeclaration {
    StructuralTypeDeclaration {
        id: id(identity, StructuralTypeId::new),
        identity: format!("validation::general-array-{identity}"),
        shape: StructuralTypeShape::FixedArray {
            element: id(element, StructuralTypeId::new),
            length,
        },
    }
}

pub(super) fn unit(
    structural_types: Vec<StructuralTypeDeclaration>,
    root_type: u64,
    moves: &[(Vec<StructuralPathSegment>, u64)],
    residuals: &[(Vec<StructuralPathSegment>, u64)],
) -> PsiOptimizationUnit {
    let caller = id(1_001, MachineId::new);
    let caller_block = id(1_002, BlockId::new);
    let caller_place = id(1_000, PlaceId::new);
    let parameter = |place, structural_type| StructuralParameterDeclaration {
        place,
        position: 0,
        is_self: false,
        structural_type,
        multiplicity: StructuralMultiplicity::Affine,
        access: StructuralAccess::Owned,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let function = |machine, block, parameter, operations| AbstractFunction {
        machine,
        attachment: None,
        entry: block,
        parameters: Vec::new(),
        structural_parameters: vec![parameter],
        result: AbstractFunctionResult::Unit,
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        block_entries: vec![AbstractBlockEntry {
            block,
            parameters: Vec::new(),
            operation_offset: 0,
        }],
        operations,
    };
    let mut callees = Vec::new();
    let mut operations = Vec::new();
    for (position, (path, moved_type)) in moves.iter().enumerate() {
        let ordinal = position as u64;
        let callee = id(2_000 + ordinal, MachineId::new);
        let place = id(5_000 + ordinal, PlaceId::new);
        callees.push(function(
            callee,
            id(3_000 + ordinal, BlockId::new),
            parameter(place, id(*moved_type, StructuralTypeId::new)),
            vec![AbstractOperation::ReturnUnit {
                psi_edge: id(4_000 + ordinal, EdgeId::new),
                cleanup_actions: vec![TerminalAffineCleanupAction::DiscardRoot(place)],
            }],
        ));
        operations.push(AbstractOperation::CallUnit {
            psi_operation: id(6_000 + ordinal, OperationId::new),
            callee,
            arguments: Vec::new(),
            structural_arguments: vec![StructuralArgument {
                place: caller_place,
                path: path.clone(),
                access: StructuralAccess::Owned,
            }],
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
        });
    }
    operations.push(AbstractOperation::ReturnUnit {
        psi_edge: id(7_000, EdgeId::new),
        cleanup_actions: residuals
            .iter()
            .map(|(path, structural_type)| {
                TerminalAffineCleanupAction::DiscardResidual(StructuralAffineDiscard {
                    place: caller_place,
                    path: path.clone(),
                    structural_type: id(*structural_type, StructuralTypeId::new),
                })
            })
            .collect(),
    });
    let mut functions = vec![function(
        caller,
        caller_block,
        parameter(caller_place, id(root_type, StructuralTypeId::new)),
        operations,
    )];
    functions.extend(callees);
    reconstruct_psi_optimization_unit_seed(
        &AbstractOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([51; 32]),
            },
            entry: caller,
            structural_types,
            boundary_machines: Vec::new(),
            provider_candidates: Vec::new(),
            functions,
        },
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap()
}

pub(super) fn mixed_unit() -> PsiOptimizationUnit {
    unit(
        vec![
            record(1, &[]),
            record(2, &[("left", 1), ("right", 1)]),
            array(3, 2, 3),
            record(4, &[("rows", 3), ("tail", 1)]),
        ],
        4,
        &[
            (vec![field("rows"), index(0), field("left")], 1),
            (vec![field("rows"), index(2), field("right")], 1),
        ],
        &[
            (vec![field("tail")], 1),
            (vec![field("rows"), index(2), field("left")], 1),
            (vec![field("rows"), index(1)], 2),
            (vec![field("rows"), index(0), field("right")], 1),
        ],
    )
}

pub(super) fn cleanup_actions(
    unit: &mut PsiOptimizationUnit,
) -> &mut Vec<TerminalAffineCleanupAction> {
    let AbstractOperation::ReturnUnit {
        cleanup_actions, ..
    } = &mut unit.functions[0].blocks[0]
        .nodes
        .last_mut()
        .unwrap()
        .operation
    else {
        panic!("fixture ends in a Unit return");
    };
    cleanup_actions
}

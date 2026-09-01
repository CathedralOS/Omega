use super::super::{id, refresh_identity};
use omega_abstract_operations::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation,
    AbstractOperationPlan,
};
use omega_optimization_unit::{PsiOptimizationUnit, reconstruct_psi_optimization_unit_seed};
use psi_core::{
    BlockId, ClaimId, EdgeId, FuelScheduleIdentity, MachineId, OperationId, PlaceId,
    StructuralPlaceKind, StructuralTypeId, ValueId,
};
use psi_terminal::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

pub(crate) fn compressed_trivial_affine_return_unit_with_prefix(
    executable_collision: bool,
    explicit_witnesses: bool,
) -> PsiOptimizationUnit {
    let machine = id(360, MachineId::new);
    let block = id(361, BlockId::new);
    let structural_type = id(362, StructuralTypeId::new);
    let source = id(363, PlaceId::new);
    let first_tail = id(364, PlaceId::new);
    let second_tail = id(365, PlaceId::new);
    let result = id(366, PlaceId::new);
    let first_local = id(367, PlaceId::new);
    let second_local = id(368, PlaceId::new);
    let claim = id(1, ClaimId::new);
    let local_type = psi_terminal::StructuralTypeDeclaration {
        id: structural_type,
        identity: "validation::trivial-affine-empty-record".into(),
        shape: psi_terminal::StructuralTypeShape::Record { fields: Vec::new() },
    };
    let parameter = |place, position, multiplicity| psi_terminal::StructuralParameterDeclaration {
        place,
        position,
        is_self: false,
        structural_type,
        multiplicity,
        access: psi_terminal::StructuralAccess::Owned,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    };
    let local = |place, declaration_ordinal| psi_terminal::StructuralPlaceDeclaration {
        id: place,
        kind: StructuralPlaceKind::TrivialAffineLocal {
            declaration_ordinal,
            structural_type,
            construction: None,
        },
    };
    let first_declaration = local(first_local, 0);
    let second_declaration = local(second_local, 1);
    let mut operations = Vec::new();
    if executable_collision {
        operations.push(AbstractOperation::BooleanConstant {
            psi_operation: id(371, OperationId::new),
            result: id(389, ValueId::new),
            value: false,
        });
    }
    if explicit_witnesses {
        operations.extend([
            AbstractOperation::EstablishTrivialAffineLocal {
                psi_operation: id(373, OperationId::new),
                place: first_declaration,
                structural_type: local_type.clone(),
            },
            AbstractOperation::EstablishTrivialAffineLocal {
                psi_operation: id(374, OperationId::new),
                place: second_declaration,
                structural_type: local_type.clone(),
            },
        ]);
    }
    operations.push(AbstractOperation::ReturnStructural {
        psi_edge: id(370, EdgeId::new),
        source,
        returned_claims: vec![claim],
        trivial_affine_locals: vec![
            (
                id(371, OperationId::new),
                first_declaration,
                local_type.clone(),
            ),
            (
                id(372, OperationId::new),
                second_declaration,
                local_type.clone(),
            ),
        ],
        trivial_affine_discards: vec![second_local, first_local, second_tail, first_tail],
    });
    let plan = AbstractOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([18; 32]),
        },
        entry: machine,
        structural_types: vec![local_type.clone()],
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![AbstractFunction {
            machine,
            attachment: None,
            entry: block,
            parameters: Vec::new(),
            structural_parameters: vec![
                parameter(source, 0, psi_terminal::StructuralMultiplicity::Linear),
                parameter(first_tail, 1, psi_terminal::StructuralMultiplicity::Affine),
                parameter(second_tail, 2, psi_terminal::StructuralMultiplicity::Affine),
            ],
            result: AbstractFunctionResult::Structural(psi_terminal::StructuralResultDeclaration {
                place: result,
                structural_type,
                multiplicity: psi_terminal::StructuralMultiplicity::Linear,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            }),
            entry_claims: vec![psi_terminal::EntryClaim {
                claim,
                input: source,
                path: Vec::new(),
            }],
            published_service_ceiling: Vec::new(),
            block_entries: vec![AbstractBlockEntry {
                block,
                parameters: Vec::new(),
                operation_offset: 0,
            }],
            operations,
        }],
    };
    let mut unit = reconstruct_psi_optimization_unit_seed(
        &plan,
        FuelScheduleIdentity::new(1).expect("nonzero schedule"),
    )
    .expect("compressed structural return unit");
    for declaration in [first_declaration, second_declaration] {
        if !unit.functions[0]
            .structural_places
            .iter()
            .any(|place| place.id == declaration.id)
        {
            unit.functions[0].structural_places.push(declaration);
        }
    }
    refresh_identity(&mut unit);
    unit
}

pub(crate) fn compressed_trivial_affine_return_unit() -> PsiOptimizationUnit {
    compressed_trivial_affine_return_unit_with_prefix(false, false)
}

pub(crate) fn explicit_trivial_affine_return_unit() -> PsiOptimizationUnit {
    let machine = id(390, MachineId::new);
    let block = id(391, BlockId::new);
    let structural_type = id(392, StructuralTypeId::new);
    let place = id(393, PlaceId::new);
    let structural_type_declaration = psi_terminal::StructuralTypeDeclaration {
        id: structural_type,
        identity: "validation::explicit-trivial-affine-empty-record".into(),
        shape: psi_terminal::StructuralTypeShape::Record { fields: Vec::new() },
    };
    let place_declaration = psi_terminal::StructuralPlaceDeclaration {
        id: place,
        kind: StructuralPlaceKind::TrivialAffineLocal {
            declaration_ordinal: 0,
            structural_type,
            construction: None,
        },
    };
    reconstruct_psi_optimization_unit_seed(
        &AbstractOperationPlan {
            psi: TerminalPsiIdentity {
                vocabulary_marker: VocabularyMarker::CURRENT,
                program_fingerprint: SemanticFingerprint::from_bytes([19; 32]),
            },
            entry: machine,
            structural_types: vec![structural_type_declaration.clone()],
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
                    operation_offset: 0,
                }],
                operations: vec![
                    AbstractOperation::EstablishTrivialAffineLocal {
                        psi_operation: id(394, OperationId::new),
                        place: place_declaration,
                        structural_type: structural_type_declaration,
                    },
                    AbstractOperation::ReturnUnit {
                        psi_edge: id(395, EdgeId::new),
                        cleanup_actions: vec![
                            psi_terminal::TerminalAffineCleanupAction::DiscardRoot(place),
                        ],
                    },
                ],
            }],
        },
        FuelScheduleIdentity::new(1).expect("nonzero schedule"),
    )
    .expect("explicit affine local unit")
}

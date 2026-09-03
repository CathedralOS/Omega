use super::super::{id, refresh_identity};
use super::type_declarations::{structural_leaf_field, structural_type};
use omega_abstract_operations::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation,
    AbstractOperationPlan,
};
use omega_optimization_unit::{PsiOptimizationUnit, reconstruct_psi_optimization_unit_seed};
use psi_core::{
    BlockId, BoundaryMachineId, EdgeId, FuelScheduleIdentity, MachineId, OperationId, PlaceId,
    StructuralPlaceKind, StructuralTypeId,
};
use psi_terminal::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

pub(crate) fn provider_attachment_specialization_unit() -> PsiOptimizationUnit {
    let machine = id(440, MachineId::new);
    let block = id(441, BlockId::new);
    let attachment = id(444, StructuralTypeId::new);
    let provider_field = id(1, psi_core::StructuralFieldId::new);
    let first_boundary = id(446, BoundaryMachineId::new);
    let second_boundary = id(447, BoundaryMachineId::new);
    let unused_boundary = id(448, BoundaryMachineId::new);
    let boundary = |id, identity: &str| psi_terminal::BoundaryMachineDeclaration {
        id,
        identity: identity.into(),
        attachment: None,
        scalar_parameters: Vec::new(),
        structural_parameters: Vec::new(),
        result: psi_terminal::BoundaryMachineResult::Unit,
        requires: Vec::new(),
        program_local_root_introductions: Vec::new(),
        content_guarantees: Vec::new(),
        published_service_ceiling: Vec::new(),
    };
    let call = |psi_operation, boundary| AbstractOperation::BoundaryCall {
        psi_operation,
        result: None,
        boundary,
        arguments: Vec::new(),
        structural_arguments: Vec::new(),
        completion_claim_sources: Vec::new(),
        completion_receipts: Vec::new(),
    };
    let plan = AbstractOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([44; 32]),
        },
        entry: machine,
        structural_types: vec![structural_type(
            444,
            psi_terminal::StructuralTypeShape::Record {
                fields: vec![structural_leaf_field(
                    1,
                    psi_terminal::BindingRelevance::Relevant,
                    psi_terminal::StructuralFieldType::Erased {
                        type_identity: "validation::provider".into(),
                    },
                )],
            },
        )],
        boundary_machines: vec![
            boundary(first_boundary, "validation::provider-first"),
            boundary(second_boundary, "validation::provider-second"),
            boundary(unused_boundary, "validation::provider-unused"),
        ],
        provider_candidates: Vec::new(),
        functions: vec![AbstractFunction {
            machine,
            attachment: Some(attachment),
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
                call(id(449, OperationId::new), first_boundary),
                call(id(450, OperationId::new), first_boundary),
                call(id(451, OperationId::new), second_boundary),
                AbstractOperation::ReturnUnit {
                    psi_edge: id(452, EdgeId::new),
                    cleanup_actions: Vec::new(),
                },
            ],
        }],
    };
    let mut unit = reconstruct_psi_optimization_unit_seed(
        &plan,
        FuelScheduleIdentity::new(1).expect("nonzero schedule"),
    )
    .expect("provider specialization fixture");
    unit.functions[0].structural_places.extend([
        psi_terminal::StructuralPlaceDeclaration {
            id: id(445, PlaceId::new),
            kind: StructuralPlaceKind::ProviderAttachment {
                attachment,
                field: provider_field,
                boundary: first_boundary,
            },
        },
        psi_terminal::StructuralPlaceDeclaration {
            id: id(446, PlaceId::new),
            kind: StructuralPlaceKind::ProviderAttachment {
                attachment,
                field: provider_field,
                boundary: second_boundary,
            },
        },
    ]);
    refresh_identity(&mut unit);
    unit
}

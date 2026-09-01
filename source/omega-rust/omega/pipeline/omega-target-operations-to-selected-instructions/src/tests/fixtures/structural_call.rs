//! Authored structural-call source and exact qualified optimization-unit reconstruction.

use omega_abstract_operations::{
    AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractOperation,
    AbstractOperationPlan,
};
use omega_optimization_unit::PsiOptimizationUnit;
use omega_target_operations::TargetOperationPlan;
use psi_core::{
    BlockId, EdgeId, FuelScheduleIdentity, IntegerSign, MachineId, ObligationId, OperationId,
    PlaceId, ScalarType, StructuralFieldId, StructuralTypeId,
};
use psi_terminal::{
    BindingRelevance, CrashCause, CrashRouteBucket, CrashRouteGuard, SemanticFingerprint,
    StructuralAccess, StructuralArgument, StructuralFieldDeclaration, StructuralFieldType,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalPsiIdentity, VocabularyMarker,
};
use std::sync::Arc;

pub(in crate::tests) fn structural_call_fixture() -> (
    AbstractOperationPlan,
    TargetOperationPlan,
    PsiOptimizationUnit,
) {
    let caller = MachineId::new(1).unwrap();
    let callee = MachineId::new(2).unwrap();
    let caller_block = BlockId::new(1).unwrap();
    let callee_block = BlockId::new(2).unwrap();
    let caller_places = [PlaceId::new(1).unwrap(), PlaceId::new(2).unwrap()];
    let callee_places = [PlaceId::new(3).unwrap(), PlaceId::new(4).unwrap()];
    let structural_type = StructuralTypeId::new(1).unwrap();
    let call = OperationId::new(1).unwrap();
    let caller_return = EdgeId::new(1).unwrap();
    let callee_return = EdgeId::new(2).unwrap();
    let parameter = |place, position| StructuralParameterDeclaration {
        place,
        position,
        is_self: false,
        structural_type,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::Owned,
        qualifications: vec![psi_core::StructuralDomainId::new(1).unwrap()],
        projected_qualifications: Vec::new(),
    };
    let abstract_plan = AbstractOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([0x51; 32]),
        },
        entry: caller,
        structural_types: vec![StructuralTypeDeclaration {
            id: structural_type,
            identity: "Extent".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![
                    StructuralFieldDeclaration {
                        id: StructuralFieldId::new(1).unwrap(),
                        identity: "base".into(),
                        relevance: BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                            psi_core::IntegerType::address(64).unwrap(),
                        )),
                    },
                    StructuralFieldDeclaration {
                        id: StructuralFieldId::new(2).unwrap(),
                        identity: "length".into(),
                        relevance: BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                            psi_core::IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                        )),
                    },
                ],
            },
        }],
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![
            AbstractFunction {
                machine: caller,
                attachment: None,
                entry: caller_block,
                parameters: Vec::new(),
                structural_parameters: caller_places
                    .into_iter()
                    .enumerate()
                    .map(|(position, place)| parameter(place, position as u32))
                    .collect(),
                result: AbstractFunctionResult::Unit,
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![AbstractBlockEntry {
                    block: caller_block,
                    parameters: Vec::new(),
                    operation_offset: 0,
                }],
                operations: vec![
                    AbstractOperation::CallUnit {
                        psi_operation: call,
                        callee,
                        structural_arguments: caller_places
                            .into_iter()
                            .map(|place| StructuralArgument {
                                place,
                                access: StructuralAccess::Owned,
                                path: Vec::new(),
                            })
                            .collect(),
                        claim_transfers: Vec::new(),
                        requirement_obligations: vec![ObligationId::new(1).unwrap()],
                        crash_continuations: vec![CrashRouteBucket {
                            cause: CrashCause::Trap,
                            alternatives: vec![CrashRouteGuard::Truth],
                        }],
                    },
                    AbstractOperation::ReturnUnit {
                        psi_edge: caller_return,
                        cleanup_actions: Vec::new(),
                    },
                ],
            },
            AbstractFunction {
                machine: callee,
                attachment: None,
                entry: callee_block,
                parameters: Vec::new(),
                structural_parameters: callee_places
                    .into_iter()
                    .enumerate()
                    .map(|(position, place)| parameter(place, position as u32))
                    .collect(),
                result: AbstractFunctionResult::Unit,
                entry_claims: Vec::new(),
                published_service_ceiling: Vec::new(),
                block_entries: vec![AbstractBlockEntry {
                    block: callee_block,
                    parameters: Vec::new(),
                    operation_offset: 0,
                }],
                operations: vec![AbstractOperation::ReturnUnit {
                    psi_edge: callee_return,
                    cleanup_actions: Vec::new(),
                }],
            },
        ],
    };
    let target = omega_abstract_operations_to_target_operations::lower_to_target_operations(
        &abstract_plan,
        omega_target::NativeTarget::uefi_x64(),
    )
    .unwrap();
    let unit = qualified_fixture_unit(
        omega_optimization_unit::reconstruct_psi_optimization_unit_seed(
            &abstract_plan,
            FuelScheduleIdentity::new(1).unwrap(),
        )
        .unwrap(),
        structural_type,
    );
    (abstract_plan, target, unit)
}

pub(in crate::tests) fn qualified_fixture_unit(
    mut unit: PsiOptimizationUnit,
    carrier: StructuralTypeId,
) -> PsiOptimizationUnit {
    unit.structural_domains = Arc::from([psi_terminal::StructuralDomainDeclaration {
        id: psi_core::StructuralDomainId::new(1).unwrap(),
        semantic_domain: psi_core::DomainSemanticId::new(1).unwrap(),
        identity: "ExtentDomain".into(),
        carrier,
        content_projection: None,
    }]);
    unit.identity = omega_optimization_unit::recompute_psi_optimization_unit_identity(&unit);
    unit
}

//! Exact all-target projected structural call/return legalization fixture.

use std::sync::Arc;

use abstract_operations::{AbstractFunctionResult, AbstractOperation};
use optimization_unit::PsiOptimizationUnit;
use semantic_vocabulary::{
    ClaimId, DomainSemanticId, EdgeId, IntegerSign, MachineId, OperationId, PlaceId, ScalarType,
    StructuralDomainId, StructuralFieldId, StructuralTypeId,
};
use target_operations::TargetOperationPlan;
use terminal_psi::{
    BindingRelevance, EntryClaim, StructuralAccess, StructuralArgument,
    StructuralDomainDeclaration, StructuralFieldDeclaration, StructuralFieldType,
    StructuralMultiplicity, StructuralOperationResult, StructuralParameterDeclaration,
    StructuralPathQualification, StructuralPathSegment, StructuralResultClaimBinding,
    StructuralResultClaimTransfer, StructuralResultDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape,
};

use super::structural_call::structural_call_fixture;

pub(in crate::tests) fn projected_fixture(
    target: target::NativeTarget,
) -> (
    abstract_operations::AbstractOperationPlan,
    TargetOperationPlan,
    PsiOptimizationUnit,
) {
    let (mut source, _, _) = structural_call_fixture();
    let caller_claim = ClaimId::new(1).unwrap();
    let callee_claim = ClaimId::new(1).unwrap();
    let call_result = PlaceId::new(12).unwrap();
    let caller_result = PlaceId::new(10).unwrap();
    let callee_result = PlaceId::new(11).unwrap();
    let root = StructuralTypeId::new(1).unwrap();
    let leaf = StructuralTypeId::new(2).unwrap();
    let rows = vec![
        StructuralPathQualification {
            path: vec![StructuralPathSegment::Field("payload".into())],
            domain: StructuralDomainId::new(1).unwrap(),
        },
        StructuralPathQualification {
            path: vec![StructuralPathSegment::Field("payload".into())],
            domain: StructuralDomainId::new(2).unwrap(),
        },
    ];
    source.structural_types = vec![
        StructuralTypeDeclaration {
            id: root,
            identity: "ProjectedRoot".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: StructuralFieldId::new(1).unwrap(),
                    identity: "payload".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Structural(leaf),
                }],
            },
        },
        StructuralTypeDeclaration {
            id: leaf,
            identity: "ProjectedLeaf".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: StructuralFieldId::new(2).unwrap(),
                    identity: "value".into(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                        semantic_vocabulary::IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                    )),
                }],
            },
        },
    ];
    let parameter = |mut parameter: StructuralParameterDeclaration| {
        parameter.position = 0;
        parameter.multiplicity = StructuralMultiplicity::Linear;
        parameter.access = StructuralAccess::Owned;
        parameter.qualifications.clear();
        parameter.projected_qualifications = rows.clone();
        parameter
    };
    let result = |place| StructuralResultDeclaration {
        place,
        structural_type: root,
        multiplicity: StructuralMultiplicity::Linear,
        qualifications: Vec::new(),
        projected_qualifications: rows.clone(),
    };
    let caller = &mut source.functions[0];
    caller.structural_parameters.truncate(1);
    caller.structural_parameters[0] = parameter(caller.structural_parameters[0].clone());
    let caller_parameter = caller.structural_parameters[0].place;
    caller.result = AbstractFunctionResult::Structural(result(caller_result));
    caller.entry_claims = vec![EntryClaim {
        claim: caller_claim,
        input: caller_parameter,
        path: Vec::new(),
    }];
    caller.operations = vec![
        AbstractOperation::CallStructural {
            psi_operation: OperationId::new(1).unwrap(),
            result: StructuralOperationResult {
                place: call_result,
                structural_type: root,
                multiplicity: StructuralMultiplicity::Linear,
                qualifications: Vec::new(),
                projected_qualifications: rows.clone(),
                claims: vec![StructuralResultClaimBinding {
                    claim: caller_claim,
                    path: Vec::new(),
                }],
            },
            callee: MachineId::new(2).unwrap(),
            arguments: Vec::new(),
            structural_arguments: vec![StructuralArgument {
                place: caller_parameter,
                access: StructuralAccess::Owned,
                path: Vec::new(),
            }],
            claim_transfers: vec![terminal_psi::ClaimTransfer {
                claim: caller_claim,
                argument_index: 0,
            }],
            returned_claim_transfers: vec![StructuralResultClaimTransfer {
                callee_claim,
                caller_claim,
            }],
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
            selected_evidence: Vec::new(),
        },
        AbstractOperation::ReturnStructural {
            psi_edge: EdgeId::new(1).unwrap(),
            source: call_result,
            returned_claims: vec![caller_claim],
            trivial_affine_locals: Vec::new(),
            trivial_affine_discards: Vec::new(),
        },
    ];
    let callee = &mut source.functions[1];
    callee.structural_parameters.truncate(1);
    callee.structural_parameters[0] = parameter(callee.structural_parameters[0].clone());
    let callee_parameter = callee.structural_parameters[0].place;
    callee.result = AbstractFunctionResult::Structural(result(callee_result));
    callee.entry_claims = vec![EntryClaim {
        claim: callee_claim,
        input: callee_parameter,
        path: Vec::new(),
    }];
    callee.operations = vec![AbstractOperation::ReturnStructural {
        psi_edge: EdgeId::new(2).unwrap(),
        source: callee_parameter,
        returned_claims: vec![callee_claim],
        trivial_affine_locals: Vec::new(),
        trivial_affine_discards: Vec::new(),
    }];
    let target_plan =
        abstract_operations_to_target_operations::lower_to_target_operations(&source, target)
            .expect("exact projected closure lowers on every supported target");
    let mut unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &source,
        semantic_vocabulary::FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    unit.structural_domains = Arc::from([domain(1, leaf), domain(2, leaf)]);
    unit.identity = optimization_unit::recompute_psi_optimization_unit_identity(&unit);
    optimization_validation::validate_psi_optimization_unit(&unit)
        .expect("projected fixture optimization unit remains independently valid");
    (source, target_plan, unit)
}

fn domain(value: u32, carrier: StructuralTypeId) -> StructuralDomainDeclaration {
    StructuralDomainDeclaration {
        id: StructuralDomainId::new(value.into()).unwrap(),
        semantic_domain: DomainSemanticId::new(value.into()).unwrap(),
        identity: format!("ProjectedDomain{value}"),
        carrier,
        content_projection: None,
    }
}

//! Boundary qualification consumption from a dominating structural operation result.

use crate::tests::{
    OperationResultCfgShape, id, operation_result_cfg_unit, refresh_function_derivatives,
    refresh_identity,
};
use crate::{OptimizationUnitValidationError, validate_psi_optimization_unit};
use omega_abstract_operations::AbstractOperation;
use psi_core::{
    BoundaryMachineId, OperationId, PlaceId, StructuralDomainId, StructuralFieldId,
    StructuralTypeId,
};

#[test]
fn structural_call_and_return_retain_the_exact_projected_result_roster() {
    let mut unit = operation_result_cfg_unit(OperationResultCfgShape::DominatingNonTopological);
    let root = unit.structural_types[0].id;
    let leaf = id(4_690, StructuralTypeId::new);
    let domain = id(4_691, StructuralDomainId::new);
    let row = psi_terminal::StructuralPathQualification {
        path: vec![psi_terminal::StructuralPathSegment::Field("payload".into())],
        domain,
    };
    unit.structural_types = vec![
        psi_terminal::StructuralTypeDeclaration {
            id: root,
            identity: "validation::projected-result-root".into(),
            shape: psi_terminal::StructuralTypeShape::Record {
                fields: vec![psi_terminal::StructuralFieldDeclaration {
                    id: id(4_692, StructuralFieldId::new),
                    identity: "payload".into(),
                    relevance: psi_terminal::BindingRelevance::Relevant,
                    field_type: psi_terminal::StructuralFieldType::Structural(leaf),
                }],
            },
        },
        psi_terminal::StructuralTypeDeclaration {
            id: leaf,
            identity: "validation::projected-result-leaf".into(),
            shape: psi_terminal::StructuralTypeShape::Record { fields: Vec::new() },
        },
    ];
    unit.structural_domains = vec![psi_terminal::StructuralDomainDeclaration {
        id: domain,
        semantic_domain: id(4_693, psi_core::DomainSemanticId::new),
        identity: "validation::projected-result-domain".into(),
        carrier: leaf,
        content_projection: None,
    }]
    .into();
    for function in &mut unit.functions {
        function.structural_parameters[0].projected_qualifications = vec![row.clone()];
        let omega_abstract_operations::AbstractFunctionResult::Structural(result) =
            &mut function.result
        else {
            panic!("fixture functions return structural values")
        };
        result.projected_qualifications = vec![row.clone()];
    }
    let call_result = unit.functions[0]
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.nodes)
        .find_map(|node| match &mut node.operation {
            AbstractOperation::CallStructural { result, .. } => Some(result),
            _ => None,
        })
        .expect("fixture has one structural call");
    call_result.projected_qualifications = vec![row.clone()];
    refresh_function_derivatives(&mut unit, 0);
    refresh_function_derivatives(&mut unit, 1);
    validate_psi_optimization_unit(&unit)
        .expect("callee result, call result, and return source retain one exact path roster");

    let mut missing_call_row = unit.clone();
    let call_result = missing_call_row.functions[0]
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.nodes)
        .find_map(|node| match &mut node.operation {
            AbstractOperation::CallStructural { result, .. } => Some(result),
            _ => None,
        })
        .expect("fixture has one structural call");
    call_result.projected_qualifications.clear();
    let omega_abstract_operations::AbstractFunctionResult::Structural(caller_result) =
        &mut missing_call_row.functions[0].result
    else {
        unreachable!()
    };
    caller_result.projected_qualifications.clear();
    refresh_function_derivatives(&mut missing_call_row, 0);
    let error = validate_psi_optimization_unit(&missing_call_row).unwrap_err();
    assert!(
        matches!(
            error,
            OptimizationUnitValidationError::StructuralCallContractMismatch { .. }
        ),
        "{error:?}"
    );

    let mut missing_return_row = unit;
    let omega_abstract_operations::AbstractFunctionResult::Structural(result) =
        &mut missing_return_row.functions[0].result
    else {
        panic!("fixture caller returns a structural value")
    };
    result.projected_qualifications.clear();
    refresh_function_derivatives(&mut missing_return_row, 0);
    assert!(matches!(
        validate_psi_optimization_unit(&missing_return_row),
        Err(OptimizationUnitValidationError::StructuralReturnSourceContractMismatch { .. })
    ));
}

#[test]
fn boundary_requirement_consumes_a_dominating_operation_result_qualification() {
    let mut unit = operation_result_cfg_unit(OperationResultCfgShape::DominatingNonTopological);
    let domain = id(4_700, StructuralDomainId::new);
    let foreign_domain = id(4_701, StructuralDomainId::new);
    let structural_type = unit.structural_types[0].id;
    unit.structural_domains = vec![domain, foreign_domain]
        .into_iter()
        .enumerate()
        .map(|(index, id)| psi_terminal::StructuralDomainDeclaration {
            id,
            semantic_domain: psi_core::DomainSemanticId::new(4_700 + index as u64)
                .expect("nonzero semantic domain"),
            identity: format!("validation::operation-result-domain-{index}"),
            carrier: structural_type,
            content_projection: None,
        })
        .collect();

    for function in &mut unit.functions {
        function.structural_parameters[0].qualifications = vec![domain];
        let omega_abstract_operations::AbstractFunctionResult::Structural(result) =
            &mut function.result
        else {
            panic!("fixture functions return one structural value")
        };
        result.qualifications = vec![domain];
    }
    let call_result = unit.functions[0]
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.nodes)
        .find_map(|node| match &mut node.operation {
            AbstractOperation::CallStructural { result, .. } => {
                result.qualifications = vec![domain];
                Some(result.place)
            }
            _ => None,
        })
        .expect("fixture has one structural call result");

    let boundary = id(4_702, BoundaryMachineId::new);
    unit.boundary_machines
        .push(psi_terminal::BoundaryMachineDeclaration {
            id: boundary,
            identity: "validation::qualified-operation-result-boundary".into(),
            attachment: None,
            scalar_parameters: Vec::new(),
            structural_parameters: vec![psi_terminal::StructuralParameterDeclaration {
                place: id(4_703, PlaceId::new),
                position: 0,
                is_self: false,
                structural_type,
                multiplicity: psi_terminal::StructuralMultiplicity::Linear,
                access: psi_terminal::StructuralAccess::SharedBorrow,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            }],
            result: psi_terminal::BoundaryMachineResult::Unit,
            requires: vec![psi_terminal::StructuralDomainRequirement {
                argument_index: 0,
                domain,
            }],
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            published_service_ceiling: Vec::new(),
        });
    let entry_claim = unit.functions[0].entry_claim_declarations[0].clone();
    let return_block = unit.functions[0]
        .blocks
        .iter_mut()
        .find(|block| {
            block.nodes.iter().any(|node| {
                matches!(
                    node.operation,
                    AbstractOperation::ReturnStructural { source, .. } if source == call_result
                )
            })
        })
        .expect("dominated block returns the structural call result");
    let return_index = return_block
        .nodes
        .iter()
        .position(|node| matches!(node.operation, AbstractOperation::ReturnStructural { .. }))
        .expect("return node");
    let mut boundary_node = return_block.nodes[return_index].clone();
    boundary_node.operation = AbstractOperation::BoundaryCall {
        psi_operation: id(4_704, OperationId::new),
        result: None,
        boundary,
        arguments: Vec::new(),
        structural_arguments: vec![psi_terminal::StructuralArgument {
            place: call_result,
            path: Vec::new(),
            access: psi_terminal::StructuralAccess::SharedBorrow,
        }],
        completion_claim_sources: vec![omega_abstract_operations::CompletionClaimSource {
            claim: entry_claim.claim,
            entry: Some(entry_claim),
            content: None,
        }],
        completion_receipts: Vec::new(),
    };
    return_block.nodes.insert(return_index, boundary_node);
    refresh_function_derivatives(&mut unit, 0);

    let mut no_requirement = unit.clone();
    no_requirement.boundary_machines[0].requires.clear();
    refresh_identity(&mut no_requirement);
    validate_psi_optimization_unit(&no_requirement)
        .expect("the dominating operation result is an ordinary structural boundary argument");

    validate_psi_optimization_unit(&unit)
        .expect("a dominating structural result carries its exact qualification to the boundary");

    unit.boundary_machines[0].requires[0].domain = foreign_domain;
    refresh_identity(&mut unit);
    assert!(matches!(
        validate_psi_optimization_unit(&unit),
        Err(OptimizationUnitValidationError::StructuralCallContractMismatch { .. })
    ));
}

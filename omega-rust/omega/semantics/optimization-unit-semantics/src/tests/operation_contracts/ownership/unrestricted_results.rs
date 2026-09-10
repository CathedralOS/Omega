//! Copyable results require dominating producers, not disposal at a join.

use crate::tests::{
    OperationResultCfgShape, id, operation_result_cfg_unit, refresh_function_derivatives,
    structural_result_call_unit,
};
use crate::{OptimizationUnitValidationError, validate_psi_optimization_unit};
use abstract_operations::{AbstractFunctionResult, AbstractOperation};
use optimization_unit::PsiOptimizationUnit;
use semantic_vocabulary::{OperationId, StructuralCaseId, StructuralPlaceKind};
use terminal_psi::{StructuralMultiplicity, StructuralTypeShape};

fn unrestricted(mut unit: PsiOptimizationUnit) -> PsiOptimizationUnit {
    let result_case = id(20_001, StructuralCaseId::new);
    unit.structural_types[0].shape = StructuralTypeShape::Sum {
        cases: vec![terminal_psi::StructuralCaseDeclaration {
            id: result_case,
            identity: "Ready".into(),
            fields: Vec::new(),
        }],
    };
    for function in &mut unit.functions {
        function.entry_claim_declarations.clear();
        function.entry_claims.clear();
        for parameter in &mut function.structural_parameters {
            parameter.multiplicity = StructuralMultiplicity::Unrestricted;
        }
        let AbstractFunctionResult::Structural(result) = &mut function.result else {
            panic!("fixture has structural results")
        };
        result.multiplicity = StructuralMultiplicity::Unrestricted;
        for node in function
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.nodes)
        {
            match &mut node.operation {
                AbstractOperation::CallStructural {
                    psi_operation,
                    result,
                    ..
                } => {
                    result.multiplicity = StructuralMultiplicity::Unrestricted;
                    result.claims.clear();
                    node.operation = AbstractOperation::EstablishScalarCase {
                        psi_operation: *psi_operation,
                        result: result.clone(),
                        result_case,
                        fields: Vec::new(),
                    };
                }
                AbstractOperation::ReturnStructural {
                    returned_claims, ..
                } => {
                    returned_claims.clear();
                }
                _ => {}
            }
        }
    }
    for function_index in 0..unit.functions.len() {
        refresh_function_derivatives(&mut unit, function_index);
    }
    unit
}

#[test]
fn unrestricted_parameters_and_constructor_results_return_without_disposal() {
    validate_psi_optimization_unit(&unrestricted(structural_result_call_unit()))
        .expect("copyable parameters and constructor results each support structural return");
}

fn use_as_unit_argument(
    unit: &mut PsiOptimizationUnit,
    source: semantic_vocabulary::PlaceId,
    ordinal: u64,
) -> optimization_unit::OptimizationNode {
    let callee = &mut unit.functions[1];
    if let AbstractFunctionResult::Structural(result) = &callee.result {
        callee.declared_places.remove(&result.place);
    }
    callee.result = AbstractFunctionResult::Unit;
    callee
        .structural_places
        .retain(|place| place.kind != StructuralPlaceKind::Result);
    let returned = &mut callee.blocks[0].nodes[0];
    if let AbstractOperation::ReturnStructural { psi_edge, .. } = returned.operation {
        returned.operation = AbstractOperation::ReturnUnit {
            psi_edge,
            cleanup_actions: Vec::new(),
        };
    }
    let mut call = returned.clone();
    call.operation = AbstractOperation::CallUnit {
        psi_operation: id(20_100 + ordinal, OperationId::new),
        callee: callee.machine,
        arguments: Vec::new(),
        structural_arguments: vec![terminal_psi::StructuralArgument {
            place: source,
            path: Vec::new(),
            access: terminal_psi::StructuralAccess::Owned,
        }],
        claim_transfers: Vec::new(),
        requirement_obligations: Vec::new(),
        crash_continuations: Vec::new(),
    };
    refresh_function_derivatives(unit, 1);
    call
}

#[test]
fn unrestricted_parameter_and_result_can_each_supply_repeated_calls() {
    let mut unit = unrestricted(structural_result_call_unit());
    let AbstractOperation::EstablishScalarCase { result, .. } =
        &unit.functions[0].blocks[0].nodes[0].operation
    else {
        panic!("fixture begins with a constructor")
    };
    let original_result = result.place;
    let input = unit.functions[0].structural_parameters[0].place;
    for (ordinal, source) in [input, input, original_result, original_result]
        .into_iter()
        .enumerate()
    {
        let repeated = use_as_unit_argument(&mut unit, source, ordinal as u64);
        unit.functions[0].blocks[0]
            .nodes
            .insert(ordinal + 1, repeated);
    }
    refresh_function_derivatives(&mut unit, 0);
    validate_psi_optimization_unit(&unit)
        .expect("copying parameters and results leaves their original values available for return");
}

#[test]
fn unused_branch_local_unrestricted_result_does_not_change_join_obligations() {
    let mut unit = unrestricted(operation_result_cfg_unit(
        OperationResultCfgShape::PartialPredecessor,
    ));
    let input = unit.functions[0].structural_parameters[0].place;
    for node in unit.functions[0]
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.nodes)
    {
        if let AbstractOperation::ReturnStructural { source, .. } = &mut node.operation {
            *source = input;
        }
    }
    refresh_function_derivatives(&mut unit, 0);
    validate_psi_optimization_unit(&unit).expect(
        "an unused copyable constructor introduces no ownership difference at reconvergence",
    );
}

#[test]
fn unrestricted_result_used_after_bypassed_producer_rejects() {
    let unit = unrestricted(operation_result_cfg_unit(
        OperationResultCfgShape::PartialPredecessor,
    ));
    assert!(matches!(
        validate_psi_optimization_unit(&unit),
        Err(OptimizationUnitValidationError::StructuralPlaceNotAvailable { .. })
    ));
}

#[test]
fn unrestricted_result_used_before_its_producer_in_same_block_rejects() {
    let mut unit = unrestricted(structural_result_call_unit());
    let AbstractOperation::EstablishScalarCase { result, .. } =
        &unit.functions[0].blocks[0].nodes[0].operation
    else {
        panic!("fixture begins with a constructor")
    };
    let source = result.place;
    let premature = use_as_unit_argument(&mut unit, source, 0);
    unit.functions[0].blocks[0].nodes.insert(0, premature);
    refresh_function_derivatives(&mut unit, 0);
    assert!(matches!(
        validate_psi_optimization_unit(&unit),
        Err(OptimizationUnitValidationError::StructuralPlaceNotAvailable { node: 0, .. })
    ));
}

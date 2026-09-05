//! Structural-call result signature and claim-interface corruption rejection.

use crate::tests::{id, refresh_node_derivatives, structural_result_call_unit};
use crate::{OptimizationUnitValidationError, validate_psi_optimization_unit};
use abstract_operations::{AbstractFunctionResult, AbstractOperation};
use semantic_vocabulary::{ClaimId, StructuralPlaceKind};

#[test]
fn rejects_structural_call_result_signature_and_claim_interface_corruption() {
    let baseline = structural_result_call_unit();
    validate_psi_optimization_unit(&baseline)
        .expect("exact linear structural result should validate");

    let mut wrong_type = baseline.clone();
    let alternate = id(360, semantic_vocabulary::StructuralTypeId::new);
    wrong_type
        .structural_types
        .push(terminal_psi::StructuralTypeDeclaration {
            id: alternate,
            identity: "validation::alternate-call-result".into(),
            shape: terminal_psi::StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BorrowedView,
            ),
        });
    let AbstractOperation::CallStructural { result, .. } =
        &mut wrong_type.functions[0].blocks[0].nodes[0].operation
    else {
        panic!("fixture begins with a structural-result call")
    };
    let result_place = result.place;
    result.structural_type = alternate;
    let StructuralPlaceKind::OperationResult {
        structural_type, ..
    } = &mut wrong_type.functions[0]
        .structural_places
        .iter_mut()
        .find(|place| place.id == result_place)
        .expect("caller retains its operation-result place")
        .kind
    else {
        unreachable!("call result has its operation-result root kind")
    };
    *structural_type = alternate;
    let AbstractFunctionResult::Structural(result) = &mut wrong_type.functions[0].result else {
        unreachable!("fixture has a structural result")
    };
    result.structural_type = alternate;
    refresh_node_derivatives(&mut wrong_type, 0, 0, 0);
    assert!(matches!(
        validate_psi_optimization_unit(&wrong_type),
        Err(OptimizationUnitValidationError::StructuralCallContractMismatch { node: 0, .. })
    ));

    let mut wrong_multiplicity = baseline.clone();
    let AbstractOperation::CallStructural { result, .. } =
        &mut wrong_multiplicity.functions[0].blocks[0].nodes[0].operation
    else {
        panic!("fixture begins with a structural-result call")
    };
    result.multiplicity = terminal_psi::StructuralMultiplicity::Affine;
    let AbstractFunctionResult::Structural(result) = &mut wrong_multiplicity.functions[0].result
    else {
        unreachable!("fixture has a structural result")
    };
    result.multiplicity = terminal_psi::StructuralMultiplicity::Affine;
    refresh_node_derivatives(&mut wrong_multiplicity, 0, 0, 0);
    assert!(matches!(
        validate_psi_optimization_unit(&wrong_multiplicity),
        Err(OptimizationUnitValidationError::StructuralCallContractMismatch { node: 0, .. })
    ));

    let mut invented_claim = baseline;
    let AbstractOperation::CallStructural { result, .. } =
        &mut invented_claim.functions[0].blocks[0].nodes[0].operation
    else {
        panic!("fixture begins with a structural-result call")
    };
    result
        .claims
        .push(terminal_psi::StructuralResultClaimBinding {
            claim: id(1, ClaimId::new),
            path: Vec::new(),
        });
    refresh_node_derivatives(&mut invented_claim, 0, 0, 0);
    assert!(matches!(
        validate_psi_optimization_unit(&invented_claim),
        Err(OptimizationUnitValidationError::StructuralCallContractMismatch { node: 0, .. })
    ));
}

//! Explicit fences for unrelated result and operation-result rosters.

use super::super::scalar::constant_conditional_plan;
use super::*;

#[test]
fn unrelated_function_and_operation_result_rosters_fail_closed() {
    let row = terminal_psi::StructuralPathQualification {
        path: vec![StructuralPathSegment::Field("payload".into())],
        domain: semantic_vocabulary::StructuralDomainId::new(900).unwrap(),
    };
    let mut function_result = constant_conditional_plan(true);
    function_result.functions[0].result =
        AbstractFunctionResult::Structural(terminal_psi::StructuralResultDeclaration {
            place: PlaceId::new(900).unwrap(),
            structural_type: StructuralTypeId::new(900).unwrap(),
            multiplicity: StructuralMultiplicity::Linear,
            qualifications: Vec::new(),
            projected_qualifications: vec![row.clone()],
        });
    assert_eq!(
        lower_to_target_operations(&function_result, NativeTarget::linux_x64()),
        Err(LoweringError::UnsupportedProjectedStructuralQualifications)
    );

    let mut operation_result = constant_conditional_plan(true);
    operation_result.functions[0].operations.insert(
        0,
        AbstractOperation::EstablishPayloadlessCase {
            psi_operation: OperationId::new(900).unwrap(),
            result: terminal_psi::StructuralOperationResult {
                place: PlaceId::new(901).unwrap(),
                structural_type: StructuralTypeId::new(900).unwrap(),
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: vec![row],
                claims: Vec::new(),
            },
            result_case: semantic_vocabulary::StructuralCaseId::new(900).unwrap(),
        },
    );
    assert_eq!(
        lower_to_target_operations(&operation_result, NativeTarget::linux_x64()),
        Err(LoweringError::UnsupportedProjectedStructuralQualifications)
    );
}

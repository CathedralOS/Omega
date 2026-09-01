//! Fail-closed target fence for projected structural result qualifications.

use super::scalar::constant_conditional_plan;
use super::*;

#[test]
fn projected_function_and_operation_result_rosters_fail_closed_before_target_lowering() {
    let row = psi_terminal::StructuralPathQualification {
        path: vec![psi_terminal::StructuralPathSegment::Field("payload".into())],
        domain: psi_core::StructuralDomainId::new(900).unwrap(),
    };
    let mut function_result = constant_conditional_plan(true);
    function_result.functions[0].result =
        AbstractFunctionResult::Structural(psi_terminal::StructuralResultDeclaration {
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
            result: psi_terminal::StructuralOperationResult {
                place: PlaceId::new(901).unwrap(),
                structural_type: StructuralTypeId::new(900).unwrap(),
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: vec![row],
                claims: Vec::new(),
            },
            result_case: psi_core::StructuralCaseId::new(900).unwrap(),
        },
    );
    assert_eq!(
        lower_to_target_operations(&operation_result, NativeTarget::linux_x64()),
        Err(LoweringError::UnsupportedProjectedStructuralQualifications)
    );
}

//! Exact parameter-rooted path qualification consumption and corruption refusal.

use crate::tests::{partial_path_qualified_boundary_unit, refresh_identity};
use crate::{OptimizationUnitValidationError, validate_psi_optimization_unit};
use omega_abstract_operations::AbstractOperation;
use psi_core::StructuralDomainId;

#[test]
fn boundary_requirement_consumes_only_the_exact_qualified_parameter_path() {
    let baseline = partial_path_qualified_boundary_unit();
    validate_psi_optimization_unit(&baseline)
        .expect("the exact qualified field should satisfy its boundary requirement");

    let assert_contract_mismatch = |mut unit: omega_optimization_unit::PsiOptimizationUnit| {
        refresh_identity(&mut unit);
        assert!(matches!(
            validate_psi_optimization_unit(&unit),
            Err(OptimizationUnitValidationError::StructuralCallContractMismatch { .. })
        ));
    };

    let mut missing = baseline.clone();
    missing.functions[0].structural_parameters[0]
        .projected_qualifications
        .clear();
    assert_contract_mismatch(missing);

    let mut sibling = baseline.clone();
    let AbstractOperation::BoundaryCall {
        structural_arguments,
        ..
    } = &mut sibling.functions[0].blocks[0].nodes[0].operation
    else {
        panic!("fixture begins with a boundary call")
    };
    structural_arguments[0].path = vec![psi_terminal::StructuralPathSegment::Field("right".into())];
    assert_contract_mismatch(sibling);

    let mut wrong_domain = baseline.clone();
    wrong_domain.boundary_machines[0].requires[0].domain = StructuralDomainId::new(4_726).unwrap();
    assert_contract_mismatch(wrong_domain);

    let mut root = baseline;
    let AbstractOperation::BoundaryCall {
        structural_arguments,
        ..
    } = &mut root.functions[0].blocks[0].nodes[0].operation
    else {
        unreachable!()
    };
    structural_arguments[0].path.clear();
    assert_contract_mismatch(root);
}

#[test]
fn projected_qualification_catalog_rejects_invalid_carrier_and_canonical_rows() {
    let baseline = partial_path_qualified_boundary_unit();

    let mut empty_path = baseline.clone();
    empty_path.functions[0].structural_parameters[0].projected_qualifications[0]
        .path
        .clear();
    refresh_identity(&mut empty_path);
    assert!(matches!(
        validate_psi_optimization_unit(&empty_path),
        Err(OptimizationUnitValidationError::StructuralCatalogMismatch { .. })
    ));

    let mut invalid_path = baseline.clone();
    invalid_path.functions[0].structural_parameters[0].projected_qualifications[0].path =
        vec![psi_terminal::StructuralPathSegment::Field("missing".into())];
    refresh_identity(&mut invalid_path);
    assert!(matches!(
        validate_psi_optimization_unit(&invalid_path),
        Err(OptimizationUnitValidationError::StructuralCatalogMismatch { .. })
    ));

    let mut duplicate = baseline;
    let row = duplicate.functions[0].structural_parameters[0].projected_qualifications[0].clone();
    duplicate.functions[0].structural_parameters[0]
        .projected_qualifications
        .push(row);
    refresh_identity(&mut duplicate);
    assert!(matches!(
        validate_psi_optimization_unit(&duplicate),
        Err(OptimizationUnitValidationError::StructuralCatalogMismatch { .. })
    ));
}

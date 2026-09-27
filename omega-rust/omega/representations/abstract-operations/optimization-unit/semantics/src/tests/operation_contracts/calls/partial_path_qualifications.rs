//! Exact parameter-rooted path qualification consumption and corruption refusal.

use crate::tests::fixtures::dominance::partial_path_qualified_boundary_unit;
use crate::tests::support::refresh_identity;
use crate::{OptimizationUnitValidationError, validate_psi_optimization_unit};

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
        vec![terminal_psi::StructuralPathSegment::Field("missing".into())];
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

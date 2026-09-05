//! Structural-call path type, multiplicity, and qualification corruption rejection.

use crate::tests::{
    id, projected_shared_structural_scalar_call_unit, refresh_identity, refresh_node_derivatives,
    structural_call_unit,
};
use crate::{OptimizationUnitValidationError, validate_psi_optimization_unit};
use abstract_operations::AbstractOperation;

#[test]
fn rejects_structural_call_path_type_multiplicity_and_qualification_corruption() {
    let baseline = structural_call_unit();

    let mut path = baseline.clone();
    let AbstractOperation::CallUnit {
        structural_arguments,
        ..
    } = &mut path.functions[0].blocks[0].nodes[0].operation
    else {
        panic!("fixture begins with a structural Unit call")
    };
    structural_arguments[0].path = vec![terminal_psi::StructuralPathSegment::FixedIndex(0)];
    refresh_node_derivatives(&mut path, 0, 0, 0);
    assert!(matches!(
        validate_psi_optimization_unit(&path),
        Err(OptimizationUnitValidationError::StructuralCallContractMismatch { node: 0, .. })
    ));

    let mut wrong_type = baseline.clone();
    let alternate = id(342, semantic_vocabulary::StructuralTypeId::new);
    wrong_type
        .structural_types
        .push(terminal_psi::StructuralTypeDeclaration {
            id: alternate,
            identity: "validation::alternate-structural-call-argument".into(),
            shape: terminal_psi::StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BorrowedView,
            ),
        });
    wrong_type.functions[1].structural_parameters[0].structural_type = alternate;
    refresh_identity(&mut wrong_type);
    assert!(matches!(
        validate_psi_optimization_unit(&wrong_type),
        Err(OptimizationUnitValidationError::StructuralCallContractMismatch { node: 0, .. })
    ));

    let mut multiplicity = baseline.clone();
    multiplicity.functions[1].structural_parameters[0].multiplicity =
        terminal_psi::StructuralMultiplicity::Affine;
    refresh_identity(&mut multiplicity);
    assert!(matches!(
        validate_psi_optimization_unit(&multiplicity),
        Err(OptimizationUnitValidationError::StructuralCallContractMismatch { node: 0, .. })
    ));

    let mut source_access = baseline.clone();
    source_access.functions[0].structural_parameters[0].access =
        terminal_psi::StructuralAccess::SharedBorrow;
    refresh_identity(&mut source_access);
    assert!(matches!(
        validate_psi_optimization_unit(&source_access),
        Err(OptimizationUnitValidationError::StructuralCallContractMismatch { node: 0, .. })
    ));

    let mut qualified = baseline;
    let domain = id(343, semantic_vocabulary::StructuralDomainId::new);
    qualified.structural_domains = vec![terminal_psi::StructuralDomainDeclaration {
        id: domain,
        semantic_domain: id(344, semantic_vocabulary::DomainSemanticId::new),
        identity: "validation::structural-call-domain".into(),
        carrier: qualified.structural_types[0].id,
        content_projection: None,
    }]
    .into();
    qualified.functions[0].structural_parameters[0].qualifications = vec![domain];
    qualified.functions[1].structural_parameters[0].qualifications = vec![domain];
    refresh_identity(&mut qualified);
    validate_psi_optimization_unit(&qualified)
        .expect("an exact retained argument qualification should validate");

    qualified.functions[0].structural_parameters[0]
        .qualifications
        .clear();
    refresh_identity(&mut qualified);
    assert!(matches!(
        validate_psi_optimization_unit(&qualified),
        Err(OptimizationUnitValidationError::StructuralCallContractMismatch { node: 0, .. })
    ));
}

#[test]
fn admits_an_unrestricted_shared_field_projection_for_a_structural_scalar_call() {
    let baseline = projected_shared_structural_scalar_call_unit();
    validate_psi_optimization_unit(&baseline)
        .expect("an exact unrestricted shared field subloan should validate");

    let mut wrong_field = baseline.clone();
    let AbstractOperation::CallStructuralScalar {
        structural_arguments,
        ..
    } = &mut wrong_field.functions[0].blocks[0].nodes[0].operation
    else {
        panic!("fixture begins with a structural scalar call")
    };
    structural_arguments[0].path =
        vec![terminal_psi::StructuralPathSegment::Field("missing".into())];
    refresh_node_derivatives(&mut wrong_field, 0, 0, 0);
    assert!(matches!(
        validate_psi_optimization_unit(&wrong_field),
        Err(OptimizationUnitValidationError::StructuralCallContractMismatch { node: 0, .. })
    ));

    let mut wrong_access = baseline;
    let AbstractOperation::CallStructuralScalar {
        structural_arguments,
        ..
    } = &mut wrong_access.functions[0].blocks[0].nodes[0].operation
    else {
        panic!("fixture begins with a structural scalar call")
    };
    structural_arguments[0].access = terminal_psi::StructuralAccess::MutableBorrow;
    refresh_node_derivatives(&mut wrong_access, 0, 0, 0);
    assert!(matches!(
        validate_psi_optimization_unit(&wrong_access),
        Err(OptimizationUnitValidationError::StructuralCallContractMismatch { node: 0, .. })
    ));
}

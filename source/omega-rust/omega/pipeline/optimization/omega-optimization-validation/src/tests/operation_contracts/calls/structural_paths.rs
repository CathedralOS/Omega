//! Structural-call path type, multiplicity, and qualification corruption rejection.

use crate::tests::{id, refresh_identity, refresh_node_derivatives, structural_call_unit};
use crate::{OptimizationUnitValidationError, validate_psi_optimization_unit};
use omega_abstract_operations::AbstractOperation;

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
    structural_arguments[0].path = vec![psi_terminal::StructuralPathSegment::FixedIndex(0)];
    refresh_node_derivatives(&mut path, 0, 0, 0);
    assert!(matches!(
        validate_psi_optimization_unit(&path),
        Err(OptimizationUnitValidationError::StructuralCallContractMismatch { node: 0, .. })
    ));

    let mut wrong_type = baseline.clone();
    let alternate = id(342, psi_core::StructuralTypeId::new);
    wrong_type
        .structural_types
        .push(psi_terminal::StructuralTypeDeclaration {
            id: alternate,
            identity: "validation::alternate-structural-call-argument".into(),
            shape: psi_terminal::StructuralTypeShape::ByteSequence(
                psi_terminal::ByteSequenceCarrier::BorrowedView,
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
        psi_terminal::StructuralMultiplicity::Affine;
    refresh_identity(&mut multiplicity);
    assert!(matches!(
        validate_psi_optimization_unit(&multiplicity),
        Err(OptimizationUnitValidationError::StructuralCallContractMismatch { node: 0, .. })
    ));

    let mut source_access = baseline.clone();
    source_access.functions[0].structural_parameters[0].access =
        psi_terminal::StructuralAccess::SharedBorrow;
    refresh_identity(&mut source_access);
    assert!(matches!(
        validate_psi_optimization_unit(&source_access),
        Err(OptimizationUnitValidationError::StructuralCallContractMismatch { node: 0, .. })
    ));

    let mut qualified = baseline;
    let domain = id(343, psi_core::StructuralDomainId::new);
    qualified.structural_domains = vec![psi_terminal::StructuralDomainDeclaration {
        id: domain,
        semantic_domain: id(344, psi_core::DomainSemanticId::new),
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

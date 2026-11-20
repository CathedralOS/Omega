use super::*;
use terminal_psi::TerminalIdentityIndex;

#[test]
fn scalar_application_reads_keep_the_exact_catalog_identity_and_carrier() {
    use semantic_vocabulary::{OperationId, ScalarQualificationSetId, ValueId};
    use terminal_psi::{
        LayoutMeasurement, OperationResult, ScalarApplication, ValueDeclaration,
    };
    let carrier = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    let mut applications = ScalarApplications::default();
    let declare = |measurement| ScalarApplication::LayoutObservation {
        subject: StructuralTypeId::new(1).unwrap(),
        measurement,
        semantics_version: LayoutMeasurement::SEMANTICS_VERSION,
        scalar_type: carrier,
    };
    let size = applications
        .insert(declare(LayoutMeasurement::Size))
        .unwrap();
    let alignment = applications
        .insert(declare(LayoutMeasurement::Alignment))
        .unwrap();
    let result = ValueDeclaration {
        id: ValueId::new(1).unwrap(),
        scalar_type: carrier,
        qualifications: ScalarQualificationSetId::ZERO,
    };
    let read = |application, result| {
        Operation::unsuspended(
            OperationId::new(1).unwrap(),
            OperationResult::Scalar(result),
            OperationKind::ReadScalarApplication { application },
        )
    };
    let equation = scalar_application_read_equation(&applications, &read(size, result));
    assert_eq!(
        equation,
        Ok(Some(Proposition::Equal(
            ScalarTerm::value(result.id, carrier),
            ScalarTerm::Application {
                application: size,
                scalar_type: carrier
            },
        )))
    );
    assert_ne!(
        equation,
        scalar_application_read_equation(&applications, &read(alignment, result)),
        "one observation cannot supply another observation's result"
    );
    assert_eq!(
        scalar_application_read_equation(
            &ScalarApplications::default(),
            &read(size, result)
        ),
        Err("scalar application read has no catalog declaration")
    );
    let wrong = ValueDeclaration {
        scalar_type: ScalarType::Boolean,
        ..result
    };
    assert_eq!(
        scalar_application_read_equation(&applications, &read(size, wrong)),
        Err("scalar application read result differs from its declared carrier")
    );
}

#[test]
fn descriptor_extents_require_exact_readable_crossings() {
    let bytes = StructuralTypeId::new(1).unwrap();
    let array = StructuralTypeId::new(2).unwrap();
    let reference = StructuralTypeId::new(3).unwrap();
    let declaration = |id, shape| StructuralTypeDeclaration {
        declaration_identity: Default::default(),
        requires: Vec::new(),
        id,
        identity: TerminalIdentityIndex(id.get() as u32),
        quotient_carrier: None,
        owns_linear_claim: false,
        no_code_affine_cleanup: true,
        multiplicity: terminal_psi::StructuralMultiplicity::Affine,
        descriptor_cleanup: true,
        nominal_cleanup: None,
        shape,
        scalar_restriction: None,
    };
    let mut types = [
        declaration(
            bytes,
            StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                access: Some(StructuralAccess::MutableBorrow),
            }),
        ),
        declaration(
            array,
            StructuralTypeShape::FixedArray {
                element: bytes,
                length: 1.into(),
            },
        ),
        declaration(
            reference,
            StructuralTypeShape::Reference {
                referent: array,
                access: StructuralAccess::MutableBorrow,
            },
        ),
    ];
    let path = vec![
        ScalarReadProjection::Referent,
        ScalarReadProjection::Static(Static::FixedIndex(0)),
        ScalarReadProjection::Extent,
    ];
    let count = ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap());
    assert_eq!(
        structural_read_scalar_type(types.iter(), reference, &path),
        Some(count)
    );
    assert_eq!(
        structural_read_scalar_type(types.iter(), reference, &path[1..]),
        None
    );
    assert_eq!(
        structural_read_scalar_type(types.iter(), array, &[ScalarReadProjection::Extent]),
        None
    );
    let mut outside = path.clone();
    outside[1] = ScalarReadProjection::Static(Static::FixedIndex(1));
    assert_eq!(
        structural_read_scalar_type(types.iter(), reference, &outside),
        None
    );
    let mut continued = path.clone();
    continued.push(ScalarReadProjection::Extent);
    assert_eq!(
        structural_read_scalar_type(types.iter(), reference, &continued),
        None
    );
    types[2].shape = StructuralTypeShape::Reference {
        referent: array,
        access: StructuralAccess::WriteOnlyBorrow,
    };
    assert_eq!(
        structural_read_scalar_type(types.iter(), reference, &path),
        None
    );
    types[2].shape = StructuralTypeShape::Reference {
        referent: array,
        access: StructuralAccess::MutableBorrow,
    };
    types[0].shape = StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
        access: Some(StructuralAccess::WriteOnlyBorrow),
    });
    assert_eq!(
        structural_read_scalar_type(types.iter(), reference, &path),
        None
    );
}

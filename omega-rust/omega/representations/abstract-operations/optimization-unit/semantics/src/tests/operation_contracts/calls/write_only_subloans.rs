//! Exact unrestricted subloan contracts, independently of native realization.

use crate::tests::fixtures::scalar_units::structural_call_unit;
use crate::tests::support::{id, refresh_node_derivatives};
use crate::{OptimizationUnitValidationError, validate_psi_optimization_unit};
use abstract_operations::AbstractOperation;
use optimization_unit::PsiOptimizationUnit;
use semantic_vocabulary::{
    IntegerSign, IntegerType, ScalarType, StructuralFieldId, StructuralTypeId,
};
use terminal_psi::{
    BindingRelevance, StructuralAccess, StructuralArgument, StructuralFieldDeclaration,
    StructuralFieldType, StructuralMultiplicity, StructuralPathSegment, StructuralTypeDeclaration,
    StructuralTypeShape,
};

fn field() -> StructuralPathSegment {
    StructuralPathSegment::Field("child".into())
}

fn arguments(unit: &mut PsiOptimizationUnit) -> &mut Vec<StructuralArgument> {
    let AbstractOperation::CallUnit {
        structural_arguments,
        ..
    } = &mut unit.functions[0].blocks[0].nodes[0].operation
    else {
        panic!("fixture begins with a Unit call")
    };
    structural_arguments
}

fn subloan(path: Vec<StructuralPathSegment>, access: StructuralAccess) -> PsiOptimizationUnit {
    let mut unit = structural_call_unit();
    let leaf = unit.structural_types[0].id;
    unit.structural_types.make_mut()[0].shape = StructuralTypeShape::Record {
        fields: vec![StructuralFieldDeclaration {
            id: id(7_000, StructuralFieldId::new),
            identity: "value".into(),
            relevance: BindingRelevance::Relevant,
            field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                IntegerType::new(IntegerSign::Unsigned, 16).unwrap(),
            )),
        }],
    };
    let mut child = leaf;
    for (position, segment) in path.iter().rev().enumerate() {
        let structural_type = id(7_100 + position as u64, StructuralTypeId::new);
        let shape = match segment {
            StructuralPathSegment::FixedByteRange { .. } => {
                panic!("byte windows are not write-only subloan fixtures")
            }
            StructuralPathSegment::RuntimeIndex { .. } => {
                panic!("runtime indexes are not write-only subloan fixtures")
            }
            StructuralPathSegment::Referent => {
                panic!("native subloan fixture requires an owned field or array projection")
            }
            StructuralPathSegment::Field(identity) => StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: id(8_100 + position as u64, StructuralFieldId::new),
                    identity: identity.clone(),
                    relevance: BindingRelevance::Relevant,
                    field_type: StructuralFieldType::Structural(child),
                }],
            },
            StructuralPathSegment::FixedIndex(_) => StructuralTypeShape::FixedArray {
                element: child,
                length: 2,
            },
        };
        unit.structural_types
            .make_mut()
            .push(StructuralTypeDeclaration {
                id: structural_type,
                identity: format!("validation::subloan::{position}"),
                shape,
            });
        child = structural_type;
    }
    unit.functions[0].structural_parameters[0].structural_type = child;
    unit.functions[0].structural_parameters[0].access = access;
    unit.functions[1].structural_parameters[0].access = StructuralAccess::WriteOnlyBorrow;
    arguments(&mut unit)[0].path = path;
    arguments(&mut unit)[0].access = StructuralAccess::WriteOnlyBorrow;
    refresh_node_derivatives(&mut unit, 0, 0, 0);
    unit
}

fn rejects_contract(mut unit: PsiOptimizationUnit) {
    refresh_node_derivatives(&mut unit, 0, 0, 0);
    let result = validate_psi_optimization_unit(&unit);
    assert!(
        matches!(
            result,
            Err(OptimizationUnitValidationError::StructuralCallContractMismatch { node: 0, .. })
        ),
        "{result:?}"
    );
}

#[test]
fn indexed_subloan_requires_a_material_root_and_scalar_or_record_leaf() {
    let baseline = subloan(
        vec![StructuralPathSegment::FixedIndex(1)],
        StructuralAccess::WriteOnlyBorrow,
    );
    let mut reference = baseline.clone();
    let StructuralTypeShape::Record { fields } =
        &mut reference.structural_types.make_mut()[0].shape
    else {
        panic!("leaf is a record")
    };
    fields[0].field_type =
        StructuralFieldType::ByteSequence(terminal_psi::ByteSequenceCarrier::BorrowedView {
            access: Some(terminal_psi::StructuralAccess::SharedBorrow),
        });
    rejects_contract(reference);
    let mut array_leaf = baseline;
    let primitive = id(9_001, StructuralTypeId::new);
    array_leaf
        .structural_types
        .make_mut()
        .push(StructuralTypeDeclaration {
            id: primitive,
            identity: "validation::array-leaf-element".into(),
            shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
        });
    array_leaf.structural_types.make_mut()[0].shape = StructuralTypeShape::FixedArray {
        element: primitive,
        length: 2,
    };
    rejects_contract(array_leaf);
}

#[test]
fn composed_projected_qualifications_require_the_exact_subloan_suffix() {
    let mut unit = subloan(
        vec![StructuralPathSegment::FixedIndex(1), field()],
        StructuralAccess::WriteOnlyBorrow,
    );
    let leaf = unit.functions[1].structural_parameters[0].structural_type;
    // Pass the containing record and retain a qualification on its child,
    // rather than a root qualification on the projected Unit argument.
    unit.functions[1].structural_parameters[0].structural_type = unit.structural_types[1].id;
    arguments(&mut unit)[0].path = vec![StructuralPathSegment::FixedIndex(1)];
    let domain = id(9_020, semantic_vocabulary::StructuralDomainId::new);
    unit.structural_domains = vec![terminal_psi::StructuralDomainDeclaration {
        establishment_routes: Vec::new(),
        id: domain,
        semantic_domain: id(9_021, semantic_vocabulary::DomainSemanticId::new),
        identity: "validation::qualified-subloan-child".into(),
        carrier: leaf,
        content_projection: None,
    }]
    .into();
    unit.functions[0].structural_parameters[0].projected_qualifications =
        vec![terminal_psi::StructuralPathQualification {
            path: vec![StructuralPathSegment::FixedIndex(1), field()],
            domain,
        }];
    unit.functions[1].structural_parameters[0].projected_qualifications =
        vec![terminal_psi::StructuralPathQualification {
            path: vec![field()],
            domain,
        }];
    refresh_node_derivatives(&mut unit, 0, 0, 0);
    validate_psi_optimization_unit(&unit)
        .expect("the exact composed path supplies the callee child qualification");

    let mut missing = unit.clone();
    missing.functions[0].structural_parameters[0]
        .projected_qualifications
        .clear();
    rejects_contract(missing);
    let mut sibling = unit;
    sibling.functions[0].structural_parameters[0].projected_qualifications[0].path =
        vec![StructuralPathSegment::FixedIndex(0), field()];
    rejects_contract(sibling);
}

#[test]
fn owned_root_lends_exclusive_projected_subloan() {
    // The loan table's owned row: an unrestricted owned root lends the same
    // exclusive projected subloans a mutable parent lends.
    for path in [
        vec![field()],
        vec![StructuralPathSegment::FixedIndex(1)],
        vec![field(), StructuralPathSegment::FixedIndex(1)],
        vec![StructuralPathSegment::FixedIndex(1), field()],
        vec![
            field(),
            StructuralPathSegment::FixedIndex(1),
            field(),
            StructuralPathSegment::FixedIndex(0),
        ],
    ] {
        let mut unit = subloan(path.clone(), StructuralAccess::Owned);
        validate_psi_optimization_unit(&unit)
            .unwrap_or_else(|error| panic!("owned write-only subloan {path:?}: {error:?}"));
        unit.functions[1].structural_parameters[0].access = StructuralAccess::MutableBorrow;
        arguments(&mut unit)[0].access = StructuralAccess::MutableBorrow;
        refresh_node_derivatives(&mut unit, 0, 0, 0);
        validate_psi_optimization_unit(&unit)
            .unwrap_or_else(|error| panic!("owned mutable subloan {path:?}: {error:?}"));
    }
}

#[test]
fn owned_root_subloan_keeps_exclusive_and_presentation_limits() {
    // A byte-view presentation remains a borrowed-parent form: an owned
    // root's bounded inline byte field cannot satisfy a view parameter.
    let mut presentation = subloan(vec![field()], StructuralAccess::Owned);
    let view = id(9_002, StructuralTypeId::new);
    presentation
        .structural_types
        .make_mut()
        .push(StructuralTypeDeclaration {
            id: view,
            identity: "validation::owned-byte-view".into(),
            shape: StructuralTypeShape::ByteSequence(
                terminal_psi::ByteSequenceCarrier::BorrowedView {
                    access: Some(terminal_psi::StructuralAccess::SharedBorrow),
                },
            ),
        });
    let StructuralTypeShape::Record { fields } =
        &mut presentation.structural_types.make_mut()[1].shape
    else {
        panic!("the path wrapper is a record")
    };
    fields[0].field_type =
        StructuralFieldType::ByteSequence(terminal_psi::ByteSequenceCarrier::BoundedOwned {
            capacity: 16,
        });
    presentation.functions[1].structural_parameters[0].structural_type = view;
    presentation.functions[1].structural_parameters[0].access = StructuralAccess::MutableBorrow;
    arguments(&mut presentation)[0].access = StructuralAccess::MutableBorrow;
    rejects_contract(presentation);

    // The owned row lends borrows, not the place itself: a projected owned
    // argument and a non-unrestricted parent still reject.
    let mut owned_argument = subloan(vec![field()], StructuralAccess::Owned);
    owned_argument.functions[1].structural_parameters[0].access = StructuralAccess::Owned;
    arguments(&mut owned_argument)[0].access = StructuralAccess::Owned;
    rejects_contract(owned_argument);
    let mut affine = subloan(vec![field()], StructuralAccess::Owned);
    affine.functions[0].structural_parameters[0].multiplicity = StructuralMultiplicity::Affine;
    rejects_contract(affine);
    let mut affine_callee = subloan(vec![field()], StructuralAccess::Owned);
    affine_callee.functions[1].structural_parameters[0].multiplicity =
        StructuralMultiplicity::Affine;
    rejects_contract(affine_callee);
}

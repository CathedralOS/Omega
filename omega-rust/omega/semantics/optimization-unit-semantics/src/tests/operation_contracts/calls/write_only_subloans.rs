//! Exact unrestricted subloan contracts, independently of native realization.

use crate::tests::{id, refresh_identity, refresh_node_derivatives, structural_call_unit};
use crate::{OptimizationUnitValidationError, validate_psi_optimization_unit};
use abstract_operations::AbstractOperation;
use optimization_unit::PsiOptimizationUnit;
use semantic_vocabulary::{
    IntegerSign, IntegerType, PlaceId, ScalarType, StructuralFieldId, StructuralPlaceKind,
    StructuralTypeId,
};
use terminal_psi::{
    BindingRelevance, StructuralAccess, StructuralArgument, StructuralFieldDeclaration,
    StructuralFieldType, StructuralMultiplicity, StructuralPathSegment, StructuralPlaceDeclaration,
    StructuralTypeDeclaration, StructuralTypeShape,
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
fn unrestricted_write_only_field_index_and_interleaved_subloans_validate() {
    for access in [
        StructuralAccess::MutableBorrow,
        StructuralAccess::WriteOnlyBorrow,
    ] {
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
            let unit = subloan(path.clone(), access);
            validate_psi_optimization_unit(&unit)
                .unwrap_or_else(|error| panic!("{access:?} {path:?}: {error:?}"));
            assert_eq!(
                unit.functions[1].structural_parameters[0].multiplicity,
                StructuralMultiplicity::Unrestricted
            );
        }
    }
}

#[test]
fn write_only_path_depth_is_not_a_one_or_two_index_roster() {
    for depth in [3, 9, 70] {
        for path in [
            vec![StructuralPathSegment::FixedIndex(1); depth],
            (0..depth)
                .map(|position| {
                    if position % 2 == 0 {
                        field()
                    } else {
                        StructuralPathSegment::FixedIndex(1)
                    }
                })
                .collect(),
        ] {
            validate_psi_optimization_unit(&subloan(path, StructuralAccess::WriteOnlyBorrow))
                .expect("each finite material path is resolved hop by hop");
        }
    }
}

#[test]
fn mutable_field_subloan_retains_unrestricted_multiplicity() {
    let mut unit = subloan(
        vec![field(), field(), field()],
        StructuralAccess::MutableBorrow,
    );
    unit.functions[1].structural_parameters[0].access = StructuralAccess::MutableBorrow;
    arguments(&mut unit)[0].access = StructuralAccess::MutableBorrow;
    refresh_node_derivatives(&mut unit, 0, 0, 0);
    validate_psi_optimization_unit(&unit)
        .expect("mutable field subloan is not an owned linear projection");
}

#[test]
fn subloan_rejects_access_amplification_and_wrong_multiplicity() {
    let baseline = subloan(
        vec![StructuralPathSegment::FixedIndex(1)],
        StructuralAccess::WriteOnlyBorrow,
    );
    for access in [
        StructuralAccess::SharedBorrow,
        StructuralAccess::MutableBorrow,
        StructuralAccess::Owned,
    ] {
        let mut unit = baseline.clone();
        unit.functions[1].structural_parameters[0].access = access;
        arguments(&mut unit)[0].access = access;
        rejects_contract(unit);
    }
    let mut shared_source = baseline.clone();
    shared_source.functions[0].structural_parameters[0].access = StructuralAccess::SharedBorrow;
    rejects_contract(shared_source);
    let mut multiplicity = baseline;
    multiplicity.functions[1].structural_parameters[0].multiplicity =
        StructuralMultiplicity::Affine;
    rejects_contract(multiplicity);
}

#[test]
fn indexed_write_only_paths_cannot_fall_back_to_linear_multiplicity() {
    // Linear borrowed parameters can fail the catalog gate first. Exercise the
    // independent call matcher directly so that gate cannot mask its fallback.
    let rejects_linear = |mut unit: PsiOptimizationUnit| {
        let arguments = arguments(&mut unit).clone();
        let types = unit
            .structural_types
            .iter()
            .map(|declaration| (declaration.id, declaration))
            .collect();
        assert!(!crate::structural_arguments_match(
            &unit.functions[0],
            &arguments,
            &unit.functions[1].structural_parameters,
            &types,
            crate::StructuralProjectionPolicy::Unit,
            false,
        ));
        refresh_node_derivatives(&mut unit, 0, 0, 0);
        assert!(validate_psi_optimization_unit(&unit).is_err());
    };
    for path in [
        vec![StructuralPathSegment::FixedIndex(1)],
        vec![field(), StructuralPathSegment::FixedIndex(1), field()],
    ] {
        let baseline = subloan(path, StructuralAccess::WriteOnlyBorrow);
        let mut linear_callee = baseline.clone();
        linear_callee.functions[1].structural_parameters[0].multiplicity =
            StructuralMultiplicity::Linear;
        rejects_linear(linear_callee.clone());
        linear_callee.functions[0].structural_parameters[0].multiplicity =
            StructuralMultiplicity::Linear;
        rejects_linear(linear_callee);

        let mut reference = baseline;
        reference.functions[1].structural_parameters[0].multiplicity =
            StructuralMultiplicity::Linear;
        let StructuralTypeShape::Record { fields } =
            &mut reference.structural_types.make_mut()[0].shape
        else {
            panic!("leaf is a record")
        };
        fields[0].field_type =
            StructuralFieldType::ByteSequence(terminal_psi::ByteSequenceCarrier::BorrowedView);
        rejects_linear(reference);
    }
}

#[test]
fn subloan_rejects_wrong_field_bounds_root_and_leaf_type() {
    let baseline = subloan(
        vec![field(), StructuralPathSegment::FixedIndex(1)],
        StructuralAccess::MutableBorrow,
    );
    let mut field = baseline.clone();
    arguments(&mut field)[0].path[0] = StructuralPathSegment::Field("absent".into());
    rejects_contract(field);
    let mut bounds = baseline.clone();
    arguments(&mut bounds)[0].path[1] = StructuralPathSegment::FixedIndex(2);
    rejects_contract(bounds);
    let mut root = baseline.clone();
    arguments(&mut root)[0].place = root.functions[1].structural_parameters[0].place;
    // An undeclared caller root is rejected by catalog/use validation, before the call join.
    refresh_node_derivatives(&mut root, 0, 0, 0);
    assert!(validate_psi_optimization_unit(&root).is_err());
    let mut leaf = baseline;
    leaf.functions[1].structural_parameters[0].structural_type =
        leaf.functions[0].structural_parameters[0].structural_type;
    rejects_contract(leaf);
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
        StructuralFieldType::ByteSequence(terminal_psi::ByteSequenceCarrier::BorrowedView);
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
fn optimizer_overlap_contract_distinguishes_equal_and_disjoint_indexes() {
    // This exercises the optimizer argument-overlap contract, not Terminal's
    // separate currently single-projection Unit call admission boundary.
    let mut unit = subloan(
        vec![StructuralPathSegment::FixedIndex(0)],
        StructuralAccess::WriteOnlyBorrow,
    );
    let mut parameter = unit.functions[1].structural_parameters[0].clone();
    parameter.place = id(9_010, PlaceId::new);
    parameter.position = 1;
    unit.functions[1].declared_places.insert(parameter.place);
    unit.functions[1]
        .structural_places
        .push(StructuralPlaceDeclaration {
            id: parameter.place,
            kind: StructuralPlaceKind::Parameter {
                position: 1,
                is_self: false,
            },
        });
    unit.functions[1].structural_parameters.push(parameter);
    let mut second = arguments(&mut unit)[0].clone();
    second.path = vec![StructuralPathSegment::FixedIndex(1)];
    arguments(&mut unit).push(second);
    refresh_node_derivatives(&mut unit, 0, 0, 0);
    validate_psi_optimization_unit(&unit).expect("different fixed indexes do not overlap");
    arguments(&mut unit)[1].path = vec![StructuralPathSegment::FixedIndex(0)];
    rejects_contract(unit);
}

#[test]
fn projected_unit_call_rejects_callee_root_qualification_even_when_supplied() {
    let mut unit = subloan(
        vec![StructuralPathSegment::FixedIndex(1)],
        StructuralAccess::WriteOnlyBorrow,
    );
    let domain = id(9_020, semantic_vocabulary::StructuralDomainId::new);
    unit.structural_domains = vec![terminal_psi::StructuralDomainDeclaration {
        id: domain,
        semantic_domain: id(9_021, semantic_vocabulary::DomainSemanticId::new),
        identity: "validation::qualified-subloan".into(),
        carrier: unit.functions[1].structural_parameters[0].structural_type,
        content_projection: None,
    }]
    .into();
    unit.functions[0].structural_parameters[0].projected_qualifications =
        vec![terminal_psi::StructuralPathQualification {
            path: vec![StructuralPathSegment::FixedIndex(1)],
            domain,
        }];
    unit.functions[1].structural_parameters[0].qualifications = vec![domain];
    rejects_contract(unit);
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
fn changed_call_ownership_evidence_does_not_inherit_subloan_admission() {
    let mut unit = subloan(
        vec![StructuralPathSegment::FixedIndex(1)],
        StructuralAccess::WriteOnlyBorrow,
    );
    unit.functions[0].blocks[0].nodes[0].ownership.clear();
    refresh_identity(&mut unit);
    assert!(validate_psi_optimization_unit(&unit).is_err());
}

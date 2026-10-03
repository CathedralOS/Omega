//! Array-element restoration windows retain their declared field and element type.

use crate::tests::fixtures::dominance::refresh_function_derivatives;
use crate::tests::fixtures::structural_catalog::structural_scalar_field_store_unit;
use crate::tests::support::id;
use crate::validate_psi_optimization_unit;
use abstract_operations::{AbstractOperation as O, StructuralWindowRoot};
use optimization_unit::PsiOptimizationUnit;
use semantic_vocabulary::{
    OperationId, PlaceId, StructuralFieldId, StructuralPlaceKind, StructuralTypeId,
};
use terminal_psi::{
    StructuralAccess, StructuralArgument, StructuralFieldType, StructuralMultiplicity,
    StructuralOperationResult, StructuralPathSegment as Path, StructuralPlaceDeclaration,
    StructuralTypeDeclaration, StructuralTypeShape as Shape,
};

fn element_window_unit(nested: bool) -> PsiOptimizationUnit {
    let mut unit = structural_scalar_field_store_unit();
    let parameter = unit.functions[0].structural_parameters[0].clone();
    let owner = unit
        .structural_types
        .make_mut()
        .iter_mut()
        .find(|declaration| declaration.id == parameter.structural_type)
        .unwrap();
    let Shape::Record { fields } = &mut owner.shape else {
        unreachable!()
    };
    let StructuralFieldType::Structural(element) = fields[0].field_type else {
        unreachable!()
    };
    let array = id(4_800, StructuralTypeId::new);
    fields[0].field_type = StructuralFieldType::Structural(array);
    let mut sibling = fields[0].clone();
    sibling.id = id(4_899, StructuralFieldId::new);
    sibling.identity = "sibling".into();
    fields.push(sibling);
    let inner = id(4_801, StructuralTypeId::new);
    unit.structural_types
        .make_mut()
        .push(StructuralTypeDeclaration {
            id: array,
            identity: "validation::window-array".into(),
            shape: Shape::FixedArray {
                element: if nested { inner } else { element },
                length: 2,
            },
        });
    if nested {
        unit.structural_types
            .make_mut()
            .push(StructuralTypeDeclaration {
                id: inner,
                identity: "validation::window-inner-array".into(),
                shape: Shape::FixedArray { element, length: 3 },
            });
    }
    let producer = id(4_820, OperationId::new);
    let result = StructuralOperationResult {
        place: id(4_821, PlaceId::new),
        structural_type: element,
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: Vec::new(),
        qualification_establishments: Vec::new(),
        projected_qualifications: Vec::new(),
        claims: Vec::new(),
    };
    let mut path = vec![Path::Field("item".into()), Path::FixedIndex(1)];
    if nested {
        path.push(Path::FixedIndex(2));
    }
    let function = &mut unit.functions[0];
    function.structural_places.push(StructuralPlaceDeclaration {
        id: result.place,
        kind: StructuralPlaceKind::OperationResult {
            producer,
            structural_type: element,
        },
    });
    function.declared_places.insert(result.place);
    function.blocks[0].nodes[0].operation = O::MoveStructuralField {
        psi_operation: producer,
        result: result.clone(),
        source: StructuralWindowRoot::BorrowedParameter(parameter.clone()),
        path: path.clone(),
        field: None,
    };
    function.blocks[0].nodes[1].operation = O::StoreStructuralField {
        psi_operation: id(4_822, OperationId::new),
        destination: StructuralWindowRoot::BorrowedParameter(parameter),
        path,
        field: None,
        value: StructuralArgument {
            place: result.place,
            path: Vec::new(),
            access: StructuralAccess::Owned,
        },
    };
    refresh_function_derivatives(&mut unit, 0);
    unit
}

#[test]
fn element_windows_replay_array_field_and_selected_element() {
    for nested in [false, true] {
        let unit = element_window_unit(nested);
        validate_psi_optimization_unit(&unit).expect("array element restoration validates");
        for operation_index in [0, 1] {
            for mutation in [
                "out of bounds",
                "wrong field",
                "missing field",
                "wrong type",
            ] {
                let mut changed = unit.clone();
                let operation =
                    &mut changed.functions[0].blocks[0].nodes[operation_index].operation;
                let (path, field) = match operation {
                    O::MoveStructuralField { path, field, .. }
                    | O::StoreStructuralField { path, field, .. } => (path, field),
                    _ => unreachable!(),
                };
                match mutation {
                    "out of bounds" => path[1] = Path::FixedIndex(2),
                    "wrong field" => *field = Some(id(4_899, StructuralFieldId::new)),
                    "missing field" => path[0] = Path::Field("absent".into()),
                    "wrong type" => {
                        path.pop();
                    }
                    _ => unreachable!(),
                }
                refresh_function_derivatives(&mut changed, 0);
                assert!(
                    validate_psi_optimization_unit(&changed).is_err(),
                    "nested={nested}, operation={operation_index}, mutation={mutation}"
                );
            }
        }
    }
}

#[test]
fn element_windows_do_not_grant_shared_or_borrowed_replacement_authority() {
    let baseline = element_window_unit(false);
    for mutation in ["shared root", "borrowed replacement"] {
        let mut changed = baseline.clone();
        if mutation == "shared root" {
            changed.functions[0].structural_parameters[0].access = StructuralAccess::SharedBorrow;
            for node in &mut changed.functions[0].blocks[0].nodes {
                let root = match &mut node.operation {
                    O::MoveStructuralField { source, .. } => source,
                    O::StoreStructuralField { destination, .. } => destination,
                    _ => continue,
                };
                let StructuralWindowRoot::BorrowedParameter(parameter) = root else {
                    unreachable!()
                };
                parameter.access = StructuralAccess::SharedBorrow;
            }
        } else {
            let O::StoreStructuralField { value, .. } =
                &mut changed.functions[0].blocks[0].nodes[1].operation
            else {
                unreachable!()
            };
            value.access = StructuralAccess::SharedBorrow;
        }
        refresh_function_derivatives(&mut changed, 0);
        assert!(
            validate_psi_optimization_unit(&changed).is_err(),
            "{mutation}"
        );
    }
}

fn reference_window_unit() -> PsiOptimizationUnit {
    let mut unit = element_window_unit(true);
    let parameter = unit.functions[0].structural_parameters[0].clone();
    let reference_type = id(4_850, StructuralTypeId::new);
    unit.structural_types
        .make_mut()
        .push(StructuralTypeDeclaration {
            id: reference_type,
            identity: "validation::window-reference".into(),
            shape: Shape::Reference {
                referent: parameter.structural_type,
                access: StructuralAccess::MutableBorrow,
            },
        });
    let producer = id(4_851, OperationId::new);
    let result = StructuralOperationResult {
        place: id(4_852, PlaceId::new),
        structural_type: reference_type,
        multiplicity: StructuralMultiplicity::Affine,
        qualifications: Vec::new(),
        qualification_establishments: Vec::new(),
        projected_qualifications: Vec::new(),
        claims: Vec::new(),
    };
    let function = &mut unit.functions[0];
    function.structural_places.push(StructuralPlaceDeclaration {
        id: result.place,
        kind: StructuralPlaceKind::OperationResult {
            producer,
            structural_type: reference_type,
        },
    });
    function.declared_places.insert(result.place);
    for node in &mut function.blocks[0].nodes {
        let (root, path) = match &mut node.operation {
            O::MoveStructuralField { source, path, .. } => (source, path),
            O::StoreStructuralField {
                destination, path, ..
            } => (destination, path),
            _ => continue,
        };
        *root = StructuralWindowRoot::OwnedResult {
            producer,
            result: result.clone(),
        };
        path.insert(0, Path::Referent);
    }
    let mut establish = function.blocks[0].nodes[0].clone();
    establish.operation = O::EstablishReference {
        psi_operation: producer,
        result: result.clone(),
        source: StructuralArgument {
            place: parameter.place,
            access: StructuralAccess::MutableBorrow,
            path: Vec::new(),
        },
    };
    let mut release = establish.clone();
    release.operation = O::ReleaseReference {
        psi_operation: id(4_853, OperationId::new),
        source: result.place,
    };
    function.blocks[0].nodes.insert(0, establish);
    function.blocks[0].nodes.insert(3, release);
    refresh_function_derivatives(&mut unit, 0);
    unit
}

#[test]
fn reference_windows_replay_carrier_crossing_and_live_authority() {
    let unit = reference_window_unit();
    validate_psi_optimization_unit(&unit)
        .expect("live mutable carrier lends its exact nested field");
    for mutation in [
        "missing crossing",
        "wrong producer",
        "forged multiplicity",
        "shared",
        "released",
        "original ingress",
    ] {
        let mut changed = unit.clone();
        match mutation {
            "shared" => {
                let declaration = changed
                    .structural_types
                    .make_mut()
                    .iter_mut()
                    .find(|row| row.id == id(4_850, StructuralTypeId::new))
                    .unwrap();
                let Shape::Reference { access, .. } = &mut declaration.shape else {
                    unreachable!()
                };
                *access = StructuralAccess::SharedBorrow;
            }
            "released" => changed.functions[0].blocks[0].nodes.swap(1, 3),
            _ => {
                let parameter = changed.functions[0].structural_parameters[0].clone();
                let O::MoveStructuralField { source, path, .. } =
                    &mut changed.functions[0].blocks[0].nodes[1].operation
                else {
                    unreachable!()
                };
                match mutation {
                    "missing crossing" => {
                        path.remove(0);
                    }
                    "original ingress" => {
                        *source = StructuralWindowRoot::BorrowedParameter(parameter);
                        path.remove(0);
                    }
                    "wrong producer" => {
                        let StructuralWindowRoot::OwnedResult { producer, .. } = source else {
                            unreachable!()
                        };
                        *producer = id(4_854, OperationId::new);
                    }
                    "forged multiplicity" => {
                        let StructuralWindowRoot::OwnedResult { result, .. } = source else {
                            unreachable!()
                        };
                        result.multiplicity = StructuralMultiplicity::Unrestricted;
                    }
                    _ => unreachable!(),
                }
            }
        }
        refresh_function_derivatives(&mut changed, 0);
        assert!(
            validate_psi_optimization_unit(&changed).is_err(),
            "{mutation}"
        );
    }
}

#[test]
fn window_result_query_distinguishes_owned_carriers_from_storage_loans() {
    use crate::unit_validation::operation_contracts::structural_operation_result_contract;
    use abstract_operations::AbstractResult;
    use semantic_vocabulary::{ScalarType, ValueId};

    let unit = reference_window_unit();
    let operation = &unit.functions[0].blocks[0].nodes[0].operation;
    let O::EstablishReference {
        psi_operation,
        result,
        source,
    } = operation
    else {
        unreachable!()
    };
    let contract = structural_operation_result_contract(&unit.functions[0], operation).unwrap();
    assert_eq!(contract.access, StructuralAccess::Owned);
    assert_eq!(contract.operation_result(), Some(result));

    // Identical result metadata cannot turn a storage loan into an owned
    // window root. The common query retains the producer's access semantics.
    let primitive = O::EstablishPrimitiveLocal {
        psi_operation: *psi_operation,
        result: result.clone(),
        value: AbstractResult {
            value: id(4_860, ValueId::new),
            scalar_type: ScalarType::Boolean,
        },
    };
    assert_eq!(
        structural_operation_result_contract(&unit.functions[0], &primitive)
            .unwrap()
            .access,
        StructuralAccess::MutableBorrow,
    );
    for access in [
        StructuralAccess::SharedBorrow,
        StructuralAccess::MutableBorrow,
    ] {
        let mut source = source.clone();
        source.access = access;
        let view = O::EstablishElementView {
            region: terminal_psi::ElementViewRegion::WholeCollection,
            psi_operation: *psi_operation,
            result: result.clone(),
            destination: result.place,
            source,
            element: result.structural_type,
        };
        let contract = structural_operation_result_contract(&unit.functions[0], &view).unwrap();
        assert_eq!(contract.operation_result(), Some(result));
        assert_eq!(contract.access, access);
        assert_ne!(contract.access, StructuralAccess::Owned);
    }
}

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
    let field = fields[0].id;
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
        field: Some(field),
    };
    function.blocks[0].nodes[1].operation = O::StoreStructuralField {
        psi_operation: id(4_822, OperationId::new),
        destination: StructuralWindowRoot::BorrowedParameter(parameter),
        path,
        field: Some(field),
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

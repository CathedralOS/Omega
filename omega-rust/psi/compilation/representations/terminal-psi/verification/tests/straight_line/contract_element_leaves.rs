//! Contract field terms may name one static element of a primitive array.
//!
//! A collection invariant's per-element row observes `grid.board[3]`: the
//! canonical path ends at a fixed index into a fixed array whose element
//! shape is the term's exact primitive scalar. The index stays within the
//! declared extent, and a term naming the array itself, another element
//! type or an out-of-bounds index still rejects before any rule runs.

use super::{
    IntegerSign, IntegerType, IntegerValue, ModuleError, PlaceId, Proposition, ScalarTerm,
    ScalarType, StructuralAccess, StructuralFieldDeclaration, StructuralFieldType,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralPlaceDeclaration,
    StructuralPlaceKind, StructuralTypeDeclaration, StructuralTypeId, StructuralTypeShape,
    payloadless_guard_module, validate_module,
};
use semantic_vocabulary::{CanonicalStructuralPathSegment, StructuralFieldId};

#[test]
fn contract_terms_may_name_one_static_primitive_array_element() {
    let (mut module, _, _, _) = payloadless_guard_module();
    let root = PlaceId::new(940).unwrap();
    let element_type = StructuralTypeId::new(940).unwrap();
    let array_type = StructuralTypeId::new(941).unwrap();
    let grid_type = StructuralTypeId::new(942).unwrap();
    let board = StructuralFieldId::new(1).unwrap();
    let integer_type = IntegerType::new(IntegerSign::Signed, 32).unwrap();
    module.structural_types.extend([
        StructuralTypeDeclaration {
            id: element_type,
            identity: "i32".into(),
            shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(integer_type)),
        },
        StructuralTypeDeclaration {
            id: array_type,
            identity: "[i32; 4]".into(),
            shape: StructuralTypeShape::FixedArray {
                element: element_type,
                length: 4,
            },
        },
        StructuralTypeDeclaration {
            id: grid_type,
            identity: "Grid".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![StructuralFieldDeclaration {
                    id: board,
                    identity: "board".into(),
                    field_type: StructuralFieldType::Structural(array_type),
                    relevance: terminal_psi::BindingRelevance::Relevant,
                }],
            },
        },
    ]);
    let machine = &mut module.machines[0];
    machine
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place: root,
            position: 0,
            is_self: false,
            structural_type: grid_type,
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::SharedBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    machine.structural_places.push(StructuralPlaceDeclaration {
        id: root,
        kind: StructuralPlaceKind::Parameter {
            position: 0,
            is_self: false,
        },
    });
    let element = |path: Vec<CanonicalStructuralPathSegment>, scalar_type| {
        ScalarTerm::integer_field_path(root, path, scalar_type)
    };
    let cell = |index| {
        vec![
            CanonicalStructuralPathSegment::Field(board),
            CanonicalStructuralPathSegment::FixedIndex(index),
        ]
    };
    let one = ScalarTerm::integer(integer_type, IntegerValue::Signed(1)).unwrap();
    let with_requirement = |requirement: Proposition| {
        let mut module = module.clone();
        module.machines[0].contract.requires = vec![requirement];
        module
    };
    validate_module(&with_requirement(Proposition::LessOrEqual(
        element(cell(3), integer_type),
        one.clone(),
    )))
    .expect("a static in-bounds primitive element is a contract leaf");
    let wide = IntegerType::new(IntegerSign::Signed, 64).unwrap();
    let wide_one = ScalarTerm::integer(wide, IntegerValue::Signed(1)).unwrap();
    for (path, scalar_type, bound) in [
        (cell(4), integer_type, one.clone()),
        (cell(3), wide, wide_one),
        (
            vec![CanonicalStructuralPathSegment::Field(board)],
            integer_type,
            one.clone(),
        ),
    ] {
        assert!(
            matches!(
                validate_module(&with_requirement(Proposition::LessOrEqual(
                    element(path.clone(), scalar_type),
                    bound,
                ))),
                Err(ModuleError::InvalidIntegerFieldTerm { .. })
            ),
            "{path:?} names no {scalar_type:?} element leaf"
        );
    }
}

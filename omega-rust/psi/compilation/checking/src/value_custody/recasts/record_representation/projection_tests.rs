//! Symbolic recast geometry does not discharge selector bounds or loan custody.

use super::*;
use checked_trees::{CheckedRuntimeIndex, CheckedUnitStructuralPathSegment as Segment};
use typed_trees::{data::DataMember, name::Identifier};

fn fixture(source: &str) -> (TypedTrees, TypeReferenceHandle, Vec<Segment>) {
    let mut program = crate::front_end::typed_program(source);
    let data = program
        .data_definitions()
        .iter()
        .find(|data| data.name.as_str() == "Matrix")
        .expect("matrix");
    let symbol = data.symbol;
    let fields = program
        .data_members(data)
        .iter()
        .filter_map(|member| match member {
            DataMember::Field(field) => Some(Segment::Field(field.path_identity())),
            _ => None,
        })
        .collect();
    let root = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol,
            name: Identifier::generated("Matrix"),
        });
    (program, root, fields)
}

fn runtime() -> Segment {
    Segment::RuntimeIndex(CheckedRuntimeIndex::Parameter { position: 0 })
}

#[test]
fn symbolic_recast_geometry_retains_each_array_occurrence_and_fixed_offsets() {
    let (program, root, fields) = fixture("data Matrix { prefix: u32; values: [[u16; 3]; 2]; }");
    let layout = recast_scalar_layout(&program, root).expect("layout");
    let path = [fields[1].clone(), runtime(), runtime()];
    let geometry = layout.scalar_path_geometry(&path).expect("symbolic leaf");
    assert_eq!(geometry.projection.offset, 4);
    assert_eq!(geometry.projection.footprint, 16);
    assert_eq!(
        geometry.projection.primitive,
        typed_trees::types::PrimitiveType::U16
    );
    assert_eq!(
        geometry.indices,
        vec![
            RecastRuntimeIndexGeometry {
                path_index: 1,
                stride: 6,
                extent: 2
            },
            RecastRuntimeIndexGeometry {
                path_index: 2,
                stride: 2,
                extent: 3
            },
        ]
    );
    assert_eq!(
        recast_scalar_path_geometry(&program, root, &path),
        Some(geometry.clone())
    );
    assert!(layout.scalar_path_projection(&path).is_none());
    // The same supplied selector still owes two independent array bounds.
    for outer in 0..2 {
        for inner in 0..3 {
            let fixed = [
                fields[1].clone(),
                Segment::FixedIndex(outer),
                Segment::FixedIndex(inner),
            ];
            let projection = layout.scalar_path_projection(&fixed).expect("fixed leaf");
            assert_eq!(
                projection.offset,
                geometry.projection.offset + outer as usize * 6 + inner as usize * 2
            );
            let mixed = [fields[1].clone(), Segment::FixedIndex(outer), runtime()];
            let mixed_geometry = layout.scalar_path_geometry(&mixed).expect("mixed leaf");
            assert_eq!(mixed_geometry.projection.offset, 4 + outer as usize * 6);
            assert_eq!(mixed_geometry.indices, vec![geometry.indices[1].clone()]);
            assert!(
                layout
                    .scalar_path_geometry(&fixed)
                    .expect("fixed geometry")
                    .indices
                    .is_empty()
            );
        }
    }
}

#[test]
fn symbolic_recast_geometry_rejects_invalid_paths_and_empty_arrays() {
    let (program, root, fields) =
        fixture("data Matrix { values: [[u16; 3]; 2]; empty: [u16; 0]; }");
    let layout = recast_scalar_layout(&program, root).expect("layout");
    for path in [
        vec![fields[0].clone(), Segment::FixedIndex(2), runtime()],
        vec![fields[0].clone(), runtime(), Segment::FixedIndex(3)],
        vec![fields[0].clone(), runtime()],
        vec![fields[1].clone(), runtime()],
        vec![Segment::Field("absent".to_owned()), runtime(), runtime()],
        vec![runtime(), runtime(), runtime()],
        vec![fields[0].clone(), runtime(), runtime(), runtime()],
    ] {
        assert!(layout.scalar_path_geometry(&path).is_none(), "{path:?}");
    }
}

#[test]
fn symbolic_recast_geometry_retains_normalized_repetition_stride() {
    let (program, root, _) = fixture("data Matrix { value: u16; }");
    let element = shared_projection_type_representation(&program, root).expect("element");
    // Explicit normalized repetition uses eight-byte slots for two-byte values.
    let repeated = repeat_representation_with_stride(&element, 3, 8).expect("padded repetition");
    let zero = &repeated.leaves[0];
    let layout = RecastScalarLayout {
        leaves: vec![RecastScalarLayoutLeaf {
            path: vec![Segment::FixedIndex(0), Segment::Field("value".to_owned())]
                .into_boxed_slice(),
            projection: scalar_leaf_projection(repeated.size, zero).expect("projection"),
            arrays: zero.arrays.clone().into_boxed_slice(),
        }]
        .into(),
    };
    let geometry = layout
        .scalar_path_geometry(&[runtime(), Segment::Field("value".to_owned())])
        .expect("padded leaf");
    assert_eq!(geometry.projection.footprint, 18);
    assert_eq!(
        geometry.indices,
        vec![RecastRuntimeIndexGeometry {
            path_index: 0,
            stride: 8,
            extent: 3
        }]
    );
    assert!(repeat_representation_with_stride(&element, 3, 1).is_none());
}

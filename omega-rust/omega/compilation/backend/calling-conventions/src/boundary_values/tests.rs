use super::{BoundaryValueClass, BoundaryValueField, BoundaryValueGraph, BoundaryValueShape};
use crate::{CallingPolicy, SystemVEightbyteClass, ValueClass, ValueShape};

fn shape(class: BoundaryValueClass, byte_size: u16, alignment: u16) -> BoundaryValueShape {
    BoundaryValueShape {
        class,
        byte_size,
        alignment,
    }
}

fn four_f32_array() -> [BoundaryValueShape; 2] {
    [
        shape(BoundaryValueClass::Float, 4, 4),
        shape(
            BoundaryValueClass::FixedArray {
                element: 0,
                length: 4,
            },
            16,
            4,
        ),
    ]
}

#[test]
fn recursive_array_classification_is_policy_specific() {
    let shapes = four_f32_array();
    let graph = BoundaryValueGraph {
        shapes: &shapes,
        fields: &[],
    };
    let hfa = ValueShape::homogeneous_float_aggregate(4, 4);
    for (policy, expected) in [
        (CallingPolicy::Aapcs64, hfa),
        (CallingPolicy::SystemVAMD64, hfa),
        (CallingPolicy::MicrosoftX64, ValueShape::integer(16, 4)),
    ] {
        assert_eq!(graph.classify(1, policy).unwrap(), expected);
        graph
            .validate_abi_shape(1, expected, policy)
            .expect("exact policy class");
    }
}

#[test]
fn large_homogeneous_float_values_follow_each_policys_memory_cutoff() {
    let shapes = [
        shape(BoundaryValueClass::Float, 8, 8),
        shape(
            BoundaryValueClass::FixedArray {
                element: 0,
                length: 4,
            },
            32,
            8,
        ),
    ];
    let graph = BoundaryValueGraph {
        shapes: &shapes,
        fields: &[],
    };
    for (policy, expected) in [
        (
            CallingPolicy::Aapcs64,
            ValueShape::homogeneous_float_aggregate(8, 4),
        ),
        (CallingPolicy::SystemVAMD64, ValueShape::integer(32, 8)),
        (CallingPolicy::MicrosoftX64, ValueShape::integer(32, 8)),
    ] {
        let classified = graph.classify(1, policy).unwrap();
        assert_eq!(classified, expected);
        graph.validate_abi_shape(1, classified, policy).unwrap();
        crate::evaluate_ordinary_boundary_entry_plan(
            policy,
            &crate::CallSignature {
                parameters: vec![classified],
                result: Some(classified),
            },
        )
        .expect("classification must be a realizable ordinary argument/result shape");
    }
    assert!(
        graph
            .validate_abi_shape(
                1,
                ValueShape::homogeneous_float_aggregate(8, 4),
                CallingPolicy::SystemVAMD64
            )
            .is_err()
    );
}

#[test]
fn authored_policy_cannot_publish_a_preclassified_shape_for_the_wrong_abi() {
    let shapes = four_f32_array();
    let graph = BoundaryValueGraph {
        shapes: &shapes,
        fields: &[],
    };
    let error = graph
        .validate_abi_shape(1, ValueShape::integer(16, 4), CallingPolicy::Aapcs64)
        .expect_err("AAPCS64 policy must preserve the HFA classification");
    assert!(
        error.contains("requires HomogeneousFloatAggregate"),
        "{error}"
    );
    assert_eq!(
        graph.validate_abi_shape(1, ValueShape::integer(8, 8), CallingPolicy::Aapcs64),
        Err("ABI shape 8/8 does not preserve semantic size/alignment 16/4".to_owned())
    );
}

#[test]
fn nested_record_classification_preserves_leaf_offsets_and_eightbyte_order() {
    let shapes = [
        shape(BoundaryValueClass::Float, 8, 8),
        shape(BoundaryValueClass::Integer, 8, 8),
        shape(
            BoundaryValueClass::Record {
                first_field: 0,
                field_count: 2,
            },
            16,
            8,
        ),
        shape(
            BoundaryValueClass::Record {
                first_field: 2,
                field_count: 1,
            },
            16,
            8,
        ),
    ];
    let mut fields = [
        BoundaryValueField {
            shape: 0,
            byte_offset: 0,
        },
        BoundaryValueField {
            shape: 1,
            byte_offset: 8,
        },
        BoundaryValueField {
            shape: 2,
            byte_offset: 0,
        },
    ];
    let graph = BoundaryValueGraph {
        shapes: &shapes,
        fields: &fields,
    };
    let mixed = ValueShape {
        class: ValueClass::SystemVAggregate {
            first: SystemVEightbyteClass::Sse,
            second: SystemVEightbyteClass::Integer,
        },
        byte_size: 16,
        alignment: 8,
    };
    assert_eq!(
        graph.classify(3, CallingPolicy::SystemVAMD64).unwrap(),
        mixed
    );
    for policy in [CallingPolicy::Aapcs64, CallingPolicy::MicrosoftX64] {
        assert_eq!(
            graph.classify(3, policy).unwrap(),
            ValueShape::integer(16, 8)
        );
    }
    fields[0].byte_offset = 8;
    fields[1].byte_offset = 0;
    let graph = BoundaryValueGraph {
        shapes: &shapes,
        fields: &fields,
    };
    assert_eq!(
        graph
            .classify(3, CallingPolicy::SystemVAMD64)
            .unwrap()
            .class,
        ValueClass::SystemVAggregate {
            first: SystemVEightbyteClass::Integer,
            second: SystemVEightbyteClass::Sse,
        }
    );
    assert!(
        graph
            .validate_abi_shape(3, mixed, CallingPolicy::SystemVAMD64)
            .is_err()
    );
}

#[test]
fn malformed_shape_links_and_cross_eightbyte_leaves_reject() {
    let mut shapes = [
        shape(BoundaryValueClass::Float, 8, 8),
        shape(
            BoundaryValueClass::Record {
                first_field: 0,
                field_count: 1,
            },
            16,
            8,
        ),
    ];
    let fields = [BoundaryValueField {
        shape: 0,
        byte_offset: 4,
    }];
    let graph = BoundaryValueGraph {
        shapes: &shapes,
        fields: &fields,
    };
    assert_eq!(
        graph.classify(1, CallingPolicy::SystemVAMD64),
        Err("aggregate has a scalar leaf crossing a SysV eightbyte boundary".to_owned())
    );
    assert_eq!(
        graph.classify(2, CallingPolicy::Aapcs64),
        Err("shape root 2 is outside the normalized graph".to_owned())
    );
    let graph = BoundaryValueGraph {
        shapes: &shapes,
        fields: &[],
    };
    assert_eq!(
        graph.classify(1, CallingPolicy::Aapcs64),
        Err("record field range is outside the normalized graph".to_owned())
    );
    shapes[1].class = BoundaryValueClass::FixedArray {
        element: 2,
        length: 1,
    };
    let graph = BoundaryValueGraph {
        shapes: &shapes,
        fields: &[],
    };
    assert_eq!(
        graph.classify(1, CallingPolicy::Aapcs64),
        Err("array element shape 2 is outside the graph".to_owned())
    );
    shapes[1].class = BoundaryValueClass::FixedArray {
        element: 1,
        length: 1,
    };
    let graph = BoundaryValueGraph {
        shapes: &shapes,
        fields: &[],
    };
    assert_eq!(
        graph.classify(1, CallingPolicy::Aapcs64),
        Err("boundary shape nesting exceeds 32 levels".to_owned())
    );
}

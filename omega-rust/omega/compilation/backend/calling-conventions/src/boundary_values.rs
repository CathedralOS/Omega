/*
We define our source-free public boundary value graph (`BoundaryValueGraph`,
`BoundaryValueShape`, `BoundaryValueField`, `BoundaryValueClass`) and classify
recursive value shapes for a specific `CallingPolicy`. We determine ABI classes
from recursive scalar leaf offsets and widths, distinguishing AAPCS64 four-member
homogeneous-float aggregates from System V AMD64 two-eightbyte SSE/integer
aggregates and Microsoft x64 integer-class storage without consulting source
declarations or provider selection.
*/

use crate::{CallingPolicy, SystemVEightbyteClass, ValueClass, ValueShape};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BoundaryValueClass {
    Integer,
    Float,
    Reference,
    FixedArray { element: u16, length: u16 },
    Record { first_field: u16, field_count: u16 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoundaryValueShape {
    pub class: BoundaryValueClass,
    pub byte_size: u16,
    pub alignment: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoundaryValueField {
    pub shape: u16,
    pub byte_offset: u16,
}

impl BoundaryValueShape {
    pub const fn class(self) -> BoundaryValueClass {
        self.class
    }

    pub const fn byte_size(self) -> u16 {
        self.byte_size
    }

    pub const fn alignment(self) -> u16 {
        self.alignment
    }
}

impl BoundaryValueField {
    pub const fn shape(self) -> u16 {
        self.shape
    }

    pub const fn byte_offset(self) -> u16 {
        self.byte_offset
    }
}

pub struct BoundaryValueGraph<'a> {
    pub shapes: &'a [BoundaryValueShape],
    pub fields: &'a [BoundaryValueField],
}

impl BoundaryValueGraph<'_> {
    pub fn classify(&self, root: u16, policy: CallingPolicy) -> Result<ValueShape, String> {
        let shape = self
            .shapes
            .get(usize::from(root))
            .ok_or_else(|| format!("shape root {root} is outside the normalized graph"))?;
        let class = match shape.class {
            BoundaryValueClass::Integer | BoundaryValueClass::Reference => ValueClass::Integer,
            BoundaryValueClass::Float => ValueClass::Float,
            BoundaryValueClass::FixedArray { .. } | BoundaryValueClass::Record { .. } => {
                classify_boundary_aggregate(self, root, policy)?
            }
        };
        Ok(ValueShape {
            class,
            byte_size: u32::from(shape.byte_size),
            alignment: shape.alignment,
        })
    }

    pub fn validate_abi_shape(
        &self,
        root: u16,
        abi: ValueShape,
        policy: CallingPolicy,
    ) -> Result<(), String> {
        let shape = self
            .shapes
            .get(usize::from(root))
            .ok_or_else(|| format!("shape root {root} is outside the normalized graph"))?;
        if abi.byte_size != u32::from(shape.byte_size) || abi.alignment != shape.alignment {
            return Err(format!(
                "ABI shape {}/{} does not preserve semantic size/alignment {}/{}",
                abi.byte_size, abi.alignment, shape.byte_size, shape.alignment
            ));
        }
        let expected = self.classify(root, policy)?.class;
        if abi.class != expected {
            return Err(format!(
                "policy published ABI class {:?}, but its recursive public shape requires {:?} under {:?}",
                abi.class, expected, policy
            ));
        }
        Ok(())
    }
}

fn classify_boundary_aggregate(
    signature: &BoundaryValueGraph<'_>,
    root: u16,
    policy: CallingPolicy,
) -> Result<ValueClass, String> {
    if policy == CallingPolicy::MicrosoftX64 {
        return Ok(ValueClass::Integer);
    }
    let shape = signature
        .shapes
        .get(usize::from(root))
        .ok_or_else(|| format!("shape root {root} is outside the normalized graph"))?;
    let mut leaves = Vec::new();
    collect_boundary_scalar_leaves(signature, root, 0, &mut leaves, 0)?;
    if let Some(members) = homogeneous_float_leaf_shape(shape, &leaves) {
        if policy == CallingPolicy::Aapcs64
            || (policy == CallingPolicy::SystemVAMD64 && members > 1 && shape.byte_size <= 16)
        {
            return Ok(ValueClass::HomogeneousFloatAggregate { members });
        }
        if policy == CallingPolicy::SystemVAMD64 && members == 1 {
            return Ok(ValueClass::Float);
        }
    }
    if policy != CallingPolicy::SystemVAMD64 || shape.byte_size > 16 {
        return Ok(ValueClass::Integer);
    }
    let mut classes = [None, None];
    for &(offset, byte_size, is_float) in &leaves {
        let eightbyte = usize::from(offset) / 8;
        let last = usize::from(offset)
            .checked_add(usize::from(byte_size))
            .and_then(|end| end.checked_sub(1))
            .ok_or_else(|| "aggregate contains a zero-width scalar leaf".to_owned())?;
        if eightbyte > 1 || eightbyte != last / 8 {
            return Err(
                "aggregate has a scalar leaf crossing a SysV eightbyte boundary".to_owned(),
            );
        }
        classes[eightbyte] = Some(match classes[eightbyte] {
            Some(existing_is_sse) => existing_is_sse && is_float,
            None => is_float,
        });
    }
    let first = classes[0].ok_or_else(|| "aggregate has no scalar leaves".to_owned())?;
    let second = if shape.byte_size > 8 {
        classes[1].ok_or_else(|| "aggregate leaves do not cover its second eightbyte".to_owned())?
    } else {
        false
    };
    if !first && !second {
        return Ok(ValueClass::Integer);
    }
    if shape.byte_size <= 8 {
        return Err(
            "one-eightbyte non-homogeneous SSE aggregates are not representable in the closed ABI vocabulary"
                .to_owned(),
        );
    }
    Ok(ValueClass::SystemVAggregate {
        first: if first {
            SystemVEightbyteClass::Sse
        } else {
            SystemVEightbyteClass::Integer
        },
        second: if second {
            SystemVEightbyteClass::Sse
        } else {
            SystemVEightbyteClass::Integer
        },
    })
}

fn homogeneous_float_leaf_shape(
    aggregate: &BoundaryValueShape,
    leaves: &[(u16, u16, bool)],
) -> Option<u8> {
    let &(first_offset, member_size, true) = leaves.first()? else {
        return None;
    };
    let members = u8::try_from(leaves.len()).ok()?;
    (first_offset == 0
        && (1..=4).contains(&members)
        && matches!(member_size, 4 | 8)
        && leaves
            .iter()
            .enumerate()
            .all(|(index, &(offset, size, is_float))| {
                is_float
                    && size == member_size
                    && usize::from(offset) == index * usize::from(member_size)
            })
        && aggregate.byte_size == member_size * u16::from(members)
        && aggregate.alignment == member_size)
        .then_some(members)
}

fn collect_boundary_scalar_leaves(
    signature: &BoundaryValueGraph<'_>,
    root: u16,
    base_offset: u16,
    leaves: &mut Vec<(u16, u16, bool)>,
    depth: usize,
) -> Result<(), String> {
    if depth > 32 {
        return Err("boundary shape nesting exceeds 32 levels".to_owned());
    }
    let shape = signature
        .shapes
        .get(usize::from(root))
        .ok_or_else(|| format!("shape root {root} is outside the normalized graph"))?;
    match shape.class {
        BoundaryValueClass::Integer | BoundaryValueClass::Reference => {
            leaves.push((base_offset, shape.byte_size, false));
        }
        BoundaryValueClass::Float => leaves.push((base_offset, shape.byte_size, true)),
        BoundaryValueClass::FixedArray { element, length } => {
            let element_shape = signature
                .shapes
                .get(usize::from(element))
                .ok_or_else(|| format!("array element shape {element} is outside the graph"))?;
            for index in 0..length {
                let offset = element_shape
                    .byte_size
                    .checked_mul(index)
                    .and_then(|offset| base_offset.checked_add(offset))
                    .ok_or_else(|| "fixed-array leaf offset overflows u16".to_owned())?;
                collect_boundary_scalar_leaves(signature, element, offset, leaves, depth + 1)?;
            }
        }
        BoundaryValueClass::Record {
            first_field,
            field_count,
        } => {
            let start = usize::from(first_field);
            let end = start
                .checked_add(usize::from(field_count))
                .ok_or_else(|| "record field range overflows".to_owned())?;
            let fields = signature
                .fields
                .get(start..end)
                .ok_or_else(|| "record field range is outside the normalized graph".to_owned())?;
            for field in fields {
                let offset = base_offset
                    .checked_add(field.byte_offset)
                    .ok_or_else(|| "record field offset overflows u16".to_owned())?;
                collect_boundary_scalar_leaves(signature, field.shape, offset, leaves, depth + 1)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{BoundaryValueClass, BoundaryValueField, BoundaryValueGraph, BoundaryValueShape};
    use crate::{CallingPolicy, SystemVEightbyteClass, ValueClass, ValueShape};

    fn value_graph<'a>(
        shapes: &'a [BoundaryValueShape],
        fields: &'a [BoundaryValueField],
    ) -> BoundaryValueGraph<'a> {
        BoundaryValueGraph { shapes, fields }
    }

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
        let graph = value_graph(&shapes, &[]);
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
        let graph = value_graph(&shapes, &[]);
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
        let graph = value_graph(&shapes, &[]);
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
        let graph = value_graph(&shapes, &fields);
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
        let graph = value_graph(&shapes, &fields);
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
        let graph = value_graph(&shapes, &fields);
        assert_eq!(
            graph.classify(1, CallingPolicy::SystemVAMD64),
            Err("aggregate has a scalar leaf crossing a SysV eightbyte boundary".to_owned())
        );
        assert_eq!(
            graph.classify(2, CallingPolicy::Aapcs64),
            Err("shape root 2 is outside the normalized graph".to_owned())
        );
        let graph = value_graph(&shapes, &[]);
        assert_eq!(
            graph.classify(1, CallingPolicy::Aapcs64),
            Err("record field range is outside the normalized graph".to_owned())
        );
        shapes[1].class = BoundaryValueClass::FixedArray {
            element: 2,
            length: 1,
        };
        let graph = value_graph(&shapes, &[]);
        assert_eq!(
            graph.classify(1, CallingPolicy::Aapcs64),
            Err("array element shape 2 is outside the graph".to_owned())
        );
        shapes[1].class = BoundaryValueClass::FixedArray {
            element: 1,
            length: 1,
        };
        let graph = value_graph(&shapes, &[]);
        assert_eq!(
            graph.classify(1, CallingPolicy::Aapcs64),
            Err("boundary shape nesting exceeds 32 levels".to_owned())
        );
    }
}

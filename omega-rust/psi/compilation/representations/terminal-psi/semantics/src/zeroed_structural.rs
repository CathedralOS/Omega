use semantic_vocabulary::{
    BoundedIntegerType, IntegerSign, IntegerType, IntegerValue, ScalarType, StructuralCaseId,
    StructuralTypeId,
};
use terminal_psi::{
    StructuralFieldType, StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape,
};

/// Resource ceiling for the leaf walk: every leaf is at least one byte of
/// zeroed storage, and a degenerate fixed-array extent must not balloon the
/// worklist or the leaf vector before the descent guard can run — the bound
/// keeps worst-case enumeration proportional to real storage.
const MAX_ZEROED_LEAVES: usize = 65_536;

/// One scalar leaf cell of a zero-constructible structural type, addressed the
/// way runtime storage keys it: a record field under its parent path, or one
/// primitive-scalar element of a fixed array whose path ends at its index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ZeroedStructuralLeaf {
    RecordField {
        path: Vec<StructuralPathSegment>,
        field: semantic_vocabulary::StructuralFieldId,
        scalar_type: ScalarType,
    },
    ArrayScalar {
        path: Vec<StructuralPathSegment>,
        scalar_type: ScalarType,
    },
    /// The case-tag cell of a sum whose zeroed image selects its first
    /// declared case — the tag stores the case ordinal and all-zero bits
    /// name ordinal 0. `structural_type` retains the sum's own identity so
    /// the runtime registers the selected case beside the tag cell.
    SumSelection {
        path: Vec<StructuralPathSegment>,
        structural_type: StructuralTypeId,
        case: StructuralCaseId,
        scalar_type: ScalarType,
    },
}

impl ZeroedStructuralLeaf {
    /// The leaf's canonical static path below the established root.
    pub fn path(&self) -> &[StructuralPathSegment] {
        match self {
            ZeroedStructuralLeaf::RecordField { path, .. }
            | ZeroedStructuralLeaf::ArrayScalar { path, .. }
            | ZeroedStructuralLeaf::SumSelection { path, .. } => path,
        }
    }

    /// The leaf's plain scalar storage type.
    pub fn scalar_type(&self) -> ScalarType {
        match self {
            ZeroedStructuralLeaf::RecordField { scalar_type, .. }
            | ZeroedStructuralLeaf::ArrayScalar { scalar_type, .. }
            | ZeroedStructuralLeaf::SumSelection { scalar_type, .. } => *scalar_type,
        }
    }
}

/// Resolve whether a structural type's whole storage may begin zero: every
/// leaf a plain primitive scalar or sum case tag, and every aggregate a
/// record, fixed array, or admissible sum. A sum admits only when its first
/// declared case exists and every non-erased field of that case admits a zero
/// scalar: the tag's all-zero bits select that case (tag = case ordinal).
/// Bounded integer fields must contain the carrier's zero; nested composites
/// in sum payloads still need case-aware leaf paths. References, views, byte
/// sequences, mixed shapes, and erased members have no universally valid zero
/// spelling, so they reject. Unknown, cyclic, or recursive membership also
/// rejects — the worklist bounds the walk at the module's declaration count.
pub fn zeroed_structural_shape<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
) -> bool {
    zeroed_structural_leaves(types, root).is_some()
}

/// Enumerate every scalar leaf a zeroed establishment of `root` must
/// materialize, or `None` when the shape is not zero-constructible (the same
/// admission rule as [`zeroed_structural_shape`]). Record fields key by their
/// parent path and field id; fixed-array elements key by the full indexed
/// path.
pub fn zeroed_structural_leaves<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
) -> Option<Vec<ZeroedStructuralLeaf>> {
    let declarations: Vec<&StructuralTypeDeclaration> = types.collect();
    let bound = declarations.len() + 1;
    let mut leaves = Vec::new();
    // (type id, path to its runtime place root)
    let mut pending = vec![(root, Vec::new())];
    let mut visited_cells = 0usize;
    while let Some((current, path)) = pending.pop() {
        let declaration = declarations
            .iter()
            .find(|declaration| declaration.id == current)?;
        match &declaration.shape {
            // No payload does not mean no validity: a selected binding must be
            // established by the entry/transfer route, never by all-zero bytes.
            StructuralTypeShape::FusedServiceBinding => return None,
            StructuralTypeShape::PrimitiveScalar(scalar_type) => {
                leaves.push(ZeroedStructuralLeaf::ArrayScalar {
                    path,
                    scalar_type: *scalar_type,
                });
            }
            StructuralTypeShape::FixedArray { element, length } => {
                visited_cells += 1;
                // Descent bound: only Record/FixedArray visits can push work,
                // so they alone need the declarations² recursion guard — a
                // scalar element enumeration is declared extent, not
                // recursion.
                if visited_cells > bound.saturating_mul(bound).max(1024) {
                    return None;
                }
                let extent = usize::try_from(*length).ok()?;
                if pending.len().saturating_add(extent) > MAX_ZEROED_LEAVES {
                    return None;
                }
                for index in 0..extent {
                    let mut element_path = path.clone();
                    element_path.push(StructuralPathSegment::FixedIndex(index as u64));
                    pending.push((*element, element_path));
                }
            }
            StructuralTypeShape::Sum { cases } => {
                visited_cells += 1;
                if visited_cells > bound.saturating_mul(bound).max(1024) {
                    return None;
                }
                // Zeroed storage selects the first declared case — the tag
                // stores the case ordinal and zero names ordinal 0. Payload
                // leaves mint for that case only; later cases carry no cells.
                // Only a first case of zero-valid scalar fields admits;
                // nested composites have no case-aware leaf path segment.
                let Some(first) = cases.first() else {
                    return None;
                };
                let tag_type = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).ok()?);
                leaves.push(ZeroedStructuralLeaf::SumSelection {
                    path: path.clone(),
                    structural_type: declaration.id,
                    case: first.id,
                    scalar_type: tag_type,
                });
                for field in &first.fields {
                    match &field.field_type {
                        StructuralFieldType::IeeeFloat(format) if !field.relevance.is_erased() => {
                            leaves.push(ZeroedStructuralLeaf::RecordField {
                                path: path.clone(),
                                field: field.id,
                                scalar_type: ScalarType::IeeeFloat(*format),
                            });
                        }
                        StructuralFieldType::Scalar(scalar_type)
                            if !field.relevance.is_erased() =>
                        {
                            leaves.push(ZeroedStructuralLeaf::RecordField {
                                path: path.clone(),
                                field: field.id,
                                scalar_type: *scalar_type,
                            });
                        }
                        StructuralFieldType::BoundedInteger(bounds)
                            if !field.relevance.is_erased() && admits_zero(*bounds) =>
                        {
                            leaves.push(ZeroedStructuralLeaf::RecordField {
                                path: path.clone(),
                                field: field.id,
                                scalar_type: ScalarType::Integer(bounds.integer_type()),
                            });
                        }
                        _ => return None,
                    }
                }
            }
            StructuralTypeShape::Record { fields } => {
                visited_cells += 1;
                if visited_cells > bound.saturating_mul(bound).max(1024) {
                    return None;
                }
                for field in fields {
                    match &field.field_type {
                        StructuralFieldType::IeeeFloat(format) if !field.relevance.is_erased() => {
                            leaves.push(ZeroedStructuralLeaf::RecordField {
                                path: path.clone(),
                                field: field.id,
                                scalar_type: ScalarType::IeeeFloat(*format),
                            });
                        }
                        StructuralFieldType::Scalar(scalar_type) => {
                            leaves.push(ZeroedStructuralLeaf::RecordField {
                                path: path.clone(),
                                field: field.id,
                                scalar_type: *scalar_type,
                            });
                        }
                        StructuralFieldType::Structural(member) => {
                            let mut member_path = path.clone();
                            member_path.push(StructuralPathSegment::Field(field.identity.clone()));
                            pending.push((*member, member_path));
                        }
                        StructuralFieldType::BoundedInteger(bounds)
                            if !field.relevance.is_erased() && admits_zero(*bounds) =>
                        {
                            leaves.push(ZeroedStructuralLeaf::RecordField {
                                path: path.clone(),
                                field: field.id,
                                scalar_type: ScalarType::Integer(bounds.integer_type()),
                            });
                        }
                        _ => return None,
                    }
                }
            }
            _ => return None,
        }
        if leaves.len() + pending.len() > MAX_ZEROED_LEAVES {
            return None;
        }
    }
    Some(leaves)
}

fn admits_zero(bounds: BoundedIntegerType) -> bool {
    bounds.contains(match bounds.integer_type().sign() {
        IntegerSign::Signed => IntegerValue::Signed(0),
        IntegerSign::Unsigned => IntegerValue::Unsigned(0),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::StructuralFieldId;
    use terminal_psi::{BindingRelevance, StructuralCaseDeclaration, StructuralFieldDeclaration};

    fn field(id: u64, field_type: StructuralFieldType) -> StructuralFieldDeclaration {
        StructuralFieldDeclaration {
            id: StructuralFieldId::new(id).unwrap(),
            identity: format!("f{id}"),
            relevance: BindingRelevance::Relevant,
            field_type,
        }
    }

    fn erased(field: &mut StructuralFieldDeclaration) {
        field.relevance = BindingRelevance::Erased;
    }

    fn declaration(id: u64, shape: StructuralTypeShape) -> StructuralTypeDeclaration {
        StructuralTypeDeclaration {
            id: StructuralTypeId::new(id).unwrap(),
            identity: format!("T{id}"),
            shape,
        }
    }

    fn case(id: u64, fields: Vec<StructuralFieldDeclaration>) -> StructuralCaseDeclaration {
        StructuralCaseDeclaration {
            id: StructuralCaseId::new(id).unwrap(),
            identity: format!("C{id}"),
            fields,
        }
    }

    fn unsigned_32() -> ScalarType {
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).unwrap())
    }

    fn signed_tag() -> ScalarType {
        ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap())
    }

    #[test]
    fn fused_service_binding_cannot_be_zero_constructed_directly_or_nested() {
        let binding = declaration(1, StructuralTypeShape::FusedServiceBinding);
        assert!(!zeroed_structural_shape([&binding].into_iter(), binding.id));
        for shape in [
            StructuralTypeShape::Record {
                fields: vec![field(1, StructuralFieldType::Structural(binding.id))],
            },
            StructuralTypeShape::FixedArray {
                element: binding.id,
                length: 1,
            },
        ] {
            let container = declaration(2, shape);
            assert!(
                zeroed_structural_leaves([&binding, &container].into_iter(), container.id)
                    .is_none()
            );
        }
        let ordinary = declaration(3, StructuralTypeShape::Record { fields: Vec::new() });
        assert_eq!(
            zeroed_structural_leaves([&ordinary].into_iter(), ordinary.id),
            Some(Vec::new())
        );
    }

    #[test]
    fn bounded_integer_fields_require_zero_in_the_declared_interval() {
        for (sign, minimum, maximum, accepted) in [
            (
                IntegerSign::Signed,
                IntegerValue::Signed(-3),
                IntegerValue::Signed(0),
                true,
            ),
            (
                IntegerSign::Signed,
                IntegerValue::Signed(0),
                IntegerValue::Signed(0),
                true,
            ),
            (
                IntegerSign::Signed,
                IntegerValue::Signed(-3),
                IntegerValue::Signed(-1),
                false,
            ),
            (
                IntegerSign::Signed,
                IntegerValue::Signed(1),
                IntegerValue::Signed(3),
                false,
            ),
            (
                IntegerSign::Unsigned,
                IntegerValue::Unsigned(0),
                IntegerValue::Unsigned(u64::MAX as u128),
                true,
            ),
            (
                IntegerSign::Unsigned,
                IntegerValue::Unsigned(1),
                IntegerValue::Unsigned(u64::MAX as u128),
                false,
            ),
        ] {
            let integer = IntegerType::new(sign, 64).unwrap();
            let bounded = field(
                3,
                StructuralFieldType::BoundedInteger(
                    BoundedIntegerType::new(integer, minimum, maximum).unwrap(),
                ),
            );
            for shape in [
                StructuralTypeShape::Record {
                    fields: vec![bounded.clone()],
                },
                StructuralTypeShape::Sum {
                    cases: vec![case(7, vec![bounded.clone()])],
                },
            ] {
                let root = declaration(1, shape);
                let leaves = zeroed_structural_leaves([&root].into_iter(), root.id);
                assert_eq!(
                    leaves.is_some(),
                    accepted,
                    "{sign:?} {minimum:?}..={maximum:?}"
                );
                if let Some(leaves) = leaves {
                    assert!(leaves.contains(&ZeroedStructuralLeaf::RecordField {
                        path: vec![],
                        field: bounded.id,
                        scalar_type: ScalarType::Integer(integer),
                    }));
                }
            }
        }
    }

    #[test]
    fn sum_with_empty_first_case_mints_tag_leaf_only() {
        let sum = declaration(
            1,
            StructuralTypeShape::Sum {
                cases: vec![
                    case(7, vec![]),
                    case(
                        8,
                        vec![field(3, StructuralFieldType::Scalar(unsigned_32()))],
                    ),
                ],
            },
        );
        let leaves = zeroed_structural_leaves([&sum].into_iter(), sum.id).unwrap();
        assert_eq!(
            leaves,
            vec![ZeroedStructuralLeaf::SumSelection {
                path: vec![],
                structural_type: sum.id,
                case: StructuralCaseId::new(7).unwrap(),
                scalar_type: signed_tag(),
            }]
        );
    }

    #[test]
    fn sum_first_case_scalar_payloads_mint_record_field_leaves() {
        let sum = declaration(
            1,
            StructuralTypeShape::Sum {
                cases: vec![
                    case(
                        7,
                        vec![
                            field(3, StructuralFieldType::Scalar(unsigned_32())),
                            field(4, StructuralFieldType::Scalar(ScalarType::Boolean)),
                        ],
                    ),
                    // A later case's non-scalar payload does not reject: zeroed
                    // storage never selects it, so it contributes no cells.
                    case(
                        8,
                        vec![field(
                            5,
                            StructuralFieldType::Structural(StructuralTypeId::new(9).unwrap()),
                        )],
                    ),
                ],
            },
        );
        let leaves = zeroed_structural_leaves([&sum].into_iter(), sum.id).unwrap();
        assert_eq!(
            leaves,
            vec![
                ZeroedStructuralLeaf::SumSelection {
                    path: vec![],
                    structural_type: sum.id,
                    case: StructuralCaseId::new(7).unwrap(),
                    scalar_type: signed_tag(),
                },
                ZeroedStructuralLeaf::RecordField {
                    path: vec![],
                    field: StructuralFieldId::new(3).unwrap(),
                    scalar_type: unsigned_32(),
                },
                ZeroedStructuralLeaf::RecordField {
                    path: vec![],
                    field: StructuralFieldId::new(4).unwrap(),
                    scalar_type: ScalarType::Boolean,
                },
            ]
        );
    }

    #[test]
    fn sum_first_case_non_scalar_payload_rejects() {
        let field_type = StructuralFieldType::Structural(StructuralTypeId::new(9).unwrap());
        let sum = declaration(
            1,
            StructuralTypeShape::Sum {
                cases: vec![case(7, vec![field(3, field_type)])],
            },
        );
        assert!(zeroed_structural_leaves([&sum].into_iter(), sum.id).is_none());
    }

    #[test]
    fn ieee_fields_have_positive_zero_storage_without_erased_cells() {
        for format in [
            semantic_vocabulary::IeeeFloatFormat::Binary32,
            semantic_vocabulary::IeeeFloatFormat::Binary64,
        ] {
            for is_sum in [false, true] {
                for is_erased in [false, true] {
                    let mut payload = field(3, StructuralFieldType::IeeeFloat(format));
                    if is_erased {
                        erased(&mut payload);
                    }
                    let root = declaration(
                        1,
                        if is_sum {
                            StructuralTypeShape::Sum {
                                cases: vec![case(7, vec![payload])],
                            }
                        } else {
                            StructuralTypeShape::Record {
                                fields: vec![payload],
                            }
                        },
                    );
                    let leaves = zeroed_structural_leaves([&root].into_iter(), root.id);
                    if is_erased {
                        assert!(leaves.is_none(), "erasure is not a zeroed runtime cell");
                    } else {
                        assert!(leaves.unwrap().iter().any(|leaf| matches!(leaf,
                            ZeroedStructuralLeaf::RecordField { scalar_type: ScalarType::IeeeFloat(found), .. } if *found == format)));
                    }
                }
            }
        }
    }

    #[test]
    fn sum_first_case_erased_payload_rejects() {
        // Erased payload members surface with the `Erased` carrier; an erased
        // relevance on a plain scalar is refused too, either spelling has no
        // zeroed cell to write.
        let mut payload = field(3, StructuralFieldType::Scalar(unsigned_32()));
        erased(&mut payload);
        for payload in [
            payload,
            field(
                3,
                StructuralFieldType::Erased {
                    type_identity: "erased".to_string(),
                },
            ),
        ] {
            let sum = declaration(
                1,
                StructuralTypeShape::Sum {
                    cases: vec![case(7, vec![payload])],
                },
            );
            assert!(zeroed_structural_leaves([&sum].into_iter(), sum.id).is_none());
        }
    }

    #[test]
    fn sum_with_no_cases_rejects() {
        let sum = declaration(1, StructuralTypeShape::Sum { cases: vec![] });
        assert!(zeroed_structural_leaves([&sum].into_iter(), sum.id).is_none());
    }

    #[test]
    fn sum_member_inside_record_mints_paths_below_member() {
        let sum = declaration(
            2,
            StructuralTypeShape::Sum {
                cases: vec![case(
                    7,
                    vec![field(3, StructuralFieldType::Scalar(unsigned_32()))],
                )],
            },
        );
        let record = declaration(
            1,
            StructuralTypeShape::Record {
                fields: vec![field(
                    1,
                    StructuralFieldType::Structural(StructuralTypeId::new(2).unwrap()),
                )],
            },
        );
        let leaves = zeroed_structural_leaves([&record, &sum].into_iter(), record.id).unwrap();
        let member_path = vec![StructuralPathSegment::Field("f1".to_string())];
        assert_eq!(
            leaves,
            vec![
                ZeroedStructuralLeaf::SumSelection {
                    path: member_path.clone(),
                    structural_type: sum.id,
                    case: StructuralCaseId::new(7).unwrap(),
                    scalar_type: signed_tag(),
                },
                ZeroedStructuralLeaf::RecordField {
                    path: member_path,
                    field: StructuralFieldId::new(3).unwrap(),
                    scalar_type: unsigned_32(),
                },
            ]
        );
    }
}

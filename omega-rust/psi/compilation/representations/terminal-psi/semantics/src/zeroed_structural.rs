use semantic_vocabulary::{
    IntegerSign, IntegerType, ScalarType, StructuralCaseId, StructuralTypeId,
};
use terminal_psi::{
    StructuralFieldDeclaration, StructuralFieldType, StructuralPathSegment,
    StructuralTypeDeclaration, StructuralTypeShape,
};

/// Resource ceiling for expanded cells and pending work. Shape admission does
/// not use this ceiling: large valid types can need another materialization
/// strategy. Aggregate visits are bounded too, including empty element shapes.
const MAX_ZEROED_LEAVES: usize = 65_536;

/// One zero-initialized leaf cell of a zero-constructible structural type,
/// addressed the way runtime storage keys it: a record/common or selected case field
/// under its parent path, or a scalar array element at its full indexed path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ZeroedStructuralLeaf {
    RecordField {
        path: Vec<StructuralPathSegment>,
        field: semantic_vocabulary::StructuralFieldId,
        scalar_type: ScalarType,
    },
    /// A bounded byte field begins with an empty live range. Its scalar cell
    /// is the length metadata, not one of the unused capacity bytes.
    ByteSequenceField {
        path: Vec<StructuralPathSegment>,
        field: semantic_vocabulary::StructuralFieldId,
        scalar_type: ScalarType,
    },
    /// A selected case's payload cell, distinct from common fields even when
    /// both declarations use the same field identity.
    CaseField {
        path: Vec<StructuralPathSegment>,
        case: StructuralCaseId,
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
            | ZeroedStructuralLeaf::ByteSequenceField { path, .. }
            | ZeroedStructuralLeaf::CaseField { path, .. }
            | ZeroedStructuralLeaf::ArrayScalar { path, .. }
            | ZeroedStructuralLeaf::SumSelection { path, .. } => path,
        }
    }

    /// The leaf's scalar storage type, including a byte field's length cell.
    pub fn scalar_type(&self) -> ScalarType {
        match self {
            ZeroedStructuralLeaf::RecordField { scalar_type, .. }
            | ZeroedStructuralLeaf::ByteSequenceField { scalar_type, .. }
            | ZeroedStructuralLeaf::CaseField { scalar_type, .. }
            | ZeroedStructuralLeaf::ArrayScalar { scalar_type, .. }
            | ZeroedStructuralLeaf::SumSelection { scalar_type, .. } => *scalar_type,
        }
    }
}

/// Resolve whether a structural type's whole storage may begin zero: every
/// leaf a zero-valid scalar, sum case tag, or empty bounded byte field, and
/// every aggregate a record, fixed array, or admissible sum. A sum admits only when its first
/// declared case exists and every non-erased field of that case admits a zero
/// scalar: the tag's all-zero bits select that case (tag = case ordinal).
/// Bounded integer fields must contain the carrier's zero; nested composites
/// in sum payloads still need case-aware leaf paths. Bounded byte fields admit
/// only when all declared byte predicates hold for the empty live range.
/// References, borrowed views, whole byte carriers and erased members have no
/// admitted zero spelling through this structural operation, so they reject.
/// Mixed shapes admit common fields and their first case. The
/// declaration graph walk rejects unknown members and actual cycles without
/// expanding array elements or applying the leaf enumeration resource ceiling.
pub fn zeroed_structural_shape<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
) -> bool {
    let declarations: Vec<_> = types.collect();
    zeroed_declaration_graph(&declarations, root)
}

fn zeroed_declaration_graph(
    declarations: &[&StructuralTypeDeclaration],
    root: StructuralTypeId,
) -> bool {
    // Declaration-indexed DFS marks distinguish a backedge from a shared
    // child reached by several fields. Array extents do not grow this walk.
    let mut active = vec![false; declarations.len()];
    let mut complete = vec![false; declarations.len()];
    let mut pending = vec![(root, false)];
    while let Some((current, exiting)) = pending.pop() {
        let Some(position) = declarations
            .iter()
            .position(|declaration| declaration.id == current)
        else {
            return false;
        };
        if exiting {
            active[position] = false;
            complete[position] = true;
            continue;
        }
        if complete[position] {
            continue;
        }
        if active[position] {
            return false;
        }
        let shape = &declarations[position].shape;
        let (fields, cases) = match shape {
            StructuralTypeShape::PrimitiveScalar(_) => (&[][..], None),
            StructuralTypeShape::FixedArray { element, .. } => {
                active[position] = true;
                pending.push((current, true));
                pending.push((*element, false));
                continue;
            }
            StructuralTypeShape::Record { fields } => (fields.as_slice(), None),
            StructuralTypeShape::Sum { cases } => (&[][..], Some(cases)),
            StructuralTypeShape::Mixed { fields, cases } => (fields.as_slice(), Some(cases)),
            _ => return false,
        };
        if let Some(cases) = cases {
            let Some(first) = cases.first() else {
                return false;
            };
            // Nested payload aggregates still need case-aware structural paths.
            if first
                .fields
                .iter()
                .any(|field| zeroed_field_scalar(field).is_none())
            {
                return false;
            }
        }
        active[position] = true;
        pending.push((current, true));
        for field in fields {
            if field.relevance.is_erased() {
                return false;
            }
            if let StructuralFieldType::Structural(member) = field.field_type {
                pending.push((member, false));
            } else if zeroed_field_scalar(field).is_none() && !zeroed_byte_field(field) {
                return false;
            }
        }
    }
    true
}

fn zeroed_field_scalar(field: &StructuralFieldDeclaration) -> Option<ScalarType> {
    if field.relevance.is_erased() {
        return None;
    }
    crate::scalar_field_accepts_zero(&field.field_type)
        .then(|| field.field_type.scalar_type())
        .flatten()
}

fn zeroed_byte_field(field: &StructuralFieldDeclaration) -> bool {
    !field.relevance.is_erased()
        && matches!(
            field.field_type.byte_sequence_carrier(),
            Some(terminal_psi::ByteSequenceCarrier::BoundedOwned { .. })
        )
        && field.field_type.byte_predicates().holds_for(&[])
}

/// Enumerate every leaf a zeroed establishment of `root` must
/// materialize, or `None` when the shape is not zero-constructible or its
/// expansion exceeds the resource ceiling. Record fields key by their
/// parent path and field id; fixed-array elements key by the full indexed
/// path.
pub fn zeroed_structural_leaves<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
) -> Option<Vec<ZeroedStructuralLeaf>> {
    let declarations: Vec<&StructuralTypeDeclaration> = types.collect();
    if !zeroed_declaration_graph(&declarations, root) {
        return None;
    }
    let mut leaves = Vec::new();
    // (type id, path to its runtime place root)
    let mut pending = vec![(root, Vec::new())];
    let mut visited_cells = 0usize;
    while let Some((current, path)) = pending.pop() {
        let declaration = declarations
            .iter()
            .find(|declaration| declaration.id == current)?;
        if !matches!(declaration.shape, StructuralTypeShape::PrimitiveScalar(_)) {
            visited_cells += 1;
            if visited_cells > MAX_ZEROED_LEAVES {
                return None;
            }
        }
        if let StructuralTypeShape::Record { fields } | StructuralTypeShape::Mixed { fields, .. } =
            &declaration.shape
        {
            for field in fields {
                if let StructuralFieldType::Structural(member) = field.field_type {
                    let mut member_path = path.clone();
                    member_path.push(StructuralPathSegment::Field(field.identity.clone()));
                    pending.push((member, member_path));
                } else if zeroed_byte_field(field) {
                    leaves.push(ZeroedStructuralLeaf::ByteSequenceField {
                        path: path.clone(),
                        field: field.id,
                        scalar_type: ScalarType::Integer(
                            IntegerType::new(IntegerSign::Unsigned, 64).ok()?,
                        ),
                    });
                } else {
                    leaves.push(ZeroedStructuralLeaf::RecordField {
                        path: path.clone(),
                        field: field.id,
                        scalar_type: zeroed_field_scalar(field)?,
                    });
                }
                if leaves.len().saturating_add(pending.len()) > MAX_ZEROED_LEAVES {
                    return None;
                }
            }
        }
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
                let extent = usize::try_from(*length).ok()?;
                if leaves
                    .len()
                    .saturating_add(pending.len())
                    .saturating_add(extent)
                    > MAX_ZEROED_LEAVES
                {
                    return None;
                }
                for index in 0..extent {
                    let mut element_path = path.clone();
                    element_path.push(StructuralPathSegment::FixedIndex(index as u64));
                    pending.push((*element, element_path));
                }
            }
            StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. } => {
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
                    leaves.push(ZeroedStructuralLeaf::CaseField {
                        path: path.clone(),
                        case: first.id,
                        field: field.id,
                        scalar_type: zeroed_field_scalar(field)?,
                    });
                    if leaves.len().saturating_add(pending.len()) > MAX_ZEROED_LEAVES {
                        return None;
                    }
                }
            }
            StructuralTypeShape::Record { .. } => {}
            _ => return None,
        }
        if leaves.len() + pending.len() > MAX_ZEROED_LEAVES {
            return None;
        }
    }
    Some(leaves)
}

#[cfg(test)]
mod tests {
    use super::*;
    use semantic_vocabulary::{BoundedIntegerType, IntegerValue, StructuralFieldId};
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
    fn bounded_byte_fields_zero_only_the_empty_live_range() {
        use terminal_psi::{ByteSequenceCarrier, ByteSequencePredicate, ByteSequencePredicateSet};
        let mut predicates = ByteSequencePredicateSet::EMPTY;
        for predicate in [
            ByteSequencePredicate::ValidUtf8,
            ByteSequencePredicate::AsciiOnly,
            ByteSequencePredicate::NoNul,
        ] {
            predicates.insert(predicate);
        }
        for capacity in [0, 128] {
            for field_type in [
                StructuralFieldType::ByteSequence(ByteSequenceCarrier::BoundedOwned { capacity }),
                StructuralFieldType::PredicateByteSequence {
                    carrier: ByteSequenceCarrier::BoundedOwned { capacity },
                    predicates,
                },
            ] {
                let child = declaration(
                    2,
                    StructuralTypeShape::Record {
                        fields: vec![field(7, field_type)],
                    },
                );
                let root = declaration(
                    1,
                    StructuralTypeShape::Record {
                        fields: vec![field(3, StructuralFieldType::Structural(child.id))],
                    },
                );
                assert!(zeroed_structural_shape(
                    [&root, &child].into_iter(),
                    root.id
                ));
                let leaves =
                    zeroed_structural_leaves([&root, &child].into_iter(), root.id).unwrap();
                assert_eq!(
                    leaves,
                    vec![ZeroedStructuralLeaf::ByteSequenceField {
                        path: vec![StructuralPathSegment::Field("f3".into())],
                        field: StructuralFieldId::new(7).unwrap(),
                        scalar_type: ScalarType::Integer(
                            IntegerType::new(IntegerSign::Unsigned, 64).unwrap()
                        ),
                    }]
                );
                assert_eq!(
                    leaves[0].path(),
                    &[StructuralPathSegment::Field("f3".into())]
                );
                assert_eq!(
                    leaves[0].scalar_type(),
                    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap())
                );
            }
        }
    }

    #[test]
    fn nonempty_borrowed_and_case_payload_byte_fields_cannot_zero_construct() {
        use terminal_psi::{
            ByteSequenceCarrier, ByteSequencePredicate, ByteSequencePredicateSet, StructuralAccess,
        };
        let mut nonempty = ByteSequencePredicateSet::EMPTY;
        nonempty.insert(ByteSequencePredicate::NonEmpty);
        let mut rejected = vec![StructuralFieldType::PredicateByteSequence {
            carrier: ByteSequenceCarrier::BoundedOwned { capacity: 128 },
            predicates: nonempty,
        }];
        for access in [
            None,
            Some(StructuralAccess::SharedBorrow),
            Some(StructuralAccess::MutableBorrow),
            Some(StructuralAccess::WriteOnlyBorrow),
        ] {
            rejected.push(StructuralFieldType::ByteSequence(
                ByteSequenceCarrier::BorrowedView { access },
            ));
        }
        for field_type in rejected {
            let root = declaration(
                1,
                StructuralTypeShape::Record {
                    fields: vec![field(3, field_type)],
                },
            );
            assert!(!zeroed_structural_shape([&root].into_iter(), root.id));
            assert!(zeroed_structural_leaves([&root].into_iter(), root.id).is_none());
        }
        let payload = field(
            3,
            StructuralFieldType::ByteSequence(ByteSequenceCarrier::BoundedOwned { capacity: 8 }),
        );
        let sum = declaration(
            1,
            StructuralTypeShape::Sum {
                cases: vec![case(7, vec![payload.clone()])],
            },
        );
        assert!(!zeroed_structural_shape([&sum].into_iter(), sum.id));
        assert!(zeroed_structural_leaves([&sum].into_iter(), sum.id).is_none());
        let mut erased_payload = payload;
        erased(&mut erased_payload);
        let erased_root = declaration(
            1,
            StructuralTypeShape::Record {
                fields: vec![erased_payload],
            },
        );
        assert!(!zeroed_structural_shape(
            [&erased_root].into_iter(),
            erased_root.id
        ));
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
    fn huge_array_shape_admission_does_not_expand_its_elements() {
        let scalar = declaration(2, StructuralTypeShape::PrimitiveScalar(unsigned_32()));
        let array = declaration(
            1,
            StructuralTypeShape::FixedArray {
                element: scalar.id,
                length: u64::MAX,
            },
        );
        let types = [&array, &scalar];
        assert!(zeroed_structural_shape(types.into_iter(), array.id));
        assert!(zeroed_structural_leaves(types.into_iter(), array.id).is_none());
    }

    #[test]
    fn scalar_array_at_leaf_ceiling_still_materializes() {
        let scalar = declaration(2, StructuralTypeShape::PrimitiveScalar(unsigned_32()));
        let array = declaration(
            1,
            StructuralTypeShape::FixedArray {
                element: scalar.id,
                length: MAX_ZEROED_LEAVES as u64,
            },
        );
        let types = [&array, &scalar];
        assert!(zeroed_structural_shape(types.into_iter(), array.id));
        assert_eq!(
            zeroed_structural_leaves(types.into_iter(), array.id)
                .unwrap()
                .len(),
            MAX_ZEROED_LEAVES
        );
    }

    #[test]
    fn declaration_graph_rejects_cycles_and_unknown_members() {
        let root = StructuralTypeId::new(1).unwrap();
        let child = StructuralTypeId::new(2).unwrap();
        for shape in [
            StructuralTypeShape::FixedArray {
                element: root,
                length: 0,
            },
            StructuralTypeShape::Record {
                fields: vec![field(1, StructuralFieldType::Structural(root))],
            },
            StructuralTypeShape::Mixed {
                fields: vec![field(1, StructuralFieldType::Structural(root))],
                cases: vec![case(1, vec![])],
            },
            StructuralTypeShape::FixedArray {
                element: child,
                length: 0,
            },
        ] {
            let declaration = declaration(1, shape);
            assert!(!zeroed_structural_shape([&declaration].into_iter(), root));
            assert!(zeroed_structural_leaves([&declaration].into_iter(), root).is_none());
        }
        let first = declaration(
            1,
            StructuralTypeShape::FixedArray {
                element: child,
                length: 1,
            },
        );
        let second = declaration(
            2,
            StructuralTypeShape::Record {
                fields: vec![field(1, StructuralFieldType::Structural(root))],
            },
        );
        assert!(!zeroed_structural_shape(
            [&first, &second].into_iter(),
            root
        ));
        assert!(zeroed_structural_leaves([&first, &second].into_iter(), root).is_none());
    }

    #[test]
    fn shared_children_and_repeated_record_elements_are_not_cycles() {
        let child = declaration(
            2,
            StructuralTypeShape::Record {
                fields: vec![field(1, StructuralFieldType::Scalar(unsigned_32()))],
            },
        );
        let parent = declaration(
            1,
            StructuralTypeShape::Record {
                fields: vec![
                    field(1, StructuralFieldType::Structural(child.id)),
                    field(2, StructuralFieldType::Structural(child.id)),
                ],
            },
        );
        assert!(zeroed_structural_shape(
            [&parent, &child].into_iter(),
            parent.id
        ));
        assert_eq!(
            zeroed_structural_leaves([&parent, &child].into_iter(), parent.id)
                .unwrap()
                .len(),
            2
        );
        let array = declaration(
            3,
            StructuralTypeShape::FixedArray {
                element: parent.id,
                length: 2048,
            },
        );
        assert!(zeroed_structural_shape(
            [&array, &parent, &child].into_iter(),
            array.id
        ));
        assert_eq!(
            zeroed_structural_leaves([&array, &parent, &child].into_iter(), array.id)
                .unwrap()
                .len(),
            4096
        );
    }

    #[test]
    fn mixed_common_and_case_fields_with_identical_ids_remain_distinct() {
        let mixed = declaration(
            1,
            StructuralTypeShape::Mixed {
                fields: vec![field(3, StructuralFieldType::Scalar(unsigned_32()))],
                cases: vec![
                    case(
                        7,
                        vec![field(3, StructuralFieldType::Scalar(unsigned_32()))],
                    ),
                    case(
                        8,
                        vec![field(3, StructuralFieldType::Scalar(ScalarType::Boolean))],
                    ),
                ],
            },
        );
        assert!(zeroed_structural_shape([&mixed].into_iter(), mixed.id));
        let leaves = zeroed_structural_leaves([&mixed].into_iter(), mixed.id).unwrap();
        assert_eq!(
            leaves,
            vec![
                ZeroedStructuralLeaf::RecordField {
                    path: vec![],
                    field: StructuralFieldId::new(3).unwrap(),
                    scalar_type: unsigned_32()
                },
                ZeroedStructuralLeaf::SumSelection {
                    path: vec![],
                    structural_type: mixed.id,
                    case: StructuralCaseId::new(7).unwrap(),
                    scalar_type: signed_tag()
                },
                ZeroedStructuralLeaf::CaseField {
                    path: vec![],
                    case: StructuralCaseId::new(7).unwrap(),
                    field: StructuralFieldId::new(3).unwrap(),
                    scalar_type: unsigned_32()
                },
            ]
        );
        assert_ne!(leaves[0], leaves[2]);
        assert_eq!(leaves[2].path(), &[]);
        assert_eq!(leaves[2].scalar_type(), unsigned_32());
    }

    #[test]
    fn mixed_common_members_recurse_but_nested_case_members_still_reject() {
        let child = declaration(2, StructuralTypeShape::PrimitiveScalar(unsigned_32()));
        let mut mixed = declaration(
            1,
            StructuralTypeShape::Mixed {
                fields: vec![field(3, StructuralFieldType::Structural(child.id))],
                cases: vec![case(7, vec![])],
            },
        );
        assert!(zeroed_structural_shape(
            [&mixed, &child].into_iter(),
            mixed.id
        ));
        let leaves = zeroed_structural_leaves([&mixed, &child].into_iter(), mixed.id).unwrap();
        assert!(leaves.contains(&ZeroedStructuralLeaf::ArrayScalar {
            path: vec![StructuralPathSegment::Field("f3".into())],
            scalar_type: unsigned_32(),
        }));
        let StructuralTypeShape::Mixed { cases, .. } = &mut mixed.shape else {
            panic!("mixed");
        };
        cases[0]
            .fields
            .push(field(4, StructuralFieldType::Structural(child.id)));
        assert!(!zeroed_structural_shape(
            [&mixed, &child].into_iter(),
            mixed.id
        ));
        assert!(zeroed_structural_leaves([&mixed, &child].into_iter(), mixed.id).is_none());
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
                    let expected = if matches!(root.shape, StructuralTypeShape::Sum { .. }) {
                        ZeroedStructuralLeaf::CaseField {
                            path: vec![],
                            case: StructuralCaseId::new(7).unwrap(),
                            field: bounded.id,
                            scalar_type: ScalarType::Integer(integer),
                        }
                    } else {
                        ZeroedStructuralLeaf::RecordField {
                            path: vec![],
                            field: bounded.id,
                            scalar_type: ScalarType::Integer(integer),
                        }
                    };
                    assert!(leaves.contains(&expected));
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
    fn sum_first_case_scalar_payloads_mint_case_field_leaves() {
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
                ZeroedStructuralLeaf::CaseField {
                    path: vec![],
                    case: StructuralCaseId::new(7).unwrap(),
                    field: StructuralFieldId::new(3).unwrap(),
                    scalar_type: unsigned_32(),
                },
                ZeroedStructuralLeaf::CaseField {
                    path: vec![],
                    case: StructuralCaseId::new(7).unwrap(),
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
                        assert!(
                            leaves
                                .unwrap()
                                .iter()
                                .any(|leaf| leaf.scalar_type() == ScalarType::IeeeFloat(format))
                        );
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
                ZeroedStructuralLeaf::CaseField {
                    path: member_path,
                    case: StructuralCaseId::new(7).unwrap(),
                    field: StructuralFieldId::new(3).unwrap(),
                    scalar_type: unsigned_32(),
                },
            ]
        );
    }
}

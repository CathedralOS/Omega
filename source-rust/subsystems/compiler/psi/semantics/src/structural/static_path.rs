use crate::exactly_one::exactly_one;
use semantic_vocabulary::{
    CanonicalStructuralPathSegment, IntegerSign, IntegerValue, Proposition, ScalarTerm, ScalarType,
    StructuralTypeId, ValueId,
};
use terminal_psi::{
    OperationKind, StructuralFieldType, StructuralPathSegment, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalMachine,
};

pub fn constant_runtime_index(machine: &TerminalMachine, index: ValueId) -> Option<u64> {
    let producer = exactly_one(machine.operations(), |operation| {
        operation
            .result
            .scalar_ref()
            .is_some_and(|result| result.id == index)
    })?;
    let OperationKind::IntegerConstant { value } = producer.kind else {
        return None;
    };
    match value {
        IntegerValue::Unsigned(value) => u64::try_from(value).ok(),
        IntegerValue::Signed(value) => u64::try_from(value).ok(),
    }
}

pub(crate) fn runtime_index_bound(
    index: ValueId,
    index_type: ScalarType,
    extent: u64,
) -> Option<Proposition> {
    let integer_type = index_type.as_integer()?;
    if integer_type.is_address() {
        return None;
    }
    let subject = ScalarTerm::value(index, index_type);
    let mut conjuncts = Vec::with_capacity(2);
    if integer_type.sign() == IntegerSign::Signed {
        conjuncts.push(Proposition::LessOrEqual(
            ScalarTerm::integer(integer_type, IntegerValue::Signed(0)).ok()?,
            subject.clone(),
        ));
    }
    let extent_value = match integer_type.sign() {
        IntegerSign::Signed => IntegerValue::Signed(i128::from(extent)),
        IntegerSign::Unsigned => IntegerValue::Unsigned(u128::from(extent)),
    };
    if integer_type.admits(extent_value) {
        conjuncts.push(Proposition::LessThan(
            subject,
            ScalarTerm::integer(integer_type, extent_value).ok()?,
        ));
    }
    Some(match conjuncts.len() {
        0 => Proposition::Truth,
        1 => conjuncts.pop().expect("one conjunct"),
        _ => Proposition::Conjunction(conjuncts),
    })
}

pub fn runtime_index_selects(value: IntegerValue, extent: u64) -> Option<u64> {
    let selected = match value {
        IntegerValue::Unsigned(value) => u64::try_from(value).ok()?,
        IntegerValue::Signed(value) => u64::try_from(value).ok()?,
    };
    (selected < extent).then_some(selected)
}

pub fn structural_paths_select_same_location(
    left: &[StructuralPathSegment],
    right: &[StructuralPathSegment],
) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(left, right)| match (left, right) {
                (
                    StructuralPathSegment::RuntimeIndex { index: left, .. },
                    StructuralPathSegment::RuntimeIndex { index: right, .. },
                ) => left == right,
                _ => left == right,
            })
}

pub fn structural_paths_may_overlap(
    left: &[StructuralPathSegment],
    right: &[StructuralPathSegment],
) -> bool {
    for (left, right) in left.iter().zip(right) {
        if matches!(left, StructuralPathSegment::RepresentationView { .. })
            || matches!(right, StructuralPathSegment::RepresentationView { .. })
        {
            return true;
        }
        if let (StructuralPathSegment::Case(left), StructuralPathSegment::Case(right)) =
            (left, right)
            && left != right
        {
            return true;
        }
        let overlaps = match (left, right) {
            (
                StructuralPathSegment::AllElements,
                StructuralPathSegment::FixedIndex(_)
                | StructuralPathSegment::RuntimeIndex { .. }
                | StructuralPathSegment::FixedRange { .. },
            )
            | (
                StructuralPathSegment::FixedIndex(_)
                | StructuralPathSegment::RuntimeIndex { .. }
                | StructuralPathSegment::FixedRange { .. },
                StructuralPathSegment::AllElements,
            ) => true,
            (
                StructuralPathSegment::FixedRange {
                    start: left_start,
                    end: left_end,
                },
                StructuralPathSegment::FixedRange {
                    start: right_start,
                    end: right_end,
                },
            ) => {
                left_start >= left_end
                    || right_start >= right_end
                    || (left_start < right_end && right_start < left_end)
            }
            (
                StructuralPathSegment::FixedRange { start, end },
                StructuralPathSegment::FixedIndex(index),
            )
            | (
                StructuralPathSegment::FixedIndex(index),
                StructuralPathSegment::FixedRange { start, end },
            ) => start >= end || (start <= index && index < end),
            (StructuralPathSegment::RuntimeIndex { .. }, _)
            | (_, StructuralPathSegment::RuntimeIndex { .. }) => true,
            _ => left == right,
        };
        if !overlaps {
            return false;
        }
    }
    true
}
pub fn canonical_structural_path_tip<'a>(
    declarations: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[CanonicalStructuralPathSegment],
) -> Option<&'a StructuralTypeDeclaration> {
    let selected = canonical_projection_type(declarations.clone(), root, path, None)?;
    let mut matching = declarations.filter(|declaration| declaration.id == selected);
    let declaration = matching.next()?;
    matching.next().is_none().then_some(declaration)
}

pub fn canonical_case_leaf_type<'a>(
    types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[CanonicalStructuralPathSegment],
) -> Option<StructuralTypeId> {
    path.iter()
        .any(|segment| matches!(segment, CanonicalStructuralPathSegment::Case(_)))
        .then_some(())?;
    canonical_projection_type(types, root, path, None)
}

pub fn canonical_structural_storage_path<'a>(
    types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[CanonicalStructuralPathSegment],
) -> Option<Vec<StructuralPathSegment>> {
    let mut storage = Vec::with_capacity(path.len());
    canonical_projection_type(types, root, path, Some(&mut storage))?;
    Some(storage)
}

pub(crate) fn canonical_storage_dependencies<'a>(
    types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[CanonicalStructuralPathSegment],
    dependencies: &mut std::collections::BTreeSet<StructuralTypeId>,
    include_root: bool,
) -> Option<()> {
    let declaration = |identity| exactly_one(types.clone(), |row| row.id == identity);
    canonical_projection_type_with_lookup(
        types.clone(),
        declaration,
        root,
        path,
        None,
        Some((include_root, dependencies)),
    )?;
    Some(())
}

/// Visit the exact owned carrier at each inline prefix until the path
/// crosses a referent, an inactive case, or a non-inline element.
pub fn visit_canonical_inline_carriers<'a>(
    types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    declaration: impl Fn(StructuralTypeId) -> Option<&'a StructuralTypeDeclaration>,
    mut carrier: &'a StructuralTypeDeclaration,
    path: &[CanonicalStructuralPathSegment],
    mut visit: impl FnMut(usize, &'a StructuralTypeDeclaration),
) -> Option<()> {
    for (index, step) in path.iter().enumerate() {
        match step {
            CanonicalStructuralPathSegment::Field(_) => {}
            CanonicalStructuralPathSegment::FixedIndex(_)
                if matches!(carrier.shape, StructuralTypeShape::FixedArray { .. }) => {}
            _ => break,
        }
        let selected = canonical_projection_type_with_lookup(
            types.clone(),
            &declaration,
            carrier.id,
            std::slice::from_ref(step),
            None,
            None,
        )?;
        carrier = declaration(selected)?;
        visit(index + 1, carrier);
    }
    Some(())
}

fn canonical_projection_type<'a>(
    types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    path: &[CanonicalStructuralPathSegment],
    storage: Option<&mut Vec<StructuralPathSegment>>,
) -> Option<StructuralTypeId> {
    let declaration = |identity| exactly_one(types.clone(), |row| row.id == identity);
    canonical_projection_type_with_lookup(types.clone(), declaration, root, path, storage, None)
}

fn canonical_projection_type_with_lookup<'a>(
    types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    declaration: impl Fn(StructuralTypeId) -> Option<&'a StructuralTypeDeclaration>,
    mut root: StructuralTypeId,
    path: &[CanonicalStructuralPathSegment],
    mut storage: Option<&mut Vec<StructuralPathSegment>>,
    mut dependencies: Option<(bool, &mut std::collections::BTreeSet<StructuralTypeId>)>,
) -> Option<StructuralTypeId> {
    let mut case_fields: Option<&[terminal_psi::StructuralFieldDeclaration]> = None;
    for (ordinal, segment) in path.iter().enumerate() {
        if let Some((include_root, dependencies)) = dependencies.as_mut()
            && (*include_root || ordinal > 0)
        {
            dependencies.insert(root);
        }
        let shape = &declaration(root)?.shape;
        match segment {
            CanonicalStructuralPathSegment::Referent => {
                if case_fields.is_some() {
                    return None;
                }
                let StructuralTypeShape::Reference { referent, .. } = shape else {
                    return None;
                };
                if let Some(storage) = storage.as_mut() {
                    storage.push(StructuralPathSegment::Referent);
                }
                root = *referent;
            }
            CanonicalStructuralPathSegment::Case(identity) => {
                if case_fields.is_some() {
                    return None;
                }
                let (StructuralTypeShape::Sum { cases }
                | StructuralTypeShape::Mixed { cases, .. }) = shape
                else {
                    return None;
                };
                case_fields =
                    Some(&exactly_one(cases.iter(), |case| case.id == *identity)?.fields);
                if let Some(storage) = storage.as_mut() {
                    storage.push(StructuralPathSegment::Case(*identity));
                }
            }
            CanonicalStructuralPathSegment::Field(identity) => {
                let fields = match case_fields.take() {
                    Some(fields) => fields,
                    None => match shape {
                        terminal_psi::record_or_mixed_shape_pattern!(fields) => fields,
                        _ => return None,
                    },
                };
                let field = exactly_one(fields.iter(), |field| field.id == *identity)?;
                if field.relevance.is_erased() {
                    return None;
                }
                let child = match &field.field_type {
                    StructuralFieldType::Structural(child) => *child,
                    leaf if leaf.byte_sequence_carrier().is_some() => {
                        // The carrier row on this field identifies the stored descriptor.
                        crate::inline_carrier_declaration(types.clone(), field)?.id
                    }
                    leaf => {
                        leaf.canonical_leaf_type()?
                            .leaf_declaration(types.clone())?
                            .id
                    }
                };
                if let Some(storage) = storage.as_mut() {
                    storage.push(StructuralPathSegment::Field(field.identity));
                }
                root = child;
            }
            CanonicalStructuralPathSegment::FixedIndex(index) => {
                if case_fields.is_some() {
                    return None;
                }
                root = match shape {
                    StructuralTypeShape::FixedArray { element, length }
                        if length
                            .literal()
                            .map_or(dependencies.is_some(), |length| *index < length) =>
                    {
                        *element
                    }
                    StructuralTypeShape::ElementView { element, .. } => *element,
                    _ => return None,
                };
                if let Some(storage) = storage.as_mut() {
                    storage.push(StructuralPathSegment::FixedIndex(*index));
                }
            }
        }
    }
    let selected =
        (case_fields.is_none() || dependencies.is_some()).then_some(declaration(root)?.id)?;
    if let Some((include_root, dependencies)) = dependencies
        && (include_root || !path.is_empty())
    {
        dependencies.insert(selected);
    }
    Some(selected)
}

pub fn structural_memberships_disjoint_from_write(
    whole: &[semantic_vocabulary::StructuralDomainId],
    projected: &[terminal_psi::StructuralPathQualification],
    written: &[StructuralPathSegment],
) -> bool {
    whole.is_empty()
        && projected
            .iter()
            .all(|row| !structural_paths_may_overlap(&row.path, written))
}

pub fn qualification_path_carrier<'types>(
    types: impl crate::StructuralDeclarations<'types>,
    mut root: StructuralTypeId,
    path: &[StructuralPathSegment],
) -> Option<StructuralTypeId> {
    let mut cursor = path;
    loop {
        let count = cursor
            .iter()
            .position(|step| matches!(step, StructuralPathSegment::AllElements))
            .unwrap_or(cursor.len());
        root = crate::runtime_structural_path_tip(types.clone(), root, &cursor[..count])?.id;
        if count == cursor.len() {
            return Some(root);
        }
        let declaration = types.unique(root)?;
        root = match declaration.shape {
            StructuralTypeShape::FixedArray { element, .. }
            | StructuralTypeShape::ElementView { element, .. } => element,
            _ => return None,
        };
        cursor = &cursor[count + 1..];
    }
}

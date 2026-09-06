//! Independently reconstruct the maximal live complement of finite structural moves.

use std::collections::{BTreeMap, BTreeSet};

use semantic_vocabulary::StructuralTypeId;
use terminal_psi::{
    ByteSequenceCarrier, StructuralAffineDiscard, StructuralFieldType, StructuralPathSegment,
    StructuralTypeDeclaration, StructuralTypeShape,
};

type Moves<'a> = &'a [(&'a [StructuralPathSegment], StructuralTypeId)];
type Declarations<'a> = BTreeMap<StructuralTypeId, &'a StructuralTypeDeclaration>;

pub(crate) fn canonical_finite_declarations(
    declarations: &[StructuralTypeDeclaration],
    root: StructuralTypeId,
) -> Option<Declarations<'_>> {
    let mut identities = BTreeSet::new();
    if declarations.is_empty()
        || declarations.windows(2).any(|pair| pair[0].id >= pair[1].id)
        || declarations.iter().any(|declaration| {
            declaration.identity.is_empty() || !identities.insert(declaration.identity.as_str())
        })
    {
        return None;
    }
    let by_id = declarations
        .iter()
        .map(|declaration| (declaration.id, declaration))
        .collect();
    finite_type(root, &by_id, &mut BTreeSet::new(), &mut BTreeSet::new())?;
    Some(by_id)
}

fn finite_type(
    structural_type: StructuralTypeId,
    declarations: &Declarations<'_>,
    active: &mut BTreeSet<StructuralTypeId>,
    complete: &mut BTreeSet<StructuralTypeId>,
) -> Option<()> {
    if complete.contains(&structural_type) {
        return Some(());
    }
    if !active.insert(structural_type) {
        return None;
    }
    match &declarations.get(&structural_type)?.shape {
        StructuralTypeShape::Record { fields } => {
            let mut identities = BTreeSet::new();
            let mut identifiers = BTreeSet::new();
            for field in fields {
                if field.identity.is_empty()
                    || field.relevance.is_erased()
                    || !identities.insert(field.identity.as_str())
                    || !identifiers.insert(field.id)
                {
                    return None;
                }
                match field.field_type {
                    StructuralFieldType::Structural(nested) => {
                        finite_type(nested, declarations, active, complete)?;
                    }
                    StructuralFieldType::Scalar(_)
                    | StructuralFieldType::IeeeFloat(_)
                    | StructuralFieldType::ByteSequence(ByteSequenceCarrier::BoundedOwned {
                        ..
                    }) => {}
                    _ => return None,
                }
            }
        }
        StructuralTypeShape::FixedArray { element, length } if *length != 0 => {
            finite_type(*element, declarations, active, complete)?;
        }
        _ => return None,
    }
    active.remove(&structural_type);
    complete.insert(structural_type);
    Some(())
}

pub(crate) fn exact_partial_cleanup_partition(
    declarations: &[StructuralTypeDeclaration],
    root_type: StructuralTypeId,
    moved: Moves<'_>,
    residuals: &[&StructuralAffineDiscard],
) -> bool {
    let Some(declarations) = canonical_finite_declarations(declarations, root_type) else {
        return false;
    };
    if moved.is_empty()
        || moved.iter().enumerate().any(|(index, (path, _))| {
            path.is_empty()
                || moved[..index]
                    .iter()
                    .any(|(earlier, _)| path.starts_with(earlier) || earlier.starts_with(path))
        })
    {
        return false;
    }
    let mut remaining = residuals;
    consume_residuals(
        root_type,
        &mut Vec::new(),
        moved,
        &declarations,
        &mut remaining,
    )
    .is_some()
        && remaining.is_empty()
}

fn consume_residuals(
    structural_type: StructuralTypeId,
    prefix: &mut Vec<StructuralPathSegment>,
    moved: Moves<'_>,
    declarations: &Declarations<'_>,
    remaining: &mut &[&StructuralAffineDiscard],
) -> Option<()> {
    if moved.is_empty() {
        let (actual, rest) = remaining.split_first()?;
        if actual.path != *prefix || actual.structural_type != structural_type {
            return None;
        }
        *remaining = rest;
        return Some(());
    }
    if moved.iter().any(|(path, _)| path.is_empty()) {
        return (moved.len() == 1 && moved[0].1 == structural_type).then_some(());
    }
    match &declarations.get(&structural_type)?.shape {
        StructuralTypeShape::Record { fields } => {
            let mut matched = 0;
            for field in fields.iter().rev() {
                let segment = StructuralPathSegment::Field(field.identity.clone());
                let nested = moved
                    .iter()
                    .filter(|(path, _)| path.first() == Some(&segment))
                    .map(|(path, leaf)| (&path[1..], *leaf))
                    .collect::<Vec<_>>();
                matched += nested.len();
                if let StructuralFieldType::Structural(field_type) = field.field_type {
                    prefix.push(segment);
                    consume_residuals(field_type, prefix, &nested, declarations, remaining)?;
                    prefix.pop();
                } else if !nested.is_empty() {
                    return None;
                }
            }
            (matched == moved.len()).then_some(())
        }
        StructuralTypeShape::FixedArray { element, length } => {
            let mut touched = BTreeSet::new();
            for (path, _) in moved {
                let Some(StructuralPathSegment::FixedIndex(index)) = path.first() else {
                    return None;
                };
                if *index >= *length {
                    return None;
                }
                touched.insert(*index);
            }
            // Each untouched child requires one maximal residual. Bound the
            // loop by supplied evidence before enumerating any array children.
            let untouched = length.checked_sub(u64::try_from(touched.len()).ok()?)?;
            if untouched > u64::try_from(remaining.len()).ok()? {
                return None;
            }
            for index in (0..*length).rev() {
                let segment = StructuralPathSegment::FixedIndex(index);
                let nested = moved
                    .iter()
                    .filter(|(path, _)| path.first() == Some(&segment))
                    .map(|(path, leaf)| (&path[1..], *leaf))
                    .collect::<Vec<_>>();
                prefix.push(segment);
                consume_residuals(*element, prefix, &nested, declarations, remaining)?;
                prefix.pop();
            }
            Some(())
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests;

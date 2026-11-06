//! Complete values retain the claims their declarations own. A containing
//! nominal claim transports its children; arrays add no claim of their own.

use super::child_types;
use crate::exactly_one::exactly_one;
use semantic_vocabulary::{IdSet, StructuralCaseId, StructuralTypeId};
use std::collections::BTreeSet;
use terminal_psi::{StructuralPathSegment as Path, StructuralTypeDeclaration, StructuralTypeShape};

/// Compare actual live claim paths with the complete selected value's typed
/// frontier. This grants no claim or initialization: each supplied path must
/// already belong to the current occurrence. A selected region retains its
/// containing nominal claim or its own declared descendant claims; unrelated
/// siblings keep their custody. Callers authenticate the region and access.
/// Borrowed referents are not owned children, and a case is selected only by
/// independent stored membership.
pub fn validate_complete_claim_frontier<'a>(
    declaration: impl Fn(StructuralTypeId) -> Option<&'a StructuralTypeDeclaration>,
    root: StructuralTypeId,
    region: &[Path],
    claims: impl IntoIterator<Item = &'a [Path]>,
    selected: &dyn Fn(&[Path], StructuralCaseId) -> bool,
) -> Result<(), &'static str> {
    let mut held = BTreeSet::new();
    for path in claims
        .into_iter()
        .filter(|path| path.starts_with(region) || region.starts_with(path))
    {
        if !held.insert(path) {
            return Err("complete value repeats a live claim path");
        }
    }
    let mut possible = ClaimTypes::default();
    let consumed = visit(
        &declaration,
        root,
        &mut Vec::new(),
        region,
        &held,
        selected,
        &mut possible,
    )?;
    if consumed != held.len() {
        return Err("complete value has a claim outside its declared frontier");
    }
    Ok(())
}

#[derive(Default)]
pub(super) struct ClaimTypes {
    known: IdSet<StructuralTypeId>,
    with_claims: IdSet<StructuralTypeId>,
    active: IdSet<StructuralTypeId>,
}

pub(super) fn contains_claim<'a>(
    declaration: &impl Fn(StructuralTypeId) -> Option<&'a StructuralTypeDeclaration>,
    current: StructuralTypeId,
    possible: &mut ClaimTypes,
) -> Result<bool, &'static str> {
    let row = declaration(current).ok_or("complete value type is absent")?;
    if row.owns_linear_claim {
        return Ok(true);
    }
    if possible.known.contains(current) {
        return Ok(possible.with_claims.contains(current));
    }
    if !possible.active.insert(current) {
        return Err("complete value has a cyclic by-value type");
    }
    let mut found = false;
    for child in child_types(row) {
        found |= contains_claim(declaration, child, possible)?;
    }
    possible.active.remove(current);
    possible.known.insert(current);
    if found {
        possible.with_claims.insert(current);
    }
    Ok(found)
}

fn visit<'a>(
    declaration: &impl Fn(StructuralTypeId) -> Option<&'a StructuralTypeDeclaration>,
    current: StructuralTypeId,
    path: &mut Vec<Path>,
    region: &[Path],
    held: &BTreeSet<&[Path]>,
    selected: &dyn Fn(&[Path], StructuralCaseId) -> bool,
    possible: &mut ClaimTypes,
) -> Result<usize, &'static str> {
    let row = declaration(current).ok_or("complete value type is absent")?;
    if row.owns_linear_claim {
        return held
            .contains(path.as_slice())
            .then_some(1)
            .ok_or("complete value is missing its declaration-owned claim");
    }
    if !contains_claim(declaration, current, possible)? {
        return Ok(0);
    }
    if let Some((step, tail)) = region.split_first() {
        let fields = match (&row.shape, step) {
            (StructuralTypeShape::Record { fields, .. }, Path::Field(_))
            | (StructuralTypeShape::Mixed { fields, .. }, Path::Field(_)) => fields,
            (
                StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. },
                Path::Case(case),
            ) => {
                let active = exactly_one(cases.iter(), |case| selected(path, case.id))
                    .ok_or("complete value region has no unique established case")?;
                if active.id != *case {
                    return Err("complete value region names an inactive case");
                }
                let Some((Path::Field(field), rest)) = tail.split_first() else {
                    return Err("complete value region ends in a case namespace");
                };
                let member = exactly_one(active.fields.iter(), |member| {
                    !member.relevance.is_erased() && member.identity == *field
                })
                .ok_or("complete value region has no exact payload field")?;
                path.extend([step.clone(), Path::Field(*field)]);
                let count = match member.field_type.as_structural() {
                    Some(child) => visit(declaration, child, path, rest, held, selected, possible),
                    None if rest.is_empty() => Ok(0),
                    None => Err("complete value region projects through a scalar"),
                }?;
                path.truncate(path.len() - 2);
                return Ok(count);
            }
            (StructuralTypeShape::FixedArray { element, length }, Path::FixedIndex(index)) => {
                if length.literal().is_none_or(|length| *index >= length) {
                    return Err("complete value region has no exact array element");
                }
                path.push(step.clone());
                let count = visit(declaration, *element, path, tail, held, selected, possible)?;
                path.pop();
                return Ok(count);
            }
            _ => return Err("complete value region has no owned storage path"),
        };
        let Path::Field(field) = step else {
            unreachable!("field branch selected above")
        };
        let member = exactly_one(fields.iter(), |member| {
            !member.relevance.is_erased() && member.identity == *field
        })
        .ok_or("complete value region has no exact field")?;
        path.push(step.clone());
        let count = match member.field_type.as_structural() {
            Some(child) => visit(declaration, child, path, tail, held, selected, possible),
            None if tail.is_empty() => Ok(0),
            None => Err("complete value region projects through a scalar"),
        }?;
        path.pop();
        return Ok(count);
    }
    if let StructuralTypeShape::FixedArray { element, length } = &row.shape {
        let length = length
            .literal()
            .ok_or("complete value requires a closed array extent")?;
        // Stream required occurrences. The array itself introduces no claim,
        // and obligation-free elements were excluded before this loop.
        let mut count = 0;
        path.push(Path::FixedIndex(0));
        for index in 0..length {
            *path.last_mut().expect("array path has its current index") = Path::FixedIndex(index);
            count += visit(declaration, *element, path, &[], held, selected, possible)?;
        }
        path.pop();
        return Ok(count);
    }
    let mut fields = |fields: &[terminal_psi::StructuralFieldDeclaration], path: &mut Vec<Path>| {
        let mut count = 0;
        for field in fields.iter().filter(|field| !field.relevance.is_erased()) {
            if let Some(child) = field.field_type.as_structural() {
                path.push(Path::Field(field.identity));
                count += visit(declaration, child, path, &[], held, selected, possible)?;
                path.pop();
            }
        }
        Ok::<_, &'static str>(count)
    };
    match &row.shape {
        StructuralTypeShape::Record { fields: common, .. } => fields(common, path),
        StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. } => {
            let case = exactly_one(cases.iter(), |case| selected(path, case.id))
                .ok_or("complete value has no unique established case")?;
            path.push(Path::Case(case.id));
            let mut count = fields(&case.fields, path)?;
            path.pop();
            if let StructuralTypeShape::Mixed { fields: common, .. } = &row.shape {
                count += fields(common, path)?;
            }
            Ok(count)
        }
        _ => Err("complete value claim frontier has no owned child geometry"),
    }
}

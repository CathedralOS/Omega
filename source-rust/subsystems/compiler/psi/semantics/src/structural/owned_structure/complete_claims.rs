use super::child_types;
use crate::exactly_one::exactly_one;
use semantic_vocabulary::{StructuralCaseId, StructuralTypeId};
use std::collections::BTreeSet;
use terminal_psi::{
    StructuralPathSegment as Path, StructuralTypeDeclaration, StructuralTypeShape,
};

pub fn validate_complete_claim_frontier<'a>(
    possible: &ClaimTypes<'_>,
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
    let consumed = visit(possible, root, &mut Vec::new(), region, &held, selected)?;
    if consumed != held.len() {
        return Err("complete value has a claim outside its declared frontier");
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum ClaimState {
    Unknown,
    Active,
    Absent,
    Present,
}

/// Claim-search scratch belongs to the declared type roster, not to
/// the numeric range of its identities. Reuse it for queries over the
/// same immutable declarations.
pub struct ClaimTypes<'types> {
    declarations: Vec<&'types StructuralTypeDeclaration>,
    states: Vec<ClaimState>,
}

impl<'types> ClaimTypes<'types> {
    pub fn new(
        declarations: impl IntoIterator<Item = &'types StructuralTypeDeclaration>,
    ) -> Result<Self, &'static str> {
        let mut declarations = declarations.into_iter().collect::<Vec<_>>();
        declarations.sort_unstable_by_key(|row| row.id);
        if declarations.windows(2).any(|rows| rows[0].id == rows[1].id) {
            return Err("claim type roster repeats a declaration identity");
        }
        let mut prepared = Self {
            states: vec![ClaimState::Unknown; declarations.len()],
            declarations,
        };
        for slot in 0..prepared.declarations.len() {
            let identity = prepared.declarations[slot].id;
            compute_claim(identity, &mut prepared)?;
        }
        Ok(prepared)
    }

    pub(super) fn declaration(
        &self,
        identity: StructuralTypeId,
    ) -> Option<&'types StructuralTypeDeclaration> {
        self.declarations
            .binary_search_by_key(&identity, |row| row.id)
            .ok()
            .map(|slot| self.declarations[slot])
    }
}

fn compute_claim(
    current: StructuralTypeId,
    possible: &mut ClaimTypes<'_>,
) -> Result<bool, &'static str> {
    let slot = possible
        .declarations
        .binary_search_by_key(&current, |row| row.id)
        .map_err(|_| "complete value type is absent")?;
    let row = possible.declarations[slot];
    if row.owns_linear_claim || super::unknown_owned_claims(row) {
        return Ok(true);
    }
    match possible.states[slot] {
        ClaimState::Present => return Ok(true),
        ClaimState::Absent => return Ok(false),
        ClaimState::Active => return Err("complete value has a cyclic by-value type"),
        ClaimState::Unknown => {}
    }
    possible.states[slot] = ClaimState::Active;
    let mut found = false;
    for child in child_types(row) {
        match compute_claim(child, possible) {
            Ok(child_found) => found |= child_found,
            Err(error) => {
                possible.states[slot] = ClaimState::Unknown;
                return Err(error);
            }
        }
    }
    possible.states[slot] = if found {
        ClaimState::Present
    } else {
        ClaimState::Absent
    };
    Ok(found)
}

pub(crate) fn contains_claim(
    current: StructuralTypeId,
    possible: &ClaimTypes<'_>,
) -> Result<bool, &'static str> {
    let slot = possible
        .declarations
        .binary_search_by_key(&current, |row| row.id)
        .map_err(|_| "complete value type is absent")?;
    Ok(possible.declarations[slot].owns_linear_claim
        || super::unknown_owned_claims(possible.declarations[slot])
        || matches!(possible.states[slot], ClaimState::Present))
}

fn visit(
    possible: &ClaimTypes<'_>,
    current: StructuralTypeId,
    path: &mut Vec<Path>,
    region: &[Path],
    held: &BTreeSet<&[Path]>,
    selected: &dyn Fn(&[Path], StructuralCaseId) -> bool,
) -> Result<usize, &'static str> {
    let row = possible
        .declaration(current)
        .ok_or("complete value type is absent")?;
    if row.owns_linear_claim || super::unknown_owned_claims(row) {
        return held
            .contains(path.as_slice())
            .then_some(1)
            .ok_or("complete value is missing its declaration-owned claim");
    }
    if !contains_claim(current, possible)? {
        return Ok(0);
    }
    if let Some((step, tail)) = region.split_first() {
        let fields = match (&row.shape, step) {
            (StructuralTypeShape::Record { fields, .. }, Path::Field(_))
            | (StructuralTypeShape::Mixed { fields, .. }, Path::Field(_)) => fields,
            (
                StructuralTypeShape::Sum { cases }
                | StructuralTypeShape::Mixed { cases, .. },
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
                    Some(child) => visit(possible, child, path, rest, held, selected),
                    None if rest.is_empty() => Ok(0),
                    None => Err("complete value region projects through a scalar"),
                }?;
                path.truncate(path.len() - 2);
                return Ok(count);
            }
            (
                StructuralTypeShape::FixedArray { element, length },
                Path::FixedIndex(index),
            ) => {
                if length.literal().is_none_or(|length| *index >= length) {
                    return Err("complete value region has no exact array element");
                }
                path.push(step.clone());
                let count = visit(possible, *element, path, tail, held, selected)?;
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
            Some(child) => visit(possible, child, path, tail, held, selected),
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
        let mut count = 0;
        path.push(Path::FixedIndex(0));
        for index in 0..length {
            *path.last_mut().expect("array path has its current index") =
                Path::FixedIndex(index);
            count += visit(possible, *element, path, &[], held, selected)?;
        }
        path.pop();
        return Ok(count);
    }
    let fields = |fields: &[terminal_psi::StructuralFieldDeclaration],
                  path: &mut Vec<Path>| {
        let mut count = 0;
        for field in fields.iter().filter(|field| !field.relevance.is_erased()) {
            if let Some(child) = field.field_type.as_structural() {
                path.push(Path::Field(field.identity));
                count += visit(possible, child, path, &[], held, selected)?;
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

#[cfg(test)]
mod tests;

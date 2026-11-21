use semantic_vocabulary::{StructuralDomainId, StructuralTypeId};
use terminal_psi::{
    StructuralAccess, StructuralPathQualification, StructuralPathSegment,
    StructuralTypeDeclaration, StructuralTypeShape,
};

#[allow(clippy::too_many_arguments)]
pub fn reference_carrier_memberships<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    whole: &[StructuralDomainId],
    projected: &[StructuralPathQualification],
    path: &[StructuralPathSegment],
    access: StructuralAccess,
    reference: StructuralTypeId,
    storage: Option<&terminal_psi::ReferenceStorageView>,
) -> Result<Vec<StructuralPathQualification>, &'static str> {
    let captured =
        reference_source_memberships(types.clone(), root, whole, projected, path, access)?;
    if storage.is_none() || captured.is_empty() {
        return Ok(captured);
    }
    let StructuralTypeShape::Reference { referent, .. } = types
        .clone()
        .find(|row| row.id == reference)
        .ok_or("reference membership carrier is absent")?
        .shape
    else {
        return Err("representation membership has no reference carrier");
    };
    let source = crate::qualification_path_carrier(types.clone(), root, path)
        .ok_or("reference membership has no declared source carrier")?;
    if crate::borrowed_referent_presentation(types, source, referent)
        == Some(crate::BorrowedReferentPresentation::Unchanged)
    {
        return Ok(captured);
    }
    if access == StructuralAccess::SharedBorrow {
        Ok(Vec::new())
    } else {
        Err("reference reinterpretation cannot erase storage qualification obligations")
    }
}

#[allow(clippy::too_many_arguments)]
pub fn reference_carrier_membership_contract<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    whole: &[StructuralDomainId],
    projected: &[StructuralPathQualification],
    additional_input: &[StructuralPathQualification],
    path: &[StructuralPathSegment],
    access: StructuralAccess,
    reference: StructuralTypeId,
    storage: Option<&terminal_psi::ReferenceStorageView>,
) -> Result<
    (
        Vec<StructuralPathQualification>,
        Vec<StructuralPathQualification>,
    ),
    &'static str,
> {
    let standing_whole = whole
        .iter()
        .copied()
        .filter(|domain| {
            !additional_input
                .iter()
                .any(|row| row.path.is_empty() && row.domain == *domain)
        })
        .collect::<Vec<_>>();
    let standing_projected = projected
        .iter()
        .filter(|row| !additional_input.contains(row))
        .cloned()
        .collect::<Vec<_>>();
    let additional = reference_carrier_memberships(
        types.clone(),
        root,
        &[],
        additional_input,
        path,
        StructuralAccess::SharedBorrow,
        reference,
        storage,
    )?;
    let mut captured = reference_carrier_memberships(
        types,
        root,
        &standing_whole,
        &standing_projected,
        path,
        access,
        reference,
        storage,
    )?;
    captured.extend_from_slice(&additional);
    captured.sort();
    captured.dedup();
    Ok((captured, additional))
}

pub fn reference_source_memberships<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    whole: &[StructuralDomainId],
    projected: &[StructuralPathQualification],
    path: &[StructuralPathSegment],
    access: StructuralAccess,
) -> Result<Vec<StructuralPathQualification>, &'static str> {
    const MAX_PATH_DEPTH: usize = 128;
    const MAX_SELECTION_WORK: usize = 1_048_576;
    if path.len() > MAX_PATH_DEPTH {
        return Err("reference membership selection exceeds the path-depth budget");
    }
    if whole.is_empty() && projected.is_empty() {
        for (position, segment) in path.iter().enumerate() {
            if matches!(
                segment,
                StructuralPathSegment::RuntimeIndex { .. } | StructuralPathSegment::AllElements
            ) {
                let parent =
                    crate::qualification_path_carrier(types.clone(), root, &path[..position])
                        .and_then(|carrier| {
                            crate::structural_declaration(types.clone(), carrier)
                        })
                        .ok_or("reference membership selection has no runtime carrier")?;
                match parent.shape {
                    StructuralTypeShape::FixedArray {
                        length: terminal_psi::ArrayExtent::Literal(0),
                        ..
                    } => {
                        return Err("reference membership selection has an empty fixed array");
                    }
                    StructuralTypeShape::FixedArray { .. }
                    | StructuralTypeShape::ElementView { .. }
                    | StructuralTypeShape::ByteSequence(_) => {}
                    _ => {
                        return Err("reference membership selection has no collection carrier");
                    }
                }
            }
        }
        return Ok(Vec::new());
    }
    let mut work = SelectionWork(MAX_SELECTION_WORK);
    work.charge(path.len())?;
    work.charge(whole.len())?;
    for row in projected {
        work.charge(row.path.len().saturating_add(1))?;
    }
    let mut initial = Vec::new();
    initial
        .try_reserve(whole.len().saturating_add(projected.len()))
        .map_err(|_| "reference membership selection allocation failed")?;
    for domain in whole {
        initial.push(MembershipSuffix {
            path: &[],
            domain: *domain,
        });
    }
    for row in projected {
        initial.push(MembershipSuffix {
            path: &row.path,
            domain: row.domain,
        });
    }
    let mut candidates = vec![initial];
    for (position, segment) in path.iter().enumerate() {
        if matches!(
            segment,
            StructuralPathSegment::RuntimeIndex { .. } | StructuralPathSegment::AllElements
        ) {
            let parent =
                crate::qualification_path_carrier(types.clone(), root, &path[..position])
                    .and_then(|carrier| crate::structural_declaration(types.clone(), carrier))
                    .ok_or("reference membership selection has no runtime carrier")?;
            let extent = match parent.shape {
                StructuralTypeShape::FixedArray {
                    length: terminal_psi::ArrayExtent::Literal(0),
                    ..
                } => {
                    return Err("reference membership selection has an empty fixed array");
                }
                StructuralTypeShape::FixedArray { length, .. } => {
                    Some(length.literal().ok_or(
                        "reference membership selection requires a closed array extent",
                    )?)
                }
                StructuralTypeShape::ElementView { .. } => None,
                _ => return Err("reference membership selection has no collection carrier"),
            };
            let mut next = Vec::new();
            for rows in candidates {
                partition_runtime_memberships(rows, segment, extent, &mut next, &mut work)?;
            }
            candidates = next;
        } else {
            for rows in &mut candidates {
                work.charge(rows.len())?;
                rows.retain_mut(|row| {
                    let Some(suffix) = StructuralPathQualification::selected_suffix(
                        row.path,
                        std::slice::from_ref(segment),
                    ) else {
                        return false;
                    };
                    row.path = suffix;
                    true
                });
            }
        }
    }
    let mut available: Option<Vec<StructuralPathQualification>> = None;
    for rows in candidates {
        let mut captured = Vec::new();
        for row in rows {
            work.charge(row.path.len().saturating_add(2))?;
            let mut path = Vec::new();
            path.try_reserve(row.path.len().saturating_add(1))
                .map_err(|_| "reference membership selection allocation failed")?;
            path.push(StructuralPathSegment::Referent);
            path.extend_from_slice(row.path);
            captured.push(StructuralPathQualification {
                path,
                domain: row.domain,
            });
        }
        captured.sort();
        captured.dedup();
        if let Some(common) = &mut available {
            if access != StructuralAccess::SharedBorrow && *common != captured {
                return Err(
                    "reference selection cannot retain every source membership obligation",
                );
            }
            common.retain(|row| captured.binary_search(row).is_ok());
        } else {
            available = Some(captured);
        }
    }
    available.ok_or("reference membership selection has no candidate")
}

#[derive(Clone, Copy)]
struct MembershipSuffix<'source> {
    path: &'source [StructuralPathSegment],
    domain: StructuralDomainId,
}

struct SelectionWork(usize);

impl SelectionWork {
    fn charge(&mut self, count: usize) -> Result<(), &'static str> {
        self.0 = self
            .0
            .checked_sub(count)
            .ok_or("reference membership selection exceeds the work budget")?;
        Ok(())
    }
}

fn append_candidate<'source>(
    candidates: &mut Vec<Vec<MembershipSuffix<'source>>>,
    mut selected: Vec<MembershipSuffix<'source>>,
    universal: &[MembershipSuffix<'source>],
    work: &mut SelectionWork,
) -> Result<(), &'static str> {
    const MAX_CANDIDATES: usize = 4096;
    if candidates.len() >= MAX_CANDIDATES {
        return Err("reference membership selection exceeds the candidate budget");
    }
    work.charge(universal.len().saturating_add(1))?;
    selected
        .try_reserve(universal.len())
        .map_err(|_| "reference membership selection allocation failed")?;
    selected.extend_from_slice(universal);
    candidates
        .try_reserve(1)
        .map_err(|_| "reference membership selection allocation failed")?;
    candidates.push(selected);
    Ok(())
}

fn partition_runtime_memberships<'source>(
    rows: Vec<MembershipSuffix<'source>>,
    selector: &StructuralPathSegment,
    extent: Option<u64>,
    candidates: &mut Vec<Vec<MembershipSuffix<'source>>>,
    work: &mut SelectionWork,
) -> Result<(), &'static str> {
    work.charge(rows.len())?;
    let mut universal = Vec::new();
    let mut concrete = Vec::new();
    let mut remainder = Vec::new();
    for row in rows {
        let Some((segment, suffix)) = row.path.split_first() else {
            continue;
        };
        let selected = MembershipSuffix {
            path: suffix,
            domain: row.domain,
        };
        match segment {
            StructuralPathSegment::AllElements => universal.push(selected),
            StructuralPathSegment::FixedIndex(index)
                if extent.is_none_or(|length| *index < length) =>
            {
                concrete.push((*index, selected));
            }
            _ if extent.is_none() && segment == selector => remainder.push(selected),
            _ => {}
        }
    }
    concrete.sort_unstable_by_key(|(index, _)| *index);
    let mut distinct = 0u64;
    let mut rows = concrete.into_iter().peekable();
    while let Some((index, row)) = rows.next() {
        distinct += 1;
        let mut selected = vec![row];
        while rows
            .peek()
            .is_some_and(|(following, _)| *following == index)
        {
            if let Some((_, row)) = rows.next() {
                selected.push(row);
            }
        }
        append_candidate(candidates, selected, &universal, work)?;
    }
    if extent.is_none_or(|length| distinct < length) {
        append_candidate(candidates, remainder, &universal, work)?;
    }
    Ok(())
}

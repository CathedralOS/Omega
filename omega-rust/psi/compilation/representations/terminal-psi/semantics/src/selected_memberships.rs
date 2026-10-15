//! Membership transport at a runtime-selected storage occurrence.
//!
//! A concrete array row does not prove a runtime selection. Resolve every
//! possible element in the actual source carrier, then intersect observations.
//! Exclusive loans must also retain the union of restoration obligations; when
//! candidates disagree, a single unconditional loan roster cannot express that
//! debt and formation declines. No receiving annotation supplies source facts.
use semantic_vocabulary::{StructuralDomainId, StructuralTypeId};
use terminal_psi::{
    StructuralAccess, StructuralPathQualification, StructuralPathSegment,
    StructuralTypeDeclaration, StructuralTypeShape,
};

/// Capture source memberships below an ordinary reference boundary. Geometry
/// only bounds the candidate set; access, bounds, live custody and selector
/// evaluation remain independent consuming judgments.
pub fn reference_source_memberships<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    whole: &[StructuralDomainId],
    projected: &[StructuralPathQualification],
    path: &[StructuralPathSegment],
    access: StructuralAccess,
) -> Result<Vec<StructuralPathQualification>, &'static str> {
    const MAX_CANDIDATES: usize = 4096;
    const MAX_PATH_DEPTH: usize = 128;
    const MAX_SELECTION_WORK: usize = 1_048_576;
    if path.len() > MAX_PATH_DEPTH {
        return Err("reference membership selection exceeds the path-depth budget");
    }
    // An empty roster carries no facts or restoration debts. Check each
    // runtime carrier without expanding its storage, then keep it empty.
    if whole.is_empty() && projected.is_empty() {
        for (position, segment) in path.iter().enumerate() {
            if matches!(segment, StructuralPathSegment::RuntimeIndex { .. }) {
                let parent =
                    crate::runtime_structural_path_tip(types.clone(), root, &path[..position])
                        .ok_or("reference membership selection has no runtime carrier")?;
                match parent.shape {
                    StructuralTypeShape::FixedArray { length: 0, .. } => {
                        return Err("reference membership selection has an empty fixed array");
                    }
                    StructuralTypeShape::FixedArray { .. }
                    | StructuralTypeShape::ElementView { .. }
                    // Byte views are collections too. An empty membership
                    // roster has no element facts or debts to transport;
                    // bounds, access and recast extent remain independent.
                    | StructuralTypeShape::ByteSequence(_) => {}
                    _ => return Err("reference membership selection has no collection carrier"),
                }
            }
        }
        return Ok(Vec::new());
    }
    let base_work = path
        .len()
        .checked_add(whole.len())
        .ok_or("reference membership selection work overflow")?;
    // Selection copies descendant suffixes too; count their actual segment
    // storage, not merely the number of qualification rows.
    let work_per_candidate = projected
        .iter()
        .try_fold(base_work, |work, row| {
            row.path
                .len()
                .checked_add(1)
                .and_then(|count| work.checked_add(count))
        })
        .ok_or("reference membership selection work overflow")?;
    if work_per_candidate > MAX_SELECTION_WORK {
        return Err("reference membership selection exceeds the work budget");
    }
    let mut candidates = vec![Vec::new()];
    for segment in path {
        let mut next = Vec::new();
        for prefix in candidates {
            if matches!(segment, StructuralPathSegment::RuntimeIndex { .. }) {
                let parent = crate::runtime_structural_path_tip(types.clone(), root, &prefix)
                    .ok_or("reference membership selection has no runtime carrier")?;
                match parent.shape {
                    StructuralTypeShape::FixedArray { length, .. } => {
                        if length == 0 {
                            return Err("reference membership selection has an empty fixed array");
                        }
                        let count = usize::try_from(length)
                            .map_err(|_| "reference membership extent overflow")?;
                        let total = next
                            .len()
                            .checked_add(count)
                            .filter(|count| *count <= MAX_CANDIDATES)
                            .ok_or("reference membership selection exceeds the candidate budget")?;
                        if total
                            .checked_mul(work_per_candidate)
                            .is_none_or(|work| work > MAX_SELECTION_WORK)
                        {
                            return Err("reference membership selection exceeds the work budget");
                        }
                        next.try_reserve(count)
                            .map_err(|_| "reference membership selection allocation failed")?;
                        for index in 0..length {
                            let mut selected = prefix.clone();
                            selected.push(StructuralPathSegment::FixedIndex(index));
                            next.push(selected);
                        }
                    }
                    StructuralTypeShape::ElementView { .. } => {
                        // The symbolic candidate includes every unlisted live
                        // element and can use universal rows only. Concrete
                        // facts contribute debts, never proof of that remainder.
                        let mut indexes = Vec::new();
                        for row in projected {
                            let Some(StructuralPathSegment::FixedIndex(index)) =
                                StructuralPathQualification::selected_suffix(&row.path, &prefix)
                                    .and_then(|suffix| suffix.first())
                            else {
                                continue;
                            };
                            if let Err(position) = indexes.binary_search(index) {
                                if indexes.len() >= MAX_CANDIDATES {
                                    return Err(
                                        "reference membership selection exceeds the candidate budget",
                                    );
                                }
                                indexes.try_reserve(1).map_err(
                                    |_| "reference membership selection allocation failed",
                                )?;
                                indexes.insert(position, *index);
                            }
                        }
                        let count = indexes
                            .len()
                            .checked_add(1)
                            .ok_or("reference membership candidate count overflow")?;
                        let total = next
                            .len()
                            .checked_add(count)
                            .filter(|count| *count <= MAX_CANDIDATES)
                            .ok_or("reference membership selection exceeds the candidate budget")?;
                        if total
                            .checked_mul(work_per_candidate)
                            .is_none_or(|work| work > MAX_SELECTION_WORK)
                        {
                            return Err("reference membership selection exceeds the work budget");
                        }
                        next.try_reserve(count)
                            .map_err(|_| "reference membership selection allocation failed")?;
                        let mut symbolic = prefix.clone();
                        symbolic.push(segment.clone());
                        next.push(symbolic);
                        for index in indexes {
                            let mut selected = prefix.clone();
                            selected.push(StructuralPathSegment::FixedIndex(index));
                            next.push(selected);
                        }
                    }
                    _ => return Err("reference membership selection has no collection carrier"),
                }
            } else {
                let mut selected = prefix;
                selected.push(segment.clone());
                next.push(selected);
            }
        }
        candidates = next;
    }
    if candidates
        .len()
        .checked_mul(work_per_candidate)
        .is_none_or(|work| work > MAX_SELECTION_WORK)
    {
        return Err("reference membership selection exceeds the work budget");
    }
    let mut available: Option<Vec<StructuralPathQualification>> = None;
    for candidate in candidates {
        let (whole, projected) = StructuralPathQualification::select(whole, projected, &candidate);
        let mut captured = Vec::new();
        StructuralPathQualification::append_under(
            &whole,
            &projected,
            &[StructuralPathSegment::Referent],
            &mut captured,
        );
        captured.sort();
        captured.dedup();
        if let Some(common) = &mut available {
            // Union equals intersection exactly when every candidate has the
            // same canonical roster. Check that before dropping observations.
            if access != StructuralAccess::SharedBorrow && *common != captured {
                return Err("reference selection cannot retain every source membership obligation");
            }
            common.retain(|row| captured.binary_search(row).is_ok());
        } else {
            available = Some(captured);
        }
    }
    available.ok_or("reference membership selection has no candidate")
}

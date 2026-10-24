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
/// evaluation remain independent consuming judgments. `AllElements` is a
/// proof-only candidate family, with the same complete-roster obligation as a
/// runtime selection; it produces no executable selector or address.
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
    // An empty roster carries no facts or restoration debts. Check each
    // runtime carrier without expanding its storage, then keep it empty.
    if whole.is_empty() && projected.is_empty() {
        for (position, segment) in path.iter().enumerate() {
            if matches!(
                segment,
                StructuralPathSegment::RuntimeIndex { .. } | StructuralPathSegment::AllElements
            ) {
                let parent =
                    crate::qualification_path_carrier(types.clone(), root, &path[..position])
                        .and_then(|carrier| {
                            types.clone().find(|declaration| declaration.id == carrier)
                        })
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
    // Carry only the qualification suffixes still reachable at each segment.
    // Fixed-index exceptions partition an array; all unmentioned elements have
    // the same universal-only roster and need one representative, not one scan
    // of every root row per element. No candidate acquires a sibling's facts.
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
            let parent = crate::qualification_path_carrier(types.clone(), root, &path[..position])
                .and_then(|carrier| types.clone().find(|declaration| declaration.id == carrier))
                .ok_or("reference membership selection has no runtime carrier")?;
            let extent = match parent.shape {
                StructuralTypeShape::FixedArray { length: 0, .. } => {
                    return Err("reference membership selection has an empty fixed array");
                }
                StructuralTypeShape::FixedArray { length, .. } => Some(length),
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

/// Invocation-local views into the immutable source roster. Empty suffixes
/// denote whole-value membership; advancing a segment drops strict ancestors.
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
            // Element views retain the symbolic candidate representing all
            // unlisted elements. A fixed array's candidates are always concrete,
            // so a runtime occurrence row cannot qualify any of its elements.
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

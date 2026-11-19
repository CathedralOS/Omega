use crate::exactly_one::exactly_one;
use semantic_vocabulary::{StructuralCaseId, StructuralTypeId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape};

/// A call transfers every currently held claim beneath an owned argument,
/// exactly once and at its current relative path. The caller reconstructs
/// `held` from its own input representation; this function neither trusts
/// a source publisher nor decides which case or callable is selected.
pub fn validate_claim_transfers<'held, 'rows>(
    arguments: &[terminal_psi::StructuralArgument],
    held: impl IntoIterator<
        Item = (
            semantic_vocabulary::ClaimId,
            semantic_vocabulary::PlaceId,
            &'held [StructuralPathSegment],
        ),
    >,
    rows: impl IntoIterator<
        Item = (
            semantic_vocabulary::ClaimId,
            u32,
            &'rows [StructuralPathSegment],
        ),
    >,
) -> Result<(), &'static str> {
    let mut arguments = arguments
        .iter()
        .enumerate()
        .filter(|(_, argument)| argument.access == terminal_psi::StructuralAccess::Owned)
        .map(|(position, argument)| {
            Ok((
                argument.place,
                argument.path.as_slice(),
                u32::try_from(position)
                    .map_err(|_| "claim argument index exceeds its representation")?,
            ))
        })
        .collect::<Result<Vec<_>, &'static str>>()?;
    arguments.sort_unstable();
    let mut held = held.into_iter().collect::<Vec<_>>();
    held.sort_unstable_by_key(|(identity, _, _)| *identity);
    if held.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return Err("current claim frontier repeats an identity");
    }
    let mut rows = rows.into_iter();
    for (identity, home, path) in held {
        let mut selected = None;
        // Look up only the claim's prefixes. Unrelated arguments do not
        // multiply the work for every held claim; overlapping owned
        // arguments still cannot transfer the same obligation twice.
        for depth in 0..=path.len() {
            let prefix = &path[..depth];
            let position =
                arguments.partition_point(|(place, path, _)| (*place, *path) < (home, prefix));
            if let Some(&(place, candidate, argument)) = arguments.get(position)
                && place == home
                && candidate == prefix
            {
                if selected.is_some()
                    || arguments
                        .get(position + 1)
                        .is_some_and(|(next, path, _)| *next == home && *path == prefix)
                {
                    return Err("owned call arguments repeat a current claim");
                }
                selected = Some((argument, depth));
            }
        }
        if let Some((argument, depth)) = selected
            && rows.next() != Some((identity, argument, &path[depth..]))
        {
            return Err("call claim rows omit, reorder or change the current owned frontier");
        }
    }
    if rows.next().is_some() {
        return Err("call claim row names no current owned argument claim");
    }
    Ok(())
}

pub(crate) mod complete_claims;
pub use complete_claims::{ClaimTypes, validate_complete_claim_frontier};

/// An unbounded parameter may contain linear claims even when its carrier
/// bound is affine. Keep that unknown packet as one whole-value debt; this
/// is not a claim that the parameter has an intrinsic nominal root.
pub fn unknown_owned_claims(row: &StructuralTypeDeclaration) -> bool {
    matches!(row.shape, StructuralTypeShape::TypeParameter) && !row.is_copyable()
}

/// A nominal linear value owes its own claim when no owned component
/// already carries that obligation. Arrays and references do not create
/// an extra nominal obligation from their container shape.
pub fn intrinsic_claim_required(
    multiplicity: terminal_psi::StructuralMultiplicity,
    shape: &StructuralTypeShape,
    contains_owned_claim: bool,
) -> bool {
    multiplicity == terminal_psi::StructuralMultiplicity::Linear
        && !contains_owned_claim
        && matches!(
            shape,
            StructuralTypeShape::Record { .. }
                | StructuralTypeShape::Sum { .. }
                | StructuralTypeShape::Mixed { .. }
                | StructuralTypeShape::Opaque
        )
}

/// Derive the root obligation from the declaration and owned components,
/// including fields in every case, independently of the root's claim flag.
pub fn intrinsic_claim_root_by(
    possible: &ClaimTypes<'_>,
    root: StructuralTypeId,
) -> Result<bool, &'static str> {
    intrinsic_claim_root(possible, root)
}

fn intrinsic_claim_root(
    possible: &ClaimTypes<'_>,
    root: StructuralTypeId,
) -> Result<bool, &'static str> {
    let row = possible
        .declaration(root)
        .ok_or("intrinsic claim type is absent")?;
    if !intrinsic_claim_required(row.multiplicity, &row.shape, false) {
        return Ok(false);
    }
    for child in child_types(row) {
        if complete_claims::contains_claim(child, possible)? {
            return Ok(false);
        }
    }
    Ok(true)
}

pub fn validate_intrinsic_claim_roots(
    types: &[StructuralTypeDeclaration],
) -> Result<(), &'static str> {
    let possible = ClaimTypes::new(types)?;
    for row in types {
        let required = intrinsic_claim_root(&possible, row.id)?;
        if required != row.owns_linear_claim {
            return Err(if required {
                "linear nominal leaf is missing its intrinsic ownership claim"
            } else {
                "intrinsic root claim disagrees with the declared storage obligations"
            });
        }
    }
    Ok(())
}

/// The exact typed catalogue used by both publication and receiving. Its
/// limit bounds the output roster, not a second interpretation of claims.
pub fn claim_catalogue(
    possible: &ClaimTypes<'_>,
    root: StructuralTypeId,
    maximum: usize,
) -> Result<BTreeSet<Vec<StructuralPathSegment>>, &'static str> {
    let mut pending = vec![(root, Vec::new())];
    let mut paths = BTreeSet::new();
    while let Some((current, path)) = pending.pop() {
        if !complete_claims::contains_claim(current, possible)? {
            continue;
        }
        let row = possible
            .declaration(current)
            .ok_or("claim catalogue type is absent")?;
        if row.owns_linear_claim || unknown_owned_claims(row) {
            if paths.len() == maximum || !paths.insert(path) {
                return Err("claim catalogue omits or repeats an intrinsic leaf");
            }
            continue;
        }
        if let StructuralTypeShape::FixedArray { element, length } = row.shape {
            let length = length
                .literal()
                .ok_or("claim catalogue array extent is not closed")?;
            if usize::try_from(length)
                .ok()
                .is_none_or(|length| length > maximum.saturating_sub(paths.len()))
            {
                return Err("claim catalogue has fewer rows than its claimed array elements");
            }
            for index in 0..length {
                let mut child = path.clone();
                child.push(StructuralPathSegment::FixedIndex(index));
                pending.push((element, child));
            }
            continue;
        }
        for (case, field) in row.shape.fields_with_case() {
            if field.relevance.is_erased() {
                continue;
            }
            if let Some(child) = field.field_type.as_structural() {
                let mut selected = path.clone();
                if let Some(case) = case {
                    selected.push(StructuralPathSegment::Case(case.id));
                }
                selected.push(StructuralPathSegment::Field(field.identity));
                pending.push((child, selected));
            }
        }
    }
    Ok(paths)
}

fn child_types(declaration: &StructuralTypeDeclaration) -> Vec<StructuralTypeId> {
    let mut children = Vec::new();
    let mut fields = |fields: &[terminal_psi::StructuralFieldDeclaration]| {
        children.extend(
            fields
                .iter()
                .filter(|field| !field.relevance.is_erased())
                .filter_map(|field| field.field_type.as_structural()),
        );
    };
    match &declaration.shape {
        StructuralTypeShape::Record { fields: common, .. } => fields(common),
        StructuralTypeShape::Mixed {
            fields: common,
            cases,
        } => {
            fields(common);
            for case in cases {
                fields(&case.fields);
            }
        }
        StructuralTypeShape::Sum { cases } => {
            for case in cases {
                fields(&case.fields);
            }
        }
        StructuralTypeShape::FixedArray { element, length } if length.literal() != Some(0) => {
            children.push(*element)
        }
        _ => {}
    }
    children
}

pub fn contains_owned_boundary(
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    boundaries: &BTreeSet<StructuralTypeId>,
    cache: &mut BTreeMap<StructuralTypeId, bool>,
    active: &mut BTreeSet<StructuralTypeId>,
    structural_type: StructuralTypeId,
) -> Option<bool> {
    if boundaries.contains(&structural_type) {
        return Some(true);
    }
    if let Some(result) = cache.get(&structural_type) {
        return Some(*result);
    }
    if !active.insert(structural_type) {
        return None;
    }
    let mut result = false;
    for child in child_types(types.get(&structural_type)?) {
        result |= contains_owned_boundary(types, boundaries, cache, active, child)?;
    }
    active.remove(&structural_type);
    cache.insert(structural_type, result);
    Some(result)
}

pub fn validate_accounted_descendants(
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    boundaries: &BTreeSet<StructuralTypeId>,
    root: StructuralTypeId,
    path: &[StructuralPathSegment],
    accounted: &[Vec<StructuralPathSegment>],
    selected: &dyn Fn(&[StructuralPathSegment], StructuralCaseId) -> bool,
) -> Result<(), &'static str> {
    fn visit(
        types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        boundaries: &BTreeSet<StructuralTypeId>,
        current: StructuralTypeId,
        path: &[StructuralPathSegment],
        accounted: &[Vec<StructuralPathSegment>],
        selected: &dyn Fn(&[StructuralPathSegment], StructuralCaseId) -> bool,
        cache: &mut BTreeMap<StructuralTypeId, bool>,
        active: &mut BTreeSet<StructuralTypeId>,
    ) -> Result<(), &'static str> {
        let declaration = types
            .get(&current)
            .ok_or("owned descendant declaration is absent")?;
        if boundaries.contains(&current) && !accounted.iter().any(|claim| claim == path) {
            return Err("consumed parent has an unaccounted descendant obligation");
        }
        if !active.insert(current) {
            return Err("owned descendant graph is cyclic");
        }
        let mut relevant = false;
        for child in child_types(declaration) {
            relevant |=
                contains_owned_boundary(types, boundaries, cache, &mut BTreeSet::new(), child)
                    .ok_or("owned descendant graph is unresolved")?;
        }
        if relevant {
            let mut visit_child = |child_path: &[StructuralPathSegment], child| {
                if !contains_owned_boundary(
                    types,
                    boundaries,
                    cache,
                    &mut BTreeSet::new(),
                    child,
                )
                .ok_or("owned descendant graph is unresolved")?
                {
                    return Ok(());
                }
                visit(
                    types, boundaries, child, child_path, accounted, selected, cache, active,
                )
            };
            if let StructuralTypeShape::FixedArray { element, length } = declaration.shape {
                let length = length
                    .literal()
                    .ok_or("owned descendant accounting requires a closed array extent")?;
                let mut child_path = path.to_vec();
                child_path.push(StructuralPathSegment::FixedIndex(0));
                for element_index in 0..length {
                    if let Some(segment) = child_path.last_mut() {
                        *segment = StructuralPathSegment::FixedIndex(element_index);
                    }
                    visit_child(&child_path, element)?;
                }
            } else {
                let mut case_payload_required = false;
                if let StructuralTypeShape::Sum { cases }
                | StructuralTypeShape::Mixed { cases, .. } = &declaration.shape
                {
                    for child in cases
                        .iter()
                        .flat_map(|case| &case.fields)
                        .filter(|field| !field.relevance.is_erased())
                        .filter_map(|field| field.field_type.as_structural())
                    {
                        case_payload_required |= contains_owned_boundary(
                            types,
                            boundaries,
                            &mut BTreeMap::new(),
                            &mut BTreeSet::new(),
                            child,
                        )
                        .ok_or("owned descendant graph is unresolved")?;
                    }
                }
                let children = selected_children(
                    types,
                    current,
                    path,
                    usize::MAX,
                    selected,
                    case_payload_required,
                )
                .ok_or("owned descendant occurrence or active case is unresolved")?;
                for (child_path, child) in children {
                    visit_child(&child_path, child)?;
                }
            }
        }
        active.remove(&current);
        Ok(())
    }
    visit(
        types,
        boundaries,
        root,
        path,
        accounted,
        selected,
        &mut BTreeMap::new(),
        &mut BTreeSet::new(),
    )
}

pub fn selected_contains_owned_boundary(
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    nominal: &BTreeSet<StructuralTypeId>,
    cache: &mut BTreeMap<StructuralTypeId, bool>,
    selected: &dyn Fn(&[StructuralPathSegment], StructuralCaseId) -> bool,
    structural_type: StructuralTypeId,
    path: &[StructuralPathSegment],
    limit: usize,
) -> Option<bool> {
    if nominal.contains(&structural_type) {
        return Some(true);
    }
    if !contains_owned_boundary(types, nominal, cache, &mut BTreeSet::new(), structural_type)? {
        return Some(false);
    }
    for (child_path, child) in
        selected_owned_children(types, structural_type, path, limit, selected)?
    {
        if selected_contains_owned_boundary(
            types,
            nominal,
            cache,
            selected,
            child,
            &child_path,
            limit,
        )? {
            return Some(true);
        }
    }
    Some(false)
}

pub fn validate_owned_claim_frontier(
    possible: &ClaimTypes<'_>,
    selected_type: StructuralTypeId,
    path: &[StructuralPathSegment],
    accounted: impl IntoIterator<Item = Vec<StructuralPathSegment>>,
    selected: &dyn Fn(&[StructuralPathSegment], StructuralCaseId) -> bool,
) -> Result<(), &'static str> {
    for (index, segment) in path.iter().enumerate() {
        if let StructuralPathSegment::Case(case) = segment
            && !selected(&path[..index], *case)
        {
            return Err("owned value path has no established active case");
        }
    }
    let mut remaining = BTreeSet::new();
    for claim in accounted {
        if !claim.starts_with(path) || !remaining.insert(claim) {
            return Err("owned value claims overlap an ancestor or repeat a path");
        }
    }
    enum Pending {
        Value(Vec<StructuralPathSegment>, StructuralTypeId),
        Array {
            path: Vec<StructuralPathSegment>,
            element: StructuralTypeId,
            next: u64,
            length: u64,
        },
    }
    let mut pending = vec![Pending::Value(path.to_vec(), selected_type)];
    while let Some(occurrence) = pending.pop() {
        let (path, current) = match occurrence {
            Pending::Value(path, current) => (path, current),
            Pending::Array {
                path,
                element,
                next,
                length,
            } => {
                if next < length {
                    let mut child = path.clone();
                    child.push(StructuralPathSegment::FixedIndex(next));
                    pending.push(Pending::Array {
                        path,
                        element,
                        next: next + 1,
                        length,
                    });
                    pending.push(Pending::Value(child, element));
                }
                continue;
            }
        };
        let current_type = possible
            .declaration(current)
            .ok_or("owned value type is absent")?;
        if current_type.owns_linear_claim || unknown_owned_claims(current_type) {
            if !remaining.remove(&path) {
                return Err("owned value has no exact live nominal claim");
            }
            continue;
        }
        if !complete_claims::contains_claim(current, possible)? {
            continue;
        }
        if let StructuralTypeShape::FixedArray { element, length } = &current_type.shape {
            pending.push(Pending::Array {
                path,
                element: *element,
                next: 0,
                length: length
                    .literal()
                    .ok_or("owned value array extent is not closed")?,
            });
            continue;
        }
        let mut fields =
            |prefix: &[StructuralPathSegment],
             fields: &[terminal_psi::StructuralFieldDeclaration]| {
                for field in fields
                    .iter()
                    .rev()
                    .filter(|field| !field.relevance.is_erased())
                {
                    if let Some(child) = field.field_type.as_structural() {
                        let mut child_path = prefix.to_vec();
                        child_path.push(StructuralPathSegment::Field(field.identity));
                        pending.push(Pending::Value(child_path, child));
                    }
                }
            };
        match &current_type.shape {
            StructuralTypeShape::Record { fields: common, .. } => fields(&path, common),
            StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. } => {
                let mut payload_required = false;
                for child in cases
                    .iter()
                    .flat_map(|case| &case.fields)
                    .filter(|field| !field.relevance.is_erased())
                    .filter_map(|field| field.field_type.as_structural())
                {
                    payload_required |= complete_claims::contains_claim(child, possible)?;
                }
                if payload_required {
                    let mut admitted = false;
                    for case in cases.iter().filter(|case| selected(&path, case.id)) {
                        admitted = true;
                        let mut payload_path = path.clone();
                        payload_path.push(StructuralPathSegment::Case(case.id));
                        fields(&payload_path, &case.fields);
                    }
                    if !admitted {
                        return Err("owned value has no admitted payload case");
                    }
                }
                if let StructuralTypeShape::Mixed { fields: common, .. } = &current_type.shape {
                    fields(&path, common);
                }
            }
            _ => return Err("owned value occurrence is unresolved"),
        }
    }
    if !remaining.is_empty() {
        return Err("owned value has claims outside its selected nominal frontier");
    }
    Ok(())
}

pub fn selected_owned_children(
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    structural_type: StructuralTypeId,
    path: &[StructuralPathSegment],
    limit: usize,
    selected: &dyn Fn(&[StructuralPathSegment], StructuralCaseId) -> bool,
) -> Option<Vec<(Vec<StructuralPathSegment>, StructuralTypeId)>> {
    selected_children(types, structural_type, path, limit, selected, true)
}

fn selected_children(
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    structural_type: StructuralTypeId,
    path: &[StructuralPathSegment],
    limit: usize,
    selected: &dyn Fn(&[StructuralPathSegment], StructuralCaseId) -> bool,
    case_payload_required: bool,
) -> Option<Vec<(Vec<StructuralPathSegment>, StructuralTypeId)>> {
    let mut children = Vec::new();
    let mut fields = |fields: &[terminal_psi::StructuralFieldDeclaration],
                      prefix: &[StructuralPathSegment]| {
        for field in fields
            .iter()
            .rev()
            .filter(|field| !field.relevance.is_erased())
        {
            if let Some(child) = field.field_type.as_structural() {
                let mut child_path = prefix.to_vec();
                child_path.push(StructuralPathSegment::Field(field.identity));
                children.push((child_path, child));
            }
        }
    };
    match &types.get(&structural_type)?.shape {
        StructuralTypeShape::Record { fields: common, .. } => fields(common, path),
        StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. } => {
            if case_payload_required {
                let case = exactly_one(cases.iter(), |case| selected(path, case.id))?;
                let mut payload_path = path.to_vec();
                payload_path.push(StructuralPathSegment::Case(case.id));
                fields(&case.fields, &payload_path);
            }
            if let StructuralTypeShape::Mixed { fields: common, .. } =
                &types.get(&structural_type)?.shape
            {
                fields(common, path);
            }
        }
        StructuralTypeShape::FixedArray {
            element,
            length: terminal_psi::ArrayExtent::Literal(length),
        } => {
            if u128::from(*length) > limit as u128 {
                return None;
            }
            for index in (0..*length).rev() {
                let mut child_path = path.to_vec();
                child_path.push(StructuralPathSegment::FixedIndex(index));
                children.push((child_path, *element));
            }
        }
        _ => return None,
    }
    Some(children)
}

//! Shared typed traversal of owned structural occurrences. Callers supply the
//! independently identified obligation kinds and actual case evidence. A path
//! is geometry, never custody; cleanup ordering and claim retirement remain
//! with their consuming transactions.
use semantic_vocabulary::{StructuralCaseId, StructuralTypeId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape};

/// By-value child declarations, without expanding array cardinality or
/// following borrowed referents. Repeated occurrences are selected below.
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
        StructuralTypeShape::FixedArray { element, length } if *length != 0 => {
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

/// A whole account never hides a descendant's own obligation. Visit only
/// potentially obligated by-value children; obligation-free arrays take no
/// per-element work. Relevant arrays are streamed, never expanded into a
/// dimension-sized scratch vector. A node count is not an occurrence bound:
/// one array constructor can establish many independently selected children.
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
                if !contains_owned_boundary(types, boundaries, cache, &mut BTreeSet::new(), child)
                    .ok_or("owned descendant graph is unresolved")?
                {
                    return Ok(());
                }
                visit(
                    types, boundaries, child, child_path, accounted, selected, cache, active,
                )
            };
            if let StructuralTypeShape::FixedArray { element, length } = declaration.shape {
                let mut child_path = path.to_vec();
                child_path.push(StructuralPathSegment::FixedIndex(0));
                for element_index in 0..length {
                    if let Some(segment) = child_path.last_mut() {
                        *segment = StructuralPathSegment::FixedIndex(element_index);
                    }
                    visit_child(&child_path, element)?;
                }
            } else {
                // Common fields are independent of the active case. An enum
                // discriminator matters only if a case can contain debt.
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
    // Universal absence needs no discriminator. A possible hook instead
    // requires the selected occurrence; missing claims do not select it.
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

/// Expand the selected payload before common fields, in reverse field order.
/// Selection is tied to this root-relative occurrence, never cached by type.
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
                let mut matches = cases.iter().filter(|case| selected(path, case.id));
                let case = matches.next()?;
                if matches.next().is_some() {
                    return None;
                }
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
        StructuralTypeShape::FixedArray { element, length } => {
            // Every untouched child contributes a subject; do not enumerate a
            // forged dimension beyond the supplied partition's work bound.
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

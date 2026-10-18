//! Public returned regions name complete output leaves and input permissions.
//! Geometry never grants an address: each row still supplies an actual checked
//! descriptor whose full footprint and parent custody are checked at completion.

use semantic_vocabulary::StructuralTypeId;
use terminal_psi::{
    BoundaryMachineResult, ReferenceOriginProjection, StructuralAccess, StructuralPathSegment,
    StructuralTypeDeclaration, StructuralTypeShape, TerminalDynamicSlot,
};

/// Validate the public contract independently of whether any implementation
/// table happens to instantiate it. Private row summaries may narrow these
/// regions, never supply omitted output leaves or invent an input permission.
pub fn validate_dynamic_reference_regions<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    slot: &TerminalDynamicSlot,
) -> Result<(), &'static str> {
    let regions = &slot.result_reference_regions;
    let BoundaryMachineResult::Structural(result) = &slot.result else {
        return if regions.is_empty() {
            Ok(())
        } else {
            Err("nonstructural dynamic result carries reference regions")
        };
    };
    if regions.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err("dynamic reference regions are not unique and ordered");
    }
    let leaves = super::borrowed_result_leaves(
        types.clone(),
        result.structural_type,
        result.multiplicity,
        regions.len(),
    )?;
    if leaves
        .iter()
        .any(|leaf| !regions.iter().any(|region| region.path == leaf.path))
    {
        return Err("dynamic reference regions omit a result leaf");
    }
    for region in regions {
        let leaf = leaves
            .iter()
            .find(|leaf| leaf.path == region.path)
            .ok_or("dynamic reference region does not name an owned result leaf")?;
        if leaf.access != region.access {
            return Err("dynamic reference region disagrees with result access");
        }
        let index = match region.origin {
            terminal_psi::TerminalDynamicReferenceOrigin::Parameter { argument_index } => {
                argument_index
            }
            terminal_psi::TerminalDynamicReferenceOrigin::Invocation { carrier_type } => {
                let path = region
                    .source_path
                    .iter()
                    .cloned()
                    .map(ReferenceOriginProjection::Exact)
                    .collect::<Vec<_>>();
                let endpoint = super::reference_origin_type(types.clone(), carrier_type, &path)
                    .ok_or("dynamic invocation origin has no typed provider storage")?;
                if let super::BorrowedResultLeafKind::Reference(referent) = leaf.kind
                    && referent != endpoint
                {
                    return Err("dynamic invocation origin changes its reference referent");
                }
                validate_input_region(
                    types.clone(),
                    carrier_type,
                    StructuralAccess::Owned,
                    &region.source_path,
                    region.access,
                )?;
                continue;
            }
        };
        let parameter = usize::try_from(index)
            .ok()
            .and_then(|position| slot.structural_parameters.get(position))
            .ok_or("dynamic reference region has no structural input")?;
        validate_input_region(
            types.clone(),
            parameter.structural_type,
            parameter.access,
            &region.source_path,
            region.access,
        )?;
    }
    Ok(())
}

fn validate_input_region<'types>(
    types: impl Iterator<Item = &'types StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    access: StructuralAccess,
    path: &[StructuralPathSegment],
    required: StructuralAccess,
) -> Result<(), &'static str> {
    if !required.admits(access) {
        return Err("dynamic reference region exceeds input access");
    }
    let mut borrowed = access != StructuralAccess::Owned;
    for (position, segment) in path.iter().enumerate() {
        // Every selected typed endpoint may be a stored view, including an
        // array element. Field metadata supplies inline byte-view access before
        // canonical type lookup deliberately peels that reference shell.
        let field_view = crate::runtime_structural_field(types.clone(), root, &path[..=position])
            .and_then(|field| field.field_type.byte_sequence_carrier());
        let view_access = match field_view {
            Some(
                terminal_psi::ByteSequenceCarrier::BorrowedView { access }
                | terminal_psi::ByteSequenceCarrier::BoundedView { access, .. },
            ) => Some(access.ok_or("stored byte view has no declared access")?),
            _ => crate::runtime_structural_path_tip(types.clone(), root, &path[..=position])
                .and_then(|row| match row.shape {
                    StructuralTypeShape::ElementView { .. } => Some(StructuralAccess::SharedBorrow),
                    StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BorrowedView { access }
                        | terminal_psi::ByteSequenceCarrier::BoundedView { access, .. },
                    ) => Some(access.unwrap_or(StructuralAccess::SharedBorrow)),
                    _ => None,
                }),
        };
        if let Some(view_access) = view_access {
            if !required.admits_referent(view_access) {
                return Err("dynamic reference region exceeds stored view access");
            }
            borrowed = true;
        }
        match segment {
            StructuralPathSegment::Field(_)
            | StructuralPathSegment::Case(_)
            | StructuralPathSegment::FixedIndex(_) => {}
            StructuralPathSegment::Referent => {
                let prefix =
                    crate::runtime_structural_path_tip(types.clone(), root, &path[..position])
                        .ok_or("dynamic input region crossing has no typed carrier")?;
                if let StructuralTypeShape::Reference { access, .. } = prefix.shape {
                    if !required.admits_referent(access) {
                        return Err("dynamic reference region exceeds stored reference access");
                    }
                    borrowed = true;
                } else if position + 1 != path.len() {
                    return Err("dynamic input region has a nonterminal storage marker");
                }
            }
            _ => return Err("dynamic input region is not an exact public storage path"),
        }
    }
    let origin = path
        .iter()
        .cloned()
        .map(ReferenceOriginProjection::Exact)
        .collect::<Vec<_>>();
    let storage_path = crate::reference_source_storage_path(types.clone(), root, path);
    let inline_view = crate::runtime_structural_field(types.clone(), root, storage_path)
        .and_then(|field| field.field_type.byte_sequence_carrier())
        .is_some_and(|carrier| {
            matches!(
                carrier,
                terminal_psi::ByteSequenceCarrier::BorrowedView { .. }
                    | terminal_psi::ByteSequenceCarrier::BoundedView { .. }
            )
        });
    // Inline byte fields retain their declaring field and have no standalone
    // structural type. The complete typed field path above is their endpoint.
    if !inline_view {
        super::reference_origin_type(types, root, &origin)
            .ok_or("dynamic input region path has no typed endpoint")?;
    }
    if !borrowed {
        return Err("dynamic result cannot borrow private owned parameter storage");
    }
    Ok(())
}

/// Exact declaration-order reference leaves of the supported owned shape.
/// `maximum_leaves` bounds reconstruction exactly like the verified rule.
pub fn reference_leaves<'types>(
    types: impl Iterator<Item = &'types terminal_psi::StructuralTypeDeclaration> + Clone,
    root: StructuralTypeId,
    maximum_leaves: usize,
) -> Option<Vec<ReferenceLeaf>> {
    let contains_reference = |root| {
        terminal_psi::structural_type_contains_reference_by(
            |id| types.clone().find(|row| row.id == id),
            root,
        )
    };
    let mut output = Vec::new();
    let mut pending = vec![(root, Vec::new())];
    while let Some((current, path)) = pending.pop() {
        if !contains_reference(current) {
            continue;
        }
        let declaration = types.clone().find(|row| row.id == current)?;
        let fields: Vec<_> = match &declaration.shape {
            terminal_psi::StructuralTypeShape::Reference { .. } => {
                if output.len() == maximum_leaves {
                    return None;
                }
                output.push(ReferenceLeaf {
                    path,
                    structural_type: current,
                });
                continue;
            }
            terminal_psi::StructuralTypeShape::Record { fields, .. } => {
                fields.iter().map(|field| (None, field)).collect()
            }
            // A field identity is local to its case. Preserve that namespace
            // in the loan path, including when two cases reuse a field name.
            terminal_psi::StructuralTypeShape::Sum { cases } => cases
                .iter()
                .flat_map(|case| case.fields.iter().map(move |field| (Some(case.id), field)))
                .collect(),
            terminal_psi::StructuralTypeShape::Mixed { fields, cases } => cases
                .iter()
                .flat_map(|case| case.fields.iter().map(move |field| (Some(case.id), field)))
                .chain(fields.iter().map(|field| (None, field)))
                .collect(),
            terminal_psi::StructuralTypeShape::FixedArray { element, length } => {
                // Each pending subtree owes at least one live leaf. Empty
                // arrays owe none; contains_reference already excludes them.
                // Bound the worklist before expanding nested array products.
                let count = usize::try_from(*length).ok()?;
                if pending
                    .len()
                    .checked_add(output.len())?
                    .checked_add(count)?
                    > maximum_leaves
                {
                    return None;
                }
                for element_index in (0..*length).rev() {
                    let mut child_path = path.clone();
                    child_path.push(terminal_psi::StructuralPathSegment::FixedIndex(
                        element_index,
                    ));
                    pending.push((*element, child_path));
                }
                continue;
            }
            // A runtime view has no declaration-owned leaf roster. Unknown
            // containment must not silently discard permissions.
            _ => return None,
        };
        for (case, field) in fields.into_iter().rev() {
            if let Some(child) = field.field_type.as_structural() {
                if !contains_reference(child) {
                    continue;
                }
                if pending.len().checked_add(output.len())? >= maximum_leaves {
                    return None;
                }
                let mut child_path = path.clone();
                if let Some(case) = case {
                    child_path.push(StructuralPathSegment::Case(case));
                }
                child_path.push(terminal_psi::StructuralPathSegment::Field(
                    field.identity.clone(),
                ));
                pending.push((child, child_path));
            }
        }
    }

    output.sort_by(|left, right| left.path.cmp(&right.path));
    if output.windows(2).any(|pair| {
        pair[0].path == pair[1].path && pair[0].structural_type != pair[1].structural_type
    }) {
        return None;
    }
    output.dedup_by(|left, right| left.path == right.path);
    Some(output)
}

pub struct ReferenceLeaf {
    pub path: Vec<StructuralPathSegment>,
    pub structural_type: StructuralTypeId,
}

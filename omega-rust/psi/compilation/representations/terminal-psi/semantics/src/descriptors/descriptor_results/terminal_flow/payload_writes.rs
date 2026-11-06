//! A call may change its borrowed record and the data reached through its
//! pointers. Keep every such footprint at the actual argument's captured cut;
//! an unavailable writable frontier remains an unknown write.

use super::*;

#[cfg(test)]
mod tests;

pub(super) fn selected_query(
    types: &[StructuralTypeDeclaration],
    root: PlaceId,
    root_type: StructuralTypeId,
    path: &[StructuralPathSegment],
    field: Option<semantic_vocabulary::StructuralFieldId>,
) -> Option<DescriptorContentsQuery> {
    let mut query = descriptor_contents_query(types, root, root_type, path, field)?;
    if field.is_none() {
        let selected = crate::runtime_structural_path_tip(types.iter(), root_type, path)?;
        match selected.shape {
            StructuralTypeShape::ElementView { element, .. } => {
                query
                    .steps
                    .push(DescriptorContentsStep::AnyElement { element });
            }
            StructuralTypeShape::ByteSequence(_) if is_descriptor(types.iter(), selected.id) => {
                return None;
            }
            _ => {}
        }
    }
    Some(query)
}

pub(super) fn for_argument(
    types: &[StructuralTypeDeclaration],
    argument: &StructuralArgument,
    root_type: StructuralTypeId,
) -> Result<Vec<DescriptorContentsQuery>, &'static str> {
    let selected = crate::runtime_structural_path_tip(types.iter(), root_type, &argument.path)
        .ok_or("the call write has no selected argument carrier")?;
    let mutation = crate::shared_mutation(types.iter(), selected.id, &[]);
    if argument.access == StructuralAccess::SharedBorrow && mutation == crate::SharedMutation::None
    {
        return Ok(Vec::new());
    }
    let selected_query = selected_query(types, argument.place, root_type, &argument.path, None)
        .ok_or("the call write has no complete selected payload query")?;
    let prefix = descriptor_contents_query(types, argument.place, root_type, &argument.path, None)
        .ok_or("the call write has no selected carrier query")?;
    let mut queries = vec![selected_query];
    let inventory = DescriptorSlotKind::BorrowedStorage
        .inventory(types, argument.place, selected.id)
        .ok_or("the call write has no complete reachable descriptor inventory")?;
    for opaque in inventory.opaque_referents {
        // Shared storage may still contain atomics. Missing layouts cannot
        // establish that a reachable subtree has no mutable payloads.
        if path_access(types, selected.id, argument.access, &opaque.path)?.is_some() {
            return Err("the call write can reach an opaque possibly mutable subtree");
        }
    }
    for slot in inventory.slots {
        let Some(permission) = path_access(types, selected.id, argument.access, &slot.path)? else {
            continue;
        };
        if permission == StructuralAccess::WriteOnlyBorrow {
            continue;
        }
        let ty = descriptor_location_type(types, selected.id, &slot.path)
            .ok_or("the call write descriptor has no selected carrier")?;
        let declaration = find_structural_type(types, ty)
            .ok_or("the call write descriptor carrier is unavailable")?;
        let suffix = match declaration.shape {
            StructuralTypeShape::Reference { referent, access } => {
                let access = if permission == StructuralAccess::SharedBorrow {
                    StructuralAccess::SharedBorrow
                } else {
                    access
                };
                if access == StructuralAccess::SharedBorrow
                    && crate::shared_mutation(types.iter(), referent, &[])
                        == crate::SharedMutation::None
                {
                    continue;
                }
                DescriptorContentsStep::Address(DescriptorPathSegment::Referent)
            }
            StructuralTypeShape::ElementView { element, access } => {
                let access = if permission == StructuralAccess::SharedBorrow {
                    StructuralAccess::SharedBorrow
                } else {
                    access.unwrap_or(permission)
                };
                let mutation = crate::shared_mutation(types.iter(), element, &[]);
                if access == StructuralAccess::SharedBorrow
                    && mutation == crate::SharedMutation::None
                {
                    continue;
                }
                // The element query covers its inline writes and atomics.
                // Further indirect mutation needs its own complete frontier.
                if mutation == crate::SharedMutation::Unbounded {
                    return Err("the call write view has an unavailable indirect element frontier");
                }
                inline_elements(types, argument.place, element, access)?;
                DescriptorContentsStep::AnyElement { element }
            }
            StructuralTypeShape::ByteSequence(carrier) => {
                if permission == StructuralAccess::SharedBorrow
                    || matches!(
                        carrier,
                        terminal_psi::ByteSequenceCarrier::BorrowedView {
                            access: Some(StructuralAccess::SharedBorrow)
                        } | terminal_psi::ByteSequenceCarrier::BoundedView {
                            access: Some(StructuralAccess::SharedBorrow),
                            ..
                        }
                    )
                {
                    continue;
                }
                return Err("the call write has unavailable mutable byte-view backing");
            }
            _ => return Err("the call write inventory has no descriptor representation"),
        };
        let mut query = prefix.clone();
        query
            .steps
            .extend(slot.path.into_iter().map(DescriptorContentsStep::Address));
        query.steps.push(suffix);
        query
            .validate(types, root_type)
            .ok_or("the call write reference frontier has no complete typed query")?;
        if !queries.contains(&query) {
            queries.push(query);
        }
    }
    Ok(queries)
}

/// A canonical descriptor's access belongs to its occurrence. An outer view
/// can lend its element storage without supplying the access of a descriptor
/// stored there, so a missing inner access is not an empty write frontier.
fn inline_elements(
    types: &[StructuralTypeDeclaration],
    root: PlaceId,
    element: StructuralTypeId,
    access: StructuralAccess,
) -> Result<(), &'static str> {
    let inventory = DescriptorSlotKind::BorrowedStorage
        .inventory(types, root, element)
        .ok_or("the call write view has no complete element inventory")?;
    for slot in inventory.slots {
        let Some(permission) = path_access(types, element, access, &slot.path)? else {
            continue;
        };
        if matches!(
            permission,
            StructuralAccess::SharedBorrow | StructuralAccess::WriteOnlyBorrow
        ) {
            continue;
        }
        let ty = descriptor_location_type(types, element, &slot.path)
            .ok_or("the call write view element descriptor has no carrier")?;
        let shape = &find_structural_type(types, ty)
            .ok_or("the call write view element descriptor is unavailable")?
            .shape;
        if matches!(
            shape,
            StructuralTypeShape::ElementView { access: None, .. }
                | StructuralTypeShape::ByteSequence(
                    terminal_psi::ByteSequenceCarrier::BorrowedView { access: None }
                        | terminal_psi::ByteSequenceCarrier::BoundedView { access: None, .. }
                )
        ) {
            return Err("the call write view element has no captured descriptor access");
        }
    }
    Ok(())
}

/// Keep the outer access ceiling while following each typed pointer. A
/// write-only carrier can be replaced but cannot supply its old pointer value.
fn path_access(
    types: &[StructuralTypeDeclaration],
    root_type: StructuralTypeId,
    mut permission: StructuralAccess,
    path: &[DescriptorPathSegment],
) -> Result<Option<StructuralAccess>, &'static str> {
    for (index, segment) in path.iter().enumerate() {
        if *segment != DescriptorPathSegment::Referent {
            continue;
        }
        if permission == StructuralAccess::WriteOnlyBorrow {
            return Ok(None);
        }
        let ty = descriptor_location_type(types, root_type, &path[..index])
            .ok_or("the call write reference prefix has no selected carrier")?;
        let StructuralTypeShape::Reference { access, .. } = find_structural_type(types, ty)
            .ok_or("the call write reference prefix carrier is unavailable")?
            .shape
        else {
            return Err("the call write reference prefix is not a pointer");
        };
        if permission != StructuralAccess::SharedBorrow {
            permission = access;
        }
    }
    Ok(Some(permission))
}

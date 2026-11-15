//! A provider's lifetime anchors keep inputs live without locating its bytes
//! inside those inputs. Validate the declared frontier independently of the
//! spatial reference-result contract and preserve unavailable metadata.

use terminal_psi::{
    BoundaryLifetimeDependency, BoundaryMachineDeclaration, BoundaryMachineResult,
    DeclaredResultLifetime, StructuralAccess, StructuralTypeDeclaration, StructuralTypeShape,
};

use crate::descriptor_results::{
    DescriptorLifetimeDebt, DescriptorProviderOrigin, DescriptorProviderOutput,
    DescriptorResultSlot, DescriptorSlotKind, DescriptorStoredOrigin,
};

pub(crate) fn provider_lifetime_debts(
    types: &[StructuralTypeDeclaration],
    parameters: &[terminal_psi::StructuralParameterDeclaration],
    dependencies: &[BoundaryLifetimeDependency],
) -> Result<Vec<DescriptorLifetimeDebt<DescriptorStoredOrigin>>, &'static str> {
    dependencies
        .iter()
        .map(|dependency| {
            let (origin, access) = match dependency {
                BoundaryLifetimeDependency::Referent {
                    argument_index,
                    access,
                } => (
                    DescriptorStoredOrigin::Parameter {
                        position: usize::try_from(*argument_index)
                            .map_err(|_| "provider lifetime input exceeds host positions")?,
                        projection: crate::borrowed_storage::Projection::new(&[]),
                    },
                    *access,
                ),
                BoundaryLifetimeDependency::Stored {
                    argument_index,
                    path,
                    access,
                } => {
                    let position = usize::try_from(*argument_index)
                        .map_err(|_| "provider lifetime input exceeds host positions")?;
                    let parameter = parameters
                        .get(position)
                        .ok_or("provider lifetime input names no structural formal")?;
                    let source = crate::descriptor_lifetimes::descriptor_location(
                        types,
                        parameter.place,
                        parameter.structural_type,
                        path,
                        None,
                    )
                    .ok_or("provider lifetime input has no exact descriptor path")?;
                    (
                        DescriptorStoredOrigin::IncomingSlot {
                            position,
                            path: source.path,
                        },
                        *access,
                    )
                }
            };
            Ok(DescriptorLifetimeDebt { origin, access })
        })
        .collect()
}

/// A provider result owns its provenance, while its dependencies describe
/// only what must remain live. Neither the declared group nor these debts
/// places the result inside an input's storage.
pub fn boundary_result_slots(
    types: &[StructuralTypeDeclaration],
    boundary: &BoundaryMachineDeclaration,
    kind: DescriptorSlotKind,
    result: &terminal_psi::StructuralOperationResult,
) -> Result<Vec<DescriptorResultSlot>, &'static str> {
    let groups = boundary_result_lifetimes(types, boundary)?;
    let BoundaryMachineResult::Structural(declared) = &boundary.result else {
        return Err("boundary descriptor result has no structural declaration");
    };
    if declared.structural_type != result.structural_type
        || declared.multiplicity != result.multiplicity
    {
        return Err("boundary descriptor result differs from its declaration");
    }
    let mut inventory = kind
        .owned_slots(types, result.place, result.structural_type)
        .ok_or("boundary result has no complete descriptor inventory")?;
    inventory.sort_unstable();
    let mut slots = Vec::with_capacity(inventory.len());
    for group in groups {
        let dependencies =
            provider_lifetime_debts(types, &boundary.structural_parameters, &group.inputs)?;
        for path in &group.results {
            let descriptor = crate::descriptor_lifetimes::descriptor_location(
                types,
                result.place,
                result.structural_type,
                path,
                None,
            )
            .ok_or("boundary result lifetime has no exact descriptor path")?;
            if inventory.binary_search(&descriptor).is_err() {
                continue;
            }
            slots.push(DescriptorResultSlot {
                path: descriptor.path,
                elements: None,
                origins: vec![DescriptorStoredOrigin::Provider(DescriptorProviderOrigin {
                    boundary: boundary.id,
                    output: DescriptorProviderOutput::Result(path.clone()),
                    invocations: Vec::new(),
                    projection: crate::borrowed_storage::Projection::new(&[]),
                    lifetime_dependencies: dependencies.clone(),
                })],
            });
        }
    }
    slots.sort_unstable_by(|left, right| left.path.cmp(&right.path));
    Ok(slots)
}

pub fn boundary_result_lifetimes<'boundary>(
    types: &[StructuralTypeDeclaration],
    boundary: &'boundary BoundaryMachineDeclaration,
) -> Result<&'boundary [DeclaredResultLifetime], &'static str> {
    // The spatial completion protocol does not carry independent keepalive
    // debts. Neither contract may silently replace the other in its replay.
    if !boundary.result_reference_regions.is_empty() {
        return Err("boundary result needs joint spatial and logical lifetime completion");
    }
    let groups = match boundary.result_lifetimes.as_deref() {
        Some(groups) => groups,
        None => {
            return if matches!(&boundary.result, BoundaryMachineResult::Structural(result)
                if terminal_psi::structural_type_has_borrowed_storage_by(
                    |id| terminal_psi::find_structural_type(types, id), result.structural_type))
            {
                Err("boundary result has an unresolved declared lifetime frontier")
            } else {
                Ok(&[])
            };
        }
    };
    let count = super::validate_result_lifetime_groups(groups)?;
    let BoundaryMachineResult::Structural(result) = &boundary.result else {
        return if groups.is_empty() {
            Ok(groups)
        } else {
            Err("nonstructural boundary result advertises borrowed lifetime leaves")
        };
    };
    let leaves = crate::borrowed_result_leaves(
        types.iter(),
        result.structural_type,
        result.multiplicity,
        count,
    )?;
    if leaves.len() != count {
        return Err("boundary lifetime groups do not cover its complete result frontier");
    }
    let mut paths: Vec<_> = groups.iter().flat_map(|group| &group.results).collect();
    paths.sort_unstable();
    if leaves
        .iter()
        .zip(paths)
        .any(|(leaf, path)| leaf.path != *path)
    {
        return Err("boundary result leaf has no unique declared lifetime");
    }
    for input in groups.iter().flat_map(|group| &group.inputs) {
        let (position, access) = match input {
            BoundaryLifetimeDependency::Referent {
                argument_index,
                access,
            }
            | BoundaryLifetimeDependency::Stored {
                argument_index,
                access,
                ..
            } => (*argument_index, *access),
        };
        let parameter = boundary
            .structural_parameters
            .get(
                usize::try_from(position)
                    .map_err(|_| "boundary lifetime input exceeds host positions")?,
            )
            .filter(|parameter| parameter.position == position)
            .ok_or("boundary lifetime input names no exact structural formal")?;
        match input {
            BoundaryLifetimeDependency::Referent { .. } => {
                if access == StructuralAccess::Owned || parameter.access != access {
                    return Err("boundary lifetime anchor differs from its borrowed formal");
                }
            }
            BoundaryLifetimeDependency::Stored { path, .. } => {
                if parameter.access == StructuralAccess::WriteOnlyBorrow {
                    return Err("boundary lifetime anchor reads write-only storage");
                }
                let selected = crate::runtime_structural_path_tip(
                    types.iter(),
                    parameter.structural_type,
                    path,
                )
                .ok_or("boundary lifetime anchor has no exact stored descriptor")?;
                let declared = match selected.shape {
                    StructuralTypeShape::Reference { access, .. } => Some(access),
                    _ => crate::descriptor_results::descriptor_value_access(
                        types.iter(),
                        selected.id,
                        parameter.multiplicity,
                    ),
                };
                if declared != Some(access) {
                    return Err("boundary stored lifetime anchor changes descriptor access");
                }
            }
        }
    }
    Ok(groups)
}

/// Component modules number identities independently. Compare the declared
/// contract through its actual field/case identities, without changing either
/// module's storage paths or confusing a lifetime anchor with backing.
pub fn boundary_result_lifetimes_match(
    left: &terminal_psi::TerminalModule,
    left_boundary: &BoundaryMachineDeclaration,
    right: &terminal_psi::TerminalModule,
    right_boundary: &BoundaryMachineDeclaration,
) -> Result<bool, &'static str> {
    type Path<'a> = Vec<(u8, &'a str, u64)>;
    type Input<'a> = (u8, u32, StructuralAccess, Path<'a>);
    type Group<'a> = (Vec<Input<'a>>, Vec<Path<'a>>);

    fn path<'a>(
        module: &'a terminal_psi::TerminalModule,
        root: semantic_vocabulary::StructuralTypeId,
        path: &[terminal_psi::StructuralPathSegment],
    ) -> Result<Path<'a>, &'static str> {
        use terminal_psi::StructuralPathSegment;
        let mut normalized = Vec::with_capacity(path.len());
        for (position, segment) in path.iter().enumerate() {
            let (tag, identity, index) = match segment {
                StructuralPathSegment::Field(identity) => (0, Some(*identity), 0),
                StructuralPathSegment::Case(case) => {
                    let selected = crate::runtime_structural_path_tip(
                        module.structural_types.iter(),
                        root,
                        &path[..position],
                    )
                    .ok_or("boundary lifetime case has no containing storage type")?;
                    let identity = selected
                        .shape
                        .sum_cases()
                        .and_then(|cases| cases.iter().find(|selected| selected.id == *case))
                        .map(|case| case.identity)
                        .ok_or("boundary lifetime case has no exact declaration")?;
                    (1, Some(identity), 0)
                }
                StructuralPathSegment::FixedIndex(index) => (2, None, *index),
                StructuralPathSegment::Referent => (3, None, 0),
                StructuralPathSegment::AllElements => (4, None, 0),
                _ => return Err("boundary lifetime comparison needs canonical static paths"),
            };
            let identity = match identity {
                Some(identity) => module
                    .identities
                    .text(identity)
                    .filter(|identity| !identity.is_empty())
                    .ok_or("boundary lifetime path lost its declaration identity")?,
                None => "",
            };
            normalized.push((tag, identity, index));
        }
        Ok(normalized)
    }

    fn groups<'a>(
        module: &'a terminal_psi::TerminalModule,
        boundary: &BoundaryMachineDeclaration,
    ) -> Result<Vec<Group<'a>>, &'static str> {
        let declared = boundary_result_lifetimes(&module.structural_types, boundary)?;
        let mut groups = Vec::with_capacity(declared.len());
        for group in declared {
            let mut inputs = Vec::with_capacity(group.inputs.len());
            for input in &group.inputs {
                inputs.push(match input {
                    BoundaryLifetimeDependency::Referent {
                        argument_index,
                        access,
                    } => (0, *argument_index, *access, Vec::new()),
                    BoundaryLifetimeDependency::Stored {
                        argument_index,
                        path: stored,
                        access,
                    } => {
                        let parameter = boundary
                            .structural_parameters
                            .get(*argument_index as usize)
                            .ok_or("boundary lifetime comparison lost its input")?;
                        (
                            1,
                            *argument_index,
                            *access,
                            path(module, parameter.structural_type, stored)?,
                        )
                    }
                });
            }
            let mut results = Vec::with_capacity(group.results.len());
            for result in &group.results {
                let BoundaryMachineResult::Structural(root) = &boundary.result else {
                    return Err("boundary lifetime comparison lost its structural result");
                };
                results.push(path(module, root.structural_type, result)?);
            }
            inputs.sort_unstable();
            results.sort_unstable();
            groups.push((inputs, results));
        }
        groups.sort_unstable();
        Ok(groups)
    }
    Ok(groups(left, left_boundary)? == groups(right, right_boundary)?)
}

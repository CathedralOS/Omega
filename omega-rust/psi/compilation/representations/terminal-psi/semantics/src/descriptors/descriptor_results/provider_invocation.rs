//! Instantiate an admitted boundary's normal-completion reference contract.
//! Arguments retain their precall contents queries. Replay captures lifetime
//! debts from that snapshot and installs output slots simultaneously; no output
//! receives an input's storage identity merely because their lifetimes agree.
//! Returned values remain on the ordinary result-origin route; output replay
//! must not intercept their lookup when a call also replaces a parameter slot.
use super::*;
use crate::descriptors::descriptor_lifetimes::descriptor_location;
use terminal_psi::{BoundaryLifetimeDependency, BoundaryReferenceDestination};

/// Every writable stored reference or view needs an explicit completion promise.
/// Absence is unknown, not preservation of the incoming pointer. This is used
/// at external decoding and by the independent native audit, not between
/// producer stages of an ordinary compilation.
pub fn provider_outputs_cover_writable_slots(
    types: &[StructuralTypeDeclaration],
    declaration: &terminal_psi::BoundaryMachineDeclaration,
) -> bool {
    declaration
        .structural_parameters
        .iter()
        .enumerate()
        .all(|(position, parameter)| {
            let mut slots = Vec::new();
            for kind in [DescriptorSlotKind::References, DescriptorSlotKind::Views] {
                let Some(writable) = kind.writable_slots(
                    types,
                    parameter.place,
                    parameter.structural_type,
                    parameter.access,
                ) else {
                    return false;
                };
                slots.extend(writable);
            }
            let outputs = declaration
                .reference_outputs
                .iter()
                .filter(|output| {
                    output.destination == BoundaryReferenceDestination::Parameter(position as u32)
                })
                .collect::<Vec<_>>();
            slots.len() == outputs.len()
                && slots.iter().all(|slot| {
                    outputs
                        .iter()
                        .filter(|output| {
                            descriptor_location(
                                types,
                                parameter.place,
                                parameter.structural_type,
                                &output.path,
                                None,
                            )
                            .as_ref()
                                == Some(slot)
                        })
                        .count()
                        == 1
                })
        })
}

pub fn provider_invocation(
    operation: semantic_vocabulary::OperationId,
    types: &[StructuralTypeDeclaration],
    slot_kind: DescriptorSlotKind,
    declaration: &terminal_psi::BoundaryMachineDeclaration,
    actuals: &[StructuralArgument],
    result: Option<&terminal_psi::StructuralOperationResult>,
    place_types: &BTreeMap<PlaceId, StructuralTypeId>,
) -> Option<DescriptorInvocation> {
    if actuals.len() != declaration.structural_parameters.len()
        || slot_kind == DescriptorSlotKind::AccessOrigins
    {
        return None;
    }
    let mut arguments = Vec::with_capacity(actuals.len());
    let mut argument_contents = Vec::with_capacity(actuals.len());
    let mut invalidates_contents = false;
    for (actual, formal) in actuals.iter().zip(&declaration.structural_parameters) {
        if actual.access != formal.access {
            return None;
        }
        let root = *place_types.get(&actual.place)?;
        arguments.push(descriptor_location(
            types,
            actual.place,
            root,
            &actual.path,
            None,
        ));
        argument_contents.push(descriptor_contents_query(
            types,
            actual.place,
            root,
            &actual.path,
            None,
        ));
        invalidates_contents |= slot_kind.may_write_indirect_contents(types, root, actual);
    }
    let mut effects = Vec::new();
    for (output_index, output) in declaration.reference_outputs.iter().enumerate() {
        let (place, root_type) = match output.destination {
            BoundaryReferenceDestination::Parameter(position) => {
                let formal = declaration
                    .structural_parameters
                    .get(usize::try_from(position).ok()?)?;
                (formal.place, formal.structural_type)
            }
        };
        let location = descriptor_location(types, place, root_type, &output.path, None)?;
        if !slot_kind
            .owned_slots(types, place, root_type)?
            .contains(&location)
        {
            continue;
        }
        let mut lifetime_dependencies = Vec::new();
        for dependency in &output.dependencies {
            let (origin, access) = match dependency {
                BoundaryLifetimeDependency::Referent {
                    argument_index,
                    access,
                } => (
                    DescriptorStoredOrigin::Parameter {
                        position: usize::try_from(*argument_index).ok()?,
                        projection: Projection::new(&[]),
                    },
                    *access,
                ),
                BoundaryLifetimeDependency::Stored {
                    argument_index,
                    path,
                    access,
                } => {
                    let position = usize::try_from(*argument_index).ok()?;
                    let formal = declaration.structural_parameters.get(position)?;
                    let source = descriptor_location(
                        types,
                        formal.place,
                        formal.structural_type,
                        path,
                        None,
                    )?;
                    (
                        DescriptorStoredOrigin::IncomingSlot {
                            position,
                            path: source.path,
                        },
                        *access,
                    )
                }
            };
            lifetime_dependencies.push(DescriptorLifetimeDebt { origin, access });
        }
        let origins = vec![DescriptorStoredOrigin::Provider(DescriptorProviderOrigin {
            boundary: declaration.id,
            output: u32::try_from(output_index).ok()?,
            invocations: Vec::new(),
            projection: Projection::new(&[]),
            lifetime_dependencies,
            forwarded_sources: None,
        })];
        match output.destination {
            BoundaryReferenceDestination::Parameter(position) => {
                effects.push(DescriptorStoredEffect {
                    parameter: usize::try_from(position).ok()?,
                    steps: location
                        .path
                        .into_iter()
                        .map(DescriptorContentsStep::Address)
                        .collect(),
                    origins,
                })
            }
        }
    }
    let results = if declaration.result_reference_regions.is_empty() {
        Vec::new()
    } else {
        super::public_result_slots(types, slot_kind, declaration, result?.place)?
    };
    let result = (!results.is_empty())
        .then(|| crate::descriptor_lifetimes::DescriptorLocation::whole(result.unwrap().place));
    Some(DescriptorInvocation {
        operation,
        target: DescriptorInvocationTarget::Provider {
            results,
            effects,
            invalidates_contents,
        },
        arguments,
        argument_contents,
        // Parameter writes and returned regions share the captured precall
        // tuple. Calls without a returned-region contract keep ordinary lookup.
        result,
    })
}

/// Capture a requested choice of reference values independently of lifetime
/// debts. These are the caller's inputs before simultaneous output updates.
pub(super) fn bind_forwarding_sources(
    types: &[StructuralTypeDeclaration],
    caller: MachineId,
    declaration: &terminal_psi::BoundaryMachineDeclaration,
    requirements: &[terminal_psi::BoundaryReferenceForwardingDemand],
    invocation: &mut DescriptorInvocation,
) -> Option<()> {
    let DescriptorInvocationTarget::Provider { effects, .. } = &mut invocation.target else {
        return None;
    };
    for requirement in requirements
        .iter()
        .filter(|row| row.machine == caller && row.operation == invocation.operation)
    {
        declaration
            .reference_outputs
            .get(usize::try_from(requirement.output).ok()?)?;
        if requirement.sources.is_empty() {
            return None;
        }
        let sources = requirement
            .sources
            .iter()
            .map(|source| {
                Some(match source {
                    terminal_psi::BoundaryReferenceSource::Reference { argument_index } => {
                        let position = usize::try_from(*argument_index).ok()?;
                        let formal = declaration.structural_parameters.get(position)?;
                        if !matches!(
                            formal.access,
                            StructuralAccess::SharedBorrow
                                | StructuralAccess::MutableBorrow
                                | StructuralAccess::WriteOnlyBorrow
                        ) {
                            return None;
                        }
                        DescriptorForwardingSource::ReferenceArgument { position }
                    }
                    terminal_psi::BoundaryReferenceSource::Stored {
                        argument_index,
                        path,
                    } => {
                        let position = usize::try_from(*argument_index).ok()?;
                        let formal = declaration.structural_parameters.get(position)?;
                        let source = descriptor_location(
                            types,
                            formal.place,
                            formal.structural_type,
                            path,
                            None,
                        )?;
                        let carrier =
                            crate::descriptors::descriptor_lifetimes::descriptor_location_type(
                                types,
                                formal.structural_type,
                                &source.path,
                            )?;
                        if !matches!(
                            terminal_psi::find_structural_type(types, carrier)?.shape,
                            StructuralTypeShape::Reference { .. }
                        ) {
                            return None;
                        }
                        DescriptorForwardingSource::Value(DescriptorStoredOrigin::IncomingSlot {
                            position,
                            path: source.path,
                        })
                    }
                })
            })
            .collect::<Option<Vec<_>>>()?;
        let mut selected = effects
            .iter_mut()
            .flat_map(|effect| &mut effect.origins)
            .filter_map(|origin| match origin {
                DescriptorStoredOrigin::Provider(provider)
                    if provider.output == requirement.output =>
                {
                    Some(provider)
                }
                _ => None,
            });
        let provider = selected.next()?;
        if selected.next().is_some() || provider.forwarded_sources.is_some() {
            return None;
        }
        provider.forwarded_sources = Some(sources);
    }
    Some(())
}

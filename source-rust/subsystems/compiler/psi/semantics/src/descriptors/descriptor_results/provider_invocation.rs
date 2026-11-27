use super::*;
use crate::descriptors::descriptor_lifetimes::descriptor_location;
use terminal_psi::BoundaryReferenceDestination;

pub(crate) fn provider_outputs_cover_writable_slots(
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
                    output.destination
                        == BoundaryReferenceDestination::Parameter(position as u32)
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
        let lifetime_dependencies = crate::references::provider_lifetime_debts(
            types,
            &declaration.structural_parameters,
            &output.dependencies,
        )
        .ok()?;
        let origins = vec![DescriptorStoredOrigin::Provider(DescriptorProviderOrigin {
            boundary: declaration.id,
            output: DescriptorProviderOutput::Stored(u32::try_from(output_index).ok()?),
            invocations: Vec::new(),
            projection: Projection::new(&[]),
            lifetime_dependencies,
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
    let results = if !declaration.result_reference_regions.is_empty() {
        super::public_result_slots(types, slot_kind, declaration, result?.place)?
    } else if let Some(result) = result {
        crate::boundary_result_slots(types, declaration, slot_kind, result).ok()?
    } else {
        Vec::new()
    };
    let result = (!results.is_empty()).then(|| {
        crate::descriptor_lifetimes::DescriptorLocation::whole(result.unwrap().place)
    });
    Some(DescriptorInvocation {
        scalar_arguments: Vec::new(),
        owner: DescriptorInvocationOwner::Operation(operation),
        target: DescriptorInvocationTarget::Provider {
            results,
            effects,
            invalidates_contents,
        },
        arguments,
        argument_contents,
        result,
    })
}

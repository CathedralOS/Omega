//! Instantiate an admitted boundary's normal-completion reference contract.
//! Arguments retain their precall contents queries. Replay captures lifetime
//! debts from that snapshot and installs output slots simultaneously; no output
//! receives an input's storage identity merely because their lifetimes agree.
//! Returned values remain on the ordinary result-origin route; output replay
//! must not intercept their lookup when a call also replaces a parameter slot.
use super::*;
use crate::descriptor_lifetimes::descriptor_location;
use terminal_psi::{BoundaryLifetimeDependency, BoundaryReferenceDestination};

/// Every writable stored reference needs an explicit completion promise.
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
            let Some(slots) = writable_reference_slots(
                types,
                parameter.place,
                parameter.structural_type,
                parameter.access,
            ) else {
                return false;
            };
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
    place_types: &BTreeMap<PlaceId, StructuralTypeId>,
) -> Option<DescriptorInvocation> {
    if actuals.len() != declaration.structural_parameters.len()
        || slot_kind == DescriptorSlotKind::PlacedOrigins
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
    Some(DescriptorInvocation {
        operation,
        target: DescriptorInvocationTarget::Provider {
            results: Vec::new(),
            effects,
            invalidates_contents,
        },
        arguments,
        argument_contents,
        // Returned values keep the ordinary declared result provenance.
        // This invocation contributes only simultaneous parameter writes;
        // intercepting result lookup here would erase its independent origin.
        result: None,
    })
}

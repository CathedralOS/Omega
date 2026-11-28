//! An execution owner keeps both its input loan obligations and the identity
//! of its caller-supplied stack. Only a returning step releases that stack.
use super::*;
use crate::calls::owned_activation::{self, PreparationShape, ResumeShape, StepShape};
use crate::descriptors::descriptor_lifetimes::descriptor_location;
use language_core::sealed_intrinsics::ActivationOperation;
use semantic_vocabulary::{BoundaryMachineId, StructuralCaseId, StructuralFieldId};
use terminal_psi::{BoundaryMachineDeclaration, BoundaryMachineResult};

enum Transfer {
    Prepare(PreparationShape),
    Step(StepShape),
    Resume(ResumeShape),
}

fn field(case: StructuralCaseId, member: StructuralFieldId) -> Vec<DescriptorPathSegment> {
    vec![
        DescriptorPathSegment::Case(case),
        DescriptorPathSegment::Field(member),
    ]
}

fn backing_from_owner(preparation: BoundaryMachineId) -> DescriptorStoredOrigin {
    DescriptorStoredOrigin::OwnerBacking(replay::OwnerBacking {
        preparation,
        owner: Box::new(DescriptorStoredOrigin::IncomingSlot {
            position: 0,
            path: vec![DescriptorPathSegment::UnknownContents],
        }),
        projection: Projection::new(&[]),
    })
}

pub(super) fn invocation(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
    boundary: &BoundaryMachineDeclaration,
    control: ActivationOperation,
    kind: DescriptorSlotKind,
) -> Option<DescriptorInvocation> {
    let transfer = match control {
        ActivationOperation::Prepare => {
            Transfer::Prepare(owned_activation::prepare_shape(module, boundary).ok()?)
        }
        ActivationOperation::StepFixed => Transfer::Step(
            owned_activation::step_shape(
                module,
                boundary,
                owned_activation::preparation_for(module, boundary).ok()?,
            )
            .ok()?,
        ),
        ActivationOperation::Resume => Transfer::Resume(
            owned_activation::resume_shape(
                module,
                boundary,
                owned_activation::preparation_for(module, boundary).ok()?,
            )
            .ok()?,
        ),
        ActivationOperation::Park => return None,
    };
    let terminal_psi::OperationResult::Structural(out) = &operation.result else {
        return None;
    };
    let BoundaryMachineResult::Structural(expected) = &boundary.result else {
        return None;
    };
    let OperationKind::BoundaryCall {
        structural_arguments,
        ..
    } = &operation.kind
    else {
        return None;
    };
    let formals = &boundary.structural_parameters;
    let (result, next, owner_type, preparation) = match &transfer {
        Transfer::Prepare(s) => (
            s.result,
            field(s.ready, s.owner),
            s.owner_type,
            s.selected.preparation,
        ),
        Transfer::Step(s) => (
            s.result,
            field(s.parked, s.owner),
            s.suspended_owner,
            s.selected.preparation,
        ),
        Transfer::Resume(s) => (
            s.result,
            field(s.ready, s.ready_field),
            s.ready_owner,
            s.selected.preparation,
        ),
    };
    if out.structural_type != result
        || out.multiplicity != expected.multiplicity
        || out.qualifications != expected.qualifications
        || out.projected_qualifications != expected.projected_qualifications
        || structural_arguments.len() != formals.len()
    {
        return None;
    }
    let types = &module.types.structural_types;
    let owner = DescriptorInvocationOwner::Operation(operation.id);
    let mut arguments = Vec::with_capacity(formals.len());
    let mut argument_contents = Vec::with_capacity(formals.len());
    let mut results = Vec::new();
    let mut contents = Vec::new();
    for (position, (actual, formal)) in structural_arguments.iter().zip(formals).enumerate() {
        let root = machine.structural_place_type(actual.place)?;
        let tip = crate::runtime_structural_path_tip(types.iter(), root, &actual.path)?;
        if formal.structural_type != tip.id || actual.access != formal.access {
            return None;
        }
        arguments.push(Some(descriptor_location(
            types,
            actual.place,
            root,
            &actual.path,
            None,
        )?));
        argument_contents.push(Some(descriptor_contents_query(
            types,
            actual.place,
            root,
            &actual.path,
            None,
        )?));
        let path = match (&transfer, formal.position) {
            (Transfer::Prepare(s), 0) => field(s.rejected, s.arguments),
            (Transfer::Prepare(s), 1) => field(s.rejected, s.retained),
            (Transfer::Resume(s), 0) => field(s.mismatch, s.mismatch_owner),
            (Transfer::Resume(s), 1) => field(s.mismatch, s.mismatch_permit),
            (Transfer::Resume(s), 2) => field(s.mismatch, s.mismatch_value),
            (Transfer::Step(_), 0) => continue,
            _ => return None,
        };
        let forward = |tail: Vec<DescriptorPathSegment>| {
            let mut destination = path.clone();
            destination.extend_from_slice(&tail);
            DescriptorResultSlot {
                path: destination,
                origins: vec![DescriptorStoredOrigin::IncomingSlot {
                    position,
                    path: tail,
                }],
            }
        };
        let inputs = kind.owned_inventory(types, formal.place, formal.structural_type)?;
        results.extend(inputs.slots.into_iter().map(|s| forward(s.path)));
        contents.extend(
            inputs
                .unknown_contents
                .into_iter()
                .map(|s| forward(s.location.path)),
        );
    }

    if let Transfer::Step(s) = &transfer {
        let returned_stack = field(s.returned, s.retained);
        let returned_value = field(s.returned, s.value);
        let inventory = kind.owned_inventory(types, out.place, result)?;
        for slot in inventory.slots {
            if slot.path == returned_stack {
                results.push(DescriptorResultSlot {
                    path: slot.path,
                    origins: vec![backing_from_owner(preparation)],
                });
            } else if slot.path.starts_with(&returned_value) {
                // A worker returning a reference needs its own checked result
                // summary applied to the original arguments at preparation.
                return None;
            }
        }
        if inventory
            .unknown_contents
            .iter()
            .any(|s| s.location.path.starts_with(&returned_value))
        {
            return None;
        }
    }
    let stack = if kind.tracks_views() {
        let origin = match transfer {
            Transfer::Prepare(_) => DescriptorStoredOrigin::IncomingSlot {
                position: formals.iter().position(|formal| formal.position == 1)?,
                path: Vec::new(),
            },
            _ => backing_from_owner(preparation),
        };
        Some(replay::ActivationBacking {
            preparation,
            sources: vec![origin],
        })
    } else {
        None
    };
    let mut hidden = next;
    hidden.push(DescriptorPathSegment::UnknownContents);
    let packet = replay::ProducedContents {
        producer: DescriptorInvocationSite {
            machine: machine.id,
            owner,
        },
        path: hidden.clone(),
        structural_type: owner_type,
        activation_backing: stack,
        lifetime_dependencies: replay::retained_parameter_debts(
            types,
            formals.iter().enumerate().filter(|(_, formal)| {
                // A successful reply consumes the answer permit. Its slot's loan
                // lives on mismatch, whose result above preserves that exact permit.
                !matches!(transfer, Transfer::Resume(_)) || formal.position != 1
            }),
        )?,
    };
    contents.push(DescriptorResultSlot {
        path: hidden,
        origins: vec![DescriptorStoredOrigin::ProducedContents(packet)],
    });
    Some(DescriptorInvocation {
        owner,
        arguments,
        argument_contents,
        native_address_arguments: Vec::new(),
        scalar_arguments: Vec::new(),
        target: DescriptorInvocationTarget::Public {
            effects_machine: None,
            realizations: Vec::new(),
            extent: semantic_vocabulary::ReferenceOriginExtent::Projected,
            results,
            contents,
            invalidates_contents: false,
        },
        result: Some(DescriptorLocation::whole(out.place)),
    })
}

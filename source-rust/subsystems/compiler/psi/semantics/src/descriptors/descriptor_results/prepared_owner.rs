//! An execution owner keeps both its input loan obligations and the identity
//! of its caller-supplied stack. Only a returning step releases that stack.
use super::*;
use crate::calls::owned_activation::{self, PreparationShape, ResumeShape, StepShape};
use crate::descriptors::descriptor_lifetimes::descriptor_location;
use language_core::sealed_intrinsics::ActivationOperation;
use semantic_vocabulary::{BoundaryMachineId, StructuralCaseId, StructuralFieldId};
use terminal_psi::{BoundaryMachineDeclaration, BoundaryMachineResult, TerminalMachineResult};

enum Transfer {
    Prepare(PreparationShape),
    Step(StepShape),
    Resume(ResumeShape),
}

impl Transfer {
    fn checked(
        module: &TerminalModule,
        boundary: &BoundaryMachineDeclaration,
        control: ActivationOperation,
    ) -> Result<Self, &'static str> {
        match control {
            ActivationOperation::Prepare => {
                owned_activation::prepare_shape(module, boundary).map(Self::Prepare)
            }
            ActivationOperation::StepFixed => owned_activation::step_shape(
                module,
                boundary,
                owned_activation::preparation_for(module, boundary)?,
            )
            .map(Self::Step),
            ActivationOperation::Resume => owned_activation::resume_shape(
                module,
                boundary,
                owned_activation::preparation_for(module, boundary)?,
            )
            .map(Self::Resume),
            ActivationOperation::Park => Err("park does not return an execution owner"),
        }
    }
}

fn field(case: StructuralCaseId, member: StructuralFieldId) -> Vec<DescriptorPathSegment> {
    vec![
        DescriptorPathSegment::Case(case),
        DescriptorPathSegment::Field(member),
    ]
}

fn backing_from_owner(
    preparation: BoundaryMachineId,
    worker_result: Option<Vec<DescriptorPathSegment>>,
) -> DescriptorStoredOrigin {
    DescriptorStoredOrigin::OwnerBacking(replay::OwnerBacking {
        preparation,
        worker_result,
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
    failure_detail: &mut Option<&'static str>,
    step: &mut crate::descriptor_lifetimes::DescriptorFlowStep,
) -> Option<DescriptorInvocation> {
    let transfer = Transfer::checked(module, boundary, control)
        .inspect_err(|detail| *failure_detail = Some(*detail))
        .ok()?;
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
    let structural_positions =
        boundary
            .parameter_order
            .iter()
            .enumerate()
            .filter_map(|(position, kind)| {
                (*kind == terminal_psi::BoundaryParameterKind::Structural).then_some(position)
            });
    if structural_positions.clone().count() != formals.len() {
        return None;
    }
    for (position, ((actual, formal), source_position)) in structural_arguments
        .iter()
        .zip(formals)
        .zip(structural_positions)
        .enumerate()
    {
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
        let path = match (&transfer, source_position) {
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

    let selected = match &transfer {
        Transfer::Prepare(s) => s.selected,
        Transfer::Step(s) => s.selected,
        Transfer::Resume(s) => s.selected,
    };
    use crate::descriptor_lifetimes::ActivationAccess;
    let access = match &transfer {
        Transfer::Prepare(_) => {
            // Its exclusive byte view funds the private stack. The runtime can
            // save and restore frames there for the lifetime of this owner.
            let stack = arguments.last()?.as_ref()?.clone();
            step.inputs.push((
                Some(stack.clone()),
                Some((
                    access_vocabulary::AccessOperation::CompoundMutation,
                    stack.into(),
                )),
            ));
            ActivationAccess::Prepare(selected.entry_machine)
        }
        Transfer::Step(_) => ActivationAccess::Step(selected.entry_machine),
        Transfer::Resume(shape) => {
            // Compare the address in the permit with the parked request. Neither
            // the referent nor the pointer in the publication is overwritten.
            let mut anchor = arguments.get(1)?.as_ref()?.clone();
            anchor
                .path
                .push(DescriptorPathSegment::Field(shape.permit.anchor));
            step.inputs.push((
                Some(anchor.clone()),
                Some((access_vocabulary::AccessOperation::Read, anchor.into())),
            ));
            ActivationAccess::Resume
        }
    };
    step.activation_access = Some((operation.id, access));
    let worker = module.machine(selected.entry_machine)?;
    let mut worker_result = BTreeMap::new();
    if let TerminalMachineResult::Structural(value) = &worker.result {
        let inventory = kind.owned_inventory(types, value.place, value.structural_type)?;
        let paths = inventory.slots.into_iter().map(|slot| slot.path).chain(
            inventory
                .unknown_contents
                .into_iter()
                .map(|slot| slot.location.path),
        );
        for path in paths {
            let origins = match transfer {
                Transfer::Prepare(_) => Vec::new(),
                _ => vec![backing_from_owner(preparation, Some(path.clone()))],
            };
            worker_result.insert(path, origins);
        }
    }
    let bind_worker_result = matches!(transfer, Transfer::Prepare(_)) && !worker_result.is_empty();
    if let Transfer::Step(s) = &transfer {
        let returned_stack = field(s.returned, s.retained);
        let returned_value = field(s.returned, s.value);
        let inventory = kind.owned_inventory(types, out.place, result)?;
        let result_slot = |path: Vec<DescriptorPathSegment>| -> Option<DescriptorResultSlot> {
            let projected = path.strip_prefix(returned_value.as_slice())?;
            let origins = worker_result.get(projected)?.clone();
            Some(DescriptorResultSlot { path, origins })
        };
        for slot in inventory.slots {
            if slot.path == returned_stack {
                results.push(DescriptorResultSlot {
                    path: slot.path,
                    origins: vec![backing_from_owner(preparation, None)],
                });
            } else if slot.path.starts_with(&returned_value) {
                results.push(result_slot(slot.path)?);
            }
        }
        for slot in inventory.unknown_contents {
            if slot.location.path.starts_with(&returned_value) {
                contents.push(result_slot(slot.location.path)?);
            }
        }
    }
    let mut stack_sources = Vec::new();
    if kind.tracks_views() {
        stack_sources.push(match transfer {
            Transfer::Prepare(_) => DescriptorStoredOrigin::IncomingSlot {
                position: formals.len().checked_sub(1)?,
                path: Vec::new(),
            },
            _ => backing_from_owner(preparation, None),
        });
    }
    let stack = (!stack_sources.is_empty() || !worker_result.is_empty()).then_some(
        replay::ActivationBacking {
            preparation,
            sources: stack_sources,
            worker_result,
        },
    );
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
        target: if bind_worker_result {
            DescriptorInvocationTarget::PrepareWorker {
                worker: selected.entry_machine,
                results,
                contents,
            }
        } else {
            DescriptorInvocationTarget::Public {
                effects_machine: None,
                realizations: Vec::new(),
                extent: semantic_vocabulary::ReferenceOriginExtent::Projected,
                results,
                contents,
                invalidates_contents: false,
            }
        },
        result: Some(DescriptorLocation::whole(out.place)),
    })
}

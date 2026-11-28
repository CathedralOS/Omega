//! Preparation has no producer body to replay: it either returns each
//! input unchanged or moves its obligations into an opaque execution owner.
use super::*;
use crate::calls::owned_activation::PreparationShape;
use crate::descriptors::descriptor_lifetimes::descriptor_location;
use terminal_psi::{BoundaryMachineDeclaration, BoundaryMachineResult};

pub(super) fn invocation(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
    boundary: &BoundaryMachineDeclaration,
    shape: PreparationShape,
    kind: DescriptorSlotKind,
) -> Option<DescriptorInvocation> {
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
    if out.structural_type != shape.result
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
        let field = match formal.position {
            0 => shape.arguments,
            1 => shape.retained,
            _ => return None,
        };
        let rejected = |path: Vec<DescriptorPathSegment>| {
            let mut destination = vec![
                DescriptorPathSegment::Case(shape.rejected),
                DescriptorPathSegment::Field(field),
            ];
            destination.extend_from_slice(&path);
            DescriptorResultSlot {
                path: destination,
                origins: vec![DescriptorStoredOrigin::IncomingSlot { position, path }],
            }
        };
        let inputs = kind.owned_inventory(types, formal.place, formal.structural_type)?;
        results.extend(inputs.slots.into_iter().map(|s| rejected(s.path)));
        contents.extend(
            inputs
                .unknown_contents
                .into_iter()
                .map(|s| rejected(s.location.path)),
        );
    }
    // The successful owner has an identity and loan dependencies, but its
    // hidden contents are not an address through which a caller can write.
    let hidden = vec![
        DescriptorPathSegment::Case(shape.ready),
        DescriptorPathSegment::Field(shape.owner),
        DescriptorPathSegment::UnknownContents,
    ];
    let packet = replay::ProducedContents {
        producer: DescriptorInvocationSite {
            machine: machine.id,
            owner,
        },
        path: hidden.clone(),
        structural_type: shape.owner_type,
        lifetime_dependencies: replay::retained_input_debts(types, formals)?,
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
        // Unlike a general requirement, rejection returns the original values,
        // not other references somewhere within their declared lifetimes.
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

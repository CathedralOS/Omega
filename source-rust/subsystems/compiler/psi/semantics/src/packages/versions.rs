//! Establish whether the same package storage is still live between two uses.
use super::*;

// A saved descriptor may be reused across blocks or intervening writes when
// every incoming path retains this package's storage. Reuse the descriptor
// replay that already accounts for field overlap, call write frames and loans.
pub(super) fn same_storage_version(
    module: &TerminalModule,
    machine: &TerminalMachine,
    opened: &TerminalPackageOpening,
    projection: OperationId,
    call: OperationId,
    materialized: Option<semantic_vocabulary::PlaceId>,
) -> bool {
    use crate::descriptors::descriptor_lifetimes::descriptor_location;
    use crate::descriptors::descriptor_results::payload_versions::require_preserved_observed_storage;
    use crate::descriptors::descriptor_results::{
        DescriptorPayloadTarget,
        terminal_borrowed_storage_graph_from,
    };
    let position = |id| {
        machine.blocks.iter().find_map(|block| {
            let index = block.operations.iter().position(|op| op.id == id)?;
            Some((block.id, u32::try_from(index).ok()?))
        })
    };
    let (Some(read), Some(use_site)) = (position(projection), position(call)) else {
        return false;
    };
    if read.0 == use_site.0 && read.1 < use_site.1 {
        let Some(block) = machine.block(&read.0) else {
            return false;
        };
        let between = &block.operations[read.1 as usize + 1..use_site.1 as usize];
        if between.iter().all(|operation| match &operation.kind {
            OperationKind::PrimitiveScalarRead { .. }
            | OperationKind::BooleanConstant { .. }
            | OperationKind::IntegerConstant { .. } => true,
            // Calling the stored machine on a fresh reborrow can mutate its
            // pointee, while neither stored field was given to the callee
            // for replacement. Other effects still require ordinary replay.
            OperationKind::CallMachine {
                contract,
                structural_arguments,
                ..
            } => module
                .execution
                .callables
                .contract(*contract)
                .is_some_and(|contract| {
                    let sites = &module.execution.packages.sites;
                    let same_opening = sites
                        .binary_search_by_key(&(machine.id, operation.id), |s| {
                            (s.machine, s.operation)
                        })
                        .ok()
                        .and_then(|index| match sites[index].event {
                            TerminalPackageEvent::Invoke { opening: index } => {
                                opening(module, machine.id, index).ok()
                            }
                            _ => None,
                        })
                        == Some(opened);
                    same_opening
                        && contract
                            .structural_parameters
                            .iter()
                            .any(|formal| opened.witnesses.contains(&formal.structural_type))
                        && contract.envelope.service_reach.is_empty()
                        && contract.envelope.invocations.is_empty()
                        && !contract.envelope.suspends
                        && !contract.envelope.blocks
                        && structural_arguments.len() == contract.structural_parameters.len()
                        && structural_arguments
                            .iter()
                            .zip(&contract.structural_parameters)
                            .all(|(arg, formal)| {
                                arg.access != StructuralAccess::Owned
                                    && formal.access == arg.access
                                    && if opened.witnesses.contains(&formal.structural_type) {
                                        opening::subject_field(module, opened, arg).is_some()
                                    } else {
                                        separate_primitive_input(
                                            module,
                                            machine,
                                            opened,
                                            materialized,
                                            arg,
                                            formal.structural_type,
                                        )
                                    }
                            })
                }),
            _ => false,
        }) {
            return true;
        }
    }
    let Ok(graph) = terminal_borrowed_storage_graph_from(module, [machine.id], &[]) else {
        return false;
    };
    let preserved = |place, path: &[StructuralPathSegment], witness: bool| {
        let Some(root) = machine.structural_place_type(place) else {
            return false;
        };
        let Some(location) =
            descriptor_location(&module.types.structural_types, place, root, path, None)
        else {
            return false;
        };
        require_preserved_observed_storage(
            &module.types.structural_types,
            &graph,
            machine.id,
            read,
            use_site,
            &DescriptorPayloadTarget::current(location),
            |(block, position)| {
                witness
                    && machine
                        .block(&block)
                        .and_then(|block| block.operations.get(position as usize))
                        .is_some_and(|operation| {
                            preserves_package_witness(module, machine, opened, operation)
                        })
            },
        )
        .is_ok()
    };
    // Field replacement preserves the package association, while an independently
    // materialized reference must also survive as its own unchanged storage.
    preserved(opened.subject.place, &opened.subject.path, true)
        && materialized.is_none_or(|place| preserved(place, &[], false))
}

/// Updating one member keeps the witness. Whole-package writes, indirect selectors
/// and changes of the separately materialized carrier still require ordinary replay.
fn preserves_package_witness(
    module: &TerminalModule,
    machine: &TerminalMachine,
    opened: &TerminalPackageOpening,
    operation: &Operation,
) -> bool {
    if !matches!(
        operation.kind,
        OperationKind::StructuralScalarFieldStore { .. }
            | OperationKind::StoreStructuralField { .. }
    ) {
        return false;
    }
    if !opened.subject.path.iter().all(|step| {
        matches!(
            step,
            StructuralPathSegment::Field(_)
                | StructuralPathSegment::Case(_)
                | StructuralPathSegment::FixedIndex(_)
        )
    }) {
        return false;
    }
    let sites = &module.execution.packages.sites;
    if let Ok(index) =
        sites.binary_search_by_key(&(machine.id, operation.id), |s| (s.machine, s.operation))
    {
        // Every Store in the returned module must independently prove its exact
        // destination, source and witness. Whole-package replacements cannot
        // supply this receipt, even when they write another value of the same type.
        return matches!(sites[index].event,
            TerminalPackageEvent::Store { opening: index, .. }
                if opening(module, machine.id, index).ok() == Some(opened));
    }
    let OperationKind::StructuralScalarFieldStore {
        destination,
        path,
        field,
        ..
    } = &operation.kind
    else {
        return false;
    };
    // An ordinary scalar member such as a tag needs no opening evidence.
    *destination == opened.subject.place
        && crate::structural_paths_select_same_location(path, &opened.subject.path)
        && fields(module, opened.record).is_ok_and(|fields| {
            fields.iter().any(|decl| {
                decl.id == *field
                    && matches!(decl.field_type, StructuralFieldType::Scalar(_))
                    && !decl.relevance.is_erased()
                    && !fields::field_needs_opening(module, opened.record, decl)
            })
        })
}

/// A separate scalar or scalar collection can lend its elements without
/// lending storage for this package, its references or its callbacks. A dynamic selector may depend
/// on mutable backing, so its version still needs ordinary storage replay.
fn separate_primitive_input(
    module: &TerminalModule,
    machine: &TerminalMachine,
    opened: &TerminalPackageOpening,
    materialized: Option<semantic_vocabulary::PlaceId>,
    argument: &StructuralArgument,
    formal: StructuralTypeId,
) -> bool {
    argument.path.is_empty()
        && argument.place != opened.subject.place
        && Some(argument.place) != materialized
        && !opened
            .subject
            .path
            .iter()
            .any(|step| matches!(step, StructuralPathSegment::RuntimeIndex { .. }))
        && machine.structural_parameters.iter().any(|parameter| {
            parameter.place == argument.place
                && parameter.structural_type == formal
                && terminal_psi::structural_access_covers(parameter.access, argument.access)
        })
        && primitive_output_storage(module, formal)
}

fn primitive_output_storage(module: &TerminalModule, formal: StructuralTypeId) -> bool {
    let Some(declared) = module.structural_type(formal) else {
        return false;
    };
    let element = match declared.shape {
        StructuralTypeShape::ByteSequence(_) => return true,
        StructuralTypeShape::ElementView { element, .. } => element,
        _ => formal,
    };
    let scalar = match module.structural_type(element).map(|ty| &ty.shape) {
        Some(StructuralTypeShape::PrimitiveScalar(scalar)) => Some(*scalar),
        Some(StructuralTypeShape::FixedArray { .. }) => {
            crate::scalar_array_leaf_shape(module.types.structural_types.iter(), element)
                .map(|(scalar, _)| scalar)
        }
        _ => None,
    };
    match scalar {
        Some(ScalarType::Boolean | ScalarType::IeeeFloat(_)) => true,
        Some(ScalarType::Integer(integer)) => !integer.is_address(),
        Some(ScalarType::Callable(_)) | None => false,
    }
}

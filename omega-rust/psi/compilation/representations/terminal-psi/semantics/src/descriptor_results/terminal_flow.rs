//! Current Terminal operations adapted once for both contents and cohort replay.
use super::*;
use crate::descriptor_lifetimes::{
    DescriptorFlowEdge, descriptor_location, descriptor_location_type,
};

pub(super) fn adapt(
    module: &TerminalModule,
    machine: &TerminalMachine,
    slot_kind: DescriptorSlotKind,
) -> Option<DescriptorMachine> {
    let mut graph = DescriptorMachine {
        id: machine.id,
        returns: machine
            .result
            .structural()
            .filter(|result| {
                slot_kind
                    .slots(
                        &module.structural_types,
                        result.place,
                        result.structural_type,
                    )
                    .is_some_and(|slots| !slots.is_empty())
            })
            .map(|_| Vec::new()),
        places: Vec::new(),
        place_types: BTreeMap::new(),
        entry: (machine.entry, 0),
        flow: BTreeMap::new(),
        originations: BTreeMap::new(),
        projections: BTreeMap::new(),
        contents_queries: BTreeMap::new(),
        contents_transfers: BTreeMap::new(),
        unresolved_contents_writes: BTreeSet::new(),
        storage_derivations: BTreeSet::new(),
        calls: BTreeMap::new(),
        normal_exits: Vec::new(),
    };
    for place in &machine.structural_places {
        if graph.places.iter().any(|row| row.place == place.id) {
            return None;
        }
        graph.places.push(DescriptorPlace {
            place: place.id,
            definition: terminal_definition(module, machine, place.id, place.kind)?,
        });
        if let Some(ty) = place_type(machine, place.id) {
            graph.place_types.insert(place.id, ty);
        }
    }
    let location = |root, path: &[StructuralPathSegment], field| {
        descriptor_location(
            &module.structural_types,
            root,
            *graph.place_types.get(&root)?,
            path,
            field,
        )
    };
    for block in &machine.blocks {
        for (index, operation) in block.operations.iter().enumerate() {
            let position = (block.id, u32::try_from(index).ok()?);
            let mut step = DescriptorFlowStep::default();
            operation.kind.clone().map_structural_uses(&mut |place| {
                step.inputs.push(DescriptorLocation::whole(place));
                place
            });
            let result = operation.result.structural();
            if let Some(result) = result {
                let mut rows = machine
                    .structural_places
                    .iter()
                    .filter(|row| row.id == result.place);
                if rows.next()?.kind
                    != (StructuralPlaceKind::OperationResult {
                        producer: operation.id,
                        structural_type: result.structural_type,
                    })
                    || rows.next().is_some()
                {
                    return None;
                }
                step.definitions
                    .push(DescriptorLocation::whole(result.place));
                if !is_descriptor(module.structural_types.iter(), result.structural_type)
                    && !slot_kind.is_slot(&module.structural_types, result.structural_type)
                {
                    graph.originations.entry(position).or_default().push((
                        DescriptorLocation::whole(result.place),
                        DescriptorCallOrigin::Argument {
                            source: result.place,
                            projection: Projection::new(&[]),
                        },
                    ));
                }
            }
            match &operation.kind {
                OperationKind::ByteSequenceLength { source }
                | OperationKind::ElementViewLength { source } => {
                    let query = descriptor_contents_query(
                        &module.structural_types,
                        source.place,
                        *graph.place_types.get(&source.place)?,
                        &source.path,
                        None,
                    )?;
                    step.inputs = vec![query.observation()];
                }
                OperationKind::EstablishZeroedStructural {} => {
                    let result = result?;
                    for slot in slot_kind
                        .inventory(
                            &module.structural_types,
                            result.place,
                            result.structural_type,
                        )?
                        .slots
                    {
                        graph
                            .originations
                            .entry(position)
                            .or_default()
                            .push((slot, DescriptorCallOrigin::Empty));
                    }
                }
                OperationKind::EstablishByteSequenceLiteral { destination, .. } => {
                    let destination = DescriptorLocation::whole(*destination);
                    step.definitions.push(destination.clone());
                    graph.originations.entry(position).or_default().push((
                        destination,
                        DescriptorCallOrigin::Literal {
                            machine: machine.id,
                            place: *destination_root(&operation.kind)?,
                        },
                    ));
                }
                OperationKind::EstablishPlacedReference { .. }
                    if slot_kind == DescriptorSlotKind::PlacedOrigins =>
                {
                    let result = result?;
                    let StructuralTypeShape::Reference {
                        referent: carrier, ..
                    } = terminal_psi::find_structural_type(
                        &module.structural_types,
                        result.structural_type,
                    )?
                    .shape
                    else {
                        return None;
                    };
                    graph.originations.entry(position).or_default().push((
                        DescriptorLocation::whole(result.place),
                        DescriptorCallOrigin::PlacedFormation {
                            machine: machine.id,
                            operation: operation.id,
                            place: result.place,
                            carrier,
                        },
                    ));
                }
                terminal_psi::element_view_establish_pair_pattern!(source, ..)
                | OperationKind::EstablishPlacedReference { source }
                | OperationKind::EstablishReference { source, .. } => {
                    let destination = DescriptorLocation::whole(result?.place);
                    let (source_location, mut backing_projection) = captured_storage_source(
                        &module.structural_types,
                        source,
                        *graph.place_types.get(&source.place)?,
                        position,
                    )?;
                    step.derivations
                        .push((source_location.clone(), destination.clone()));
                    if slot_kind == DescriptorSlotKind::References
                        && matches!(
                            operation.kind,
                            OperationKind::EstablishReference { .. }
                                | OperationKind::EstablishPlacedReference { .. }
                        )
                    {
                        graph.storage_derivations.insert((
                            position,
                            source_location.clone(),
                            destination.clone(),
                        ));
                    }
                    let preserves_coordinates = match &operation.kind {
                        terminal_psi::element_view_establish_pair_pattern!(
                            source,
                            element,
                            region,
                            ..
                        ) => crate::runtime_structural_path_tip(
                            module.structural_types.iter(),
                            *graph.place_types.get(&source.place)?,
                            &source.path,
                        )
                        .is_some_and(|collection| {
                            crate::collection_view_preserves_coordinates(
                                module.structural_types.iter(),
                                collection.id,
                                *element,
                                *region,
                            )
                        }),
                        _ => true,
                    };
                    if !preserves_coordinates {
                        // Viewed element paths cannot refine differently named backing paths.
                        backing_projection
                            .get_or_insert_with(|| Projection::new(&[]))
                            .forget_address();
                    }
                    if let Some(projection) = backing_projection {
                        graph
                            .projections
                            .insert((position, source_location, destination.clone()), projection);
                    }
                    if let Some(origins) = graph.originations.get_mut(&position) {
                        origins.retain(|(location, _)| location != &destination);
                    }
                }
                OperationKind::ByteSequenceSubslice {
                    source, start, end, ..
                }
                | OperationKind::ElementViewSubslice {
                    source, start, end, ..
                } => {
                    let query = descriptor_contents_query(
                        &module.structural_types,
                        source.place,
                        *graph.place_types.get(&source.place)?,
                        &source.path,
                        None,
                    )?;
                    let source = query.observation();
                    step.inputs = vec![source.clone()];
                    let destination = DescriptorLocation::whole(result?.place);
                    step.derivations.push((source.clone(), destination.clone()));
                    graph
                        .contents_queries
                        .insert((position, source.clone(), destination.clone()), query);
                    graph.projections.insert(
                        (position, source, destination),
                        Projection::window(*start, *end, position),
                    );
                }
                terminal_psi::structural_leaf_copy_pair_pattern!() => {
                    let destination = DescriptorLocation::whole(result?.place);
                    let source = match &operation.kind {
                        OperationKind::StructuralLeafCopy { source, path } => {
                            location(*source, path, None)
                        }
                        OperationKind::StructuralCaseLeafCopy { source, path } => {
                            crate::descriptor_lifetimes::canonical_descriptor_location(
                                &module.structural_types,
                                *source,
                                *graph.place_types.get(source)?,
                                path,
                            )
                        }
                        _ => unreachable!("selected structural copy"),
                    };
                    if let Some(source) = source {
                        step.inputs = vec![source.clone()];
                        step.derivations.push((source, destination.clone()));
                    } else {
                        step.may_escape = true;
                    }
                    if (is_descriptor(module.structural_types.iter(), result?.structural_type)
                        || module.structural_types.iter().any(|ty| {
                            ty.id == result.unwrap().structural_type
                                && matches!(ty.shape, StructuralTypeShape::Reference { .. })
                        }))
                        && let Some(origins) = graph.originations.get_mut(&position)
                    {
                        origins.retain(|(location, _)| location != &destination);
                    }
                }
                OperationKind::StructuralByteSequenceFieldLength {
                    source,
                    path,
                    field,
                }
                | OperationKind::StructuralByteSequenceFieldRead {
                    source,
                    path,
                    field,
                    ..
                } => {
                    if let Some(source) = location(*source, path, Some(*field)) {
                        step.inputs = vec![source];
                    } else {
                        step.may_escape = true;
                    }
                }
                OperationKind::MoveStructuralField {
                    source,
                    path,
                    field,
                    ..
                } => {
                    let result = result?;
                    graph.contents_transfers.insert(
                        position,
                        descriptor_contents_transfers(
                            &module.structural_types,
                            slot_kind,
                            DescriptorContentsOperand {
                                root: *source,
                                root_type: *graph.place_types.get(source)?,
                                path,
                                field: *field,
                            },
                            DescriptorContentsOperand {
                                root: result.place,
                                root_type: result.structural_type,
                                path: &[],
                                field: None,
                            },
                            result.structural_type,
                            true,
                        )?,
                    );
                    if let Some(source) = location(*source, path, *field) {
                        let destination = DescriptorLocation::whole(result.place);
                        step.inputs = vec![source.clone()];
                        step.definitions.push(source.clone());
                        step.derivations.push((source, destination));
                    } else {
                        step.may_escape = true;
                    }
                }
                OperationKind::StoreStructuralField {
                    destination,
                    path,
                    field,
                    value,
                    ..
                } => {
                    let value_type = crate::runtime_structural_path_tip(
                        module.structural_types.iter(),
                        *graph.place_types.get(&value.place)?,
                        &value.path,
                    )?
                    .id;
                    graph.contents_transfers.insert(
                        position,
                        descriptor_contents_transfers(
                            &module.structural_types,
                            slot_kind,
                            DescriptorContentsOperand {
                                root: value.place,
                                root_type: *graph.place_types.get(&value.place)?,
                                path: &value.path,
                                field: None,
                            },
                            DescriptorContentsOperand {
                                root: *destination,
                                root_type: *graph.place_types.get(destination)?,
                                path,
                                field: *field,
                            },
                            value_type,
                            false,
                        )?,
                    );
                    if let (Some(destination), Some(source)) = (
                        location(*destination, path, *field),
                        location(value.place, &value.path, None),
                    ) {
                        step.definitions.push(destination.clone());
                        step.derivations.push((source, destination.clone()));
                        // Reseating contents retains the destination storage home.
                        if destination.path.is_empty()
                            && !is_descriptor(
                                module.structural_types.iter(),
                                *graph.place_types.get(&destination.root)?,
                            )
                            && !slot_kind.is_slot(
                                &module.structural_types,
                                *graph.place_types.get(&destination.root)?,
                            )
                        {
                            graph.originations.entry(position).or_default().push((
                                destination.clone(),
                                DescriptorCallOrigin::Argument {
                                    source: destination.root,
                                    projection: Projection::new(&[]),
                                },
                            ));
                        }
                    } else {
                        step.definitions.extend(slot_kind.overwritten_slots(
                            &module.structural_types,
                            *destination,
                            *graph.place_types.get(destination)?,
                            path,
                            *field,
                        )?);
                        step.may_escape = true;
                    }
                }
                OperationKind::StructuralByteViewFieldStore {
                    destination,
                    path,
                    field,
                    source,
                    ..
                } => {
                    if let Some(destination) = location(*destination, path, *field) {
                        step.definitions.push(destination.clone());
                        step.derivations
                            .push((DescriptorLocation::whole(*source), destination));
                    } else {
                        graph.unresolved_contents_writes.insert(position);
                        step.definitions.extend(slot_kind.overwritten_slots(
                            &module.structural_types,
                            *destination,
                            *graph.place_types.get(destination)?,
                            path,
                            *field,
                        )?);
                        step.may_escape = true;
                    }
                    step.inputs = vec![DescriptorLocation::whole(*source)];
                }
                OperationKind::EstablishStructuralArray { elements } => {
                    for (index, source) in elements.iter().enumerate() {
                        let destination = location(
                            result?.place,
                            &[StructuralPathSegment::FixedIndex(
                                u64::try_from(index).ok()?,
                            )],
                            None,
                        )?;
                        let source = location(source.place, &source.path, None)?;
                        step.derivations.push((source, destination));
                    }
                }
                OperationKind::EstablishRecord { fields }
                | OperationKind::EstablishStructuralCase { fields, .. } => {
                    if let OperationKind::EstablishStructuralCase { result_case, .. } =
                        &operation.kind
                    {
                        let result_type = result?.structural_type;
                        let shape = &module.structural_type(result_type)?.shape;
                        let cases = match shape {
                            StructuralTypeShape::Sum { cases }
                            | StructuralTypeShape::Mixed { cases, .. } => cases,
                            _ => return None,
                        };
                        if terminal_psi::find_structural_case(cases, *result_case).is_none() {
                            return None;
                        }
                        // The retained constructor tag proves that sibling-case
                        // payload slots do not exist in this value. Only those
                        // slots have no backing; an unresolved live field stays
                        // unknown and cannot be exported by a result summary.
                        for slot in slot_kind
                            .inventory(
                                &module.structural_types,
                                result?.place,
                                result?.structural_type,
                            )?
                            .slots
                        {
                            if matches!(slot.path.first(), Some(crate::descriptor_lifetimes::DescriptorPathSegment::Case(case)) if case != result_case)
                            {
                                graph
                                    .originations
                                    .entry(position)
                                    .or_default()
                                    .push((slot, DescriptorCallOrigin::Empty));
                            }
                        }
                    }
                    for (field_ordinal, field) in fields.iter().enumerate() {
                        if let Some(source) = field.value.as_structural() {
                            let destination = match &operation.kind {
                                OperationKind::EstablishStructuralCase { result_case, .. } => {
                                    let shape =
                                        &module.structural_type(result?.structural_type)?.shape;
                                    let path = terminal_psi::structural_case_initializer_path(
                                        shape,
                                        *result_case,
                                        field_ordinal,
                                        field.field,
                                    )?;
                                    location(result?.place, &path, None)?
                                }
                                _ => location(result?.place, &[], Some(field.field))?,
                            };
                            if let Some(source) = location(source.place, &source.path, None) {
                                step.derivations.push((source, destination));
                            } else {
                                step.may_escape = true;
                            }
                        }
                    }
                }
                OperationKind::CallUnit {
                    callee,
                    structural_arguments,
                    ..
                }
                | OperationKind::CallStructuralScalar {
                    callee,
                    structural_arguments,
                    ..
                }
                | terminal_psi::call_structural_pair_pattern!(callee, structural_arguments, ..) => {
                    let mut matching = module
                        .machines
                        .iter()
                        .filter(|machine| machine.id == *callee);
                    let callee = matching.next()?;
                    if matching.next().is_some()
                        || callee.structural_parameters.len() != structural_arguments.len()
                    {
                        return None;
                    }
                    let mut arguments = Vec::new();
                    let mut exact_arguments = true;
                    for (actual, formal) in structural_arguments
                        .iter()
                        .zip(&callee.structural_parameters)
                    {
                        if actual.access != formal.access {
                            return None;
                        }
                        let argument = location(actual.place, &actual.path, None);
                        exact_arguments &= argument.is_some();
                        arguments.push(argument);
                    }
                    let descriptor_result = result.filter(|result| {
                        slot_kind
                            .slots(
                                &module.structural_types,
                                result.place,
                                result.structural_type,
                            )
                            .is_some_and(|slots| !slots.is_empty())
                    });
                    if let Some(result) = descriptor_result {
                        let expected = callee.result.structural()?;
                        if result.structural_type != expected.structural_type
                            || result.multiplicity != expected.multiplicity
                            || result.qualifications != expected.qualifications
                            || result.projected_qualifications != expected.projected_qualifications
                            || !result.claims.is_empty()
                        {
                            return None;
                        }
                    }
                    if !exact_arguments {
                        step.may_escape = true;
                        if structural_arguments.iter().any(|actual| {
                            graph
                                .place_types
                                .get(&actual.place)
                                .is_none_or(|root_type| {
                                    slot_kind.may_write_indirect_contents(
                                        &module.structural_types,
                                        *root_type,
                                        actual,
                                    )
                                })
                        }) {
                            graph.unresolved_contents_writes.insert(position);
                        }
                        for actual in structural_arguments
                            .iter()
                            .filter(|actual| actual.access != StructuralAccess::SharedBorrow)
                        {
                            step.definitions.extend(slot_kind.slots(
                                &module.structural_types,
                                actual.place,
                                *graph.place_types.get(&actual.place)?,
                            )?);
                        }
                    }
                    graph.calls.insert(
                        position,
                        DescriptorInvocation {
                            callee: callee.id,
                            arguments,
                            result: descriptor_result
                                .map(|result| DescriptorLocation::whole(result.place)),
                        },
                    );
                }
                OperationKind::BoundaryCall {
                    boundary,
                    structural_arguments,
                    ..
                } => {
                    let preserves_placed_token = slot_kind == DescriptorSlotKind::PlacedOrigins
                        && module.placed_view_plans.iter().any(|plan|
                            module.placed_view_plans.iter().filter(|other| other.carrier == plan.carrier).count() == 1
                            && crate::placed_ram_plan_correspondence(&module.structural_types,
                                &module.placed_view_plans, plan, plan.schema).is_ok()
                            && plan.fields.iter().any(|field| field.operations.iter().any(|accessor|
                                accessor.boundary == *boundary && matches!(accessor.kind,
                                    terminal_psi::TerminalPlacedAccessorKind::Read | terminal_psi::TerminalPlacedAccessorKind::Write))));
                    if !preserves_placed_token
                        && !boundary_call_preserves_descriptor_storage(module, machine, operation)
                    {
                        step.may_escape = true;
                        if structural_arguments.iter().any(|actual| {
                            graph
                                .place_types
                                .get(&actual.place)
                                .is_none_or(|root_type| {
                                    slot_kind.may_write_indirect_contents(
                                        &module.structural_types,
                                        *root_type,
                                        actual,
                                    )
                                })
                        }) {
                            graph.unresolved_contents_writes.insert(position);
                        }
                        for actual in structural_arguments
                            .iter()
                            .filter(|actual| actual.access != StructuralAccess::SharedBorrow)
                        {
                            step.definitions.extend(slot_kind.slots(
                                &module.structural_types,
                                actual.place,
                                *graph.place_types.get(&actual.place)?,
                            )?);
                        }
                    }
                }
                _ => {}
            }
            step.authority_derivations = step.derivations.clone();
            step.successors.push(DescriptorFlowEdge {
                target: (block.id, position.1 + 1),
                parameters: Vec::new(),
                bindings: Vec::new(),
            });
            if graph.flow.insert(position, step).is_some() {
                return None;
            }
        }
        let position = (block.id, u32::try_from(block.operations.len()).ok()?);
        let mut step = DescriptorFlowStep::default();
        let mut edge = |target, arguments: &[StructuralArgument]| -> Option<()> {
            let mut matching = machine.blocks.iter().filter(|block| block.id == target);
            let block = matching.next()?;
            if matching.next().is_some() || arguments.len() != block.structural_parameters.len() {
                return None;
            }
            let parameters = block
                .structural_parameters
                .iter()
                .map(|parameter| DescriptorLocation::whole(parameter.place))
                .collect::<Vec<_>>();
            let mut bindings = Vec::new();
            for (argument, parameter) in arguments.iter().zip(&parameters) {
                if let Some(source) = location(argument.place, &argument.path, None) {
                    bindings.push((source, parameter.clone()));
                } else {
                    step.may_escape = true;
                    step.inputs.push(DescriptorLocation::whole(argument.place));
                }
            }
            step.inputs
                .extend(bindings.iter().map(|(source, _)| source.clone()));
            step.successors.push(DescriptorFlowEdge {
                target: (target, 0),
                parameters,
                bindings,
            });
            Some(())
        };
        match &block.terminator {
            Terminator::Jump {
                target,
                structural_arguments,
                ..
            } => edge(*target, structural_arguments)?,
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => {
                for successor in [when_true, when_false] {
                    edge(successor.target, &successor.structural_arguments)?;
                }
            }
            Terminator::StructuralCase { cases, .. } => {
                for case in cases {
                    edge(case.target, &[])?;
                }
            }
            Terminator::ReturnStructural { source, .. } => {
                if let Some(returns) = &mut graph.returns {
                    if place_type(machine, *source)? != machine.result.structural()?.structural_type
                    {
                        return None;
                    }
                    returns.push((position, DescriptorLocation::whole(*source)));
                }
                step.inputs.push(DescriptorLocation::whole(*source));
                step.may_escape = true;
                graph.normal_exits.push(position);
            }
            Terminator::Crash { .. } => {}
            Terminator::ExternalCompletion {
                structural_arguments,
                ..
            } => {
                // External completion observes its actuals but never contributes
                // a normal-return descriptor or a post-call ownership frontier.
                for argument in structural_arguments {
                    if let Some(source) = location(argument.place, &argument.path, None) {
                        step.inputs.push(source);
                    } else {
                        step.inputs.push(DescriptorLocation::whole(argument.place));
                    }
                }
                step.may_escape = true;
            }
            _ => {
                graph.normal_exits.push(position);
            }
        }
        // Mutable output descriptors are observed on normal completion even
        // for Unit/scalar returns; their borrowed backing escapes activation.
        if graph.normal_exits.contains(&position) {
            for parameter in &machine.structural_parameters {
                if matches!(
                    parameter.access,
                    terminal_psi::exclusive_borrow_pair_pattern!()
                ) {
                    step.inputs.extend(slot_kind.slots(
                        &module.structural_types,
                        parameter.place,
                        parameter.structural_type,
                    )?);
                }
            }
        }
        if graph.flow.insert(position, step).is_some() {
            return None;
        }
    }
    Some(graph)
}

/// Descriptor storage has an exact address, while selecting its backing may
/// capture runtime coordinates. Keep those roles separate: a dynamic element
/// below ordinary owned storage is a projection of that storage, never a
/// fabricated static descriptor slot. The complete typed path and formation
/// position survive even when later relative windows must remain conservative.
fn captured_storage_source(
    types: &[StructuralTypeDeclaration],
    source: &StructuralArgument,
    root_type: StructuralTypeId,
    position: DescriptorPosition,
) -> Option<(DescriptorLocation, Option<Projection>)> {
    let prefix_length = source
        .path
        .iter()
        .position(|segment| matches!(segment, StructuralPathSegment::RuntimeIndex { .. }))
        .unwrap_or(source.path.len());
    let location = descriptor_location(
        types,
        source.place,
        root_type,
        &source.path[..prefix_length],
        None,
    )?;
    if prefix_length == source.path.len() {
        return Some((location, None));
    }
    let prefix_type = descriptor_location_type(types, root_type, &location.path)?;
    // An incoming/stored descriptor is not its payload. Dynamic selection of
    // such a slot needs its own contents lineage; a parent storage address
    // cannot supply that missing relationship.
    if !plain_owned_shape(types.iter(), prefix_type) {
        return None;
    }
    crate::runtime_structural_path_tip(types.iter(), prefix_type, &source.path[prefix_length..])?;
    Some((
        location,
        Some(Projection::new(&source.path[prefix_length..]).at(position)),
    ))
}
fn destination_root(kind: &OperationKind) -> Option<&PlaceId> {
    match kind {
        OperationKind::EstablishByteSequenceLiteral { destination, .. } => Some(destination),
        _ => None,
    }
}

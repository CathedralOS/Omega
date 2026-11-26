use super::*;
use crate::descriptors::descriptor_lifetimes::{
    DescriptorFlowBinding, DescriptorFlowEdge, DescriptorFlowSource, descriptor_location,
    descriptor_location_type,
};
use crate::exactly_one::exactly_one;
use semantic_vocabulary::ScalarTerm;
use terminal_psi::BoundaryMachineResult;
use terminal_psi::RecordFieldValue;
use terminal_psi::element_view_establish_pair_pattern;
use terminal_psi::find_structural_type;

enum ScalarArrival<'a> {
    Values(&'a [semantic_vocabulary::ValueId]),
    CasePayload {
        source: PlaceId,
        path: &'a [StructuralPathSegment],
        case: semantic_vocabulary::StructuralCaseId,
        fields: &'a [semantic_vocabulary::StructuralFieldId],
    },
}

// Owned access also presents unrestricted copies. Only a call's
// retained affine/linear formal transfers its source custody.
fn consume_call(
    arguments: &[StructuralArgument],
    mut parameters: impl Iterator<Item = (StructuralAccess, StructuralMultiplicity)>,
    mut consume: impl FnMut(&StructuralArgument) -> Option<()>,
) -> Option<()> {
    for argument in arguments {
        let (access, multiplicity) = parameters.next()?;
        if argument.access == StructuralAccess::Owned
            && access == StructuralAccess::Owned
            && multiplicity != StructuralMultiplicity::Unrestricted
        {
            consume(argument)?;
        }
    }
    parameters.next().is_none().then_some(())
}

// Authority follows the value into its destination, but a consumed
// carrier cannot remain another authorized spelling. Keep these
// definitions alongside the operation's derivations: replay reads
// every transported origin from the same pre-operation state.
fn record_owned_consumption(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
    step: &mut DescriptorFlowStep,
) -> Option<()> {
    let mut consume = |argument: &StructuralArgument| {
        let consumed = descriptor_location(
            &module.types.structural_types,
            argument.place,
            machine.structural_place_type(argument.place)?,
            &argument.path,
            None,
        )?;
        step.definitions.push(consumed.clone());
        step.consumed.push(consumed);
        Some(())
    };
    let retained = |argument: &StructuralArgument| {
        if argument.access != StructuralAccess::Owned {
            return Some(false);
        }
        if let Some(source) = crate::structural_argument_source(machine, argument.place, 0)
        {
            return Some(source.multiplicity != StructuralMultiplicity::Unrestricted);
        }
        match &exactly_one(machine.structural_places.iter(), |place| {
            place.id == argument.place
        })?
        .kind
        {
            StructuralPlaceKind::TrivialAffineLocal { .. } => Some(true),
            StructuralPlaceKind::ByteSequenceLiteral { .. } => Some(false),
            _ => None,
        }
    };
    match &operation.kind {
        OperationKind::DecomposeStructuralValue { .. } => {
            // Only the nominal disposition ends. Its contained
            // descriptors keep their same storage and derivations.
        }
        OperationKind::MoveStructuralValue { source }
        | OperationKind::TransformContent { source, .. } => {
            if source.access == StructuralAccess::Owned {
                consume(source)?;
            }
        }
        OperationKind::StoreStructuralField { value, .. } => {
            if retained(value)? {
                consume(value)?;
            }
        }
        kind => {
            for argument in kind.constructor_arguments() {
                if retained(argument)? {
                    consume(argument)?;
                }
            }
        }
    }
    if let Some(invocation) = operation.declared_invocation() {
        let parameters = module.invocation_structural_parameters(
            invocation.callee,
            invocation.required_contract,
        )?;
        consume_call(
            invocation.structural_arguments,
            parameters
                .iter()
                .map(|parameter| (parameter.access, parameter.multiplicity)),
            &mut consume,
        )?;
    } else if let OperationKind::CallDynamic {
        interface,
        slot,
        structural_arguments,
        ..
    } = &operation.kind
    {
        let parameters = &module
            .execution.dynamic_dispatch
            .interface(*interface)?
            .slots
            .get(usize::try_from(*slot).ok()?)?
            .structural_parameters;
        consume_call(
            structural_arguments,
            parameters
                .iter()
                .map(|parameter| (parameter.access, parameter.multiplicity)),
            &mut consume,
        )?;
    }
    Some(())
}

pub(crate) fn residence_rules(
    module: &TerminalModule,
    machine: &TerminalMachine,
) -> Option<crate::descriptors::descriptor_lifetimes::DescriptorResidenceRules> {
    let mut rules =
        crate::descriptors::descriptor_lifetimes::DescriptorResidenceRules::default();
    for block in &machine.blocks {
        for (ordinal, operation) in block.operations.iter().enumerate() {
            let position = (block.id, u32::try_from(ordinal).ok()?);
            match &operation.kind {
                element_view_establish_pair_pattern!(source, ..)
                | OperationKind::EstablishAccessReference { source, .. }
                | OperationKind::EstablishReference { source, .. } => {
                    let (source, _, capture) = captured_storage_source(
                        &module.types.structural_types,
                        source.place,
                        &source.path,
                        machine.structural_place_type(source.place)?,
                        position,
                    )?;
                    if capture == CapturedBacking::StorageAddress {
                        rules.captures.insert((
                            position,
                            source,
                            DescriptorLocation::whole(operation.result.structural()?.place),
                        ));
                    }
                }
                _ => {}
            }
        }
    }
    Some(rules)
}

pub(crate) fn adapt(
    module: &TerminalModule,
    machines: &[&TerminalMachine],
    machine: &TerminalMachine,
    slot_kind: DescriptorSlotKind,
    selected_calls: &[(semantic_vocabulary::BoundaryMachineId, MachineId)],
    selector_graph: Option<&[DescriptorMachine]>,
    nominal_calls: &[terminal_psi::generic_calls::NominalCallArgument],
    zeroed_cases: &mut super::ZeroedCaseTable<'_>,
) -> Result<DescriptorMachine, super::DescriptorGraphError> {
    // Setup failures have no operation. Reuse the same typed error
    // as the reader advances, so a later failure keeps its real site.
    let mut failure = super::DescriptorGraphError::IncompleteMachine {
        machine: machine.id,
        position: None,
        successor: None,
        detail: None,
    };
    adapt_flow(
        module,
        machines,
        machine,
        slot_kind,
        selected_calls,
        selector_graph,
        nominal_calls,
        zeroed_cases,
        &mut failure,
    )
    .ok_or(failure)
}

#[allow(clippy::too_many_arguments)]
fn adapt_flow(
    module: &TerminalModule,
    machines: &[&TerminalMachine],
    machine: &TerminalMachine,
    slot_kind: DescriptorSlotKind,
    selected_calls: &[(semantic_vocabulary::BoundaryMachineId, MachineId)],
    selector_graph: Option<&[DescriptorMachine]>,
    nominal_calls: &[terminal_psi::generic_calls::NominalCallArgument],
    zeroed_cases: &mut super::ZeroedCaseTable<'_>,
    failure: &mut super::DescriptorGraphError,
) -> Option<DescriptorMachine> {
    let super::DescriptorGraphError::IncompleteMachine {
        position: failure_position,
        successor: failure_successor,
        detail: failure_detail,
        ..
    } = failure
    else {
        return None;
    };
    let scalar_expressions =
        super::payload_predicates::TerminalScalarExpressions::new(machine);
    let mut scalar_formals = machine
        .parameters
        .iter()
        .map(|parameter| parameter.id)
        .collect::<Vec<_>>();
    scalar_formals.sort_unstable();
    let selector = |value| {
        super::entry_selector(scalar_expressions.expression(value)?, |id| {
            scalar_formals.binary_search(&id).is_ok()
        })
    };
    let mut graph = DescriptorMachine {
        id: machine.id,
        returns: machine
            .result
            .structural()
            .filter(|result| {
                slot_kind.tracks_returned_value(
                    &module.types.structural_types,
                    result.place,
                    result.structural_type,
                )
            })
            .map(|_| Vec::new()),
        places: Vec::new(),
        place_types: BTreeMap::new(),
        entry: (machine.entry, 0),
        flow: BTreeMap::new(),
        case_constructors: BTreeMap::new(),
        case_successors: BTreeMap::new(),
        originations: BTreeMap::new(),
        projections: BTreeMap::new(),
        contents_queries: BTreeMap::new(),
        contents_transfers: BTreeMap::new(),
        payload_invalidations: BTreeMap::new(),
        payload_producers: BTreeMap::new(),
        payload_entry_predicates: Some((
            semantic_vocabulary::PropositionContext::from_value_types_and_places(
                machine
                    .parameters
                    .iter()
                    .chain(&machine.contract.erased_scalar_formals)
                    .map(|value| (value.id, value.scalar_type)),
                machine
                    .structural_places
                    .iter()
                    .map(|place| (place.id, place.kind)),
            )
            .ok()?,
            machine.contract.requires.clone(),
        )),
        scalar_payload_writes: BTreeMap::new(),
        scalar_payload_reads: BTreeMap::new(),
        unsigned_index_literals: Vec::new(),
        payload_scalar_definitions: BTreeMap::new(),
        payload_scalar_postconditions: BTreeMap::new(),
        payload_scalar_parameters: BTreeMap::new(),
        payload_edge_predicates: BTreeMap::new(),
        payload_scalar_bindings: BTreeMap::new(),
        private_observation_inputs: super::private_parameters::across_closed_observations(module, machine),
        unresolved_contents_writes: BTreeSet::new(),
        storage_derivations: BTreeSet::new(),
        calls: BTreeMap::new(),
        normal_exits: Vec::new(),
    };
    for place in &machine.structural_places {
        graph.places.push(DescriptorPlace {
            producer: None,
            place: place.id,
            definition: terminal_definition(module, machine, place.id, place.kind)?,
        });
        if let Some(ty) = machine.structural_place_type(place.id) {
            graph.place_types.insert(place.id, ty);
        }
    }
    graph.places.sort_unstable_by_key(|row| row.place);
    if graph
        .places
        .windows(2)
        .any(|pair| pair[0].place == pair[1].place)
    {
        return None;
    }
    let location = |root, path: &[StructuralPathSegment], field| {
        descriptor_location(
            &module.types.structural_types,
            root,
            *graph.place_types.get(&root)?,
            path,
            field,
        )
    };
    for block in &machine.blocks {
        graph.payload_scalar_parameters.insert(
            block.id,
            block.parameters.iter().map(|value| value.id).collect(),
        );
        for (index, operation) in block.operations.iter().enumerate() {
            *failure_position = None;
            *failure_successor = None;
            *failure_detail = None;
            let position = (block.id, u32::try_from(index).ok()?);
            *failure_position = Some(position);
            if let Some(postconditions) = super::scalar_contracts::terminal_postconditions(
                module,
                machine,
                operation,
                &scalar_expressions,
                nominal_calls,
            ) {
                graph
                    .payload_scalar_postconditions
                    .insert(position, postconditions.scalar);
                graph
                    .scalar_payload_writes
                    .entry(position)
                    .or_default()
                    .extend(postconditions.stored);
            }
            if let Some(result) = operation.result.scalar_ref() {
                if let OperationKind::IntegerConstant {
                    value: semantic_vocabulary::IntegerValue::Unsigned(value),
                } = operation.kind
                    && let Ok(index) = u64::try_from(value)
                {
                    graph.unsigned_index_literals.push((result.id, index));
                }
                graph
                    .payload_scalar_definitions
                    .insert(position, vec![result.id]);
            }
            let mut step = DescriptorFlowStep::default();
            let scalar_store = match &operation.kind {
                OperationKind::WriteOnlyPrimitiveStore {
                    destination,
                    path,
                    value,
                    ..
                } => Some((*destination, path.as_slice(), None, *value)),
                OperationKind::StructuralScalarFieldStore {
                    destination,
                    path,
                    field,
                    value,
                    ..
                } => Some((*destination, path.as_slice(), Some(*field), *value)),
                _ => None,
            };
            if let Some((destination, path, field, source)) = scalar_store
                && let Some(destination) = location(destination, path, field)
            {
                step.scalar_value_stores.push((destination.clone(), source));
                if let Some(value) = scalar_expressions.expression(source) {
                    graph
                        .scalar_payload_writes
                        .entry(position)
                        .or_default()
                        .push((destination, value));
                }
            }
            if let OperationKind::ScalarStructuralField {
                source,
                path,
                field,
            }
            | OperationKind::BooleanStructuralField {
                source,
                path,
                field,
            } = &operation.kind
            {
                let mut path = path.clone();
                path.push(semantic_vocabulary::CanonicalStructuralPathSegment::Field(
                    *field,
                ));
                if let Some(location) =
                    crate::descriptors::descriptor_lifetimes::canonical_descriptor_location(
                        &module.types.structural_types,
                        *source,
                        *graph.place_types.get(source)?,
                        &path,
                    )
                    && let Some(result) = operation.result.scalar_ref()
                {
                    graph
                        .scalar_payload_reads
                        .entry(position)
                        .or_default()
                        .push((location, ScalarTerm::value(result.id, result.scalar_type)));
                }
            }
            if let OperationKind::PrimitiveScalarRead { source, path } = &operation.kind
                && let Some(location) = location(*source, path, None)
                && let Some(result) = operation.result.scalar_ref()
            {
                graph
                    .scalar_payload_reads
                    .entry(position)
                    .or_default()
                    .push((location, ScalarTerm::value(result.id, result.scalar_type)));
            }
            let is_invocation = matches!(
                &operation.kind,
                terminal_psi::call_structural_argument_variants!(_arguments)
            ) || matches!(&operation.kind, OperationKind::Call { .. });
            // A scalar sibling does not observe the reference leaves
            // beside it. Retain the representation's projected uses;
            // reducing them to roots spuriously extends those loans.
            for (used, access) in operation.kind.storage_uses() {
                use terminal_psi::StructuralStorageUse;
                let (root, selected) = match used {
                    StructuralStorageUse::Place(root) => {
                        (root, Some(DescriptorLocation::whole(root)))
                    }
                    StructuralStorageUse::Projection(projection)
                    | StructuralStorageUse::Element {
                        descriptor: projection,
                        ..
                    } => (
                        projection.root,
                        location(projection.root, projection.path, projection.field),
                    ),
                    StructuralStorageUse::Canonical { root, path } => (
                        root,
                        crate::descriptor_lifetimes::canonical_descriptor_location(
                            &module.types.structural_types,
                            root,
                            *graph.place_types.get(&root)?,
                            path,
                        ),
                    ),
                    StructuralStorageUse::ObservedField { root, path, field } => {
                        let mut selected = path.to_vec();
                        selected.push(
                            semantic_vocabulary::CanonicalStructuralPathSegment::Field(
                                field,
                            ),
                        );
                        (
                            root,
                            crate::descriptor_lifetimes::canonical_descriptor_location(
                                &module.types.structural_types,
                                root,
                                *graph.place_types.get(&root)?,
                                &selected,
                            ),
                        )
                    }
                    StructuralStorageUse::Type(_) => continue,
                };
                if let Some(selected) = selected {
                    let access = access.map(|access| {
                        (
                            access,
                            descriptor_storage_use_source(
                                &module.types.structural_types,
                                graph.place_types.get(&root).copied(),
                                &operation.kind,
                                used,
                                |value| scalar_expressions.expression(value),
                            ),
                        )
                    });
                    step.inputs.push((Some(selected), access));
                } else {
                    // A scalar access uses its reference to address
                    // storage; it cannot carry a descriptor out of this
                    // operation. Keep observing the root and retain the
                    // exact memory query. Unknown or descriptor-bearing
                    // selections still retain authority conservatively.
                    let primitive = access.is_some()
                        && match used {
                            StructuralStorageUse::Projection(projection) => {
                                crate::primitive_operation_projection_type(
                                    module.types.structural_types.iter(),
                                    *graph.place_types.get(&root)?,
                                    projection,
                                )
                                .is_some()
                            }
                            _ => false,
                        };
                    step.may_escape |= !primitive;
                    let selected = DescriptorLocation::whole(root);
                    let access = access.map(|access| {
                        (
                            access,
                            descriptor_storage_use_source(
                                &module.types.structural_types,
                                graph.place_types.get(&root).copied(),
                                &operation.kind,
                                used,
                                |value| scalar_expressions.expression(value),
                            ),
                        )
                    });
                    step.inputs.push((Some(selected), access));
                }
            }
            let result = operation.result.structural();
            if let Some(result) = result {
                graph.payload_producers.insert(position, result.place);
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
                let index = graph
                    .places
                    .binary_search_by_key(&result.place, |row| row.place)
                    .ok()?;
                if graph.places[index].producer.replace(position).is_some() {
                    return None;
                }
                step.definitions
                    .push(DescriptorLocation::whole(result.place));
                if !is_descriptor(module.types.structural_types.iter(), result.structural_type)
                    && !slot_kind.is_slot(&module.types.structural_types, result.structural_type)
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
            let value_query = |root, path: &[StructuralPathSegment], field| {
                descriptor_contents_query(
                    &module.types.structural_types,
                    root,
                    *graph.place_types.get(&root)?,
                    path,
                    field,
                )
            };
            let payload_query = |root, path: &[StructuralPathSegment], field| {
                let mut query = value_query(root, path, field)?;
                if let Some(index) = path.iter().position(|segment| {
                    matches!(segment, StructuralPathSegment::RuntimeIndex { .. })
                }) {
                    let collection = crate::runtime_structural_path_tip(
                        module.types.structural_types.iter(),
                        *graph.place_types.get(&root)?,
                        &path[..index],
                    )?;
                    if matches!(collection.shape, StructuralTypeShape::FixedArray { .. }) {
                        // A later reference/view step names backing
                        // outside this array. Preserve that full query
                        // rather than turning it into an array write.
                        if query
                            .inline_steps(
                                &module.types.structural_types,
                                *graph.place_types.get(&root)?,
                            )
                            .is_none_or(|inline| inline[index..].contains(&false))
                        {
                            return Some(query);
                        }
                        // Any element may be written. Keep the array
                        // region rather than enumerating its extent;
                        // the complete destination was checked above.
                        return descriptor_contents_query(
                            &module.types.structural_types,
                            root,
                            *graph.place_types.get(&root)?,
                            &path[..index],
                            None,
                        );
                    }
                }
                if field.is_none() {
                    let selected = crate::runtime_structural_path_tip(
                        module.types.structural_types.iter(),
                        *graph.place_types.get(&root)?,
                        path,
                    )?;
                    match selected.shape {
                        StructuralTypeShape::ElementView { element, .. } => {
                            query
                                .steps
                                .push(DescriptorContentsStep::any_element(element));
                        }
                        StructuralTypeShape::ByteSequence(_)
                            if is_descriptor(
                                module.types.structural_types.iter(),
                                selected.id,
                            ) =>
                        {
                            return None;
                        }
                        _ => {}
                    }
                }
                Some(query)
            };
            let mut payload_writes = Vec::new();
            match &operation.kind {
                OperationKind::DecomposeStructuralValue { subject } => {
                    if operation.result != terminal_psi::OperationResult::Unit {
                        return None;
                    }
                    crate::nominal_cleanup::decomposition(module, machine.id, subject)
                        .ok()?;
                    // Partitioning ownership does not write the value
                    // or any backing reached through its descriptors.
                }
                OperationKind::ActivateTaskRegister { gdt, scratch, .. }
                | OperationKind::ReloadSegmentDescriptors { gdt, scratch, .. } => {
                    for source in [gdt, scratch] {
                        payload_writes.push(payload_query(
                            source.place,
                            &source.path,
                            None,
                        ));
                    }
                }
                OperationKind::GlobalDescriptorTable {
                    operation: terminal_psi::GlobalDescriptorTableOperation::Store,
                    source,
                    ..
                } => {
                    payload_writes.push(payload_query(source.place, &source.path, None));
                }
                OperationKind::WriteOnlyPrimitiveStore {
                    destination, path, ..
                } => {
                    payload_writes.push(payload_query(*destination, path, None));
                }
                // A move retires the selected value, including a
                // view descriptor. It does not write the backing the
                // descriptor names. Explicit source projections still
                // retain any actual referent or element selection.
                OperationKind::MoveStructuralValue { source } => {
                    payload_writes.push(value_query(source.place, &source.path, None));
                }
                OperationKind::MoveStructuralField {
                    source,
                    path,
                    field,
                } => {
                    payload_writes.push(value_query(*source, path, *field));
                }
                OperationKind::StructuralScalarFieldStore {
                    destination,
                    path,
                    field,
                    ..
                } => {
                    payload_writes.push(payload_query(*destination, path, Some(*field)));
                }
                OperationKind::StructuralByteSequenceFieldByteStore {
                    destination,
                    path,
                    field,
                    ..
                }
                | OperationKind::StructuralByteSequenceFieldStore {
                    destination,
                    path,
                    field,
                    ..
                } => {
                    payload_writes.push(payload_query(*destination, path, Some(*field)));
                }
                OperationKind::StructuralByteViewFieldStore {
                    destination,
                    path,
                    field,
                    ..
                }
                | OperationKind::StoreStructuralField {
                    destination,
                    path,
                    field,
                    ..
                } => {
                    payload_writes.push(payload_query(*destination, path, *field));
                }
                OperationKind::ByteSequenceWrite { .. }
                | OperationKind::ByteSequenceStore { .. } => {
                    payload_writes.push(None);
                }
                OperationKind::ElementViewWrite {
                    destination,
                    index,
                    obligation,
                    path,
                    ..
                } => {
                    let mut selected = destination.path.clone();
                    selected.push(StructuralPathSegment::RuntimeIndex {
                        index: *index,
                        obligation: *obligation,
                    });
                    selected.extend_from_slice(path);
                    payload_writes.push(payload_query(destination.place, &selected, None));
                }
                terminal_psi::call_structural_argument_variants!(arguments) => {
                    for argument in arguments {
                        let selected = crate::runtime_structural_path_tip(
                            module.types.structural_types.iter(),
                            *graph.place_types.get(&argument.place)?,
                            &argument.path,
                        );
                        let mutation = selected
                            .map(|selected| {
                                crate::shared_mutation(
                                    module.types.structural_types.iter(),
                                    selected.id,
                                    &[],
                                )
                            })
                            .unwrap_or(crate::SharedMutation::Unbounded);
                        if mutation == crate::SharedMutation::Unbounded {
                            payload_writes.push(None);
                        } else if argument.access != StructuralAccess::SharedBorrow
                            || mutation != crate::SharedMutation::None
                        {
                            payload_writes.push(payload_query(
                                argument.place,
                                &argument.path,
                                None,
                            ));
                        }
                    }
                }
                OperationKind::AtomicAccess {
                    place, path, event, ..
                } if event.modifies_resident() => {
                    payload_writes.push(payload_query(*place, path, None));
                }
                OperationKind::AtomicAccess { .. }
                | OperationKind::AtomicFence { .. }
                | OperationKind::StorageAddress { .. }
                | OperationKind::StorageAlignment { .. }
                | OperationKind::StructuralLeafCopy { .. }
                | OperationKind::StructuralCaseLeafCopy { .. }
                | OperationKind::EstablishStructuralLocal { .. }
                | OperationKind::EstablishZeroedStructural { .. } => {}
                kind if kind.declared_invocation().is_some() => {}
                kind => {
                    let tag = crate::OperationSemanticTag::for_operation(kind);
                    let scalar = crate::operation_semantic_row(kind)
                        .ok()?
                        .goal_free_scalar_leaf()
                        .is_some()
                        || crate::ProofBearingScalarSemanticRow::ALL
                            .iter()
                            .any(|row| row.tag() == tag);
                    let structural =
                        crate::StructuralEffectSemanticRow::ALL.iter().any(|row| {
                            row.tag() == tag
                                && row.schema().external_effect()
                                    == crate::StructuralEffectExternalEffect::None
                        });
                    if !scalar && !structural {
                        // A semantic family not represented by this
                        // storage flow cannot publish an empty frame.
                        payload_writes.push(None);
                    }
                }
            }
            if !payload_writes.is_empty() {
                graph.payload_invalidations.insert(position, payload_writes);
            }
            let direct_call = match &operation.kind {
                kind if terminal_psi::direct_call_parts(kind).is_some() => {
                    let call = terminal_psi::direct_call_parts(kind)?;
                    Some((call.callee, call.structural_arguments))
                }
                OperationKind::BoundaryCall {
                    boundary,
                    structural_arguments,
                    ..
                } => {
                    let mut selections =
                        selected_calls.iter().filter(|(head, _)| head == boundary);
                    match (selections.next(), selections.next()) {
                        (Some((_, callee)), None) => {
                            Some((*callee, structural_arguments.as_slice()))
                        }
                        (None, None) => None,
                        _ => return None,
                    }
                }
                _ => None,
            };
            // Every constructor establishes the same case membership,
            // whether its payload contains scalars, values or no fields.
            if let OperationKind::EstablishScalarCase { result_case, .. }
            | OperationKind::EstablishStructuralCase { result_case, .. } =
                &operation.kind
            {
                graph.case_constructors.insert(
                    position,
                    vec![DescriptorCasePredicate {
                        subject: DescriptorLocation::whole(result?.place).into(),
                        case: *result_case,
                    }],
                );
                let result_type = result?.structural_type;
                let shape = &module.structural_type(result_type)?.shape;
                let cases = match shape {
                    StructuralTypeShape::Sum { cases }
                    | StructuralTypeShape::Mixed { cases, .. } => cases,
                    _ => return None,
                };
                terminal_psi::find_structural_case(cases, *result_case)?;
                for slot in slot_kind
                    .inventory(
                        &module.types.structural_types,
                        result?.place,
                        result?.structural_type,
                    )?
                    .slots
                {
                    if matches!(slot.path.first(), Some(crate::descriptors::descriptor_lifetimes::DescriptorPathSegment::Case(case)) if case != result_case)
                    {
                        graph
                            .originations
                            .entry(position)
                            .or_default()
                            .push((slot, DescriptorCallOrigin::Empty));
                    }
                }
            }
            match &operation.kind {
                OperationKind::DecomposeStructuralValue { subject } => {
                    // Observe every contained descriptor through the
                    // event; no value is consumed or newly derived.
                    step.replace_value_inputs([location(
                        subject.place,
                        &subject.path,
                        None,
                    )?]);
                }
                OperationKind::ReleaseReference { source, path } => {
                    let source = location(*source, path, None)?;
                    step.replace_value_inputs([source.clone()]);
                    step.definitions.push(source);
                }
                OperationKind::ByteSequenceLength { source }
                | OperationKind::ElementViewLength { source } => {
                    let query = descriptor_contents_query(
                        &module.types.structural_types,
                        source.place,
                        *graph.place_types.get(&source.place)?,
                        &source.path,
                        None,
                    )?;
                    step.replace_value_inputs([query.observation()]);
                }
                OperationKind::EstablishNativeCodeLayout { .. }
                    if slot_kind.tracks_views() =>
                {
                    let result = result?;
                    let destination =
                        native_code_layout_regions(&module.types.structural_types, result)?;
                    step.definitions.push(destination.clone());
                    graph.originations.entry(position).or_default().push((
                        destination,
                        DescriptorCallOrigin::Literal {
                            machine: machine.id,
                            place: result.place,
                        },
                    ));
                }
                OperationKind::EstablishZeroedStructural {} => {
                    let result = result?;
                    graph.case_constructors.insert(
                        position,
                        zeroed_cases.predicates_for(result.place, result.structural_type),
                    );
                    for slot in slot_kind
                        .inventory(
                            &module.types.structural_types,
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
                    graph.payload_producers.insert(position, *destination);
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
                OperationKind::EstablishAccessReference { .. }
                    if slot_kind == DescriptorSlotKind::AccessOrigins =>
                {
                    let result = result?;
                    let StructuralTypeShape::Reference {
                        referent: carrier, ..
                    } = find_structural_type(
                        &module.types.structural_types,
                        result.structural_type,
                    )?
                    .shape
                    else {
                        return None;
                    };
                    graph.originations.entry(position).or_default().push((
                        DescriptorLocation::whole(result.place),
                        DescriptorCallOrigin::AccessFormation {
                            machine: machine.id,
                            operation: operation.id,
                            place: result.place,
                            carrier,
                            projection: Projection::new(&[]),
                        },
                    ));
                }
                element_view_establish_pair_pattern!(source, ..)
                | OperationKind::EstablishAccessReference { source, .. }
                | OperationKind::EstablishReference { source, .. } => {
                    let destination = DescriptorLocation::whole(result?.place);
                    let (source_location, mut backing_projection, capture) =
                        captured_storage_source(
                            &module.types.structural_types,
                            source.place,
                            &source.path,
                            *graph.place_types.get(&source.place)?,
                            position,
                        )?;
                    if let OperationKind::EstablishReference {
                        storage: Some(storage),
                        ..
                    } = &operation.kind
                    {
                        let StructuralTypeShape::Reference { referent, .. } =
                            find_structural_type(
                                &module.types.structural_types,
                                result?.structural_type,
                            )?
                            .shape
                        else {
                            return None;
                        };
                        // The loan keeps the source address and its
                        // new presentation together. Dropping this
                        // step would compare the record to its bytes
                        // as though they were different allocations.
                        let view =
                            Projection::new(&[StructuralPathSegment::RepresentationView {
                                structural_type: referent,
                                stored_scalar: storage.stored_scalar,
                            }]);
                        backing_projection = Some(match backing_projection {
                            Some(prefix) => prefix.then(&view),
                            None => view,
                        });
                    }
                    step.derivations
                        .push((source_location.clone(), destination.clone()));
                    // Inline collections lend their current storage
                    // address, just as references do. A view of another
                    // descriptor instead retains that descriptor's
                    // payload origin, never the descriptor's home.
                    let captures_storage = match &operation.kind {
                        element_view_establish_pair_pattern!(source, element, ..) => {
                            crate::runtime_structural_path_tip(
                                module.types.structural_types.iter(),
                                *graph.place_types.get(&source.place)?,
                                &source.path,
                            )
                            .and_then(|collection| {
                                crate::collection_backing(
                                    module.types.structural_types.iter(),
                                    collection.id,
                                    *element,
                                )
                            })
                            .is_some_and(|backing| {
                                backing.storage == crate::CollectionStorage::Inline
                            })
                        }
                        _ => true,
                    };
                    if capture == CapturedBacking::StorageAddress && captures_storage {
                        graph.storage_derivations.insert((
                            position,
                            source_location.clone(),
                            destination.clone(),
                        ));
                    }
                    let preserves_coordinates = match &operation.kind {
                        element_view_establish_pair_pattern!(
                            source,
                            element,
                            region,
                            ..
                        ) => crate::runtime_structural_path_tip(
                            module.types.structural_types.iter(),
                            *graph.place_types.get(&source.place)?,
                            &source.path,
                        )
                        .is_some_and(|collection| {
                            crate::collection_view_preserves_coordinates(
                                module.types.structural_types.iter(),
                                collection.id,
                                *element,
                                *region,
                            )
                        }),
                        _ => true,
                    };
                    if !preserves_coordinates {
                        backing_projection
                            .get_or_insert_with(|| Projection::new(&[]))
                            .forget_address();
                    }
                    if let Some(projection) = backing_projection {
                        graph.projections.insert(
                            (position, source_location, destination.clone()),
                            projection,
                        );
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
                        &module.types.structural_types,
                        source.place,
                        *graph.place_types.get(&source.place)?,
                        &source.path,
                        None,
                    )?;
                    let source = query.observation();
                    step.replace_value_inputs([source.clone()]);
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
                terminal_psi::structural_leaf_copy_pair_pattern!()
                | OperationKind::MoveStructuralValue { .. }
                | OperationKind::TransformContent { .. } => {
                    let destination = DescriptorLocation::whole(result?.place);
                    let source = match &operation.kind {
                        OperationKind::StructuralLeafCopy { source, path } => {
                            location(*source, path, None)
                        }
                        OperationKind::MoveStructuralValue { source }
                        | OperationKind::TransformContent { source, .. } => {
                            location(source.place, &source.path, None)
                        }
                        OperationKind::StructuralCaseLeafCopy { source, path } => {
                            crate::descriptors::descriptor_lifetimes::canonical_descriptor_location(
                                &module.types.structural_types,
                                *source,
                                *graph.place_types.get(source)?,
                                path,
                            )
                        }
                        _ => unreachable!("selected structural value transport"),
                    };
                    if let Some(source) = source {
                        step.replace_value_inputs([source.clone()]);
                        if matches!(operation.kind, OperationKind::TransformContent { .. })
                        {
                            step.derivations.push((source, destination.clone()));
                        } else {
                            step.copy_value(source, destination.clone());
                        }
                    } else if let OperationKind::StructuralLeafCopy { source, path } =
                        &operation.kind
                    {
                        let query = super::contents::descriptor_contents_query_with(
                            &module.types.structural_types,
                            *source,
                            *graph.place_types.get(source)?,
                            path,
                            None,
                            selector,
                        );
                        step.record_selected_copy(
                            &module.types.structural_types,
                            slot_kind,
                            position,
                            &mut graph.contents_queries,
                            (query, *graph.place_types.get(source)?),
                            (result?.place, result?.structural_type),
                        );
                    } else {
                        step.may_escape = true;
                    }
                    if (is_descriptor(
                        module.types.structural_types.iter(),
                        result?.structural_type,
                    ) || module.types.structural_types.iter().any(|ty| {
                        ty.id == result.unwrap().structural_type
                            && matches!(ty.shape, StructuralTypeShape::Reference { .. })
                    })) && let Some(origins) = graph.originations.get_mut(&position)
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
                        step.replace_value_inputs([source]);
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
                            &module.types.structural_types,
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
                        step.replace_value_inputs([source.clone()]);
                        step.definitions.push(source.clone());
                        step.copy_value(source, destination);
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
                        module.types.structural_types.iter(),
                        *graph.place_types.get(&value.place)?,
                        &value.path,
                    )?
                    .id;
                    graph.contents_transfers.insert(
                        position,
                        descriptor_contents_transfers(
                            &module.types.structural_types,
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
                        step.copy_value(source, destination.clone());
                        if destination.path.is_empty()
                            && !is_descriptor(
                                module.types.structural_types.iter(),
                                *graph.place_types.get(&destination.root)?,
                            )
                            && !slot_kind.is_slot(
                                &module.types.structural_types,
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
                            &module.types.structural_types,
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
                    let root_type = *graph.place_types.get(destination)?;
                    super::descriptor_leaf_store(
                        super::DescriptorStoreFlow {
                            position,
                            transfers: &mut graph.contents_transfers,
                            unresolved: &mut graph.unresolved_contents_writes,
                        },
                        &mut step,
                        &module.types.structural_types,
                        slot_kind,
                        super::DescriptorContentsOperand {
                            root: *destination,
                            root_type,
                            path,
                            field: *field,
                        },
                        *source,
                        selector,
                    )?;
                }
                OperationKind::EstablishPrimitiveLocal { value, .. } => {
                    step.scalar_value_stores
                        .push((location(result?.place, &[], None)?, *value));
                    if let Some(value) = scalar_expressions.expression(*value) {
                        graph
                            .scalar_payload_writes
                            .entry(position)
                            .or_default()
                            .push((location(result?.place, &[], None)?, value));
                    }
                }
                OperationKind::EstablishScalarArray { elements } => {
                    let result = result?;
                    for (ordinal, value) in elements.iter().enumerate() {
                        let path = crate::scalar_array_leaf_path(
                            module.types.structural_types.iter(),
                            result.structural_type,
                            u64::try_from(ordinal).ok()?,
                        )?;
                        step.scalar_value_stores
                            .push((location(result.place, &path, None)?, *value));
                        if let Some(value) = scalar_expressions.expression(*value) {
                            graph
                                .scalar_payload_writes
                                .entry(position)
                                .or_default()
                                .push((location(result.place, &path, None)?, value));
                        }
                    }
                }
                OperationKind::EstablishStructuralArray { elements } => {
                    let result = result?;
                    let (element, stored) =
                        match module.structural_type(result.structural_type)?.shape {
                            StructuralTypeShape::ElementView {
                                element,
                                access: None | Some(StructuralAccess::SharedBorrow),
                            } => (element, true),
                            StructuralTypeShape::FixedArray {
                                element,
                                length: terminal_psi::ArrayExtent::Literal(length),
                            } if usize::try_from(length).ok() == Some(elements.len()) => {
                                (element, false)
                            }
                            _ => return None,
                        };
                    if stored {
                        // These ordinals belong to this constructor's
                        // real operand roster. They are not unchecked
                        // source indexes under a view descriptor.
                        graph.originations.entry(position).or_default().push((
                            DescriptorLocation::whole(result.place),
                            DescriptorCallOrigin::Argument {
                                source: result.place,
                                projection: Projection::new(&[]),
                            },
                        ));
                    }
                    for (index, source) in elements.iter().enumerate() {
                        if source.access != StructuralAccess::Owned
                            || source.place == result.place
                            || crate::runtime_structural_path_tip(
                                module.types.structural_types.iter(),
                                *graph.place_types.get(&source.place)?,
                                &source.path,
                            )?
                            .id != element
                        {
                            return None;
                        }
                        let index = u64::try_from(index).ok()?;
                        let destination = if stored {
                            DescriptorLocation {
                                root: result.place,
                                path: vec![DescriptorPathSegment::FixedIndex(index)],
                            }
                        } else {
                            location(
                                result.place,
                                &[StructuralPathSegment::FixedIndex(index)],
                                None,
                            )?
                        };
                        let source = location(source.place, &source.path, None)?;
                        step.copy_value(source, destination);
                    }
                }
                OperationKind::ElementViewRead {
                    source,
                    index,
                    obligation,
                    path,
                    ..
                } => {
                    if let Some(result) = result {
                        let mut selected = source.path.clone();
                        selected.push(StructuralPathSegment::RuntimeIndex {
                            index: *index,
                            obligation: *obligation,
                        });
                        selected.extend_from_slice(path);
                        graph.contents_transfers.insert(
                            position,
                            descriptor_contents_transfers(
                                &module.types.structural_types,
                                slot_kind,
                                DescriptorContentsOperand {
                                    root: source.place,
                                    root_type: *graph.place_types.get(&source.place)?,
                                    path: &selected,
                                    field: None,
                                },
                                DescriptorContentsOperand {
                                    root: result.place,
                                    root_type: result.structural_type,
                                    path: &[],
                                    field: None,
                                },
                                result.structural_type,
                                false,
                            )?,
                        );
                    } else if operation.result.scalar_ref().is_none() {
                        return None;
                    }
                }
                OperationKind::EstablishRecord { fields }
                | OperationKind::EstablishStructuralCase { fields, .. } => {
                    for (field_ordinal, field) in fields.iter().enumerate() {
                        let destination = match &operation.kind {
                            OperationKind::EstablishStructuralCase {
                                result_case, ..
                            } => {
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
                        if let RecordFieldValue::Scalar { value, .. } = &field.value {
                            step.scalar_value_stores.push((destination.clone(), *value));
                            if let Some(value) = scalar_expressions.expression(*value) {
                                graph
                                    .scalar_payload_writes
                                    .entry(position)
                                    .or_default()
                                    .push((destination.clone(), value));
                            }
                        }
                        if let Some(source) = field.value.as_structural() {
                            if let Some(source) = location(source.place, &source.path, None)
                            {
                                step.copy_value(source, destination);
                            } else {
                                step.may_escape = true;
                            }
                        }
                    }
                }
                OperationKind::CallMachine {
                    contract,
                    structural_arguments,
                    ..
                } => {
                    let contract = crate::references::callable_reference_result(
                        &module.types.structural_types,
                        module.execution.callables.contract(*contract)?,
                    )
                    .inspect_err(|detail| *failure_detail = Some(*detail))
                    .ok()?;
                    // A public callable supplies result origins and access ceilings.
                    // Its unknown body supplies no stored-content summary. Invalidate
                    // only writable argument paths, retaining unrelated storage.
                    let call = public_invocation(
                        operation.id,
                        &module.types.structural_types,
                        slot_kind,
                        &contract,
                        Vec::new(),
                        structural_arguments,
                        result,
                        &graph.place_types,
                    )?;
                    for actual in structural_arguments
                        .iter()
                        .filter(|actual| actual.access != StructuralAccess::SharedBorrow)
                    {
                        step.definitions.extend(writable_descriptor_slots(
                            &module.types.structural_types,
                            &graph.place_types,
                            actual,
                            slot_kind,
                        )?);
                    }
                    if matches!(
                        call.target,
                        DescriptorInvocationTarget::Public {
                            invalidates_contents: true,
                            ..
                        }
                    ) {
                        graph.unresolved_contents_writes.insert(position);
                        step.may_escape = true;
                    }
                    insert_invocation(&mut graph.calls, position, call)?;
                }
                OperationKind::CallDynamic {
                    selector,
                    interface,
                    slot,
                    structural_arguments,
                    ..
                } => {
                    let effects_machine = selected_effects_machine(
                        module,
                        machine,
                        selector_graph,
                        *selector,
                        *interface,
                        *slot,
                    );
                    let realizations =
                        public_realizations(&module.execution.dynamic_dispatch, *interface, *slot)?;
                    let slot = module
                        .execution.dynamic_dispatch
                        .interface(*interface)?
                        .slots
                        .get(usize::try_from(*slot).ok()?)?;
                    let mut call = public_invocation(
                        operation.id,
                        &module.types.structural_types,
                        slot_kind,
                        slot,
                        realizations.clone(),
                        structural_arguments,
                        result,
                        &graph.place_types,
                    )?;
                    let complete_contents =
                        call.argument_contents.iter().all(Option::is_some);
                    let effects_machine = effects_machine.filter(|_| complete_contents);
                    if let DescriptorInvocationTarget::Public {
                        effects_machine: target,
                        ..
                    } = &mut call.target
                    {
                        *target = effects_machine;
                    }
                    let has_stored_effects = complete_contents
                        && (effects_machine.is_some() || !realizations.is_empty());
                    if !has_stored_effects {
                        for actual in structural_arguments.iter().filter(|actual| {
                            actual.access != StructuralAccess::SharedBorrow
                        }) {
                            step.definitions.extend(writable_descriptor_slots(
                                &module.types.structural_types,
                                &graph.place_types,
                                actual,
                                slot_kind,
                            )?);
                        }
                    }
                    if matches!(
                        call.target,
                        DescriptorInvocationTarget::Public {
                            invalidates_contents: true,
                            ..
                        }
                    ) && !has_stored_effects
                    {
                        graph.unresolved_contents_writes.insert(position);
                        step.may_escape = true;
                    }
                    insert_invocation(&mut graph.calls, position, call)?;
                }
                _ if direct_call.is_some() => {
                    let (callee, structural_arguments) = direct_call?;
                    let returns_reference = result.is_some_and(|result| {
                        terminal_psi::structural_type_contains_reference_by(
                            |id| module.structural_type(id),
                            result.structural_type,
                        )
                    });
                    // A selected call reads its public requirement;
                    // a rigid call reaches the bodiless requirement
                    // itself. Both read one declared lifetime account,
                    // and only a selected body supplies stored effects.
                    let public = if returns_reference {
                        crate::public_reference_result(
                            module,
                            machine.id,
                            callee,
                            operation.required_contract,
                        )
                        .inspect_err(|detail| *failure_detail = Some(*detail))
                        .ok()?
                    } else {
                        crate::requirement_reference_result(module, callee)
                            .inspect_err(|detail| *failure_detail = Some(*detail))
                            .ok()?
                    };
                    let implemented = operation.required_contract.is_some();
                    if let Some(public) = public {
                        let mut call = public_invocation(
                            operation.id,
                            &module.types.structural_types,
                            slot_kind,
                            &public,
                            if implemented {
                                vec![callee]
                            } else {
                                Vec::new()
                            },
                            structural_arguments,
                            result,
                            &graph.place_types,
                        )?;
                        call.retain_whole_scalar_result_address(module, &public);
                        let complete = call.arguments.iter().all(Option::is_some)
                            && call.argument_contents.iter().all(Option::is_some);
                        if !complete {
                            mark_opaque_call_occupancy(
                                &graph.place_types,
                                &mut graph.unresolved_contents_writes,
                                &mut step,
                                slot_kind,
                                &module.types.structural_types,
                                position,
                                structural_arguments,
                            )?;
                            if matches!(
                                call.target,
                                DescriptorInvocationTarget::Public {
                                    invalidates_contents: true,
                                    ..
                                }
                            ) {
                                graph.unresolved_contents_writes.insert(position);
                                step.may_escape = true;
                            }
                        }
                        if let DescriptorInvocationTarget::Public {
                            effects_machine,
                            contents,
                            ..
                        } = &mut call.target
                        {
                            *effects_machine = (complete && implemented).then_some(callee);
                            if !implemented
                                && slot_kind.tracks_references()
                                && let Some(result) = result
                            {
                                let inventory = slot_kind.owned_inventory(
                                    &module.types.structural_types,
                                    result.place,
                                    result.structural_type,
                                )?;
                                let mut lifetime_dependencies = Vec::new();
                                if !inventory.unknown_contents.is_empty() {
                                    for (position, formal) in public
                                        .contract()
                                        .structural_parameters
                                        .iter()
                                        .enumerate()
                                    {
                                        let ty = module
                                            .structural_type(formal.structural_type)?;
                                        if formal.access != StructuralAccess::Owned
                                            && !ty.shape.is_view_descriptor()
                                        {
                                            lifetime_dependencies.push(DescriptorLifetimeDebt {
                                                origin: DescriptorStoredOrigin::Parameter {
                                                    position,
                                                    projection: Projection::new(&[]),
                                                },
                                                access: formal.access,
                                            });
                                        }
                                        if formal.access
                                            == StructuralAccess::WriteOnlyBorrow
                                        {
                                            continue;
                                        }
                                        for origin in DescriptorSlotKind::BorrowedStorage
                                            .input_contents_origins(
                                                &module.types.structural_types,
                                                formal,
                                                position,
                                            )?
                                        {
                                            lifetime_dependencies.push(DescriptorLifetimeDebt {
                                                origin,
                                                access: formal.access,
                                            });
                                        }
                                    }
                                }
                                for packet in inventory.unknown_contents {
                                    let path = packet.location.path;
                                    let (last, prefix) = path.split_last()?;
                                    if *last != DescriptorPathSegment::UnknownContents {
                                        return None;
                                    }
                                    let structural_type = descriptor_location_type(
                                        &module.types.structural_types,
                                        result.structural_type,
                                        prefix,
                                    )?;
                                    let produced =
                                        crate::descriptors::replay::ProducedContents {
                                            producer: DescriptorInvocationSite {
                                                machine: machine.id,
                                                owner: call.owner,
                                            },
                                            path: path.clone(),
                                            structural_type,
                                            lifetime_dependencies: lifetime_dependencies
                                                .clone(),
                                        };
                                    contents.push(DescriptorResultSlot {
                                        path,
                                        origins: vec![
                                            DescriptorStoredOrigin::ProducedContents(
                                                produced,
                                            ),
                                        ],
                                    });
                                }
                            }
                        }
                        insert_invocation(&mut graph.calls, position, call)?;
                    } else {
                        let inputs = operation.module_invocation()?;
                        if inputs.callee != callee
                            || inputs.structural_arguments != structural_arguments
                        {
                            return None;
                        }
                        let call = graph.direct_invocation(
                            module,
                            machines,
                            slot_kind,
                            terminal_psi::ModuleInvocation {
                                site: semantic_vocabulary::InvocationSite::Operation(
                                    operation.id,
                                ),
                                static_reach_binding: operation.static_reach_binding,
                                inputs,
                            },
                            result,
                            selector,
                        )?;
                        if call.arguments.iter().any(Option::is_none) {
                            mark_opaque_call_occupancy(
                                &graph.place_types,
                                &mut graph.unresolved_contents_writes,
                                &mut step,
                                slot_kind,
                                &module.types.structural_types,
                                position,
                                structural_arguments,
                            )?;
                        }
                        insert_invocation(&mut graph.calls, position, call)?;
                    }
                }
                OperationKind::BoundaryCall {
                    boundary,
                    structural_arguments,
                    ..
                } => {
                    if slot_kind == DescriptorSlotKind::AccessOrigins
                        && let Some(plan) =
                            module.access.access_lifecycle_plans.iter().find(|plan| {
                                plan.boundary == *boundary
                                && plan.operation
                                    == terminal_psi::TerminalAccessLifecycleOperation::View
                            })
                    {
                        let crate::AccessResultContract::View { ready, carrier, .. } =
                            crate::access_result_contract(module, plan).ok()?
                        else {
                            return None;
                        };
                        let result = result?;
                        let shape = &module.structural_type(result.structural_type)?.shape;
                        let path = terminal_psi::structural_case_initializer_path(
                            shape,
                            ready.id,
                            0,
                            ready.fields.first()?.id,
                        )?;
                        graph.originations.entry(position).or_default().push((
                            location(result.place, &path, None)?,
                            DescriptorCallOrigin::AccessFormation {
                                machine: machine.id,
                                operation: operation.id,
                                place: result.place,
                                carrier,
                                projection: Projection::new(&[]),
                            },
                        ));
                    }
                    let preserves_access_token = slot_kind == DescriptorSlotKind::AccessOrigins
                        && module.access.access_view_plans.iter().any(|plan| {
                            module
                                .access.access_view_plans
                                .iter()
                                .filter(|other| other.carrier == plan.carrier)
                                .count()
                                == 1
                                && crate::access_ram_plan_correspondence(
                                    &module.types.structural_types,
                                    &module.access.access_view_plans,
                                    plan,
                                    plan.schema,
                                )
                                .is_ok()
                                && plan.fields.iter().any(|field| {
                                    field.operations.iter().any(|accessor| {
                                        accessor.boundary == *boundary
                                            && matches!(
                                                accessor.kind,
                                                terminal_psi::TerminalAccessorKind::Read
                                                    | terminal_psi::TerminalAccessorKind::Write
                                            )
                                    })
                                })
                        });
                    if !preserves_access_token
                        && !boundary_call_preserves_descriptor_storage(
                            module, machine, operation,
                        )
                    {
                        mark_opaque_call_occupancy(
                            &graph.place_types,
                            &mut graph.unresolved_contents_writes,
                            &mut step,
                            slot_kind,
                            &module.types.structural_types,
                            position,
                            structural_arguments,
                        )?;
                    }
                    let mut declarations = module
                        .interfaces.boundary_machines
                        .iter()
                        .filter(|declaration| declaration.id == *boundary);
                    if let (Some(declaration), None) =
                        (declarations.next(), declarations.next())
                        && slot_kind != DescriptorSlotKind::AccessOrigins
                    {
                        insert_invocation(
                            &mut graph.calls,
                            position,
                            provider_invocation(
                                operation.id,
                                &module.types.structural_types,
                                slot_kind,
                                declaration,
                                structural_arguments,
                                result,
                                &graph.place_types,
                            )?,
                        )?;
                    }
                }
                _ => {}
            }
            // Record unresolved invocations only after attempting exact
            // construction; a later cohort cannot overwrite unknown effects.
            if is_invocation && !graph.calls.contains_key(&position) {
                graph.calls.insert(position, None);
            }
            record_owned_consumption(module, machine, operation, &mut step)?;
            // Borrowing a carrier slot makes a child resource, not
            // another spelling of the parent's permission. Payload
            // and residence still follow the captured address.
            if !matches!(
                operation.kind,
                OperationKind::EstablishReference { .. }
                    | OperationKind::EstablishAccessReference { .. }
                    | OperationKind::EstablishElementView { .. }
                    | OperationKind::EstablishMutableElementView { .. }
            ) {
                step.authority_derivations = step.derivations.clone();
            }
            step.successors.push(DescriptorFlowEdge {
                edge: None,
                effects: Vec::new(),
                definitions: Vec::new(),
                target: (block.id, position.1 + 1),
                parameters: Vec::new(),
                bindings: Vec::new(),
            });
            if graph.flow.insert(position, step).is_some() {
                return None;
            }
        }
        *failure_position = None;
        *failure_successor = None;
        *failure_detail = None;
        let position = (block.id, u32::try_from(block.operations.len()).ok()?);
        *failure_position = Some(position);
        let mut step = DescriptorFlowStep::default();
        let mut edge = |identity,
                        target,
                        scalar_arrival: ScalarArrival<'_>,
                        arguments: &[StructuralArgument]|
         -> Option<()> {
            *failure_successor = Some((identity, target));
            let result = (|| -> Result<(), &'static str> {
                let block = exactly_one(machine.blocks.iter(), |block| block.id == target)
                    .ok_or("descriptor arrival has no unique target block")?;
                if arguments.len() != block.structural_parameters.len() {
                    return Err(
                        "descriptor arrival structural arity differs from target parameters",
                    );
                }
                let scalar_bindings = match scalar_arrival {
                    ScalarArrival::Values(arguments) => {
                        if arguments.len() != block.parameters.len() {
                            return Err(
                                "descriptor arrival scalar arity differs from target parameters",
                            );
                        }
                        block.parameters.iter().zip(arguments).map(|(parameter, actual)| {
                            let value = scalar_expressions.expression(*actual)
                                .ok_or("descriptor arrival scalar operand has no current typed expression")?;
                            if value.scalar_type() != parameter.scalar_type {
                                return Err("descriptor arrival scalar type differs from target parameter");
                            }
                            Ok((parameter.id, value))
                        }).collect::<Result<Vec<_>, _>>()?
                    }
                    ScalarArrival::CasePayload {
                        source,
                        path,
                        case,
                        fields,
                    } => {
                        if fields.len() != block.parameters.len() {
                            return Err(
                                "descriptor arrival case payload arity differs from target parameters",
                            );
                        }
                        let root = *graph.place_types.get(&source).ok_or(
                            "descriptor arrival case payload has no declared source type",
                        )?;
                        let captured = super::scalar_contracts::case_payload_scalar_types(
                            &module.types.structural_types, root, path, case, fields,
                        )?;
                        if captured
                            .iter()
                            .zip(&block.parameters)
                            .any(|(actual, parameter)| *actual != parameter.scalar_type)
                        {
                            return Err(
                                "descriptor arrival case payload type differs from target parameter",
                            );
                        }
                        Vec::new()
                    }
                };
                graph
                    .payload_scalar_bindings
                    .entry(position)
                    .or_default()
                    .push(scalar_bindings);
                let parameters = block
                    .structural_parameters
                    .iter()
                    .map(|parameter| DescriptorLocation::whole(parameter.place))
                    .collect::<Vec<_>>();
                let mut bindings = Vec::new();
                let mut definitions = Vec::new();
                for (argument, parameter) in
                    arguments.iter().zip(&block.structural_parameters)
                {
                    if argument.access != parameter.access {
                        return Err(
                            "descriptor arrival access differs from target parameter",
                        );
                    }
                    let source = if let Some(source) =
                        location(argument.place, &argument.path, None)
                    {
                        DescriptorFlowSource::Location(source)
                    } else {
                        DescriptorFlowSource::from_query(
                            argument.place,
                            descriptor_contents_query(
                                &module.types.structural_types, argument.place,
                                *graph.place_types.get(&argument.place)
                                    .ok_or("descriptor arrival structural source has no declared type")?,
                                &argument.path, None,
                            ),
                        )
                    };
                    step.inputs.push((Some(source.observation()), None));
                    if parameter.access == StructuralAccess::Owned
                        && parameter.multiplicity != StructuralMultiplicity::Unrestricted
                    {
                        definitions.push(source.observation());
                    }
                    let mut binding = DescriptorFlowBinding::new(
                        source,
                        DescriptorLocation::whole(parameter.place),
                        argument.access,
                        parameter.access,
                    )
                    .ok_or("descriptor arrival has no valid source-to-parameter binding")?;
                    if binding.source.exact_location().is_none()
                        && let Some(index) = argument
                            .path
                            .iter()
                            .position(|step| step.runtime_index().is_some())
                    {
                        let descriptor = location(argument.place, &argument.path[..index], None)
                            .ok_or("descriptor arrival indexed source has no exact descriptor prefix")?;
                        binding.captured_storage = Some((
                            descriptor,
                            Projection::new(&argument.path[index..]).at(position),
                        ));
                    }
                    binding.source_access = crate::structural_projection_authority(
                        machine, &module.types.structural_types, argument,
                    ).ok_or("descriptor arrival structural source has no exact projection authority")?;
                    if binding.source.exact_location().is_none() {
                        step.may_escape |= !graph.borrows_plain_element(
                            &module.types.structural_types,
                            &binding, parameter.structural_type,
                        );
                    }
                    bindings.push(binding);
                }
                step.successors.push(DescriptorFlowEdge {
                    edge: None,
                    effects: Vec::new(),
                    definitions,
                    target: (target, 0),
                    parameters,
                    bindings,
                });
                Ok(())
            })();
            if let Err(detail) = result {
                *failure_detail = Some(detail);
                return None;
            }
            *failure_successor = None;
            Some(())
        };
        match &block.terminator {
            Terminator::Jump {
                edge: identity,
                target,
                arguments,
                structural_arguments,
                ..
            } => edge(
                *identity,
                *target,
                ScalarArrival::Values(arguments),
                structural_arguments,
            )?,
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => {
                for successor in [when_true, when_false] {
                    edge(
                        successor.edge,
                        successor.target,
                        ScalarArrival::Values(&successor.arguments),
                        &successor.structural_arguments,
                    )?;
                }
            }
            Terminator::StructuralCase {
                source,
                path,
                cases,
            } => {
                for case in cases {
                    edge(
                        case.edge,
                        case.target,
                        ScalarArrival::CasePayload {
                            source: *source,
                            path,
                            case: case.case,
                            fields: &case.payload_fields,
                        },
                        &case.structural_arguments,
                    )?;
                }
            }
            Terminator::PendingNormalCompletion { storage, .. } => {
                // The successor still holds these descriptors and their backings
                // at this cut. The parked machine has yet to return a result.
                step.inputs.extend(storage.iter().map(|&place| {
                    (Some(DescriptorLocation::whole(place)), None)
                }));
            }
            Terminator::ReturnStructural { source, .. } => {
                if let Some(returns) = &mut graph.returns {
                    *failure_detail =
                        Some("descriptor structural return source has no declared type");
                    let actual = machine.structural_place_type(*source)?;
                    *failure_detail = Some(
                        "descriptor structural return has no declared structural result",
                    );
                    let expected = machine.result.structural()?.structural_type;
                    if actual != expected {
                        *failure_detail = Some(
                            "descriptor structural return source type differs from declared result",
                        );
                        return None;
                    }
                    returns.push((position, DescriptorLocation::whole(*source)));
                }
                step.inputs
                    .push((Some(DescriptorLocation::whole(*source)), None));
                step.may_escape = true;
                graph.normal_exits.push(position);
            }
            Terminator::Crash { .. } | Terminator::EliminateEmpty { .. } => {}
            Terminator::ExternalCompletion {
                structural_arguments,
                ..
            }
            | Terminator::InvokeNoNormal {
                structural_arguments,
                ..
            } => {
                let call = block
                    .terminator
                    .module_invocation()
                    .filter(|inputs| inputs.required_contract.is_none())
                    .and_then(|inputs| {
                        graph.direct_invocation(
                            module,
                            machines,
                            slot_kind,
                            terminal_psi::ModuleInvocation {
                                site: semantic_vocabulary::InvocationSite::InvokeNoNormal(
                                    block.terminator.edge(),
                                ),
                                static_reach_binding: match block.terminator {
                                    Terminator::InvokeNoNormal {
                                        static_reach_binding,
                                        ..
                                    } => static_reach_binding,
                                    _ => None,
                                },
                                inputs,
                            },
                            None,
                            selector,
                        )
                    });
                if call
                    .as_ref()
                    .is_none_or(|call| !call.arguments.iter().all(Option::is_some))
                {
                    // An absent result never proves that a call writes
                    // nothing. Unknown and public calls keep the same
                    // conservative invalidation until their owner serves them.
                    graph.unresolved_contents_writes.insert(position);
                }
                match call {
                    Some(call) => {
                        *failure_detail = Some(
                            "descriptor terminal invocation conflicts with an existing call at this site",
                        );
                        insert_invocation(&mut graph.calls, position, call)?;
                    }
                    None => {
                        graph.calls.insert(position, None);
                    }
                }
                for argument in structural_arguments {
                    if let Some(source) = location(argument.place, &argument.path, None) {
                        step.inputs.push((Some(source), None));
                    } else {
                        step.inputs
                            .push((Some(DescriptorLocation::whole(argument.place)), None));
                    }
                }
                step.may_escape = true;
            }
            _ => {
                graph.normal_exits.push(position);
            }
        }
        for (edge, actions) in block.terminator.cleanup_rosters() {
            for (action_ordinal, action) in actions.iter().enumerate() {
                if let terminal_psi::TerminalAffineCleanupAction::InvokeOwned(cleanup) =
                    action
                {
                    let callee = crate::nominal_cleanup::owned_target(
                        module.types.structural_types.iter(),
                        module.machines.iter(),
                        cleanup,
                    )
                    .inspect_err(|reason| *failure_detail = Some(*reason))
                    .ok()?;
                    *failure_detail = Some(
                        "descriptor terminal cleanup subject has no declared root type",
                    );
                    let root = *graph.place_types.get(&cleanup.subject.place)?;
                    *failure_detail = Some(
                        "descriptor terminal cleanup subject has no exact projected type",
                    );
                    if crate::runtime_structural_path_tip(
                        module.types.structural_types.iter(),
                        root,
                        &cleanup.subject.path,
                    )?
                    .id != cleanup.structural_type
                    {
                        *failure_detail = Some(
                            "descriptor terminal cleanup subject type differs from its declared carrier",
                        );
                        return None;
                    }
                    *failure_detail =
                        Some("descriptor terminal cleanup action ordinal is out of range");
                    let action_ordinal = u32::try_from(action_ordinal).ok()?;
                    *failure_detail = Some(
                        "descriptor terminal cleanup has no exact invocation correspondence",
                    );
                    let call = machine_invocation(
                        DescriptorInvocationOwner::Cleanup {
                            edge,
                            action_ordinal,
                        },
                        &module.types.structural_types,
                        slot_kind,
                        callee,
                        std::slice::from_ref(&cleanup.subject),
                        None,
                        std::iter::once(Some(root)),
                    )?;
                    // A known cleanup requires the same exact actual
                    // correspondence as a source call. An opaque frame
                    // cannot replace its retained executable effects.
                    if call.arguments.iter().any(Option::is_none)
                        || call.argument_contents.iter().any(Option::is_none)
                    {
                        *failure_detail = Some(
                            "descriptor terminal cleanup has unresolved actual backing or contents",
                        );
                        return None;
                    }
                    step.inputs.extend(
                        call.arguments
                            .iter()
                            .flatten()
                            .cloned()
                            .map(|input| (Some(input), None)),
                    );
                    *failure_detail = Some(
                        "descriptor terminal cleanup conflicts with an existing call at this site",
                    );
                    insert_invocation(&mut graph.calls, position, call)?;
                }
            }
        }
        // Selected-edge disposal is a real carrier disposition.
        // Keep its definitions apart from the destination roster:
        // every binding reads the old frontier before both are killed.
        let selected = block.terminator.successor_edges();
        if selected.len() != step.successors.len() {
            *failure_detail = Some(
                "descriptor terminator successor count differs from reconstructed arrivals",
            );
            return None;
        }
        for ((identity, target), edge) in selected.into_iter().zip(&mut step.successors) {
            *failure_successor = Some((identity, target));
            if edge.target != (target, 0) {
                *failure_detail = Some(
                    "descriptor terminator successor target differs from reconstructed arrival",
                );
                return None;
            }
            edge.edge = Some(identity);
            *failure_detail =
                Some("descriptor terminator successor has no exact cleanup roster");
            let (_, cleanup) = block
                .terminator
                .cleanup_rosters()
                .find(|(at, _)| *at == identity)?;
            for action in cleanup {
                *failure_detail =
                    Some("descriptor terminator cleanup has no exact disposal location");
                edge.definitions
                    .push(location(action.place(), action.path(), None)?);
            }
            edge.definitions.sort();
            edge.definitions.dedup();
            *failure_successor = None;
        }
        if let Terminator::StructuralCase {
            source,
            path,
            cases,
        } = &block.terminator
        {
            *failure_detail =
                Some("descriptor case terminator has no checked subject projection");
            let subject = match location(*source, path, None) {
                Some(location) => DescriptorFlowSource::Location(location),
                None => DescriptorFlowSource::Contents(
                    super::contents::descriptor_contents_query_with(
                        &module.types.structural_types,
                        *source,
                        *graph.place_types.get(source)?,
                        path,
                        None,
                        |value| scalar_expressions.expression(value),
                    )?,
                ),
            };
            graph.case_successors.insert(
                position,
                cases
                    .iter()
                    .map(|case| {
                        (
                            DescriptorCasePredicate {
                                subject: subject.clone(),
                                case: case.case,
                            },
                            true,
                        )
                    })
                    .collect(),
            );
        }
        if let Terminator::Conditional { condition, .. } = &block.terminator
            && let Some(term) = scalar_expressions.expression(*condition)
        {
            graph.payload_edge_predicates.insert(
                position,
                [true, false]
                    .into_iter()
                    .map(|polarity| {
                        Some(semantic_vocabulary::Proposition::Equal(
                            ScalarTerm::Boolean(polarity),
                            term.clone(),
                        ))
                    })
                    .collect(),
            );
        }
        if graph.normal_exits.contains(&position) {
            for parameter in &machine.structural_parameters {
                if matches!(
                    parameter.access,
                    terminal_psi::exclusive_borrow_pair_pattern!()
                ) {
                    *failure_detail = Some(
                        "descriptor normal exit has no complete borrowed parameter slots",
                    );
                    let inventory = slot_kind.inventory(
                        &module.types.structural_types,
                        parameter.place,
                        parameter.structural_type,
                    )?;
                    step.inputs.extend(
                        inventory.slots.into_iter().map(|input| (Some(input), None)),
                    );
                    for contents in inventory.unknown_contents {
                        if contents.kind != super::UnknownContentsKind::Value {
                            return None;
                        }
                        let mut carrier = contents.location;
                        if carrier.path.pop()? != DescriptorPathSegment::UnknownContents {
                            return None;
                        }
                        // Handback owes the intact unknown value. It
                        // does not resolve or end a hidden referent.
                        step.inputs.push((Some(carrier), None));
                    }
                }
            }
        }
        if graph.flow.insert(position, step).is_some() {
            *failure_detail =
                Some("descriptor terminator repeats an existing storage-flow position");
            return None;
        }
    }
    graph.finish_index_literals()
}

impl DescriptorMachine {
    /// A borrowed header retains its parent view while it is used. An element
    /// carrying a separate descriptor may retain other loans and still needs
    /// the conservative escape account.
    fn borrows_plain_element(
        &self,
        types: &[StructuralTypeDeclaration],
        binding: &DescriptorFlowBinding,
        parameter: StructuralTypeId,
    ) -> bool {
        let Some(parent) = binding.indexed_borrow_source() else {
            return false;
        };
        let parent_type = self
            .place_types
            .get(&parent.root)
            .and_then(|root| descriptor_location_type(types, *root, &parent.path))
            .and_then(|ty| find_structural_type(types, ty));
        matches!(parent_type.map(|declaration| &declaration.shape),
            Some(StructuralTypeShape::ElementView { element, .. }) if *element == parameter
        ) && plain_owned_shape(types.iter(), parameter)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum CapturedBacking {
    StorageAddress,
    DescriptorPayload,
    AccessInterpretation,
    ByteWindow,
}

pub(crate) fn captured_storage_source(
    types: &[StructuralTypeDeclaration],
    source: PlaceId,
    path: &[StructuralPathSegment],
    root_type: StructuralTypeId,
    position: DescriptorPosition,
) -> Option<(DescriptorLocation, Option<Projection>, CapturedBacking)> {
    let prefix_length = path
        .iter()
        .position(|segment| matches!(segment, StructuralPathSegment::RuntimeIndex { .. }))
        .unwrap_or(path.len());
    let location =
        descriptor_location(types, source, root_type, &path[..prefix_length], None)?;
    if prefix_length == path.len() {
        return Some((location, None, CapturedBacking::StorageAddress));
    }
    let prefix_type = descriptor_location_type(types, root_type, &location.path)?;
    let suffix = &path[prefix_length..];
    let prefix_shape = &find_structural_type(types, prefix_type)?.shape;
    let capture = if matches!(prefix_shape, StructuralTypeShape::ByteSequence(_)) {
        CapturedBacking::ByteWindow
    } else if plain_owned_shape(types.iter(), prefix_type) {
        CapturedBacking::StorageAddress
    } else if is_descriptor(types.iter(), prefix_type) {
        CapturedBacking::DescriptorPayload
    } else if crate::access::access_view_plans::is_access_projection_shape(
        types,
        prefix_type,
    ) {
        CapturedBacking::AccessInterpretation
    } else {
        return None;
    };
    if matches!(capture, CapturedBacking::ByteWindow)
        && suffix.iter().any(|segment| {
            !matches!(
                segment,
                StructuralPathSegment::RuntimeIndex { .. }
                    | StructuralPathSegment::FixedIndex(_)
            )
        })
    {
        return None;
    }
    if crate::runtime_structural_path_tip(types.iter(), prefix_type, suffix).is_none()
        && crate::primitive_projection_type(types.iter(), prefix_type, suffix).is_none()
    {
        return None;
    }
    Some((
        location,
        Some(Projection::new(suffix).at(position)),
        capture,
    ))
}
fn destination_root(kind: &OperationKind) -> Option<&PlaceId> {
    match kind {
        OperationKind::EstablishByteSequenceLiteral { destination, .. } => {
            Some(destination)
        }
        _ => None,
    }
}

fn mark_opaque_call_occupancy(
    place_types: &BTreeMap<PlaceId, StructuralTypeId>,
    unresolved_contents_writes: &mut BTreeSet<DescriptorPosition>,
    step: &mut DescriptorFlowStep,
    slot_kind: DescriptorSlotKind,
    types: &[StructuralTypeDeclaration],
    position: DescriptorPosition,
    actuals: &[StructuralArgument],
) -> Option<()> {
    step.may_escape = true;
    if actuals.iter().any(|actual| {
        place_types.get(&actual.place).is_none_or(|root_type| {
            slot_kind.may_write_indirect_contents(types, *root_type, actual)
        })
    }) {
        unresolved_contents_writes.insert(position);
    }
    for actual in actuals
        .iter()
        .filter(|actual| actual.access != StructuralAccess::SharedBorrow)
    {
        step.definitions.extend(slot_kind.slots(
            types,
            actual.place,
            *place_types.get(&actual.place)?,
        )?);
    }
    Some(())
}

fn selected_effects_machine(
    module: &TerminalModule,
    machine: &TerminalMachine,
    selector_graph: Option<&[DescriptorMachine]>,
    selector: semantic_vocabulary::ValueId,
    interface: u32,
    slot: u32,
) -> Option<semantic_vocabulary::MachineId> {
    #[derive(Clone, PartialEq, Eq)]
    struct SelectedTable(u32);
    let table = crate::unchanged_scalar_identity(selector, |value| {
        let mut definitions = machine.operations().filter(|operation| {
            operation
                .result
                .scalar_ref()
                .is_some_and(|result| result.id == value)
        });
        if let Some(operation) = definitions.next() {
            if definitions.next().is_some() {
                return None;
            }
            if let OperationKind::SelectImplementation { table } = operation.kind {
                return Some(crate::UnchangedScalarOrigin::Known(SelectedTable(table)));
            }
            let selector_machine =
                selector_graph?.iter().find(|row| row.id == machine.id)?;
            let sources = super::unchanged_stored_scalar_sources(
                &module.types.structural_types,
                selector_graph?,
                machine.id,
                value,
                |(block, index)| {
                    let Some(operation) = machine
                        .blocks
                        .iter()
                        .find(|row| row.id == block)
                        .and_then(|block| block.operations.get(index as usize))
                    else {
                        return Vec::new();
                    };
                    super::scalar_storage_writes(
                        module,
                        |root| selector_machine.place_types.get(&root).copied(),
                        operation,
                    )
                    .and_then(Result::ok)
                    .unwrap_or_default()
                },
            )?;
            return Some(crate::UnchangedScalarOrigin::Incoming(sources));
        }
        let mut owners = machine.blocks.iter().filter_map(|block| {
            block
                .parameters
                .iter()
                .position(|parameter| parameter.id == value)
                .map(|ordinal| (block.id, ordinal))
        });
        let (owner, ordinal) = owners.next()?;
        if owners.next().is_some() || owner == machine.entry {
            return None;
        }
        let mut incoming = Vec::new();
        for block in &machine.blocks {
            if crate::control_graph::reaches_by_case(block, owner) {
                return None;
            }
            for binding in crate::control_graph::forwarded_bindings(block) {
                if binding.target == owner {
                    incoming.push(*binding.arguments?.get(ordinal)?);
                }
            }
        }
        Some(crate::UnchangedScalarOrigin::Incoming(incoming))
    })?
    .0;
    let target = super::selected_stored_effects_row(
        &module.execution.dynamic_dispatch,
        table,
        interface,
        slot,
    )?;
    let signature = module
        .execution.dynamic_dispatch
        .interface(interface)?
        .slots
        .get(usize::try_from(slot).ok()?)?;
    let mut bodies = module.machines.iter().filter(|body| body.id == target);
    let body = bodies.next()?;
    if bodies.next().is_some()
        || matches!(signature.result, BoundaryMachineResult::Structural(_))
        || matches!(&signature.result, BoundaryMachineResult::Scalar(result)
            if result.qualifications != semantic_vocabulary::ScalarQualificationSetId::ZERO)
        || !signature.result.matches_machine_result(&body.result)
        || body.parameters.len() != signature.scalar_parameters.len()
        || !body
            .parameters
            .iter()
            .zip(&signature.scalar_parameters)
            .all(|(actual, expected)| actual.scalar_type == *expected)
        || body.structural_parameters.len() != signature.structural_parameters.len()
        || !body
            .structural_parameters
            .iter()
            .zip(&signature.structural_parameters)
            .all(|(actual, expected)| {
                actual.position == expected.position
                    && actual.structural_type == expected.structural_type
                    && actual.access == expected.access
                    && actual.multiplicity == expected.multiplicity
            })
    {
        return None;
    }
    Some(target)
}

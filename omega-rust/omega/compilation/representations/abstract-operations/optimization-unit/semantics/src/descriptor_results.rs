//! Current Phi operations and successor bindings adapted to the shared kernel.
//! Catalog rows authenticate declarations; one cohort graph owns both returned
//! origins and position-sensitive stored descriptor effects.
use abstract_operations::{AbstractOperation as O, StructuralCopyPath};
use optimization_unit::{PsiOptimizationFunction, PsiOptimizationUnit};
use semantic_vocabulary::{MachineId, PlaceId, StructuralPlaceKind};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{
    StructuralAccess, StructuralMultiplicity, StructuralPathSegment, StructuralTypeDeclaration,
    StructuralTypeShape,
};
use terminal_semantics::borrowed_storage::Projection;
use terminal_semantics::descriptor_lifetimes::{
    DescriptorFlowEdge, DescriptorFlowStep, DescriptorLocation, descriptor_location,
};
use terminal_semantics::descriptor_results::{
    self as kernel, DescriptorDefinition, DescriptorInvocation, DescriptorMachine, DescriptorPlace,
};

pub fn optimized_graph(unit: &PsiOptimizationUnit) -> Option<Vec<DescriptorMachine>> {
    optimized_catalog_graph(unit.functions.iter(), &unit.structural_types)
}

/// Prepare current executable bodies once, without retained producer summaries.
pub fn optimized_catalog_graph<'a>(
    functions: impl IntoIterator<Item = &'a PsiOptimizationFunction>,
    types: &[StructuralTypeDeclaration],
) -> Option<Vec<DescriptorMachine>> {
    let functions = functions.into_iter().collect::<Vec<_>>();
    let mut identities = BTreeSet::new();
    functions
        .iter()
        .map(|function| {
            if !identities.insert(function.machine) {
                return None;
            }
            adapt(types, &functions, function)
        })
        .collect()
}

pub fn call_origins(
    unit: &PsiOptimizationUnit,
    caller: MachineId,
    result: PlaceId,
) -> Option<Vec<kernel::DescriptorCallOrigin>> {
    kernel::reconstruct_call_origins(
        &unit.structural_types,
        &optimized_graph(unit)?,
        caller,
        result,
    )
}

fn adapt(
    types: &[StructuralTypeDeclaration],
    functions: &[&PsiOptimizationFunction],
    function: &PsiOptimizationFunction,
) -> Option<DescriptorMachine> {
    let mut graph = DescriptorMachine {
        id: function.machine,
        entry: (function.entry, 0),
        returns: function
            .result
            .structural()
            .filter(|result| {
                result.multiplicity == StructuralMultiplicity::Unrestricted
                    && kernel::is_shared_descriptor(types.iter(), result.structural_type)
            })
            .map(|_| Vec::new()),
        places: Vec::new(),
        place_types: BTreeMap::new(),
        flow: BTreeMap::new(),
        originations: BTreeMap::new(),
        projections: BTreeMap::new(),
        calls: BTreeMap::new(),
        normal_exits: Vec::new(),
    };
    let declaration = |place| {
        let mut matching = function
            .structural_places
            .iter()
            .filter(|row| row.id == place);
        let row = matching.next()?;
        matching.next().is_none().then_some(row)
    };
    for (position, parameter) in function.structural_parameters.iter().enumerate() {
        if declaration(parameter.place)?.kind
            != (StructuralPlaceKind::Parameter {
                position: parameter.position,
                is_self: parameter.is_self,
            })
        {
            return None;
        }
        if graph
            .place_types
            .insert(parameter.place, parameter.structural_type)
            .is_some()
        {
            return None;
        }
        graph.places.push(DescriptorPlace {
            place: parameter.place,
            definition: DescriptorDefinition::Parameter {
                position,
                access: parameter.access,
                structural_type: parameter.structural_type,
            },
        });
    }
    for block in &function.blocks {
        for (position, parameter) in block.structural_parameters.iter().enumerate() {
            if declaration(parameter.place)?.kind
                != (StructuralPlaceKind::BlockParameter {
                    block: block.id,
                    position: u32::try_from(position).ok()?,
                })
            {
                return None;
            }
            if graph
                .place_types
                .insert(parameter.place, parameter.structural_type)
                .is_some()
            {
                return None;
            }
            graph.places.push(DescriptorPlace {
                place: parameter.place,
                definition: DescriptorDefinition::Local,
            });
        }
        for node in &block.nodes {
            if let O::EstablishByteSequenceLiteral {
                place,
                structural_type,
                ..
            }
            | O::EstablishTrivialAffineLocal {
                place,
                structural_type,
                ..
            } = &node.operation
            {
                if declaration(place.id)? != place
                    || graph
                        .place_types
                        .insert(place.id, structural_type.id)
                        .is_some()
                {
                    return None;
                }
                graph.places.push(DescriptorPlace {
                    place: place.id,
                    definition: if matches!(node.operation, O::EstablishByteSequenceLiteral { .. })
                    {
                        DescriptorDefinition::Literal
                    } else {
                        DescriptorDefinition::Local
                    },
                });
            } else if let Some((producer, result)) = node.operation.structural_result() {
                if declaration(result.place)?.kind
                    != (StructuralPlaceKind::OperationResult {
                        producer,
                        structural_type: result.structural_type,
                    })
                    || graph
                        .place_types
                        .insert(result.place, result.structural_type)
                        .is_some()
                {
                    return None;
                }
                graph.places.push(DescriptorPlace {
                    place: result.place,
                    definition: DescriptorDefinition::Local,
                });
            }
        }
    }
    let location = |root, path: &[StructuralPathSegment], field| {
        descriptor_location(types, root, *graph.place_types.get(&root)?, path, field)
    };
    for block in &function.blocks {
        for (index, node) in block.nodes.iter().enumerate() {
            let position = (block.id, u32::try_from(index).ok()?);
            let operation = &node.operation;
            let result = operation.structural_result().map(|(_, result)| result);
            let mut step = DescriptorFlowStep {
                inputs: crate::unit_validation::function_structure::operation_place_inputs(
                    operation,
                )
                .into_iter()
                .map(DescriptorLocation::whole)
                .collect(),
                ..Default::default()
            };
            if let Some(result) = result {
                let destination = DescriptorLocation::whole(result.place);
                step.definitions.push(destination.clone());
                if !kernel::is_shared_descriptor(types.iter(), result.structural_type) {
                    graph.originations.entry(position).or_default().push((
                        destination,
                        kernel::DescriptorCallOrigin::Argument {
                            source: result.place,
                            projection: Projection::new(&[]),
                        },
                    ));
                }
            }
            match operation {
                O::EstablishByteSequenceLiteral { place, .. } => {
                    let destination = DescriptorLocation::whole(place.id);
                    step.definitions.push(destination.clone());
                    graph.originations.entry(position).or_default().push((
                        destination,
                        kernel::DescriptorCallOrigin::Literal {
                            machine: function.machine,
                            place: place.id,
                        },
                    ));
                }
                O::EstablishZeroedStructural { result, .. } => {
                    for slot in
                        kernel::descriptor_slots(types, result.place, result.structural_type)?
                    {
                        graph
                            .originations
                            .entry(position)
                            .or_default()
                            .push((slot, kernel::DescriptorCallOrigin::Empty));
                    }
                }
                O::EstablishElementView { result, source, .. }
                | O::EstablishReference { result, source, .. } => {
                    let destination = DescriptorLocation::whole(result.place);
                    if let Some(source) = location(source.place, &source.path, None) {
                        if let O::EstablishElementView {
                            source: argument,
                            element,
                            region,
                            ..
                        } = operation
                            && !element_coordinates_preserved(
                                function,
                                types.iter(),
                                argument,
                                *element,
                                *region,
                            )
                        {
                            // Location owns the source prefix. Later target-element
                            // coordinates must not refine differently typed backing.
                            let mut projection = Projection::new(&[]);
                            projection.exact = false;
                            graph.projections.insert(
                                (position, source.clone(), destination.clone()),
                                projection,
                            );
                        }
                        step.derivations.push((source, destination.clone()));
                    } else {
                        step.may_escape = true;
                    }
                    if let Some(origins) = graph.originations.get_mut(&position) {
                        origins.retain(|(location, _)| location != &destination);
                    }
                }
                O::ElementViewSubslice {
                    result,
                    source,
                    start,
                    end,
                    ..
                }
                | O::ByteSequenceSubslice {
                    result,
                    source,
                    start,
                    end,
                    ..
                } => {
                    let source = DescriptorLocation::whole(*source);
                    let destination = DescriptorLocation::whole(result.place);
                    step.derivations.push((source.clone(), destination.clone()));
                    graph.projections.insert(
                        (position, source, destination),
                        Projection::window(*start, *end, position),
                    );
                }
                O::StructuralLeafCopy {
                    result,
                    source,
                    path,
                    ..
                } => {
                    let destination = DescriptorLocation::whole(result.place);
                    let source = match path {
                        StructuralCopyPath::Ordinary(path) => location(*source, path, None),
                        _ => None,
                    };
                    if let Some(source) = source {
                        step.inputs = vec![source.clone()];
                        step.derivations.push((source, destination.clone()));
                    } else {
                        step.may_escape = true;
                    }
                    // Copying an aggregate makes fresh carrier storage while
                    // its descendant descriptor slots retain their backing.
                    if (kernel::is_shared_descriptor(types.iter(), result.structural_type)
                        || types.iter().any(|ty| {
                            ty.id == result.structural_type
                                && matches!(ty.shape, StructuralTypeShape::Reference { .. })
                        }))
                        && let Some(origins) = graph.originations.get_mut(&position)
                    {
                        origins.retain(|(location, _)| location != &destination);
                    }
                }
                O::StructuralByteSequenceFieldLength {
                    source,
                    path,
                    field,
                    ..
                }
                | O::StructuralByteSequenceFieldRead {
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
                O::MoveStructuralField {
                    source,
                    path,
                    field,
                    ..
                } => {
                    if let Some(source) = location(source.place(), path, *field) {
                        let destination = DescriptorLocation::whole(result?.place);
                        step.inputs = vec![source.clone()];
                        step.definitions.push(source.clone());
                        step.derivations.push((source, destination));
                    } else {
                        step.may_escape = true;
                    }
                }
                O::StoreStructuralField {
                    destination,
                    path,
                    field,
                    value,
                    ..
                } => {
                    if let (Some(destination), Some(source)) = (
                        location(destination.place(), path, *field),
                        location(value.place, &value.path, None),
                    ) {
                        step.definitions.push(destination.clone());
                        step.derivations.push((source, destination.clone()));
                        // Reseating contents retains the destination storage home.
                        if destination.path.is_empty()
                            && !kernel::is_shared_descriptor(
                                types.iter(),
                                *graph.place_types.get(&destination.root)?,
                            )
                        {
                            graph.originations.entry(position).or_default().push((
                                destination.clone(),
                                kernel::DescriptorCallOrigin::Argument {
                                    source: destination.root,
                                    projection: Projection::new(&[]),
                                },
                            ));
                        }
                    } else {
                        step.definitions.extend(kernel::descriptor_slots(
                            types,
                            destination.place(),
                            *graph.place_types.get(&destination.place())?,
                        )?);
                        step.may_escape = true;
                    }
                }
                O::StructuralByteViewFieldStore {
                    destination,
                    path,
                    field,
                    source,
                    ..
                } => {
                    if let Some(destination) = location(*destination, path, Some(*field)) {
                        step.definitions.push(destination.clone());
                        step.derivations
                            .push((DescriptorLocation::whole(*source), destination));
                    } else {
                        step.definitions.extend(kernel::descriptor_slots(
                            types,
                            *destination,
                            *graph.place_types.get(destination)?,
                        )?);
                        step.may_escape = true;
                    }
                    step.inputs = vec![DescriptorLocation::whole(*source)];
                }
                O::EstablishRecord { result, fields, .. }
                | O::EstablishCase { result, fields, .. } => {
                    let path = match operation {
                        O::EstablishCase { result_case, .. } => {
                            vec![StructuralPathSegment::Case(*result_case)]
                        }
                        _ => Vec::new(),
                    };
                    for field in fields {
                        if let terminal_psi::RecordFieldValue::Structural(source) = &field.value {
                            let destination = location(result.place, &path, Some(field.field))?;
                            if let Some(source) = location(source.place, &source.path, None) {
                                step.derivations.push((source, destination));
                            } else {
                                step.may_escape = true;
                            }
                        }
                    }
                }
                O::CallStructural {
                    callee,
                    structural_arguments,
                    dynamic_arguments,
                    claim_transfers,
                    result: call_result,
                    ..
                } => {
                    let mut matching = functions
                        .iter()
                        .filter(|function| function.machine == *callee);
                    if let Some(callee) = matching.next().copied() {
                        if matching.next().is_some()
                            || callee.structural_parameters.len() != structural_arguments.len()
                        {
                            return None;
                        }
                        let mut arguments = Vec::new();
                        let mut exact_arguments = dynamic_arguments.is_empty();
                        for (actual, formal) in structural_arguments
                            .iter()
                            .zip(&callee.structural_parameters)
                        {
                            if actual.access != formal.access {
                                return None;
                            }
                            let actual = dynamic_arguments
                                .is_empty()
                                .then(|| location(actual.place, &actual.path, None))
                                .flatten();
                            exact_arguments &= actual.is_some();
                            arguments.push(actual);
                        }
                        let descriptor_result = result.filter(|result| {
                            kernel::is_shared_descriptor(types.iter(), result.structural_type)
                        });
                        if let Some(result) = descriptor_result {
                            let expected = callee.result.structural()?;
                            if result.structural_type != expected.structural_type
                                || result.multiplicity != expected.multiplicity
                                || result.qualifications != expected.qualifications
                                || result.projected_qualifications
                                    != expected.projected_qualifications
                                || !result.claims.is_empty()
                            {
                                return None;
                            }
                        }
                        graph.calls.insert(
                            position,
                            DescriptorInvocation {
                                callee: callee.machine,
                                arguments,
                                result: descriptor_result
                                    .map(|result| DescriptorLocation::whole(result.place)),
                            },
                        );
                        if !exact_arguments {
                            step.may_escape = true;
                            for actual in structural_arguments
                                .iter()
                                .filter(|actual| actual.access != StructuralAccess::SharedBorrow)
                            {
                                step.definitions.extend(writable_descriptor_slots(
                                    types,
                                    &graph.place_types,
                                    actual,
                                )?);
                            }
                        }
                    } else {
                        // An unavailable body supplies no stored-origin summary.
                        // Its signature still proves that a scalar/Unit result
                        // and closed writable storage cannot retain an alias.
                        let declarations = types.iter().map(|ty| (ty.id, ty)).collect();
                        let mut closed_storage = BTreeMap::new();
                        let closed = matches!(
                            call_result,
                            abstract_operations::AbstractStructuralCallResult::Unit
                                | abstract_operations::AbstractStructuralCallResult::Scalar(_)
                        ) && dynamic_arguments.is_empty()
                            && claim_transfers.is_empty()
                            && structural_arguments.iter().all(|actual| {
                                actual.access == StructuralAccess::SharedBorrow
                                    || graph.place_types.get(&actual.place).is_some_and(|ty| {
                                        crate::current_ownership::argument_storage_is_closed(
                                            &declarations,
                                            *ty,
                                            &actual.path,
                                            actual.access,
                                            &mut closed_storage,
                                        )
                                    })
                            });
                        step.may_escape |= !closed;
                        for actual in structural_arguments
                            .iter()
                            .filter(|actual| actual.access != StructuralAccess::SharedBorrow)
                        {
                            // Unknown replacement invalidates contained slots;
                            // it never invents a source for their later reads.
                            step.definitions.extend(writable_descriptor_slots(
                                types,
                                &graph.place_types,
                                actual,
                            )?);
                        }
                    }
                }
                O::BoundaryCall {
                    structural_arguments,
                    ..
                } => {
                    step.may_escape = true;
                    for actual in structural_arguments
                        .iter()
                        .filter(|actual| actual.access != StructuralAccess::SharedBorrow)
                    {
                        step.definitions.extend(writable_descriptor_slots(
                            types,
                            &graph.place_types,
                            actual,
                        )?);
                    }
                }
                O::CallDynamic { .. } | O::StoreDynamicDescriptor { .. } => {
                    step.may_escape = true;
                }
                O::ReturnStructural { source, .. } => {
                    if let Some(returns) = &mut graph.returns {
                        if graph.place_types.get(source)
                            != Some(&function.result.structural()?.structural_type)
                        {
                            return None;
                        }
                        returns.push((position, DescriptorLocation::whole(*source)));
                    }
                    graph.normal_exits.push(position);
                    step.may_escape = true;
                }
                O::Return { .. } | O::ReturnUnit { .. } => {
                    graph.normal_exits.push(position);
                }
                _ => {}
            }
            step.authority_derivations = step.derivations.clone();
            if graph.normal_exits.contains(&position) {
                for parameter in &function.structural_parameters {
                    if matches!(
                        parameter.access,
                        StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
                    ) {
                        step.inputs.extend(kernel::descriptor_slots(
                            types,
                            parameter.place,
                            parameter.structural_type,
                        )?);
                    }
                }
            }
            if node.successors.is_empty() {
                if index + 1 < block.nodes.len() {
                    step.successors.push(DescriptorFlowEdge {
                        target: (block.id, position.1 + 1),
                        parameters: Vec::new(),
                        bindings: Vec::new(),
                    });
                }
            } else {
                for edge in &node.successors {
                    let mut matching = function
                        .blocks
                        .iter()
                        .filter(|block| block.id == edge.target);
                    let target = matching.next()?;
                    if matching.next().is_some()
                        || target.structural_parameters.len() != edge.structural_bindings.len()
                    {
                        return None;
                    }
                    let parameters = target
                        .structural_parameters
                        .iter()
                        .map(|parameter| DescriptorLocation::whole(parameter.place))
                        .collect::<Vec<_>>();
                    let mut bindings = Vec::new();
                    for parameter in &target.structural_parameters {
                        let mut matching = edge
                            .structural_bindings
                            .iter()
                            .filter(|binding| binding.parameter == parameter.place);
                        let binding = matching.next()?;
                        if matching.next().is_some() {
                            return None;
                        }
                        if let Some(source) =
                            location(binding.argument.place, &binding.argument.path, None)
                        {
                            bindings.push((source, DescriptorLocation::whole(parameter.place)));
                        } else {
                            step.inputs
                                .push(DescriptorLocation::whole(binding.argument.place));
                            step.may_escape = true;
                        }
                    }
                    step.inputs
                        .extend(bindings.iter().map(|(source, _)| source.clone()));
                    step.successors.push(DescriptorFlowEdge {
                        target: (edge.target, 0),
                        parameters,
                        bindings,
                    });
                }
            }
            if graph.flow.insert(position, step).is_some() {
                return None;
            }
        }
    }
    Some(graph)
}

/// Borrowing a descriptor grants its backing access, not replacement of the
/// descriptor's own carrier. Only contained writable slots can be reseated.
fn writable_descriptor_slots(
    types: &[StructuralTypeDeclaration],
    place_types: &BTreeMap<PlaceId, semantic_vocabulary::StructuralTypeId>,
    actual: &terminal_psi::StructuralArgument,
) -> Option<Vec<DescriptorLocation>> {
    let ty = *place_types.get(&actual.place)?;
    let selected = descriptor_location(types, actual.place, ty, &actual.path, None);
    Some(
        kernel::descriptor_slots(types, actual.place, ty)?
            .into_iter()
            .filter(|slot| {
                !slot.path.is_empty()
                    && selected
                        .as_ref()
                        .is_none_or(|selected| selected.contains(slot))
            })
            .collect(),
    )
}

/// Coordinate precision is distinct from backing identity. A representation
/// view can rename fields or change index units while borrowing the same home.
pub(crate) fn element_coordinates_preserved<'a>(
    function: &PsiOptimizationFunction,
    types: impl Iterator<Item = &'a StructuralTypeDeclaration> + Clone,
    source: &terminal_psi::StructuralArgument,
    element: semantic_vocabulary::StructuralTypeId,
    region: terminal_psi::ElementViewRegion,
) -> bool {
    let Some(root) = crate::unit_validation::operation_contracts::structural_source_contract(
        function,
        source.place,
        false,
    ) else {
        return false;
    };
    let Some(collection) = terminal_semantics::runtime_structural_path_tip(
        types.clone(),
        root.structural_type,
        &source.path,
    ) else {
        return false;
    };
    terminal_semantics::collection_view_preserves_coordinates(types, collection.id, element, region)
}

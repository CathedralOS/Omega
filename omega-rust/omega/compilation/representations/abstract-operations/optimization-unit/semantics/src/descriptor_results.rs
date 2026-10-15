//! Current Phi operations and successor bindings adapted to the shared kernel.
//! Catalog rows authenticate declarations; one cohort graph owns both returned
//! origins and position-sensitive stored descriptor effects.
use abstract_operations::{AbstractOperation as O, StructuralCopyPath};
use optimization_unit::{PsiOptimizationFunction, PsiOptimizationUnit};
use semantic_vocabulary::{MachineId, PlaceId, StructuralPlaceKind};
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{
    BoundaryMachineDeclaration, StructuralAccess, StructuralPathSegment, StructuralTypeDeclaration,
    StructuralTypeShape,
};
use terminal_semantics::borrowed_storage::Projection;
use terminal_semantics::descriptor_lifetimes::{
    DescriptorFlowEdge, DescriptorFlowStep, DescriptorLocation, descriptor_location,
};
use terminal_semantics::descriptor_results::{
    self as kernel, DescriptorDefinition, DescriptorInvocation, DescriptorMachine, DescriptorPlace,
};

/// Adapt current flat abstract bodies through the same block reconstruction
/// and descriptor flow used by optimization-unit consumers. No retained source
/// summary supplies origin authority after transformations.
pub fn abstract_catalog_graph<'function, 'boundary>(
    functions: impl IntoIterator<Item = &'function abstract_operations::AbstractFunction>,
    boundaries: impl IntoIterator<Item = &'boundary BoundaryMachineDeclaration>,
    types: &[StructuralTypeDeclaration],
) -> Option<Vec<DescriptorMachine>> {
    let functions = functions
        .into_iter()
        .map(optimization_unit::reconstruct_psi_optimization_function_seed)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    optimized_catalog_graph(functions.iter(), boundaries, types, |_, node| {
        Some(Cow::Borrowed(&node.operation))
    })
}

/// Ordinary reference-pointer contents reconstructed from current flat bodies.
pub fn abstract_reference_catalog_graph<'function, 'boundary>(
    functions: impl IntoIterator<Item = &'function abstract_operations::AbstractFunction>,
    boundaries: impl IntoIterator<Item = &'boundary BoundaryMachineDeclaration>,
    types: &[StructuralTypeDeclaration],
) -> Option<Vec<DescriptorMachine>> {
    let functions = functions
        .into_iter()
        .map(optimization_unit::reconstruct_psi_optimization_function_seed)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    optimized_reference_catalog_graph(functions.iter(), boundaries, types, |_, node| {
        Some(Cow::Borrowed(&node.operation))
    })
}

/// Adapt flat current bodies with their independently selected execution and
/// explicit guarded-boundary obligations. Portable callers use the default API.
pub fn abstract_reference_catalog_graph_with_boundary_preservation<'function, 'boundary>(
    functions: impl IntoIterator<Item = &'function abstract_operations::AbstractFunction>,
    boundaries: impl IntoIterator<Item = &'boundary BoundaryMachineDeclaration>,
    types: &[StructuralTypeDeclaration],
    operation: impl for<'node> Fn(
        &PsiOptimizationFunction,
        &'node optimization_unit::OptimizationNode,
    ) -> Option<Cow<'node, O>>,
    preserves_boundary: impl Fn(&PsiOptimizationFunction, &optimization_unit::OptimizationNode) -> bool,
) -> Option<Vec<DescriptorMachine>> {
    let functions = functions
        .into_iter()
        .map(optimization_unit::reconstruct_psi_optimization_function_seed)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    optimized_reference_catalog_graph_with_boundary_preservation(
        functions.iter(),
        boundaries,
        types,
        operation,
        preserves_boundary,
    )
}

pub(crate) fn optimized_graph(unit: &PsiOptimizationUnit) -> Option<Vec<DescriptorMachine>> {
    optimized_catalog_graph(
        unit.functions.iter(),
        &unit.boundary_machines,
        &unit.structural_types,
        |_, node| Some(Cow::Borrowed(&node.operation)),
    )
}

/// Prepare current executable bodies once, without retained producer summaries.
/// `operation` chooses the execution relation available to this consumer.
/// Portable consumers retain boundary declarations; realization consumers can
/// supply an independently admitted callee using the same call-flow adapter.
/// Selection cannot change operands or result custody. No body is cloned and
/// no signature-only loan claim substitutes for an executable return origin.
pub fn optimized_catalog_graph<'function, 'boundary>(
    functions: impl IntoIterator<Item = &'function PsiOptimizationFunction>,
    boundaries: impl IntoIterator<Item = &'boundary BoundaryMachineDeclaration>,
    types: &[StructuralTypeDeclaration],
    operation: impl for<'node> Fn(
        &PsiOptimizationFunction,
        &'node optimization_unit::OptimizationNode,
    ) -> Option<Cow<'node, O>>,
) -> Option<Vec<DescriptorMachine>> {
    catalog_graph(
        functions,
        boundaries,
        types,
        operation,
        kernel::DescriptorSlotKind::Views,
        |_, _| false,
    )
}

/// Independently track ordinary pointer slots, using the same executable flow
/// and call substitution as view backing. This grants no copy or access rights.
pub fn optimized_reference_catalog_graph<'function, 'boundary>(
    functions: impl IntoIterator<Item = &'function PsiOptimizationFunction>,
    boundaries: impl IntoIterator<Item = &'boundary BoundaryMachineDeclaration>,
    types: &[StructuralTypeDeclaration],
    operation: impl for<'node> Fn(
        &PsiOptimizationFunction,
        &'node optimization_unit::OptimizationNode,
    ) -> Option<Cow<'node, O>>,
) -> Option<Vec<DescriptorMachine>> {
    optimized_reference_catalog_graph_with_boundary_preservation(
        functions,
        boundaries,
        types,
        operation,
        |_, _| false,
    )
}

/// Reference storage may survive an opaque boundary only when the realization
/// supplies a specific independently discharged preservation obligation. This
/// callback grants no portable fact: selected Omega bodies must instead use
/// `operation` to supply their actual executable call relation. A guarded foreign
/// implementation must emit its exact guard and every receiving audit must
/// reconstruct the guard from the authenticated binding and byte geometry.
pub fn optimized_reference_catalog_graph_with_boundary_preservation<'function, 'boundary>(
    functions: impl IntoIterator<Item = &'function PsiOptimizationFunction>,
    boundaries: impl IntoIterator<Item = &'boundary BoundaryMachineDeclaration>,
    types: &[StructuralTypeDeclaration],
    operation: impl for<'node> Fn(
        &PsiOptimizationFunction,
        &'node optimization_unit::OptimizationNode,
    ) -> Option<Cow<'node, O>>,
    preserves_boundary: impl Fn(&PsiOptimizationFunction, &optimization_unit::OptimizationNode) -> bool,
) -> Option<Vec<DescriptorMachine>> {
    catalog_graph(
        functions,
        boundaries,
        types,
        operation,
        kernel::DescriptorSlotKind::References,
        preserves_boundary,
    )
}

fn catalog_graph<'function, 'boundary>(
    functions: impl IntoIterator<Item = &'function PsiOptimizationFunction>,
    boundaries: impl IntoIterator<Item = &'boundary BoundaryMachineDeclaration>,
    types: &[StructuralTypeDeclaration],
    operation: impl for<'node> Fn(
        &PsiOptimizationFunction,
        &'node optimization_unit::OptimizationNode,
    ) -> Option<Cow<'node, O>>,
    slot_kind: kernel::DescriptorSlotKind,
    preserves_boundary: impl Fn(&PsiOptimizationFunction, &optimization_unit::OptimizationNode) -> bool,
) -> Option<Vec<DescriptorMachine>> {
    let functions = functions.into_iter().collect::<Vec<_>>();
    let boundaries = boundaries.into_iter().collect::<Vec<_>>();
    let mut identities = BTreeSet::new();
    functions
        .iter()
        .map(|function| {
            if !identities.insert(function.machine) {
                return None;
            }
            adapt(
                types,
                &boundaries,
                &functions,
                function,
                &operation,
                slot_kind,
                &preserves_boundary,
            )
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

/// An aggregate owns its returned carrier while each shared byte slot retains
/// independently replayed nonlocal backing. Rebuild from current functions:
/// an optimizer's changed constructor, arrival or callee cannot reuse Terminal
/// provenance. Root descriptors and reference returns keep their own contracts.
pub(crate) fn owned_aggregate_descriptor_returns<'function, 'boundary>(
    function: &PsiOptimizationFunction,
    functions: impl IntoIterator<Item = &'function PsiOptimizationFunction>,
    boundaries: impl IntoIterator<Item = &'boundary BoundaryMachineDeclaration>,
    types: &[StructuralTypeDeclaration],
) -> bool {
    let Some(graph) = optimized_catalog_graph(functions, boundaries, types, |_, node| {
        Some(Cow::Borrowed(&node.operation))
    }) else {
        return false;
    };
    let Some(returns) = kernel::descriptor_machine(&graph, function.machine)
        .and_then(|machine| machine.returns.as_ref())
    else {
        return false;
    };
    !returns.is_empty()
        && returns.iter().all(|(position, location)| {
            location.path.is_empty()
                && crate::unit_validation::operation_contracts::structural_source_contract(
                    function,
                    location.root,
                )
                .is_some_and(|source| source.access == StructuralAccess::Owned)
                && kernel::reconstruct_return_slots_external(
                    types,
                    &graph,
                    function.machine,
                    *position,
                    location.root,
                )
        })
}

fn adapt(
    types: &[StructuralTypeDeclaration],
    boundaries: &[&BoundaryMachineDeclaration],
    functions: &[&PsiOptimizationFunction],
    function: &PsiOptimizationFunction,
    operation: &impl for<'node> Fn(
        &PsiOptimizationFunction,
        &'node optimization_unit::OptimizationNode,
    ) -> Option<Cow<'node, O>>,
    slot_kind: kernel::DescriptorSlotKind,
    preserves_boundary: &impl Fn(&PsiOptimizationFunction, &optimization_unit::OptimizationNode) -> bool,
) -> Option<DescriptorMachine> {
    let mut graph = DescriptorMachine {
        id: function.machine,
        entry: (function.entry, 0),
        returns: function
            .result
            .structural()
            .filter(|result| {
                slot_kind
                    .slots(types, result.place, result.structural_type)
                    .is_some_and(|slots| !slots.is_empty())
            })
            .map(|_| Vec::new()),
        places: Vec::new(),
        place_types: BTreeMap::new(),
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
            if let abstract_operations::byte_literal_establish_pair_pattern!(
                place,
                structural_type,
                ..
            ) = &node.operation
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
            let selected = operation(function, node)?;
            let operation = selected.as_ref();
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
                if !kernel::is_descriptor(types.iter(), result.structural_type)
                    && !slot_kind.is_slot(types, result.structural_type)
                {
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
                O::ByteSequenceLength { source, .. } | O::ElementViewLength { source, .. } => {
                    let query = kernel::descriptor_contents_query(
                        types,
                        source.place,
                        *graph.place_types.get(&source.place)?,
                        &source.path,
                        None,
                    )?;
                    step.inputs = vec![query.observation()];
                }
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
                    for slot in slot_kind
                        .inventory(types, result.place, result.structural_type)?
                        .slots
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
                    // A folded byte window selects backing contents, not a
                    // descriptor slot address. Keep its extent as a projection
                    // on the derivation; the original operation still supplies
                    // the independently checked array type and range bounds.
                    let (path, mut projection) = match source.path.split_last() {
                        Some((range @ StructuralPathSegment::FixedByteRange { .. }, path))
                            if matches!(operation, O::EstablishElementView { .. }) =>
                        {
                            (path, Some(Projection::new(std::slice::from_ref(range))))
                        }
                        _ => (source.path.as_slice(), None),
                    };
                    if let O::EstablishElementView {
                        element, region, ..
                    } = operation
                        && !element_coordinates_preserved(
                            function,
                            types.iter(),
                            source,
                            *element,
                            *region,
                        )
                    {
                        // Retyped element coordinates cannot refine source
                        // geometry, including an already narrowed fixed window.
                        projection
                            .get_or_insert_with(|| Projection::new(&[]))
                            .forget_address();
                    }
                    if let Some(source) = location(source.place, path, None) {
                        step.derivations.push((source.clone(), destination.clone()));
                        if slot_kind == kernel::DescriptorSlotKind::References
                            && matches!(operation, O::EstablishReference { .. })
                        {
                            graph.storage_derivations.insert((
                                position,
                                source.clone(),
                                destination.clone(),
                            ));
                        }
                        if let Some(projection) = projection {
                            graph
                                .projections
                                .insert((position, source, destination.clone()), projection);
                        }
                    } else {
                        step.may_escape = true;
                    }
                    if let Some(origins) = graph.originations.get_mut(&position) {
                        origins.retain(|(location, _)| location != &destination);
                    }
                }
                abstract_operations::element_view_or_byte_subslice_pattern!(
                    result,
                    source,
                    start,
                    end,
                    ..
                ) => {
                    let query = kernel::descriptor_contents_query(
                        types,
                        source.place,
                        *graph.place_types.get(&source.place)?,
                        &source.path,
                        None,
                    )?;
                    let source = query.observation();
                    step.inputs = vec![source.clone()];
                    let destination = DescriptorLocation::whole(result.place);
                    step.derivations.push((source.clone(), destination.clone()));
                    graph
                        .contents_queries
                        .insert((position, source.clone(), destination.clone()), query);
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
                        StructuralCopyPath::Case(path) => {
                            terminal_semantics::descriptor_lifetimes::canonical_descriptor_location(
                                types,
                                *source,
                                *graph.place_types.get(source)?,
                                path,
                            )
                        }
                    };
                    if let Some(source) = source {
                        step.inputs = vec![source.clone()];
                        step.derivations.push((source, destination.clone()));
                    } else {
                        step.may_escape = true;
                    }
                    // Copying an aggregate makes fresh carrier storage while
                    // its descendant descriptor slots retain their backing.
                    if (kernel::is_descriptor(types.iter(), result.structural_type)
                        || types.iter().any(|ty| {
                            ty.id == result.structural_type
                                && matches!(ty.shape, StructuralTypeShape::Reference { .. })
                        }))
                        && let Some(origins) = graph.originations.get_mut(&position)
                    {
                        origins.retain(|(location, _)| location != &destination);
                    }
                }
                abstract_operations::byte_len_or_field_read_pattern!(source, path, field, ..) => {
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
                    let result = result?;
                    graph.contents_transfers.insert(
                        position,
                        kernel::descriptor_contents_transfers(
                            types,
                            slot_kind,
                            kernel::DescriptorContentsOperand {
                                root: source.place(),
                                root_type: *graph.place_types.get(&source.place())?,
                                path,
                                field: *field,
                            },
                            kernel::DescriptorContentsOperand {
                                root: result.place,
                                root_type: result.structural_type,
                                path: &[],
                                field: None,
                            },
                            result.structural_type,
                            true,
                        )?,
                    );
                    if let Some(source) = location(source.place(), path, *field) {
                        let destination = DescriptorLocation::whole(result.place);
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
                    let value_type = terminal_semantics::runtime_structural_path_tip(
                        types.iter(),
                        *graph.place_types.get(&value.place)?,
                        &value.path,
                    )?
                    .id;
                    graph.contents_transfers.insert(
                        position,
                        kernel::descriptor_contents_transfers(
                            types,
                            slot_kind,
                            kernel::DescriptorContentsOperand {
                                root: value.place,
                                root_type: *graph.place_types.get(&value.place)?,
                                path: &value.path,
                                field: None,
                            },
                            kernel::DescriptorContentsOperand {
                                root: destination.place(),
                                root_type: *graph.place_types.get(&destination.place())?,
                                path,
                                field: *field,
                            },
                            value_type,
                            false,
                        )?,
                    );
                    if let (Some(destination), Some(source)) = (
                        location(destination.place(), path, *field),
                        location(value.place, &value.path, None),
                    ) {
                        step.definitions.push(destination.clone());
                        step.derivations.push((source, destination.clone()));
                        // Reseating contents retains the destination storage home.
                        if destination.path.is_empty()
                            && !kernel::is_descriptor(
                                types.iter(),
                                *graph.place_types.get(&destination.root)?,
                            )
                            && !slot_kind.is_slot(types, *graph.place_types.get(&destination.root)?)
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
                        step.definitions.extend(slot_kind.overwritten_slots(
                            types,
                            destination.place(),
                            *graph.place_types.get(&destination.place())?,
                            path,
                            *field,
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
                    if let Some(destination) = location(*destination, path, *field) {
                        step.definitions.push(destination.clone());
                        step.derivations
                            .push((DescriptorLocation::whole(*source), destination));
                    } else {
                        graph.unresolved_contents_writes.insert(position);
                        step.definitions.extend(slot_kind.overwritten_slots(
                            types,
                            *destination,
                            *graph.place_types.get(destination)?,
                            path,
                            *field,
                        )?);
                        step.may_escape = true;
                    }
                    step.inputs = vec![DescriptorLocation::whole(*source)];
                }
                O::EstablishStructuralArray {
                    result, elements, ..
                } => {
                    for (index, argument) in elements.iter().enumerate() {
                        let destination = location(
                            result.place,
                            &[StructuralPathSegment::FixedIndex(index as u64)],
                            None,
                        )?;
                        if let Some(source) = location(argument.place, &argument.path, None) {
                            step.derivations.push((source, destination));
                        } else {
                            step.may_escape = true;
                        }
                    }
                }
                abstract_operations::establish_record_or_case_pattern!(result, fields, ..) => {
                    if let O::EstablishCase { result_case, .. } = operation {
                        let shape = &types
                            .iter()
                            .find(|row| row.id == result.structural_type)?
                            .shape;
                        let cases = match shape {
                            StructuralTypeShape::Sum { cases }
                            | StructuralTypeShape::Mixed { cases, .. } => cases,
                            _ => return None,
                        };
                        if !cases.iter().any(|case| case.id == *result_case) {
                            return None;
                        }
                        for slot in slot_kind
                            .inventory(types, result.place, result.structural_type)?
                            .slots
                        {
                            if matches!(slot.path.first(), Some(terminal_semantics::descriptor_lifetimes::DescriptorPathSegment::Case(case)) if case != result_case)
                            {
                                graph
                                    .originations
                                    .entry(position)
                                    .or_default()
                                    .push((slot, kernel::DescriptorCallOrigin::Empty));
                            }
                        }
                    }
                    for (field_ordinal, field) in fields.iter().enumerate() {
                        if let terminal_psi::RecordFieldValue::Structural(source) = &field.value {
                            let destination = match operation {
                                O::EstablishCase { result_case, .. } => {
                                    let shape = &types
                                        .iter()
                                        .find(|row| row.id == result.structural_type)?
                                        .shape;
                                    let path = terminal_psi::structural_case_initializer_path(
                                        shape,
                                        *result_case,
                                        field_ordinal,
                                        field.field,
                                    )?;
                                    location(result.place, &path, None)?
                                }
                                _ => location(result.place, &[], Some(field.field))?,
                            };
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
                    claim_transfers,
                    result: call_result,
                    ..
                } => {
                    let mut matching = functions
                        .iter()
                        .filter(|function| Some(function.machine) == callee.machine());
                    if let Some(callee) = matching.next().copied() {
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
                            let actual = location(actual.place, &actual.path, None);
                            exact_arguments &= actual.is_some();
                            arguments.push(actual);
                        }
                        let descriptor_result = result.filter(|result| {
                            slot_kind
                                .slots(types, result.place, result.structural_type)
                                .is_some_and(|slots| !slots.is_empty())
                        });
                        if let Some(result) = descriptor_result {
                            // Claim bindings record which linear custody the
                            // result transports; backing provenance follows the
                            // same descriptor slots whether or not a claim rides
                            // along, so they constrain nothing here.
                            let expected = callee.result.structural()?;
                            if result.structural_type != expected.structural_type
                                || result.multiplicity != expected.multiplicity
                                || result.qualifications != expected.qualifications
                                || result.projected_qualifications
                                    != expected.projected_qualifications
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
                            if structural_arguments.iter().any(|actual| {
                                graph
                                    .place_types
                                    .get(&actual.place)
                                    .is_none_or(|root_type| {
                                        slot_kind
                                            .may_write_indirect_contents(types, *root_type, actual)
                                    })
                            }) {
                                graph.unresolved_contents_writes.insert(position);
                            }
                            for actual in structural_arguments
                                .iter()
                                .filter(|actual| actual.access != StructuralAccess::SharedBorrow)
                            {
                                step.definitions.extend(writable_descriptor_slots(
                                    types,
                                    &graph.place_types,
                                    actual,
                                    slot_kind,
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
                        ) && claim_transfers.is_empty()
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
                        if structural_arguments.iter().any(|actual| {
                            graph
                                .place_types
                                .get(&actual.place)
                                .is_none_or(|root_type| {
                                    slot_kind.may_write_indirect_contents(types, *root_type, actual)
                                })
                        }) {
                            graph.unresolved_contents_writes.insert(position);
                        }
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
                                slot_kind,
                            )?);
                        }
                    }
                }
                O::BoundaryCall {
                    boundary,
                    result,
                    structural_arguments,
                    completion_receipts,
                    ..
                } => {
                    // The same completion judgment Terminal's descriptor flow
                    // applies: a declared, receipt-free call returning no
                    // descriptor custody cannot retain or replace a lent view.
                    let mut declarations = boundaries
                        .iter()
                        .filter(|declaration| declaration.id == *boundary);
                    let preserved = match (declarations.next(), declarations.next()) {
                        (Some(declaration), None) => {
                            kernel::boundary_completion_preserves_descriptor_storage(
                                types.iter(),
                                declaration,
                                match result {
                                    abstract_operations::AbstractBoundaryResult::Unit => {
                                        kernel::BoundaryCompletionResult::Unit
                                    }
                                    abstract_operations::AbstractBoundaryResult::Scalar(result) => {
                                        kernel::BoundaryCompletionResult::Scalar(result.scalar_type)
                                    }
                                    abstract_operations::AbstractBoundaryResult::Structural(
                                        result,
                                    ) => kernel::BoundaryCompletionResult::Structural(
                                        result.structural_type,
                                    ),
                                },
                                !completion_receipts.is_empty(),
                                structural_arguments,
                                |place| graph.place_types.get(&place).copied(),
                            )
                        }
                        _ => false,
                    };
                    let preserved = preserved
                        || (slot_kind == kernel::DescriptorSlotKind::References
                            && preserves_boundary(function, node));
                    if !preserved {
                        step.may_escape = true;
                        if structural_arguments.iter().any(|actual| {
                            graph
                                .place_types
                                .get(&actual.place)
                                .is_none_or(|root_type| {
                                    slot_kind.may_write_indirect_contents(types, *root_type, actual)
                                })
                        }) {
                            graph.unresolved_contents_writes.insert(position);
                        }
                        for actual in structural_arguments
                            .iter()
                            .filter(|actual| actual.access != StructuralAccess::SharedBorrow)
                        {
                            step.definitions.extend(writable_descriptor_slots(
                                types,
                                &graph.place_types,
                                actual,
                                slot_kind,
                            )?);
                        }
                    }
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
                abstract_operations::return_or_return_unit_pattern!() => {
                    graph.normal_exits.push(position);
                }
                _ => {}
            }
            step.authority_derivations = step.derivations.clone();
            if graph.normal_exits.contains(&position) {
                for parameter in &function.structural_parameters {
                    if matches!(
                        parameter.access,
                        terminal_psi::exclusive_borrow_pair_pattern!()
                    ) {
                        step.inputs.extend(slot_kind.slots(
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
    slot_kind: kernel::DescriptorSlotKind,
) -> Option<Vec<DescriptorLocation>> {
    let ty = *place_types.get(&actual.place)?;
    let selected = descriptor_location(types, actual.place, ty, &actual.path, None);
    Some(
        slot_kind
            .slots(types, actual.place, ty)?
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
    ) else {
        return false;
    };
    // A fixed byte window changes extent, not the backing's element type.
    // Its range stays on the loan projection; only declaration lookup strips
    // that final presentation. Byte coordinates require an exact u8 element.
    let path = match source.path.split_last() {
        Some((StructuralPathSegment::FixedByteRange { .. }, backing)) => {
            if !types.clone().any(|declaration| {
                declaration.id == element
                    && matches!(declaration.shape,
                        StructuralTypeShape::PrimitiveScalar(semantic_vocabulary::ScalarType::Integer(integer))
                            if integer.sign() == semantic_vocabulary::IntegerSign::Unsigned
                                && integer.bits() == 8 && !integer.is_address())
            }) {
                return false;
            }
            backing
        }
        _ => source.path.as_slice(),
    };
    let Some(collection) =
        terminal_semantics::runtime_structural_path_tip(types.clone(), root.structural_type, path)
    else {
        return false;
    };
    terminal_semantics::collection_view_preserves_coordinates(types, collection.id, element, region)
}

/// Reuse the current-graph scalar adapter for exact captured address queries.
/// This returns analysis scratch, never a producer-provided coordinate fact.
pub fn captured_scalar_ordering(
    function: &PsiOptimizationFunction,
    functions: &BTreeMap<semantic_vocabulary::MachineId, &PsiOptimizationFunction>,
) -> terminal_semantics::scalar_ordering::ScalarOrdering {
    crate::current_ownership::address_joins::range_relations::RangeRelations::new(
        function, functions,
    )
    .into_ordering()
}

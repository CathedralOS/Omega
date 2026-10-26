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
    DescriptorFlowBinding, DescriptorFlowEdge, DescriptorFlowSource, DescriptorFlowStep,
    DescriptorLocation, descriptor_location,
};
use terminal_semantics::descriptor_results::{
    self as kernel, DescriptorDefinition, DescriptorInvocation, DescriptorMachine, DescriptorPlace,
    writable_descriptor_slots,
};

pub mod placed_results;
mod scalar_expressions;
use scalar_expressions::ScalarExpressions;

/// Adapt current flat abstract bodies through the same block reconstruction
/// and descriptor flow used by optimization-unit consumers. No retained source
/// summary supplies origin authority after transformations.
pub fn abstract_catalog_graph<'function, 'boundary>(
    functions: impl IntoIterator<Item = &'function abstract_operations::AbstractFunction>,
    boundaries: impl IntoIterator<Item = &'boundary BoundaryMachineDeclaration>,
    types: &[StructuralTypeDeclaration],
    dynamic_dispatch: &terminal_psi::TerminalDynamicDispatchCatalog,
) -> Option<Vec<DescriptorMachine>> {
    abstract_graph_with_slots(
        functions,
        boundaries,
        types,
        dynamic_dispatch,
        kernel::DescriptorSlotKind::Views,
    )
}

/// Ordinary reference-pointer contents reconstructed from current flat bodies.
pub fn abstract_reference_catalog_graph<'function, 'boundary>(
    functions: impl IntoIterator<Item = &'function abstract_operations::AbstractFunction>,
    boundaries: impl IntoIterator<Item = &'boundary BoundaryMachineDeclaration>,
    types: &[StructuralTypeDeclaration],
    dynamic_dispatch: &terminal_psi::TerminalDynamicDispatchCatalog,
) -> Option<Vec<DescriptorMachine>> {
    abstract_graph_with_slots(
        functions,
        boundaries,
        types,
        dynamic_dispatch,
        kernel::DescriptorSlotKind::References,
    )
}

/// The complete inventory for post-effect caller membership queries.
pub fn optimized_borrowed_storage_catalog_graph<'function, 'boundary>(
    functions: impl IntoIterator<Item = &'function PsiOptimizationFunction>,
    boundaries: impl IntoIterator<Item = &'boundary BoundaryMachineDeclaration>,
    types: &[StructuralTypeDeclaration],
    dynamic_dispatch: &terminal_psi::TerminalDynamicDispatchCatalog,
) -> Option<Vec<DescriptorMachine>> {
    catalog_graph(
        functions,
        boundaries,
        types,
        dynamic_dispatch,
        |_, node| Some(Cow::Borrowed(&node.operation)),
        kernel::DescriptorSlotKind::BorrowedStorage,
        |_, _| false,
    )
}

fn abstract_graph_with_slots<'function, 'boundary>(
    functions: impl IntoIterator<Item = &'function abstract_operations::AbstractFunction>,
    boundaries: impl IntoIterator<Item = &'boundary BoundaryMachineDeclaration>,
    types: &[StructuralTypeDeclaration],
    dynamic_dispatch: &terminal_psi::TerminalDynamicDispatchCatalog,
    slot_kind: kernel::DescriptorSlotKind,
) -> Option<Vec<DescriptorMachine>> {
    let functions = functions
        .into_iter()
        .map(optimization_unit::reconstruct_psi_optimization_function_seed)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    catalog_graph(
        functions.iter(),
        boundaries,
        types,
        dynamic_dispatch,
        |_, node| Some(Cow::Borrowed(&node.operation)),
        slot_kind,
        |_, _| false,
    )
}

/// Adapt flat current bodies with their independently selected execution and
/// explicit guarded-boundary obligations. Portable callers use the default API.
pub fn abstract_reference_catalog_graph_with_boundary_preservation<'function, 'boundary>(
    functions: impl IntoIterator<Item = &'function abstract_operations::AbstractFunction>,
    boundaries: impl IntoIterator<Item = &'boundary BoundaryMachineDeclaration>,
    types: &[StructuralTypeDeclaration],
    dynamic_dispatch: &terminal_psi::TerminalDynamicDispatchCatalog,
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
        dynamic_dispatch,
        operation,
        preserves_boundary,
    )
}

pub(crate) fn optimized_graph(unit: &PsiOptimizationUnit) -> Option<Vec<DescriptorMachine>> {
    optimized_catalog_graph(
        unit.functions.iter(),
        &unit.boundary_machines,
        &unit.structural_types,
        &unit.dynamic_dispatch,
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
    dynamic_dispatch: &terminal_psi::TerminalDynamicDispatchCatalog,
    operation: impl for<'node> Fn(
        &PsiOptimizationFunction,
        &'node optimization_unit::OptimizationNode,
    ) -> Option<Cow<'node, O>>,
) -> Option<Vec<DescriptorMachine>> {
    catalog_graph(
        functions,
        boundaries,
        types,
        dynamic_dispatch,
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
    dynamic_dispatch: &terminal_psi::TerminalDynamicDispatchCatalog,
    operation: impl for<'node> Fn(
        &PsiOptimizationFunction,
        &'node optimization_unit::OptimizationNode,
    ) -> Option<Cow<'node, O>>,
) -> Option<Vec<DescriptorMachine>> {
    optimized_reference_catalog_graph_with_boundary_preservation(
        functions,
        boundaries,
        types,
        dynamic_dispatch,
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
    dynamic_dispatch: &terminal_psi::TerminalDynamicDispatchCatalog,
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
        dynamic_dispatch,
        operation,
        kernel::DescriptorSlotKind::References,
        preserves_boundary,
    )
}

fn catalog_graph<'function, 'boundary>(
    functions: impl IntoIterator<Item = &'function PsiOptimizationFunction>,
    boundaries: impl IntoIterator<Item = &'boundary BoundaryMachineDeclaration>,
    types: &[StructuralTypeDeclaration],
    dynamic_dispatch: &terminal_psi::TerminalDynamicDispatchCatalog,
    operation: impl for<'node> Fn(
        &PsiOptimizationFunction,
        &'node optimization_unit::OptimizationNode,
    ) -> Option<Cow<'node, O>>,
    slot_kind: kernel::DescriptorSlotKind,
    preserves_boundary: impl Fn(&PsiOptimizationFunction, &optimization_unit::OptimizationNode) -> bool,
) -> Option<Vec<DescriptorMachine>> {
    let functions = functions.into_iter().collect::<Vec<_>>();
    let boundaries = boundaries.into_iter().collect::<Vec<_>>();
    let build = |selector_graph: Option<&[DescriptorMachine]>| {
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
                    dynamic_dispatch,
                    &functions,
                    function,
                    &operation,
                    slot_kind,
                    &preserves_boundary,
                    selector_graph,
                )
            })
            .collect::<Option<Vec<_>>>()
    };
    let initial = build(None)?;
    // Only current projected dynamic invocations can read selector_graph.
    // Their absence makes the initial adaptation the complete result.
    if !functions.iter().any(|function| {
        function.block_nodes().any(|node| {
            operation(function, node).is_some_and(|operation| {
                matches!(
                    operation.as_ref(),
                    O::CallStructural {
                        callee: abstract_operations::AbstractCallTarget::Dynamic { .. },
                        ..
                    }
                )
            })
        })
    }) {
        return Some(initial);
    }
    build(Some(&initial))
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
pub(crate) fn owned_aggregate_descriptor_returns_with_graph(
    function: &PsiOptimizationFunction,
    graph: Option<&[DescriptorMachine]>,
    types: &[StructuralTypeDeclaration],
) -> bool {
    let Some(graph) = graph else {
        return false;
    };
    let Some(returns) = kernel::descriptor_machine(graph, function.machine)
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
                    graph,
                    function.machine,
                    *position,
                    location.root,
                )
        })
}

fn adapt(
    types: &[StructuralTypeDeclaration],
    boundaries: &[&BoundaryMachineDeclaration],
    dynamic_dispatch: &terminal_psi::TerminalDynamicDispatchCatalog,
    functions: &[&PsiOptimizationFunction],
    function: &PsiOptimizationFunction,
    operation_projection: &impl for<'node> Fn(
        &PsiOptimizationFunction,
        &'node optimization_unit::OptimizationNode,
    ) -> Option<Cow<'node, O>>,
    slot_kind: kernel::DescriptorSlotKind,
    preserves_boundary: &impl Fn(&PsiOptimizationFunction, &optimization_unit::OptimizationNode) -> bool,
    selector_graph: Option<&[DescriptorMachine]>,
) -> Option<DescriptorMachine> {
    let scalar_expressions = ScalarExpressions::new(function);
    let mut graph = DescriptorMachine {
        id: function.machine,
        entry: (function.entry, 0),
        returns: function
            .result
            .structural()
            .filter(|result| {
                slot_kind
                    .owned_slots(types, result.place, result.structural_type)
                    .is_some_and(|slots| !slots.is_empty())
            })
            .map(|_| Vec::new()),
        places: Vec::new(),
        place_types: BTreeMap::new(),
        flow: BTreeMap::new(),
        case_constructors: BTreeMap::new(),
        case_successors: BTreeMap::new(),
        originations: BTreeMap::new(),
        projections: BTreeMap::new(),
        contents_queries: BTreeMap::new(),
        contents_transfers: BTreeMap::new(),
        payload_invalidations: BTreeMap::new(),
        payload_producers: BTreeMap::new(),
        payload_entry_predicates: function.verified_contract.as_ref().and_then(|contract| {
            Some((
                semantic_vocabulary::PropositionContext::from_value_types_and_places(
                    function
                        .parameters
                        .iter()
                        .map(|value| (value.value, value.scalar_type))
                        .chain(
                            contract
                                .erased_scalar_formals
                                .iter()
                                .map(|value| (value.id, value.scalar_type)),
                        ),
                    function
                        .structural_places
                        .iter()
                        .map(|place| (place.id, place.kind)),
                )
                .ok()?,
                contract.requires.clone(),
            ))
        }),
        scalar_payload_writes: BTreeMap::new(),
        scalar_payload_reads: BTreeMap::new(),
        payload_scalar_definitions: BTreeMap::new(),
        payload_scalar_postconditions: BTreeMap::new(),
        payload_scalar_parameters: BTreeMap::new(),
        payload_edge_predicates: BTreeMap::new(),
        payload_scalar_bindings: BTreeMap::new(),
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
        graph.payload_scalar_parameters.insert(
            block.id,
            block.parameters.iter().map(|value| value.value).collect(),
        );
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
                definition: DescriptorDefinition::Arrival {
                    access: parameter.access,
                },
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
            let selected = operation_projection(function, node)?;
            let operation = selected.as_ref();
            if let Some(guarantees) = scalar_call_guarantees(
                operation,
                &|identity| {
                    let mut matching = functions
                        .iter()
                        .copied()
                        .filter(|candidate| candidate.machine == identity);
                    let candidate = matching.next()?;
                    matching.next().is_none().then_some(candidate)
                },
                &|value| {
                    scalar_expressions
                        .expression(value)
                        .map(|term| term.scalar_type())
                },
            ) {
                graph
                    .payload_scalar_postconditions
                    .insert(position, guarantees);
            }
            if let O::StructuralScalarFieldStore {
                destination,
                path,
                field,
                value,
                ..
            } = operation
                && let Some(value) = scalar_expressions.expression(value.value)
                // Runtime selectors keep their typed invalidation below; only
                // an exact coordinate establishes a new stored scalar fact.
                && let Some(destination) = location(destination.place(), path, Some(*field))
            {
                graph
                    .scalar_payload_writes
                    .entry(position)
                    .or_default()
                    .push((destination, value));
            }
            let result = operation.structural_result().map(|(_, result)| result);
            if !node.definitions.is_empty() {
                graph.payload_scalar_definitions.insert(
                    position,
                    node.definitions.iter().map(|value| value.value).collect(),
                );
            }
            if let O::PrimitiveScalarRead {
                source,
                path,
                result,
                ..
            } = operation
                && let Some(location) = location(*source, path, None)
            {
                graph
                    .scalar_payload_reads
                    .entry(position)
                    .or_default()
                    .push((
                        location,
                        semantic_vocabulary::ScalarTerm::value(result.value, result.scalar_type),
                    ));
            }
            if let O::StructuralScalarFieldRead {
                source,
                path,
                field,
                result,
                ..
            } = operation
            {
                let mut path = path.clone();
                path.push(semantic_vocabulary::CanonicalStructuralPathSegment::Field(
                    *field,
                ));
                if let Some(location) =
                    terminal_semantics::descriptor_lifetimes::canonical_descriptor_location(
                        types,
                        *source,
                        *graph.place_types.get(source)?,
                        &path,
                    )
                {
                    graph
                        .scalar_payload_reads
                        .entry(position)
                        .or_default()
                        .push((
                            location,
                            semantic_vocabulary::ScalarTerm::value(
                                result.value,
                                result.scalar_type,
                            ),
                        ));
                }
            }
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
                graph.payload_producers.insert(position, result.place);
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
            // Payload footprints are typed observations, never fabricated
            // exact addresses for dynamic selectors. Unknown writes remain
            // explicit and retire evidence conservatively in membership replay.
            let payload_query = |root, path: &[StructuralPathSegment], field| {
                let mut query = kernel::descriptor_contents_query(
                    types,
                    root,
                    *graph.place_types.get(&root)?,
                    path,
                    field,
                )?;
                if field.is_none() {
                    let selected = terminal_semantics::runtime_structural_path_tip(
                        types.iter(),
                        *graph.place_types.get(&root)?,
                        path,
                    )?;
                    match selected.shape {
                        StructuralTypeShape::ElementView { element, .. } => {
                            query
                                .steps
                                .push(kernel::DescriptorContentsStep::AnyElement { element });
                        }
                        StructuralTypeShape::ByteSequence(_)
                            if kernel::is_descriptor(types.iter(), selected.id) =>
                        {
                            return None;
                        }
                        _ => {}
                    }
                }
                Some(query)
            };
            let mut payload_writes = Vec::new();
            match operation {
                O::WriteOnlyPrimitiveStore {
                    destination, path, ..
                } => {
                    payload_writes.push(payload_query(destination.place(), path, None));
                }
                O::StructuralScalarFieldStore {
                    destination,
                    path,
                    field,
                    ..
                } => {
                    payload_writes.push(payload_query(destination.place(), path, Some(*field)));
                }
                O::StructuralByteSequenceFieldByteStore {
                    destination,
                    path,
                    field,
                    ..
                }
                | O::StructuralByteSequenceFieldStore {
                    destination,
                    path,
                    field,
                    ..
                } => {
                    payload_writes.push(payload_query(*destination, path, Some(*field)));
                }
                O::StructuralByteViewFieldStore {
                    destination,
                    path,
                    field,
                    ..
                } => {
                    payload_writes.push(payload_query(*destination, path, *field));
                }
                O::ByteSequenceWrite { .. } | O::ByteSequenceStore { .. } => {
                    payload_writes.push(None);
                }
                O::ElementViewWrite {
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
                O::CallStructural {
                    structural_arguments: arguments,
                    ..
                }
                | O::BoundaryCall {
                    structural_arguments: arguments,
                    ..
                } => {
                    for argument in arguments {
                        let selected = terminal_semantics::runtime_structural_path_tip(
                            types.iter(),
                            *graph.place_types.get(&argument.place)?,
                            &argument.path,
                        );
                        let mutation = selected
                            .map(|selected| {
                                terminal_semantics::shared_mutation(types.iter(), selected.id, &[])
                            })
                            .unwrap_or(terminal_semantics::SharedMutation::Unbounded);
                        if mutation == terminal_semantics::SharedMutation::Unbounded {
                            payload_writes.push(None);
                        } else if argument.access != StructuralAccess::SharedBorrow
                            || mutation != terminal_semantics::SharedMutation::None
                        {
                            payload_writes.push(payload_query(
                                argument.place,
                                &argument.path,
                                None,
                            ));
                        }
                    }
                }
                O::AtomicEvent { event, .. } => {
                    if let Some(location) = event.location() {
                        payload_writes.push(payload_query(
                            location.root,
                            &location.path,
                            Some(location.field),
                        ));
                    }
                }
                _ => {}
            }
            if !payload_writes.is_empty() {
                graph.payload_invalidations.insert(position, payload_writes);
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
                    graph.payload_producers.insert(position, place.id);
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
                O::EstablishNativeCodeLayout { result, .. } if slot_kind.tracks_views() => {
                    let destination = kernel::native_code_layout_regions(types, result)?;
                    step.definitions.push(destination.clone());
                    graph.originations.entry(position).or_default().push((
                        destination,
                        kernel::DescriptorCallOrigin::Literal {
                            machine: function.machine,
                            place: result.place,
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
                O::EstablishPlacedReference {
                    psi_operation,
                    result,
                    correspondence,
                    ..
                } if slot_kind.tracks_references() => {
                    // A placed presentation is not a typed alias to the source
                    // schema. Retain its formation through reference transport
                    // without letting ordinary slot traversal reinterpret it.
                    // Physical geometry belongs to the formation's actual
                    // source; this token supplies no allocation guarantee.
                    graph.originations.entry(position).or_default().push((
                        DescriptorLocation::whole(result.place),
                        kernel::DescriptorCallOrigin::PlacedFormation {
                            machine: function.machine,
                            operation: *psi_operation,
                            place: result.place,
                            carrier: correspondence.carrier(),
                            projection: Projection::new(&[]),
                        },
                    ));
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
                    let alternatives =
                        terminal_semantics::descriptor_lifetimes::fixed_runtime_index_alternatives(
                            types,
                            source.place,
                            *graph.place_types.get(&source.place)?,
                            path,
                        );
                    if let Some(alternatives) = alternatives {
                        // Every fixed element stays a storage origin. A
                        // projection would turn that address into backing
                        // contents and lose the pointer the callee loads.
                        for alternative in alternatives {
                            step.derivations.push((alternative, destination.clone()));
                        }
                    } else if let Some(source) = location(source.place, path, None) {
                        step.derivations.push((source.clone(), destination.clone()));
                        if slot_kind.tracks_references()
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
                O::StructuralLeafCopy { .. }
                | O::MoveStructuralValue { .. }
                | O::TransformContent { .. } => {
                    let owned_path;
                    let (result, source, path) = match &node.operation {
                        O::StructuralLeafCopy {
                            result,
                            source,
                            path,
                            ..
                        } => (result, source, path),
                        O::MoveStructuralValue { result, source, .. }
                        | O::TransformContent { result, source, .. } => {
                            owned_path = StructuralCopyPath::Ordinary(source.path.clone());
                            (result, &source.place, &owned_path)
                        }
                        _ => return None,
                    };
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
                O::EstablishPrimitiveLocal { result, value, .. } => {
                    if let Some(value) = scalar_expressions.expression(value.value) {
                        graph
                            .scalar_payload_writes
                            .entry(position)
                            .or_default()
                            .push((location(result.place, &[], None)?, value));
                    }
                }
                O::PrimitiveLocalStore {
                    destination, value, ..
                } => {
                    if let Some(value) = scalar_expressions.expression(value.value) {
                        graph
                            .scalar_payload_writes
                            .entry(position)
                            .or_default()
                            .push((location(*destination, &[], None)?, value));
                    }
                }
                O::EstablishScalarArray {
                    result, elements, ..
                } => {
                    for (ordinal, value) in elements.iter().enumerate() {
                        let path = terminal_semantics::scalar_array_leaf_path(
                            types.iter(),
                            result.structural_type,
                            u64::try_from(ordinal).ok()?,
                        )?;
                        if let Some(value) = scalar_expressions.expression(*value) {
                            graph
                                .scalar_payload_writes
                                .entry(position)
                                .or_default()
                                .push((location(result.place, &path, None)?, value));
                        }
                    }
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
                        if let terminal_psi::RecordFieldValue::Scalar { value, .. } = &field.value
                            && let Some(value) = scalar_expressions.expression(*value)
                        {
                            graph
                                .scalar_payload_writes
                                .entry(position)
                                .or_default()
                                .push((destination.clone(), value));
                        }
                        if let terminal_psi::RecordFieldValue::Structural(source) = &field.value {
                            if let Some(source) = location(source.place, &source.path, None) {
                                step.derivations.push((source, destination));
                            } else {
                                step.may_escape = true;
                            }
                        }
                    }
                }
                O::CallStructural {
                    callee:
                        abstract_operations::AbstractCallTarget::Dynamic {
                            selector,
                            interface,
                            slot,
                        },
                    structural_arguments,
                    ..
                } => {
                    let effects_machine = selected_effects_machine(
                        dynamic_dispatch,
                        functions,
                        function,
                        types,
                        selector_graph,
                        operation_projection,
                        *selector,
                        *interface,
                        *slot,
                    );
                    let realizations =
                        kernel::public_realizations(dynamic_dispatch, *interface, *slot)?;
                    let slot = dynamic_dispatch
                        .interface(*interface)?
                        .slots
                        .get(usize::try_from(*slot).ok()?)?;
                    let mut call = kernel::public_invocation(
                        node.operation.psi_operation()?,
                        types,
                        slot_kind,
                        slot,
                        realizations.clone(),
                        structural_arguments,
                        result,
                        &graph.place_types,
                    )?;
                    // Replay skips an incomplete invocation, so neither an
                    // exact selection nor a catalog union may remove its fence.
                    let complete_contents = call.argument_contents.iter().all(Option::is_some);
                    let effects_machine = effects_machine.filter(|_| complete_contents);
                    if let kernel::DescriptorInvocationTarget::Public {
                        effects_machine: target,
                        ..
                    } = &mut call.target
                    {
                        *target = effects_machine;
                    }
                    let has_stored_effects = complete_contents
                        && (effects_machine.is_some() || !realizations.is_empty());
                    if !has_stored_effects {
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
                    if matches!(
                        call.target,
                        kernel::DescriptorInvocationTarget::Public {
                            invalidates_contents: true,
                            ..
                        }
                    ) && !has_stored_effects
                    {
                        graph.unresolved_contents_writes.insert(position);
                        step.may_escape = true;
                    }
                    graph.calls.insert(position, call);
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
                        let mut argument_contents = Vec::new();
                        let mut exact_arguments = true;
                        for (actual, formal) in structural_arguments
                            .iter()
                            .zip(&callee.structural_parameters)
                        {
                            if actual.access != formal.access {
                                return None;
                            }
                            argument_contents.push(graph.place_types.get(&actual.place).and_then(
                                |root_type| {
                                    kernel::descriptor_contents_query(
                                        types,
                                        actual.place,
                                        *root_type,
                                        &actual.path,
                                        None,
                                    )
                                },
                            ));
                            let actual = location(actual.place, &actual.path, None);
                            exact_arguments &= actual.is_some();
                            arguments.push(actual);
                        }
                        let descriptor_result = result.filter(|result| {
                            slot_kind
                                .owned_slots(types, result.place, result.structural_type)
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
                                || !expected
                                    .projected_qualifications
                                    .iter()
                                    .all(|row| result.projected_qualifications.contains(row))
                            {
                                return None;
                            }
                        }
                        graph.calls.insert(
                            position,
                            DescriptorInvocation {
                                operation: node.operation.psi_operation()?,
                                target: kernel::DescriptorInvocationTarget::Machine(callee.machine),
                                arguments,
                                argument_contents,
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
                    if slot_kind == kernel::DescriptorSlotKind::References
                        && let abstract_operations::AbstractBoundaryResult::Structural(result) =
                            result
                        && let Some(declaration) = boundaries.iter().find(|row| row.id == *boundary)
                        && declaration.declaration.as_ref().is_some_and(|head| {
                            head.execution.placement_operation()
                                == Some(terminal_psi::TerminalPlacementLifecycleOperation::View)
                        })
                        && let Some((path, carrier)) =
                            terminal_semantics::placement_view_reference_slot(
                                types,
                                result.structural_type,
                            )
                    {
                        graph.originations.entry(position).or_default().push((
                            descriptor_location(
                                types,
                                result.place,
                                result.structural_type,
                                &path,
                                None,
                            )?,
                            kernel::DescriptorCallOrigin::PlacedFormation {
                                machine: function.machine,
                                operation: operation.psi_operation()?,
                                place: result.place,
                                carrier,
                                projection: Projection::new(&[]),
                            },
                        ));
                    }
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
                        || (slot_kind.tracks_references() && preserves_boundary(function, node));
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
                    let mut declarations = boundaries
                        .iter()
                        .filter(|declaration| declaration.id == *boundary);
                    if let (Some(declaration), None) = (declarations.next(), declarations.next())
                        && !declaration.reference_outputs.is_empty()
                        && slot_kind != kernel::DescriptorSlotKind::PlacedOrigins
                        && !(slot_kind == kernel::DescriptorSlotKind::References
                            && declaration.declaration.as_ref().is_some_and(|head| {
                                head.execution.placement_operation()
                                    == Some(terminal_psi::TerminalPlacementLifecycleOperation::View)
                            }))
                    {
                        graph.calls.insert(
                            position,
                            kernel::provider_invocation(
                                node.operation.psi_operation()?,
                                types,
                                slot_kind,
                                declaration,
                                structural_arguments,
                                &graph.place_types,
                            )?,
                        );
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
            if let O::EstablishZeroedStructural { result, .. } = operation {
                graph.case_constructors.insert(
                    position,
                    kernel::zeroed_case_predicates(types, result.place, result.structural_type),
                );
            }
            if let O::EstablishCase {
                result,
                result_case,
                ..
            } = operation
            {
                graph.case_constructors.insert(
                    position,
                    vec![kernel::DescriptorCasePredicate {
                        subject: DescriptorLocation::whole(result.place),
                        case: *result_case,
                    }],
                );
            }
            if let O::StructuralCase {
                source,
                path,
                cases,
            } = operation
            {
                let subject = location(*source, path, None)?;
                let predicates = node
                    .successors
                    .iter()
                    .map(|edge| {
                        let mut matching = cases.iter().filter(|case| {
                            case.psi_edge == edge.psi_edge && case.target == edge.target
                        });
                        let case = matching.next()?;
                        matching.next().is_none().then_some((
                            kernel::DescriptorCasePredicate {
                                subject: subject.clone(),
                                case: case.case,
                            },
                            true,
                        ))
                    })
                    .collect::<Option<Vec<_>>>()?;
                graph.case_successors.insert(position, predicates);
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
                    let scalar_bindings = if let O::StructuralCase {
                        source,
                        path,
                        cases,
                        ..
                    } = operation
                    {
                        let mut matching = cases.iter().filter(|case| {
                            case.psi_edge == edge.psi_edge && case.target == edge.target
                        });
                        let case = matching.next()?;
                        if matching.next().is_some()
                            || !edge.bindings.is_empty()
                            || case.payloads.len() != target.parameters.len()
                        {
                            return None;
                        }
                        let fields = target
                            .parameters
                            .iter()
                            .map(|parameter| {
                                let mut matching = case
                                    .payloads
                                    .iter()
                                    .filter(|payload| payload.parameter == parameter.value);
                                let payload = matching.next()?;
                                (matching.next().is_none()
                                    && payload.scalar_type == parameter.scalar_type)
                                    .then_some(payload.field)
                            })
                            .collect::<Option<Vec<_>>>()?;
                        let captured = kernel::case_payload_scalar_types(
                            types,
                            *graph.place_types.get(source)?,
                            path,
                            case.case,
                            &fields,
                        )?;
                        if captured
                            .iter()
                            .zip(&target.parameters)
                            .any(|(actual, parameter)| *actual != parameter.scalar_type)
                        {
                            return None;
                        }
                        // Case payloads are fresh scalar captures, not SSA
                        // operands from the predecessor. Keep their facts
                        // unknown and retain the surrounding storage history.
                        Vec::new()
                    } else {
                        target
                            .parameters
                            .iter()
                            .map(|parameter| {
                                let binding = edge.binding(parameter.value)?;
                                let value = scalar_expressions.expression(binding.argument)?;
                                (value.scalar_type() == parameter.scalar_type)
                                    .then_some((parameter.value, value))
                            })
                            .collect::<Option<Vec<_>>>()?
                    };
                    graph
                        .payload_scalar_bindings
                        .entry(position)
                        .or_default()
                        .push(scalar_bindings);
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
                        if matching.next().is_some() || binding.argument.access != parameter.access
                        {
                            return None;
                        }
                        let source = if let Some(source) =
                            location(binding.argument.place, &binding.argument.path, None)
                        {
                            DescriptorFlowSource::Location(source)
                        } else {
                            step.may_escape = true;
                            DescriptorFlowSource::from_query(
                                binding.argument.place,
                                kernel::descriptor_contents_query(
                                    types,
                                    binding.argument.place,
                                    *graph.place_types.get(&binding.argument.place)?,
                                    &binding.argument.path,
                                    None,
                                ),
                            )
                        };
                        step.inputs.push(source.observation());
                        bindings.push(DescriptorFlowBinding::new(
                            source,
                            DescriptorLocation::whole(parameter.place),
                            binding.argument.access,
                            parameter.access,
                        )?);
                    }
                    step.successors.push(DescriptorFlowEdge {
                        target: (edge.target, 0),
                        parameters,
                        bindings,
                    });
                }
            }
            if let O::Conditional { condition, .. } = operation
                && let Some(term) = scalar_expressions.expression(*condition)
            {
                graph.payload_edge_predicates.insert(
                    position,
                    [true, false]
                        .into_iter()
                        .map(|polarity| {
                            Some(semantic_vocabulary::Proposition::Equal(
                                semantic_vocabulary::ScalarTerm::Boolean(polarity),
                                term.clone(),
                            ))
                        })
                        .collect(),
                );
            }
            if graph.flow.insert(position, step).is_some() {
                return None;
            }
        }
    }
    scalar_expressions.retain_case_conditions(types, &mut graph);
    Some(graph)
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

/// Apply an exact current call signature once for payload and ordering facts.
/// The two consumers project different consequences, not different call rules.
pub(crate) fn scalar_call_guarantees<'function>(
    operation: &O,
    callee: &impl Fn(MachineId) -> Option<&'function PsiOptimizationFunction>,
    scalar_type: &impl Fn(semantic_vocabulary::ValueId) -> Option<semantic_vocabulary::ScalarType>,
) -> Option<Vec<semantic_vocabulary::Proposition>> {
    let (identity, arguments, result) = match operation {
        O::Call {
            callee,
            arguments,
            result,
            scalar_type,
            ..
        } => (
            *callee,
            arguments,
            abstract_operations::AbstractResult {
                value: *result,
                scalar_type: *scalar_type,
            },
        ),
        O::CallStructural {
            callee,
            arguments,
            result,
            ..
        } => (callee.machine()?, arguments, result.scalar()?),
        _ => return None,
    };
    let callee = callee(identity)?;
    let contract = callee.verified_contract.as_ref()?;
    let signature = callee.result.scalar()?;
    if arguments.contains(&result.value)
        || signature.scalar_type != result.scalar_type
        || arguments.len() != callee.parameters.len()
        || !contract.erased_scalar_formals.is_empty()
        || !contract.erased_proof_formals.is_empty()
    {
        return None;
    }
    let mut bindings = Vec::new();
    for (formal, actual) in callee.parameters.iter().zip(arguments) {
        if scalar_type(*actual) != Some(formal.scalar_type) {
            return None;
        }
        bindings.push((
            formal.value,
            semantic_vocabulary::ScalarTerm::value(*actual, formal.scalar_type),
        ));
    }
    bindings.push((
        signature.value,
        semantic_vocabulary::ScalarTerm::value(result.value, result.scalar_type),
    ));
    Some(kernel::instantiate_payload_scalar_guarantees(
        contract, &bindings,
    ))
}

pub(crate) fn scalar_expression(
    function: &PsiOptimizationFunction,
    value: semantic_vocabulary::ValueId,
) -> Option<semantic_vocabulary::ScalarTerm> {
    ScalarExpressions::new(function).expression(value)
}

fn selected_effects_machine(
    catalog: &terminal_psi::TerminalDynamicDispatchCatalog,
    functions: &[&PsiOptimizationFunction],
    function: &PsiOptimizationFunction,
    types: &[StructuralTypeDeclaration],
    selector_graph: Option<&[DescriptorMachine]>,
    operation: &impl for<'node> Fn(
        &PsiOptimizationFunction,
        &'node optimization_unit::OptimizationNode,
    ) -> Option<Cow<'node, O>>,
    selector: semantic_vocabulary::ValueId,
    interface: u32,
    slot: u32,
) -> Option<MachineId> {
    #[derive(Clone, PartialEq, Eq)]
    struct SelectedTable(u32);
    let table = terminal_semantics::unchanged_scalar_identity(selector, |value| {
        let mut definitions = function.block_nodes().filter(|node| {
            node.definitions
                .iter()
                .any(|definition| definition.value == value)
        });
        if let Some(node) = definitions.next() {
            if definitions.next().is_some() {
                return None;
            }
            let projected = operation(function, node)?;
            if let O::SelectImplementation { table, .. } = projected.as_ref() {
                return Some(terminal_semantics::UnchangedScalarOrigin::Known(
                    SelectedTable(*table),
                ));
            }
            let sources = kernel::unchanged_stored_scalar_sources(
                types,
                selector_graph?,
                function.machine,
                value,
                |(block, index)| {
                    let Some(node) = function
                        .blocks
                        .iter()
                        .find(|row| row.id == block)
                        .and_then(|block| block.nodes.get(index as usize))
                    else {
                        return Vec::new();
                    };
                    let Some(graph) = selector_graph
                        .and_then(|graph| graph.iter().find(|row| row.id == function.machine))
                    else {
                        return Vec::new();
                    };
                    let Some(projected) = operation(function, node) else {
                        return Vec::new();
                    };
                    let mut writes = Vec::new();
                    let mut write = |root, path: &[StructuralPathSegment], field, actual| {
                        if let Some(location) = graph
                            .place_types
                            .get(&root)
                            .and_then(|ty| descriptor_location(types, root, *ty, path, field))
                        {
                            writes.push((location, actual));
                        }
                    };
                    match projected.as_ref() {
                        O::EstablishPrimitiveLocal { result, value, .. } => {
                            write(result.place, &[], None, value.value)
                        }
                        O::PrimitiveLocalStore {
                            destination, value, ..
                        } => write(*destination, &[], None, value.value),
                        O::WriteOnlyPrimitiveStore {
                            destination,
                            path,
                            value,
                            ..
                        } => write(destination.place(), path, None, value.value),
                        O::StructuralScalarFieldStore {
                            destination,
                            path,
                            field,
                            value,
                            ..
                        } => write(destination.place(), path, Some(*field), value.value),
                        O::EstablishRecord { result, fields, .. } => {
                            for field in fields {
                                if let terminal_psi::RecordFieldValue::Scalar { value, .. } =
                                    &field.value
                                {
                                    write(result.place, &[], Some(field.field), *value);
                                }
                            }
                        }
                        _ => {}
                    }
                    writes
                },
            )?;
            return Some(terminal_semantics::UnchangedScalarOrigin::Incoming(sources));
        }
        let mut owners = function.blocks.iter().filter(|block| {
            block
                .parameters
                .iter()
                .any(|parameter| parameter.value == value)
        });
        let owner = owners.next()?;
        if owners.next().is_some() || owner.id == function.entry {
            return None;
        }
        let incoming = function
            .block_nodes()
            .flat_map(|node| &node.successors)
            .filter(|edge| edge.target == owner.id)
            .map(|edge| edge.binding(value).map(|binding| binding.argument))
            .collect::<Option<Vec<_>>>()?;
        Some(terminal_semantics::UnchangedScalarOrigin::Incoming(
            incoming,
        ))
    })?
    .0;
    let target = kernel::selected_stored_effects_row(catalog, table, interface, slot)?;
    let signature = catalog
        .interface(interface)?
        .slots
        .get(usize::try_from(slot).ok()?)?;
    let mut bodies = functions
        .iter()
        .copied()
        .filter(|body| body.machine == target);
    let body = bodies.next()?;
    let result_matches = match &signature.result {
        terminal_psi::BoundaryMachineResult::Unit => matches!(
            body.result,
            abstract_operations::AbstractFunctionResult::Unit
        ),
        terminal_psi::BoundaryMachineResult::Scalar(expected) => {
            body.result.scalar().is_some_and(|result| {
                expected.qualifications == semantic_vocabulary::ScalarQualificationSetId::ZERO
                    && result.scalar_type == expected.scalar_type
            })
        }
        terminal_psi::BoundaryMachineResult::Structural(_) => false,
    };
    if bodies.next().is_some()
        || !result_matches
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

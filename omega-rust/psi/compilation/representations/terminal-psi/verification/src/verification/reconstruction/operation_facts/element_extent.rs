//! Join an element-view length observation to its exact validated producer or
//! to the descriptor every block arrival transfers.

use semantic_vocabulary::{Proposition, StructuralPlaceKind};
use terminal_psi::{
    Operation, OperationKind, StructuralAccess, StructuralTypeShape, TerminalMachine,
    TerminalModule,
};
use terminal_semantics::{
    StructuralEffectObservation, element_establishment_length_equation,
    element_subslice_length_equation, structural_effect_leaf_observation,
};

use crate::ModuleError;
use crate::control_graph::DominatorTree;

use super::byte_extent::{block_length_equation, descriptor_origins, operation_position};

/// Resolve the collection through the same canonical static projections as
/// establishment validation. A nested array's extent belongs to the selected
/// child, not its outer backing. Reference crossings remain explicit, while
/// byte windows need their own extent denotation and do not enter this walk.
fn path_end_type(
    module: &TerminalModule,
    mut current: semantic_vocabulary::StructuralTypeId,
    path: &[terminal_psi::StructuralPathSegment],
) -> Option<semantic_vocabulary::StructuralTypeId> {
    for segment in path {
        let declaration = module
            .structural_types
            .iter()
            .find(|item| item.id == current)?;
        current = match (segment, &declaration.shape) {
            (
                terminal_psi::StructuralPathSegment::Field(_)
                | terminal_psi::StructuralPathSegment::FixedIndex(_),
                _,
            ) => {
                terminal_semantics::canonical_static_projection(
                    module.structural_types.iter(),
                    current,
                    std::slice::from_ref(segment),
                )?
                .1
                .id
            }
            (
                terminal_psi::StructuralPathSegment::Referent,
                StructuralTypeShape::Reference { referent, .. },
            ) => *referent,
            _ => return None,
        };
    }
    Some(current)
}

/// The root structural type a place carries for an element-view source.
fn source_root_type(
    machine: &TerminalMachine,
    place: semantic_vocabulary::PlaceId,
) -> Option<semantic_vocabulary::StructuralTypeId> {
    if let Some(parameter) = machine
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == place)
    {
        return Some(parameter.structural_type);
    }
    if let Some(parameter) = machine
        .blocks
        .iter()
        .flat_map(|block| &block.structural_parameters)
        .find(|parameter| parameter.place == place)
    {
        return Some(parameter.structural_type);
    }
    machine
        .structural_places
        .iter()
        .find_map(|row| match row.kind {
            StructuralPlaceKind::OperationResult {
                structural_type, ..
            } if row.id == place => Some(structural_type),
            _ => None,
        })
}

/// Exact byte-region operands and reconstructed geometry of its formation.
pub(super) fn representation_region(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
) -> Option<(
    semantic_vocabulary::ValueId,
    semantic_vocabulary::ValueId,
    semantic_vocabulary::ObligationId,
    u64,
    u64,
)> {
    let (source, element, region) = match &operation.kind {
        OperationKind::EstablishElementView {
            source,
            element,
            region,
            ..
        }
        | OperationKind::EstablishMutableElementView {
            source,
            element,
            region,
            ..
        } => (source, *element, *region),
        _ => return None,
    };
    let terminal_psi::ElementViewRegion::RepresentationBytes {
        start,
        end,
        obligation,
    } = region
    else {
        return None;
    };
    let root = source_root_type(machine, source.place)?;
    let collection = path_end_type(module, root, &source.path)?;
    let terminal_semantics::ElementViewRegionGeometry::RepresentationBytes {
        byte_extent,
        element,
    } = terminal_semantics::element_view_region_geometry(
        module.structural_types.iter(),
        collection,
        element,
        region,
    )?
    else {
        return None;
    };
    Some((start, end, obligation, byte_extent, element.size))
}

/// The declared element count of the fixed collection an establishment names.
pub(super) fn establishment_collection_length(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
) -> Option<u64> {
    if matches!(
        operation.kind,
        OperationKind::EstablishElementView {
            region: terminal_psi::ElementViewRegion::RepresentationBytes { .. },
            ..
        } | OperationKind::EstablishMutableElementView {
            region: terminal_psi::ElementViewRegion::RepresentationBytes { .. },
            ..
        }
    ) {
        return None;
    }
    let (source, element) = match &operation.kind {
        OperationKind::EstablishElementView {
            source, element, ..
        }
        | OperationKind::EstablishMutableElementView {
            source, element, ..
        } => (source, *element),
        _ => return None,
    };
    let root = source_root_type(machine, source.place)?;
    let collection = path_end_type(module, root, &source.path)?;
    terminal_semantics::collection_backing(module.structural_types.iter(), collection, element)
        .map(|backing| backing.length)
}

/// Retain both the source's extent and equality with a previous observation.
/// These facts are independent: a caller-supplied view may have only the
/// observation, while a loop header may retain its fixed backing extent but
/// drop predecessor path facts. Choosing one equation would make unrelated
/// proofs depend on whether another length read happens to dominate the site.
pub(super) fn append_length_equations(
    module: &TerminalModule,
    machine: &TerminalMachine,
    current: &Operation,
    observation: &StructuralEffectObservation,
    context: &super::super::machine_context::MachineReconstructionContext<'_>,
    axioms: &mut Vec<Proposition>,
) -> Result<(), ModuleError> {
    let StructuralEffectObservation::ElementViewLengthRead { source, .. } = observation else {
        return Ok(());
    };
    if let Some(equation) = source_length_equation(module, machine, current, observation, context)?
    {
        crate::verification::substitution::push_unique_fact(axioms, equation);
    }
    if let Some(StructuralPlaceKind::BlockParameter { block, position }) = machine
        .structural_places
        .iter()
        .find(|place| place.id == *source)
        .map(|place| place.kind)
        && let Some(equation) = block_length_equation(
            module,
            machine,
            current,
            *source,
            block,
            position,
            context.dominators(),
        )
    {
        crate::verification::substitution::push_unique_fact(axioms, equation);
    }
    // Existing producer/arrival equations stay ahead of the descriptor alias:
    // join inference transports through the first definition of each value.
    // Otherwise an exact backing count can disappear behind a local view name.
    if let Some(equation) =
        super::byte_extent::view_length_equation(module, machine, current, *source)
    {
        crate::verification::substitution::push_unique_fact(axioms, equation);
    }
    Ok(())
}

/// Relate an `ElementViewLength` result to the extent its exact producer
/// established. Module validation has checked the producer/result join, source
/// type and dominance, so the equation is introduced only at the later read.
///
/// A block-parameter view has no producer of its own. It is related to the
/// nearest dominating producer whose exact result every arrival transfers; see
/// [`arrival_producer`] for why dominance keeps that producer's operands current
/// at this read. A signature view's own observation names its runtime extent.
fn source_length_equation(
    module: &TerminalModule,
    machine: &TerminalMachine,
    current: &Operation,
    observation: &StructuralEffectObservation,
    context: &super::super::machine_context::MachineReconstructionContext<'_>,
) -> Result<Option<Proposition>, ModuleError> {
    let StructuralEffectObservation::ElementViewLengthRead { source, result } = observation else {
        return Ok(None);
    };
    let Some(place_kind) = machine
        .structural_places
        .iter()
        .find(|place| place.id == *source)
        .map(|place| place.kind)
    else {
        return Ok(None);
    };
    let (operation, observation) = match place_kind {
        StructuralPlaceKind::OperationResult { producer, .. } => {
            let Some(operation) = machine
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .find(|operation| operation.id == producer)
            else {
                return Ok(None);
            };
            (operation, observation.clone())
        }
        StructuralPlaceKind::BlockParameter { block, position } => {
            let Some((operation, produced)) = arrival_producer(
                module,
                machine,
                current,
                *source,
                block,
                position,
                context.dominators(),
            ) else {
                return Ok(None);
            };
            // Every arrival transfers the producer's exact descriptor, so this
            // read measures the producer's own destination.
            (
                operation,
                StructuralEffectObservation::ElementViewLengthRead {
                    source: produced,
                    result: *result,
                },
            )
        }
        StructuralPlaceKind::Parameter { .. } => {
            return Ok(None);
        }
        _ => return Ok(None),
    };
    producer_length_equation(module, machine, operation, &observation)
}

/// Apply one validated producer's extent denotation to a read of its result.
fn producer_length_equation(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &Operation,
    observation: &StructuralEffectObservation,
) -> Result<Option<Proposition>, ModuleError> {
    if let Some((start, end, _, _, stride)) = representation_region(module, machine, operation) {
        let StructuralEffectObservation::ElementViewLengthRead { source, result } = observation
        else {
            return Ok(None);
        };
        if operation.result.structural().map(|row| row.place) != Some(*source) {
            return Ok(None);
        }
        let length = terminal_semantics::representation_region_length(start, end, stride)
            .ok_or(ModuleError::InvalidElementViewEstablishment(operation.id))?;
        let scalar = semantic_vocabulary::ScalarType::Integer(
            semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Unsigned, 64)
                .expect("u64"),
        );
        return Ok(Some(Proposition::Equal(
            semantic_vocabulary::ScalarTerm::value(*result, scalar),
            length,
        )));
    }
    let Some(producer_observation) = structural_effect_leaf_observation(operation)
        .map_err(ModuleError::OperationSemanticSchema)?
    else {
        return Ok(None);
    };
    if let StructuralEffectObservation::ElementViewEstablished { .. } = producer_observation {
        let Some(length) = establishment_collection_length(module, machine, operation) else {
            return Ok(None);
        };
        return element_establishment_length_equation(observation, &producer_observation, length)
            .map_err(ModuleError::OperationSemanticSchema);
    }
    element_subslice_length_equation(observation, &producer_observation)
        .map_err(ModuleError::OperationSemanticSchema)
}

/// The nearest producer whose exact result every arrival binds to the borrowed
/// block parameter `source`, with the producer's own destination place.
///
/// The producer must execute before `current` on every path: earlier in its
/// block, or in a dominating block. That restriction carries the soundness of
/// applying its extent equation here. Its start and end operands dominate the
/// producer and therefore the read, and no path from the producer's latest
/// execution to the read re-executes an operand's definition: such a path,
/// joined to the producer-free path from entry to that definition, would reach
/// the read without the producer. The backward descriptor walk then proves
/// every arrival carries that latest result. A path that meets the producer
/// under another name, or redefines a tracked name, supplies no equation, so a
/// different view on one incoming edge or a producer re-executed after the
/// binding rejects. A mutably borrowed parameter additionally requires every
/// name on the chain to be unchanged by any writer since the producer. Module
/// validation currently admits element-view length reads only under shared
/// custody, so that check guards a later mutable admission rather than any
/// accepted module today.
fn arrival_producer<'machine>(
    module: &TerminalModule,
    machine: &'machine TerminalMachine,
    current: &Operation,
    source: semantic_vocabulary::PlaceId,
    block: semantic_vocabulary::BlockId,
    position: u32,
    dominators: &DominatorTree,
) -> Option<(&'machine Operation, semantic_vocabulary::PlaceId)> {
    let destination = machine
        .blocks
        .iter()
        .find(|candidate| candidate.id == block)?
        .structural_parameters
        .get(position as usize)?;
    if destination.place != source
        || !matches!(
            destination.access,
            StructuralAccess::MutableBorrow | StructuralAccess::SharedBorrow
        )
    {
        return None;
    }
    let needs_freshness = destination.access == StructuralAccess::MutableBorrow;
    let current_position = operation_position(machine, current.id)?;
    let current_block = machine.blocks[current_position.0].id;
    let mut candidates = Vec::new();
    for (block_position, candidate_block) in machine.blocks.iter().enumerate() {
        if !dominators.dominates(candidate_block.id, current_block) {
            continue;
        }
        for (operation_position, producer) in candidate_block.operations.iter().enumerate() {
            if block_position == current_position.0 && operation_position >= current_position.1 {
                continue;
            }
            if !matches!(
                producer.kind,
                OperationKind::EstablishElementView { .. }
                    | OperationKind::EstablishMutableElementView { .. }
                    | OperationKind::ElementViewSubslice { .. }
            ) {
                continue;
            }
            if let Some(result) = producer.result.structural() {
                candidates.push((block_position, operation_position, producer, result.place));
            }
        }
    }
    // Dominating blocks form a chain. Prefer the nearest producer, retaining a
    // stable identity order independently of the serialized block roster.
    candidates.sort_by_key(|(block_position, operation_position, _, _)| {
        let block = machine.blocks[*block_position].id;
        (
            std::cmp::Reverse(dominators.depth(block).unwrap_or(0)),
            block,
            std::cmp::Reverse(*operation_position),
        )
    });
    candidates
        .into_iter()
        .find(|(block_position, operation_position, producer, produced)| {
            descriptor_origins(
                machine,
                current_position,
                source,
                (*block_position, *operation_position),
                *produced,
            )
            .is_some_and(|aliases| {
                !needs_freshness
                    || aliases.into_iter().all(|place| {
                        crate::validation::view_length_is_current(
                            module,
                            machine,
                            producer.id,
                            current.id,
                            place,
                        )
                    })
            })
        })
        .map(|(_, _, producer, produced)| (producer, produced))
}

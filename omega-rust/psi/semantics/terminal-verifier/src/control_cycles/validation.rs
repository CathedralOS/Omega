//! Exact full-graph coverage and successor-rank substitution.

use std::collections::{BTreeMap, BTreeSet};

use semantic_vocabulary::{
    BlockId, IntegerCarrier, IntegerSign, PlaceId, ScalarType, StructuralPlaceKind, ValueId,
};
use terminal_psi::{
    OperationKind, TerminalMachine, TerminalNaturalRankComparison, TerminalRankedScc, Terminator,
};

use crate::{ModuleError, control_graph};

pub(crate) fn validate_natural_cycles(machine: &TerminalMachine) -> Result<(), ModuleError> {
    let Some(TerminalRankedScc::Natural(components)) = &machine.ranked_scc else {
        return Ok(());
    };
    let invalid = || ModuleError::InvalidRankedScc(machine.id);
    let topology = control_graph::cyclic_components(machine);
    if components.is_empty() || components.len() != topology.len() {
        return Err(invalid());
    }
    let dominators = control_graph::dominators(machine);
    let outgoing = control_graph::successors(machine);
    for (component, members) in components.iter().zip(topology) {
        if component.rank_type.carrier() != IntegerCarrier::Fixed
            || component.rank_type.sign() != IntegerSign::Unsigned
            || component
                .ranks
                .iter()
                .map(|rank| rank.block)
                .collect::<Vec<_>>()
                != members
            || component
                .edges
                .windows(2)
                .any(|edges| edges[0].edge >= edges[1].edge)
        {
            return Err(invalid());
        }
        let ranks = component
            .ranks
            .iter()
            .map(|rank| (rank.block, rank.value))
            .collect::<BTreeMap<_, _>>();
        let expected_edges = members
            .iter()
            .flat_map(|block| {
                outgoing[block].iter().filter_map(|(edge, target)| {
                    ranks
                        .contains_key(target)
                        .then_some((*edge, *block, *target))
                })
            })
            .collect::<BTreeSet<_>>();
        if component
            .edges
            .iter()
            .map(|edge| (edge.edge, edge.source, edge.target))
            .collect::<BTreeSet<_>>()
            != expected_edges
        {
            return Err(invalid());
        }
        for rank in &component.ranks {
            if value_type(machine, rank.value) != Some(ScalarType::Integer(component.rank_type))
                || !available(machine, &dominators, rank.value, rank.block)
                || !rank_origin(machine, rank.value)
            {
                return Err(invalid());
            }
        }
        for edge in &component.edges {
            if value_type(machine, edge.successor_rank)
                != Some(ScalarType::Integer(component.rank_type))
                || !available(machine, &dominators, edge.successor_rank, edge.source)
                || !substituted_rank(
                    machine,
                    edge.source,
                    edge.edge,
                    edge.target,
                    ranks[&edge.target],
                    edge.successor_rank,
                )
            {
                return Err(invalid());
            }
        }
        // Every cycle must encounter strict descent. Checking DFS-discovered
        // cycles alone misses preserving cross-edge cycles.
        let mut incoming = members
            .iter()
            .map(|block| (*block, 0usize))
            .collect::<BTreeMap<_, _>>();
        for edge in component
            .edges
            .iter()
            .filter(|edge| edge.comparison == TerminalNaturalRankComparison::Preserving)
        {
            *incoming.get_mut(&edge.target).ok_or_else(invalid)? += 1;
        }
        let mut ready = incoming
            .iter()
            .filter_map(|(block, count)| (*count == 0).then_some(*block))
            .collect::<Vec<_>>();
        let mut visited = 0;
        while let Some(block) = ready.pop() {
            visited += 1;
            for edge in component.edges.iter().filter(|edge| {
                edge.source == block && edge.comparison == TerminalNaturalRankComparison::Preserving
            }) {
                let count = incoming.get_mut(&edge.target).ok_or_else(invalid)?;
                *count -= 1;
                if *count == 0 {
                    ready.push(edge.target);
                }
            }
        }
        if visited != members.len() {
            return Err(invalid());
        }
    }
    Ok(())
}

fn value_type(machine: &TerminalMachine, value: ValueId) -> Option<ScalarType> {
    machine
        .parameters
        .iter()
        .chain(machine.blocks.iter().flat_map(|block| &block.parameters))
        .copied()
        .chain(
            machine
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .filter_map(|operation| operation.result.scalar()),
        )
        .find(|declaration| declaration.id == value)
        .map(|declaration| declaration.scalar_type)
}

fn definition(machine: &TerminalMachine, value: ValueId) -> Option<BlockId> {
    machine
        .blocks
        .iter()
        .find(|block| {
            block
                .parameters
                .iter()
                .any(|parameter| parameter.id == value)
                || block.operations.iter().any(|operation| {
                    operation
                        .result
                        .scalar()
                        .is_some_and(|result| result.id == value)
                })
        })
        .map(|block| block.id)
}

fn available(
    machine: &TerminalMachine,
    dominators: &BTreeMap<BlockId, BTreeSet<BlockId>>,
    value: ValueId,
    block: BlockId,
) -> bool {
    machine
        .parameters
        .iter()
        .any(|parameter| parameter.id == value)
        || definition(machine, value).is_some_and(|owner| {
            dominators
                .get(&block)
                .is_some_and(|blocks| blocks.contains(&owner))
        })
}

fn observed_view(machine: &TerminalMachine, value: ValueId) -> Option<PlaceId> {
    machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| {
            if operation
                .result
                .scalar()
                .is_none_or(|result| result.id != value)
            {
                return None;
            }
            match operation.kind {
                OperationKind::ByteSequenceLength { source } => Some(source),
                _ => None,
            }
        })
}

fn rank_origin(machine: &TerminalMachine, value: ValueId) -> bool {
    machine
        .parameters
        .iter()
        .chain(machine.blocks.iter().flat_map(|block| &block.parameters))
        .any(|parameter| parameter.id == value)
        || observed_view(machine, value).is_some()
}

fn substituted_rank(
    machine: &TerminalMachine,
    source: BlockId,
    edge: semantic_vocabulary::EdgeId,
    target: BlockId,
    target_rank: ValueId,
    successor_rank: ValueId,
) -> bool {
    let Some(source_block) = machine.blocks.iter().find(|block| block.id == source) else {
        return false;
    };
    let Some(target_block) = machine.blocks.iter().find(|block| block.id == target) else {
        return false;
    };
    let (arguments, views) = match &source_block.terminator {
        Terminator::Jump {
            edge: actual,
            target: destination,
            arguments,
            structural_arguments,
            ..
        } if *actual == edge && *destination == target => (arguments, structural_arguments),
        Terminator::Conditional {
            when_true,
            when_false,
            ..
        } => {
            let Some(successor) = [when_true, when_false]
                .into_iter()
                .find(|successor| successor.edge == edge && successor.target == target)
            else {
                return false;
            };
            (&successor.arguments, &successor.structural_arguments)
        }
        _ => return false,
    };
    if let Some(position) = target_block
        .parameters
        .iter()
        .position(|parameter| parameter.id == target_rank)
    {
        return arguments.get(position).copied() == Some(successor_rank);
    }
    if let Some(target_view) = observed_view(machine, target_rank) {
        let arriving = if let Some(position) = target_block
            .structural_parameters
            .iter()
            .position(|parameter| parameter.place == target_view)
        {
            let Some(argument) = views.get(position) else {
                return false;
            };
            if !argument.path.is_empty() {
                return false;
            }
            argument.place
        } else {
            // A descriptor produced in the destination is recreated on its
            // next execution. Its current SSA storage identity is not an
            // arriving value; substituting its producer needs separate proof.
            if machine.structural_places.iter().any(|place| {
                place.id == target_view
                    && matches!(place.kind,
                    StructuralPlaceKind::OperationResult { producer, .. }
                    if target_block.operations.iter().any(|operation| operation.id == producer))
            }) {
                return false;
            }
            target_view
        };
        // Ordinary validation gives each descriptor one establishing block
        // (and one producer for an operation result). Its length observation
        // dominates this edge, so reestablishment cannot bypass that read.
        return observed_view(machine, successor_rank) == Some(arriving);
    }
    target_rank == successor_rank
}

//! Current-graph exclusive record/view bindings and fresh view observations.
//!
//! Each invocation prepares exact edge transfers from this graph, then solves
//! definite availability and possible liveness separately. Transfers do not
//! depend on the evolving arrival sets; rediscovering them for every destination
//! in every round made validation scan the whole graph quadratically. Keeping
//! edge occurrences (not predecessor sets) also preserves parallel-edge loans.
use crate::OptimizationUnitValidationError;
use abstract_operations::AbstractOperation as O;
use optimization_unit::PsiOptimizationFunction;
use semantic_vocabulary::{BlockId, PlaceId, StructuralTypeId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{ByteSequenceCarrier, StructuralAccess, StructuralTypeShape};

#[cfg(test)]
mod tests;

/// Exact exclusive spellings live at each block entrance. Ownership pins and
/// operand validation consume the same replay, so a descriptor address cannot
/// keep retired authority alive after an explicit whole-name handoff.
pub(crate) struct CurrentMutableBorrows {
    mutable: BTreeSet<PlaceId>,
    arrivals: BTreeMap<BlockId, BTreeSet<PlaceId>>,
    possible_arrivals: BTreeMap<BlockId, BTreeSet<PlaceId>>,
}

impl CurrentMutableBorrows {
    pub(crate) fn may_be_live(&self, block: BlockId, place: PlaceId) -> bool {
        self.possible_arrivals
            .get(&block)
            .is_some_and(|places| places.contains(&place))
    }
}

/// Invocation-local transfer: (incoming - retired) union generated. Produced
/// names are included in generated only after this edge's retirements, while
/// destination bindings are generated after retirement of their previous names.
struct MutableBorrowTransfer {
    source: BlockId,
    target: BlockId,
    retired: BTreeSet<PlaceId>,
    generated: BTreeSet<PlaceId>,
}

fn prepare_transfers(
    function: &PsiOptimizationFunction,
    mutable: &BTreeSet<PlaceId>,
    descriptor_views: &BTreeSet<PlaceId>,
    produced: &BTreeMap<BlockId, BTreeSet<PlaceId>>,
) -> Vec<MutableBorrowTransfer> {
    let blocks = function
        .blocks
        .iter()
        .map(|block| (block.id, block))
        .collect::<BTreeMap<_, _>>();
    let block_names = function
        .blocks
        .iter()
        .flat_map(|block| &block.structural_parameters)
        .map(|parameter| parameter.place)
        .collect::<BTreeSet<_>>();
    let mut transfers = Vec::new();
    for source in &function.blocks {
        for edge in source.nodes.iter().flat_map(|node| &node.successors) {
            // Unknown successors are rejected by CFG validation, not repaired
            // here. Like the prior replay, only existing destinations contribute.
            let Some(target) = blocks.get(&edge.target) else {
                continue;
            };
            let mut retired = target
                .structural_parameters
                .iter()
                .map(|parameter| parameter.place)
                .collect::<BTreeSet<_>>();
            for binding in &edge.structural_bindings {
                let argument = &binding.argument;
                let attenuated_block_name = argument.access == StructuralAccess::SharedBorrow
                    && block_names.contains(&argument.place)
                    && target.structural_parameters.iter().any(|parameter| {
                        parameter.place == binding.parameter
                            && parameter.access == StructuralAccess::SharedBorrow
                    });
                // A whole descriptor handoff retires its exclusive spelling,
                // including attenuation to shared access. A projected loan
                // keeps the parent name; invocation records retain their own
                // restoration lifetime unless they transfer mutable authority.
                if mutable.contains(&argument.place)
                    && argument.path.is_empty()
                    && (descriptor_views.contains(&argument.place)
                        || attenuated_block_name
                        || argument.access == StructuralAccess::MutableBorrow)
                {
                    retired.insert(argument.place);
                }
            }
            let mut generated = produced[&source.id]
                .difference(&retired)
                .copied()
                .collect::<BTreeSet<_>>();
            generated.extend(
                edge.structural_bindings
                    .iter()
                    .map(|binding| binding.parameter)
                    .filter(|place| mutable.contains(place)),
            );
            transfers.push(MutableBorrowTransfer {
                source: source.id,
                target: edge.target,
                retired,
                generated,
            });
        }
    }
    transfers
}

pub(crate) fn current_mutable_borrows(
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
) -> CurrentMutableBorrows {
    let produced = function
        .blocks
        .iter()
        .map(|block| {
            let views = block
                .nodes
                .iter()
                .filter_map(|node| {
                    let source =
                        super::super::operation_contracts::structural_operation_result_contract(
                            function,
                            &node.operation,
                        )?;
                    let result = source.operation_result()?;
                    (source.access == StructuralAccess::MutableBorrow
                        && types
                            .get(&source.structural_type)
                            .is_some_and(|declaration| {
                                matches!(
                                    declaration.shape,
                                    StructuralTypeShape::ByteSequence(
                                        ByteSequenceCarrier::BorrowedView { .. }
                                    ) | StructuralTypeShape::ElementView { .. }
                                )
                            }))
                    .then_some(result.place)
                })
                .collect::<BTreeSet<_>>();
            (block.id, views)
        })
        .collect::<BTreeMap<_, _>>();
    let mutable = function
        .structural_parameters
        .iter()
        .chain(
            function
                .blocks
                .iter()
                .flat_map(|block| &block.structural_parameters),
        )
        .filter(|parameter| {
            parameter.access == StructuralAccess::MutableBorrow
                && types
                    .get(&parameter.structural_type)
                    .is_some_and(|declaration| {
                        matches!(
                            declaration.shape,
                            StructuralTypeShape::ByteSequence(
                                ByteSequenceCarrier::BorrowedView { .. }
                            ) | StructuralTypeShape::ElementView { .. }
                        ) || {
                            // A mutable record join carries the same exclusive
                            // permission as an incoming mutable record. Its
                            // source name expires on transfer, even though the
                            // physical pointer still names the same referent.
                            let mut joined = (**parameter).clone();
                            joined.is_self = false;
                            abstract_operations::control_flow::address_joins::is_address_join(
                                &joined,
                                |identity| types.get(&identity).copied(),
                            )
                        }
                    })
        })
        .map(|parameter| parameter.place)
        .chain(produced.values().flatten().copied())
        .collect::<BTreeSet<_>>();
    if mutable.is_empty() {
        return CurrentMutableBorrows {
            mutable,
            arrivals: BTreeMap::new(),
            possible_arrivals: BTreeMap::new(),
        };
    }
    // A whole descriptor handoff retires its exclusive spelling even when
    // the receiving parameter attenuates it to shared access.
    // Record arguments instead retain their per-occurrence access: several
    // shared observations are legal, and a shared reborrow does not transfer
    // the parent's exclusive permission to a new mutable name.
    let descriptor_views = function
        .structural_parameters
        .iter()
        .chain(
            function
                .blocks
                .iter()
                .flat_map(|block| &block.structural_parameters),
        )
        .filter(|parameter| {
            mutable.contains(&parameter.place)
                && types
                    .get(&parameter.structural_type)
                    .is_some_and(|declaration| {
                        matches!(
                            declaration.shape,
                            StructuralTypeShape::ByteSequence(
                                ByteSequenceCarrier::BorrowedView { .. }
                            ) | StructuralTypeShape::ElementView { .. }
                        )
                    })
        })
        .map(|parameter| parameter.place)
        .chain(produced.values().flatten().copied())
        .collect::<BTreeSet<_>>();
    let entry = function
        .structural_parameters
        .iter()
        .map(|parameter| parameter.place)
        .filter(|place| mutable.contains(place))
        .collect::<BTreeSet<_>>();
    let transfers = prepare_transfers(function, &mutable, &descriptor_views, &produced);
    let reached = transfers
        .iter()
        .map(|transfer| transfer.target)
        .chain(std::iter::once(function.entry))
        .collect::<BTreeSet<_>>();
    let mut arrivals = function
        .blocks
        .iter()
        .map(|block| {
            (
                block.id,
                if block.id == function.entry {
                    entry.clone()
                } else {
                    mutable.clone()
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    // Operand availability requires every path; suspension requires any path.
    // A diamond that attenuates only one arm cannot authorize a stale owner on
    // the other arm merely because its exclusive name is no longer definite.
    let mut possible_arrivals = function
        .blocks
        .iter()
        .map(|block| {
            (
                block.id,
                if block.id == function.entry {
                    entry.clone()
                } else {
                    BTreeSet::new()
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    // Start from the greatest candidate set and intersect every incoming path,
    // including backedges. A descriptor address does not keep a consumed loan live.
    loop {
        let mut next_arrivals = function
            .blocks
            .iter()
            .map(|block| {
                let incoming = if block.id == function.entry {
                    entry.clone()
                } else if reached.contains(&block.id) {
                    mutable.clone()
                } else {
                    BTreeSet::new()
                };
                (block.id, incoming)
            })
            .collect::<BTreeMap<_, _>>();
        let mut next_possible = function
            .blocks
            .iter()
            .map(|block| {
                (
                    block.id,
                    if block.id == function.entry {
                        entry.clone()
                    } else {
                        BTreeSet::new()
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        for transfer in &transfers {
            // Read only the previous round, even for backward and parallel
            // edges. Merge directly without allocating a carried set per edge.
            if let Some(incoming) = next_arrivals.get_mut(&transfer.target) {
                incoming.retain(|place| {
                    transfer.generated.contains(place)
                        || (!transfer.retired.contains(place)
                            && arrivals[&transfer.source].contains(place))
                });
            }
            if let Some(incoming) = next_possible.get_mut(&transfer.target) {
                incoming.extend(
                    possible_arrivals[&transfer.source]
                        .iter()
                        .copied()
                        .filter(|place| !transfer.retired.contains(place))
                        .chain(transfer.generated.iter().copied()),
                );
            }
        }
        if next_arrivals == arrivals && next_possible == possible_arrivals {
            break;
        }
        arrivals = next_arrivals;
        possible_arrivals = next_possible;
    }
    CurrentMutableBorrows {
        mutable,
        arrivals,
        possible_arrivals,
    }
}

pub(super) fn validate(
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &terminal_psi::StructuralTypeDeclaration>,
) -> Result<(), OptimizationUnitValidationError> {
    let CurrentMutableBorrows {
        mutable, arrivals, ..
    } = current_mutable_borrows(function, types);
    if mutable.is_empty() {
        return Ok(());
    }
    for block in &function.blocks {
        let mut available = arrivals[&block.id].clone();
        for (position, node) in block.nodes.iter().enumerate() {
            let invalid = |place| OptimizationUnitValidationError::StructuralPlaceNotAvailable {
                machine: function.machine,
                block: block.id,
                node: position as u32,
                place,
            };
            // Availability is per name. Each operation's contract separately
            // checks argument access and path overlap; repeated shared reads
            // or disjoint projected loans do not repeat exclusive authority.
            for place in super::structural_roots::operation_place_accesses(&node.operation) {
                if mutable.contains(&place) && !available.contains(&place) {
                    return Err(invalid(place));
                }
            }
            for edge in &node.successors {
                for (position, binding) in edge.structural_bindings.iter().enumerate() {
                    let place = binding.argument.place;
                    if !mutable.contains(&place) {
                        continue;
                    }
                    for prior in &edge.structural_bindings[..position] {
                        if prior.argument.place != place {
                            continue;
                        }
                        let shared = prior.argument.access == StructuralAccess::SharedBorrow;
                        let other_shared =
                            binding.argument.access == StructuralAccess::SharedBorrow;
                        // Terminal edges admit repeated shared observations
                        // and disjoint shared/mutable subloans, but retain the
                        // single mutable arrival per root rule.
                        if (!shared && !other_shared)
                            || (shared != other_shared
                                && terminal_semantics::structural_paths_may_overlap(
                                    &prior.argument.path,
                                    &binding.argument.path,
                                ))
                        {
                            return Err(invalid(place));
                        }
                    }
                }
            }
            if let Some((_, result)) = node.operation.structural_result()
                && mutable.contains(&result.place)
            {
                available.insert(result.place);
            }
            if let O::ByteSequenceWrite {
                destination,
                length,
                ..
            } = node.operation
            {
                // Rebound descriptors receive fresh observations at their current
                // block. Calls cannot preserve an old extent merely by preserving
                // the descriptor's address; a following measurement must rejoin it.
                let observed = block.nodes[..position].iter().rposition(|prior| matches!(prior.operation,
                    O::ByteSequenceLength { source, result, .. } if source == destination && result.value == length));
                if observed.is_none_or(|observed| {
                    block.nodes[observed + 1..position].iter().any(|prior| {
                        !super::structural_roots::operation_place_inputs(&prior.operation)
                            .is_empty()
                            && !matches!(
                                prior.operation,
                                O::ByteSequenceLength { .. } | O::ByteSequenceWrite { .. }
                            )
                    })
                }) {
                    return Err(invalid(destination));
                }
            }
        }
    }
    Ok(())
}

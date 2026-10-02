//! Current-graph exclusive record/view bindings and fresh view observations.
use crate::OptimizationUnitValidationError;
use abstract_operations::AbstractOperation as O;
use optimization_unit::PsiOptimizationFunction;
use semantic_vocabulary::{BlockId, PlaceId, StructuralTypeId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{ByteSequenceCarrier, StructuralAccess, StructuralTypeShape};

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
        let previous = arrivals.clone();
        let previous_possible = possible_arrivals.clone();
        for block in &function.blocks {
            let mut incoming = if block.id == function.entry {
                entry.clone()
            } else {
                mutable.clone()
            };
            let mut possible_incoming = if block.id == function.entry {
                entry.clone()
            } else {
                BTreeSet::new()
            };
            let mut reached = block.id == function.entry;
            for predecessor in &function.blocks {
                for edge in predecessor
                    .nodes
                    .iter()
                    .flat_map(|node| &node.successors)
                    .filter(|edge| edge.target == block.id)
                {
                    reached = true;
                    let mut carried = previous[&predecessor.id].clone();
                    let mut possible_carried = previous_possible[&predecessor.id].clone();
                    carried.extend(produced[&predecessor.id].iter().copied());
                    possible_carried.extend(produced[&predecessor.id].iter().copied());
                    for binding in &edge.structural_bindings {
                        let attenuated_block_name = binding.argument.access
                            == StructuralAccess::SharedBorrow
                            && binding.argument.path.is_empty()
                            && function.blocks.iter().any(|definition| {
                                definition
                                    .structural_parameters
                                    .iter()
                                    .any(|parameter| parameter.place == binding.argument.place)
                            })
                            && block.structural_parameters.iter().any(|parameter| {
                                parameter.place == binding.parameter
                                    && parameter.access == StructuralAccess::SharedBorrow
                            });
                        // A whole-root mutable borrow transfers the argument's
                        // custody to the receiving loan. A subtree borrow
                        // binds only the projected path, so the ambient root
                        // stays carried; uses through the lent path while the
                        // loan is live are not yet distinguished here.
                        // Whole block-name attenuation retires that exclusive
                        // spelling too; the shared arrival retains its original
                        // backing. Invocation resources keep their lifetime and
                        // can only restore through their own loan replay.
                        if mutable.contains(&binding.argument.place)
                            && binding.argument.path.is_empty()
                            && (descriptor_views.contains(&binding.argument.place)
                                || attenuated_block_name
                                || binding.argument.access == StructuralAccess::MutableBorrow)
                        {
                            carried.remove(&binding.argument.place);
                            possible_carried.remove(&binding.argument.place);
                        }
                    }
                    for parameter in &block.structural_parameters {
                        carried.remove(&parameter.place);
                        possible_carried.remove(&parameter.place);
                    }
                    for binding in &edge.structural_bindings {
                        if mutable.contains(&binding.parameter) {
                            carried.insert(binding.parameter);
                            possible_carried.insert(binding.parameter);
                        }
                    }
                    incoming.retain(|place| carried.contains(place));
                    possible_incoming.extend(possible_carried);
                }
            }
            if !reached {
                incoming.clear();
            }
            arrivals.insert(block.id, incoming);
            possible_arrivals.insert(block.id, possible_incoming);
        }
        if arrivals == previous && possible_arrivals == previous_possible {
            break;
        }
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

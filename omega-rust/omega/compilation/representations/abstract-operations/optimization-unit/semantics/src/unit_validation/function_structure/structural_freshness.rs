//! Current-CFG freshness for observations of structural storage metadata.
//! A byte length or sum discriminator remains evidence only while every path
//! from its establishment preserves the same storage and metadata. Node and
//! edge establishments share this one traversal, including exact root rebinding.

use super::byte_field_freshness::{field_path, overlaps};
use abstract_operations::AbstractOperation as O;
use optimization_unit::PsiOptimizationFunction;
use semantic_vocabulary::{BlockId, PlaceId, StructuralTypeId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{StructuralAccess, StructuralPathSegment, StructuralTypeDeclaration};

/// View names retain their backing's identity across calls. Unknown offsets
/// or aliases cannot certify that an earlier field observation is current.
fn call_argument_changes(
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    root: PlaceId,
    observed: &[StructuralPathSegment],
    argument: &terminal_psi::StructuralArgument,
) -> bool {
    use semantic_vocabulary::StructuralPlaceKind;

    let may_alias = |identity, region: &[StructuralPathSegment]| {
        terminal_semantics::runtime_structural_path_tip(types.values().copied(), identity, region)
            .is_none_or(|selected| {
                !crate::current_ownership::storage_cannot_retain_loan(
                    types,
                    selected.id,
                    &mut BTreeMap::new(),
                )
            })
    };
    let mut place = argument.place;
    let mut region = argument.path.clone();
    let mut visited = BTreeSet::new();
    loop {
        if region
            .iter()
            .any(|segment| matches!(segment, StructuralPathSegment::Referent))
        {
            return true;
        }
        if place == root {
            return terminal_semantics::structural_paths_may_overlap(observed, &region);
        }
        if !visited.insert(place)
            || function
                .block_structural_parameters()
                .any(|parameter| parameter.place == place || parameter.place == root)
        {
            return true;
        }
        let Some(declaration) = function.structural_place(place) else {
            return true;
        };
        let Some(contract) =
            crate::unit_validation::operation_contracts::structural_source_contract(
                function, place,
            )
        else {
            return true;
        };
        let StructuralPlaceKind::OperationResult {
            producer,
            structural_type,
        } = declaration.kind
        else {
            return may_alias(contract.structural_type, &region);
        };
        if contract.structural_type != structural_type {
            return true;
        }
        let Some(operation) = function
            .block_nodes()
            .map(|node| &node.operation)
            .find(|operation| operation.psi_operation() == Some(producer))
        else {
            return true;
        };
        match operation {
            O::EstablishElementView {
                source,
                result,
                destination,
                ..
            } if result.place == place
                && *destination == place
                && result.structural_type == contract.structural_type =>
            {
                let mut backing = source.path.clone();
                if let Some(range) = backing.iter().position(|segment| {
                    matches!(segment, StructuralPathSegment::FixedByteRange { .. })
                }) {
                    // A view-relative index cannot supply an absolute range offset.
                    backing.truncate(range);
                } else {
                    backing.extend(region);
                }
                region = backing;
                place = source.place;
            }
            abstract_operations::element_view_or_byte_subslice_pattern!(source, result, ..)
                if result.place == place && result.structural_type == contract.structural_type =>
            {
                // Descendant indices are relative to the narrowed view. The
                // selected descriptor's containing path still distinguishes
                // its siblings, but cannot donate an absolute child offset.
                region = source.path.clone();
                place = source.place;
            }
            _ => {
                // Distinct owner storage does not imply distinct reachable
                // storage: an aggregate may contain a view of the observed
                // bytes. Preserve history only for a selected subtree proven
                // recursively unable to retain a loan.
                return may_alias(structural_type, &region);
            }
        }
    }
}

/// Whether an operation may change a structural carrier's stored length or
/// discriminator. Scalar and byte leaf stores cannot address either metadata
/// word; whole-field replacement and unresolved effects invalidate evidence.
/// This is not value freshness: ordinary payload values can change while
/// these two metadata observations remain true.
pub(super) fn structural_metadata_may_change(
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    root: PlaceId,
    path: &[StructuralPathSegment],
    operation: &O,
) -> bool {
    match operation {
        O::ByteSequenceStore { destination, .. } => *destination == root,
        O::StructuralByteSequenceFieldStore {
            destination,
            path: written,
            field,
            ..
        } => {
            *destination == root
                && field_path(function, types, *destination, written, *field)
                    .is_none_or(|written| overlaps(path, &written))
        }
        O::WriteOnlyPrimitiveStore {
            destination,
            path: written,
            ..
        } => {
            // Match Terminal freshness: a sibling primitive array write cannot
            // invalidate this carrier's length or ASCII history. Prefixes and
            // possibly aliasing runtime indices still overlap.
            destination.place() == root
                && terminal_semantics::structural_paths_may_overlap(path, written)
        }
        // Shared boundary inputs can carry checked interior authority. Without
        // a disjoint effect footprint they are not evidence of a read-only call.
        O::BoundaryCall {
            structural_arguments,
            ..
        } => structural_arguments
            .iter()
            .any(|argument| call_argument_changes(function, types, root, path, argument)),
        O::CallStructural {
            structural_arguments,
            ..
        } => structural_arguments.iter().any(|argument| {
            matches!(
                argument.access,
                StructuralAccess::MutableBorrow
                    | StructuralAccess::WriteOnlyBorrow
                    | StructuralAccess::Owned
            ) && call_argument_changes(function, types, root, path, argument)
        }),
        // These operations cannot alter this bounded field's live length.
        // Scalar stores target a separately validated primitive leaf, and byte
        // stores preserve length even when they replace the measured field's contents.
        O::StructuralByteSequenceFieldByteStore { .. }
        | abstract_operations::byte_len_or_field_read_pattern!(..)
        | O::StructuralScalarFieldStore { .. }
        | O::ByteSequenceWrite { .. }
        | abstract_operations::byte_read_subslice_triple_pattern!(..)
        | abstract_operations::establish_byte_literal_triple_pattern!(..)
        | O::EstablishPrimitiveLocal { .. }
        | O::PrimitiveLocalStore { .. }
        | O::PrimitiveScalarRead { .. }
        | O::EstablishScalarArray { .. }
        | O::EstablishStructuralArray { .. }
        | abstract_operations::establish_record_or_case_pattern!(..)
        | O::StructuralCaseMembership { .. }
        | O::StructuralScalarFieldRead { .. }
        | O::IntegerConstant { .. }
        | O::IeeeFloatConstant { .. }
        | O::NearestIeeeFloatFusedMultiplyAdd { .. }
        | O::IeeeFloatCompare { .. }
        | O::FloatResultPolicy { .. }
        | O::BooleanConstant { .. }
        | abstract_operations::boolean_not_or_equal_pair_pattern!(..)
        | abstract_operations::integer_equal_less_pair_pattern!(..)
        | abstract_operations::bitwise_not_or_less_pair_pattern!(..)
        | abstract_operations::integer_widen_or_exact_cast_pattern!(..)
        | abstract_operations::bitwise_and_or_view_pair_pattern!(..)
        | abstract_operations::bitwise_or_xor_pair_pattern!(..)
        | abstract_operations::integer_dispatch_triple_pattern!(..)
        | O::Call { .. }
        | abstract_operations::jump_or_conditional_pattern!()
        | O::StructuralCase { .. }
        | abstract_operations::return_or_return_unit_pattern!()
        | abstract_operations::return_structural_or_crash_pattern!() => false,
        // Other effects have no independently established disjoint write
        // footprint here.
        _ => true,
    }
}

/// Every finite path arriving at the use must reach the observation first,
/// on the same backing, without a metadata-changing operation. Revisiting a
/// point closes only that graph search branch; other predecessors and mutations
/// on backedges still run.
pub(super) fn unchanged_since(
    function: &PsiOptimizationFunction,
    predecessors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
    producer: (usize, usize),
    producer_root: PlaceId,
    use_site: (usize, usize),
    root: PlaceId,
    invalidates: impl Fn(&O, PlaceId) -> bool,
) -> bool {
    established_on_every_path(
        function,
        predecessors,
        use_site,
        root,
        |position, _, root| position == producer && root == producer_root,
        invalidates,
    )
}

/// Every finite path arriving at the use must pass a point that
/// `establishes` the fact, with no `invalidates` operation after the nearest
/// such point. Each path stops at its own nearest establishment, so different
/// arrivals may rest on different establishments. Reaching the entry block's
/// start, or a block without an arrival, fails: parameters arrive with no
/// established fact.
pub(super) fn established_on_every_path(
    function: &PsiOptimizationFunction,
    predecessors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
    use_site: (usize, usize),
    root: PlaceId,
    establishes: impl Fn((usize, usize), &O, PlaceId) -> bool,
    invalidates: impl Fn(&O, PlaceId) -> bool,
) -> bool {
    established_on_every_path_with_edges(
        function,
        predecessors,
        use_site,
        root,
        establishes,
        |_, _, _, _| false,
        invalidates,
    )
}

/// The same frontier walk with an edge-local establishment, for observations
/// whose successful guard supplies evidence on only one outgoing edge.
/// The callback sees the predecessor spelling of the storage, after exact
/// whole-place rebinding; every other arrival still has to establish the fact.
pub(super) fn established_on_every_path_with_edges(
    function: &PsiOptimizationFunction,
    predecessors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
    use_site: (usize, usize),
    root: PlaceId,
    establishes: impl Fn((usize, usize), &O, PlaceId) -> bool,
    establishes_edge: impl Fn((usize, usize), &O, &optimization_unit::OptimizationEdge, PlaceId) -> bool,
    invalidates: impl Fn(&O, PlaceId) -> bool,
) -> bool {
    established_on_every_path_with_entry(
        function,
        predecessors,
        use_site,
        root,
        establishes,
        establishes_edge,
        |_| false,
        invalidates,
    )
}

/// Replay byte-content facts from either an operation or an authenticated entry
/// contract. Entry facts still traverse every intervening write and join.
pub(super) fn established_on_every_path_with_entry(
    function: &PsiOptimizationFunction,
    predecessors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
    use_site: (usize, usize),
    root: PlaceId,
    establishes: impl Fn((usize, usize), &O, PlaceId) -> bool,
    establishes_edge: impl Fn((usize, usize), &O, &optimization_unit::OptimizationEdge, PlaceId) -> bool,
    establishes_entry: impl Fn(PlaceId) -> bool,
    invalidates: impl Fn(&O, PlaceId) -> bool,
) -> bool {
    let mut pending = vec![(use_site.0, use_site.1, root)];
    let mut visited = BTreeSet::new();
    let mut established = false;
    while let Some((block_position, position, root)) = pending.pop() {
        if !visited.insert((block_position, position, root)) {
            continue;
        }
        let block = &function.blocks[block_position];
        if position != 0 {
            let previous = (block_position, position - 1);
            let operation = &block.nodes[position - 1].operation;
            if establishes(previous, operation, root) {
                established = true;
            } else {
                if invalidates(operation, root) {
                    return false;
                }
                pending.push((previous.0, previous.1, root));
            }
        } else {
            if block.id == function.entry {
                if !establishes_entry(root) {
                    return false;
                }
                established = true;
                continue;
            }
            let Some(arrivals) = predecessors
                .get(&block.id)
                .filter(|arrivals| !arrivals.is_empty())
            else {
                return false;
            };
            for arrival in arrivals {
                let Some((owner, predecessor)) = function
                    .blocks
                    .iter()
                    .enumerate()
                    .find(|(_, candidate)| candidate.id == *arrival)
                else {
                    return false;
                };
                let mut found_edge = false;
                for (position, node) in predecessor.nodes.iter().enumerate() {
                    for edge in node
                        .successors
                        .iter()
                        .filter(|edge| edge.target == block.id)
                    {
                        found_edge = true;
                        // The predecessor names this storage by whatever the
                        // edge bound. A binding that forwards only part of it
                        // leaves the fact unproven on this path.
                        let Some(before) =
                            crate::unit_validation::arrival_storage::root_before_edge(edge, root)
                        else {
                            return false;
                        };
                        if establishes_edge((owner, position), &node.operation, edge, before) {
                            established = true;
                        } else {
                            pending.push((owner, position + 1, before));
                        }
                    }
                }
                if !found_edge {
                    return false;
                }
            }
        }
    }
    established
}

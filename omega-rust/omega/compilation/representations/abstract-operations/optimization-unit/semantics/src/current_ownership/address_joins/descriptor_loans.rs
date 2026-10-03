//! Derived descriptors protect their projected backing for their observable life.
//!
//! A descriptor freshly reborrowed from an exclusive state arrival is usable;
//! that does not permit its parent to mutate the same storage while the child
//! remains observable. Current guard relations can separate a write from a
//! captured subrange; they never retire the backing loan. Reconstruct the
//! interval from current operations and
//! exact descriptor forwarding, including every possible child at a join.
//! Instruction positions matter at both ends: a preceding write did not touch
//! a live child, and a following write cannot disturb a completed scalar read.
//! Value names are followed through actual definitions and simultaneous edge
//! bindings. A loop's redefinition ends that name's old value, not its aliases.
//! Unknown escapes conservatively retain the loan until the activation ends.
//! Possible origins establish interference only, never permission or equality.

use super::super::call_accesses::argument_storage_is_closed;
use super::borrowed_regions::{BorrowedRegions, Position, Projection, Region};
use abstract_operations::AbstractOperation as O;
use optimization_unit::{OptimizationEdge, PsiOptimizationFunction};
use semantic_vocabulary::{BlockId, PlaceId, StructuralTypeId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{StructuralAccess, StructuralTypeDeclaration};
#[cfg(test)]
use terminal_psi::{StructuralFieldDeclaration, StructuralFieldType, StructuralTypeShape};
use terminal_semantics::descriptor_lifetimes::{DescriptorFlowEdge, DescriptorFlowStep};

struct DescriptorLoan {
    regions: Vec<Region>,
    live: BTreeSet<Position>,
    access: StructuralAccess,
    authorized: BTreeMap<Position, BTreeSet<PlaceId>>,
}

#[derive(Default)]
pub(super) struct DescriptorLoans {
    loans: Vec<DescriptorLoan>,
    observations: BTreeMap<Position, Vec<(PlaceId, Vec<Region>)>>,
}

impl DescriptorLoans {
    pub(super) fn new(
        function: &PsiOptimizationFunction,
        borrowed_regions: &BorrowedRegions,
        types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    ) -> Self {
        let mut births = Vec::new();
        for block in &function.blocks {
            for (index, node) in block.nodes.iter().enumerate() {
                let position = (
                    block.id,
                    u32::try_from(index).expect("unit node index fits u32"),
                );
                let descriptor = match &node.operation {
                    O::EstablishElementView { result, source, .. }
                        if source.access == StructuralAccess::SharedBorrow => Some((
                        result.place,
                        source.place,
                        Projection::new(&source.path),
                        source.access,
                    )),
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
                    } => Some((
                        result.place,
                        *source,
                        Projection::window(*start, *end, position),
                        crate::unit_validation::operation_contracts::structural_operation_result_contract(
                            function, &node.operation,
                        ).map_or(StructuralAccess::MutableBorrow, |source| source.access),
                    )),
                    _ => None,
                };
                if let Some((_, result)) = node.operation.structural_result() {
                    if let Some(origins) = borrowed_regions.call_origins(result.place) {
                        births.extend(origins.iter().filter_map(|origin| match origin {
                            terminal_semantics::descriptor_results::DescriptorCallOrigin::Argument { source, projection } => Some((position, result.place, *source, projection.clone(), StructuralAccess::SharedBorrow)),
                            terminal_semantics::descriptor_results::DescriptorCallOrigin::Literal { .. } => None,
                        }));
                    } else if matches!(node.operation, O::StructuralLeafCopy { .. })
                        && terminal_semantics::descriptor_results::is_shared_descriptor(
                            types.values().copied(),
                            result.structural_type,
                        )
                        && let Some(aliases) = borrowed_regions.direct_aliases(result.place)
                    {
                        births.extend(aliases.iter().map(|(source, projection)| {
                            (
                                position,
                                result.place,
                                *source,
                                projection.clone(),
                                StructuralAccess::SharedBorrow,
                            )
                        }));
                    }
                }
                if let Some((result, source, projection, access)) = descriptor {
                    births.push((position, result, source, projection, access));
                }
            }
        }
        if births.is_empty() {
            return Self::default();
        }
        let flow = DescriptorFlow::new(function, types, borrowed_regions);
        let mut loans = Vec::with_capacity(births.len());
        for (birth, descriptor, source, projection, access) in births {
            loans.push(DescriptorLoan {
                regions: borrowed_regions.resolve(source, projection),
                live: flow.live_positions(birth, descriptor),
                access,
                authorized: terminal_semantics::descriptor_lifetimes::descriptor_authorized_places(
                    &flow.steps,
                    birth,
                    descriptor,
                ),
            });
        }
        let observations = function
            .blocks
            .iter()
            .flat_map(|block| {
                block.nodes.iter().enumerate().map(move |(index, node)| {
                    let position = (block.id, index as u32);
                    (
                        position,
                        operation_observations(
                            function,
                            types,
                            borrowed_regions,
                            position,
                            &node.operation,
                        ),
                    )
                })
            })
            .collect();
        Self {
            loans,
            observations,
        }
    }

    pub(super) fn conflicted_operation(
        &self,
        borrowed_regions: &BorrowedRegions,
        block: BlockId,
        node: u32,
        operation: &O,
        moved: &[PlaceId],
    ) -> Option<PlaceId> {
        let position = (block, node);
        // Normal return cleanup closes the activation after its scalar result
        // is observed. It is not a move while an escaped child can still run.
        let moved = if matches!(operation, O::Return { .. } | O::ReturnUnit { .. }) {
            &[][..]
        } else {
            moved
        };
        let moved = moved
            .iter()
            .map(|place| {
                (
                    *place,
                    borrowed_regions.resolve(*place, Projection::new(&[])),
                )
            })
            .collect::<Vec<_>>();
        for loan in self
            .loans
            .iter()
            .filter(|loan| loan.live.contains(&position))
        {
            let observations = (loan.access != StructuralAccess::SharedBorrow)
                .then(|| self.observations.get(&position))
                .flatten()
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            for (place, regions) in borrowed_regions
                .writes_at(position)
                .iter()
                .chain(observations)
            {
                if !loan.authorizes(position, *place, regions)
                    && regions.iter().any(|region| {
                        loan.regions.iter().any(|backing| {
                            borrowed_regions.regions_overlap_at(backing, region, position)
                        })
                    })
                {
                    return Some(*place);
                }
            }
            // Descendant access never authorizes relocating or disposing its
            // backing while another live name still observes that backing.
            for (place, regions) in &moved {
                if regions.iter().any(|region| {
                    loan.regions.iter().any(|backing| {
                        borrowed_regions.regions_overlap_at(backing, region, position)
                    })
                }) {
                    return Some(*place);
                }
            }
        }
        None
    }

    pub(super) fn conflicted_edge(
        &self,
        borrowed_regions: &BorrowedRegions,
        block: BlockId,
        node: u32,
        edge: &OptimizationEdge,
        destination: &[terminal_psi::StructuralParameterDeclaration],
    ) -> Option<PlaceId> {
        for loan in &self.loans {
            if !loan.live.contains(&(block, node)) || !loan.live.contains(&(edge.target, 0)) {
                continue;
            }
            let disturbed = edge
                .structural_bindings
                .iter()
                .zip(destination)
                .filter(|(binding, parameter)| match binding.argument.access {
                    StructuralAccess::SharedBorrow => loan.access != StructuralAccess::SharedBorrow,
                    StructuralAccess::Owned => {
                        parameter.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
                    }
                    StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow => true,
                })
                .map(|(binding, _)| {
                    (
                        binding.argument.place,
                        Projection::new(&binding.argument.path),
                        binding.argument.access == StructuralAccess::Owned,
                    )
                })
                .chain(
                    edge.trivial_affine_discards
                        .iter()
                        .map(|place| (*place, Projection::new(&[]), true)),
                )
                .chain(
                    edge.residual_affine_discards
                        .iter()
                        .map(|discard| (discard.place, Projection::new(&[]), true)),
                );
            for (place, projection, forced) in disturbed {
                let regions = borrowed_regions.resolve(place, projection.at((block, node)));
                if (forced || !loan.authorizes((block, node), place, &regions))
                    && regions.iter().any(|region| {
                        loan.regions.iter().any(|backing| {
                            borrowed_regions.regions_overlap_at(backing, region, (block, node))
                        })
                    })
                {
                    return Some(place);
                }
            }
        }
        None
    }
}

impl DescriptorLoan {
    fn authorizes(&self, position: Position, place: PlaceId, regions: &[Region]) -> bool {
        self.authorized
            .get(&position)
            .is_some_and(|places| places.contains(&place))
            && !regions.is_empty()
            && regions
                .iter()
                .all(|region| region.root.is_some() && !region.projection.indirect)
    }
}

/// The common operand inventory supplies conservative observation demand.
/// Refine only operations retaining exact projection geometry. Metadata does
/// not read borrowed payload, and selected edge transport is checked separately.
fn operation_observations(
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    borrowed_regions: &BorrowedRegions,
    position: Position,
    operation: &O,
) -> Vec<(PlaceId, Vec<Region>)> {
    use super::super::storage_regions;
    use terminal_psi::StructuralPathSegment;
    if matches!(
        operation,
        O::ElementViewLength { .. }
            | O::ByteSequenceLength { .. }
            | O::Jump { .. }
            | O::Conditional { .. }
    ) {
        return Vec::new();
    }
    let mut remaining = descriptor_inputs(function, types, operation);
    let mut result = Vec::new();
    let mut closed_storage = BTreeMap::new();
    let mut retain = |place, projection: Projection| {
        if let Some(index) = remaining.iter().position(|input| *input == place) {
            remaining.remove(index);
        }
        result.push((
            place,
            borrowed_regions.resolve(place, projection.at(position)),
        ));
    };
    let projected = |place, path: &[StructuralPathSegment], field| {
        let region = storage_regions::projected(function, types, place, path, field);
        let mut projection = Projection::new(&region.path);
        projection.indirect |= region.indirect;
        let root = crate::unit_validation::operation_contracts::structural_source_contract(
            function, place, false,
        );
        // The shared path owner interprets selectors. A nested view/reference
        // dereferences storage outside this record; no contained-field alias
        // has been reconstructed here, so it cannot justify descendant access.
        projection.indirect |= root.is_none_or(|root| {
            (0..path.len()).any(|length| {
                terminal_semantics::runtime_structural_path_tip(
                    types.values().copied(),
                    root.structural_type,
                    &path[..length],
                )
                .is_none_or(|declaration| {
                    matches!(
                        declaration.shape,
                        terminal_psi::StructuralTypeShape::Reference { .. }
                    ) || (length != 0
                        && (matches!(
                            declaration.shape,
                            terminal_psi::StructuralTypeShape::ElementView { .. }
                        ) || declaration.shape.is_borrowed_byte_view()))
                })
            })
        });
        projection
    };
    match operation {
        O::PrimitiveScalarRead { source, path, .. }
        | O::StructuralCaseMembership { source, path, .. } => {
            retain(*source, projected(*source, path, None));
        }
        O::StructuralScalarFieldRead {
            source,
            path,
            field,
            ..
        } => {
            let path = storage_regions::canonical_field(function, types, *source, path, *field);
            retain(*source, projected(*source, &path, None));
        }
        O::ElementViewRead {
            source,
            index,
            obligation,
            path,
            ..
        } => {
            let mut selected = vec![StructuralPathSegment::RuntimeIndex {
                index: *index,
                obligation: *obligation,
            }];
            selected.extend_from_slice(path);
            retain(*source, projected(*source, &selected, None));
        }
        O::ByteSequenceRead {
            source,
            index,
            obligation,
            ..
        } => {
            retain(
                *source,
                projected(
                    *source,
                    &[StructuralPathSegment::RuntimeIndex {
                        index: *index,
                        obligation: *obligation,
                    }],
                    None,
                ),
            );
        }
        O::ElementViewSubslice {
            source, start, end, ..
        }
        | O::ByteSequenceSubslice {
            source, start, end, ..
        } => {
            retain(*source, Projection::window(*start, *end, position));
        }
        O::StructuralByteSequenceFieldLength {
            source,
            path,
            field,
            ..
        } => {
            retain(*source, projected(*source, path, Some(*field)));
        }
        O::StructuralByteSequenceFieldRead {
            source,
            path,
            field,
            ..
        } => {
            let mut projection = projected(*source, path, Some(*field));
            // The field may hold a descriptor whose backing is not the record
            // slot. Without an exact captured-field alias, keep it indirect.
            projection.indirect = true;
            retain(*source, projection);
        }
        O::CallStructural {
            structural_arguments,
            ..
        }
        | O::BoundaryCall {
            structural_arguments,
            ..
        } => {
            for argument in structural_arguments {
                let mut projection = projected(argument.place, &argument.path, None);
                // A callee can read through a descriptor stored inside an
                // otherwise disjoint record. Only closed addressed storage or
                // a direct view's reconstructed backing bounds that read.
                projection.indirect |=
                    !crate::unit_validation::operation_contracts::structural_source_contract(
                        function,
                        argument.place,
                        false,
                    )
                    .is_some_and(|source| {
                        argument_storage_is_closed(
                            types,
                            source.structural_type,
                            &argument.path,
                            argument.access,
                            &mut closed_storage,
                        )
                    });
                retain(argument.place, projection);
            }
        }
        O::EstablishReference { source, .. } | O::EstablishElementView { source, .. } => {
            retain(source.place, projected(source.place, &source.path, None));
        }
        _ => {
            // Reuse the mutation owner's projection inventory rather than
            // turning a field write into an invented whole-owner read.
            for (place, region) in
                storage_regions::written_or_exclusive_regions(function, types, operation)
            {
                let mut projection = Projection::new(&region.path);
                projection.indirect |= region.indirect;
                retain(place, projection);
            }
        }
    }
    for place in remaining {
        result.push((place, borrowed_regions.resolve(place, Projection::new(&[]))));
    }
    result
}

fn known_descriptor_use(
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    operation: &O,
    borrowed_regions: &BorrowedRegions,
    closed_storage: &mut BTreeMap<StructuralTypeId, bool>,
) -> bool {
    if let O::CallStructural {
        result,
        structural_arguments,
        dynamic_arguments,
        claim_transfers,
        ..
    } = operation
    {
        // The checked ordinary call cannot publish a borrowed alias through
        // a scalar/Unit result, a read-only input, or writable storage whose
        // closed type cannot retain a loan. Inspect the actual projection:
        // unrelated fields of its owner are not writable through this input.
        // Unknown storage, descriptor dispatch and explicit claim transfers
        // still require retained-lineage evidence and remain escapes here.
        let result_is_closed = match result {
            abstract_operations::AbstractStructuralCallResult::Unit
            | abstract_operations::AbstractStructuralCallResult::Scalar(_) => true,
            abstract_operations::AbstractStructuralCallResult::Structural {
                result,
                returned_claim_transfers,
                ..
            } => {
                returned_claim_transfers.is_empty()
                    && borrowed_regions.call_origins(result.place).is_some()
            }
        };
        return result_is_closed
            && structural_arguments.iter().all(|argument| {
                argument.access == StructuralAccess::SharedBorrow
                    || crate::unit_validation::operation_contracts::structural_source_contract(
                        function,
                        argument.place,
                        false,
                    )
                    .is_some_and(|source| {
                        argument_storage_is_closed(
                            types,
                            source.structural_type,
                            &argument.path,
                            argument.access,
                            closed_storage,
                        )
                    })
            })
            && dynamic_arguments.is_empty()
            && claim_transfers.is_empty();
    }
    if let O::StructuralLeafCopy { result, .. } = operation {
        return borrowed_regions.direct_aliases(result.place).is_some();
    }
    matches!(
        operation,
        O::ElementViewLength { .. }
            | O::ElementViewRead { .. }
            | O::ByteSequenceLength { .. }
            | O::ByteSequenceRead { .. }
            | O::EstablishElementView { .. }
            | O::ElementViewSubslice { .. }
            | O::ByteSequenceSubslice { .. }
            | O::Jump { .. }
            | O::Conditional { .. }
            | O::Return { .. }
            | O::ReturnUnit { .. }
            | O::Crash { .. }
            | O::EstablishRecord { .. }
    )
}

/// Current-value transport, separate from the conservative backing-region
/// graph. Redefinition ends one name, not every alias of that earlier value.
struct DescriptorFlow {
    steps: BTreeMap<Position, DescriptorFlowStep>,
}

impl DescriptorFlow {
    fn new(
        function: &PsiOptimizationFunction,
        types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
        borrowed_regions: &BorrowedRegions,
    ) -> Self {
        let mut steps = BTreeMap::new();
        let mut closed_storage = BTreeMap::new();
        for block in &function.blocks {
            for (node_index, node) in block.nodes.iter().enumerate() {
                let position = (
                    block.id,
                    u32::try_from(node_index).expect("unit node index fits u32"),
                );
                let derivations = match &node.operation {
                    O::EstablishElementView { result, source, .. } => {
                        vec![(source.place, result.place)]
                    }
                    O::ElementViewSubslice { result, source, .. }
                    | O::ByteSequenceSubslice { result, source, .. } => {
                        vec![(*source, result.place)]
                    }
                    O::CallStructural {
                        result:
                            abstract_operations::AbstractStructuralCallResult::Structural {
                                result, ..
                            },
                        ..
                    }
                    | O::StructuralLeafCopy { result, .. } => borrowed_regions
                        .direct_aliases(result.place)
                        .into_iter()
                        .flatten()
                        .map(|(source, _)| (*source, result.place))
                        .collect(),
                    // Observing the record can observe any captured descriptor.
                    // Every field contributes its cohort; copying the carrier
                    // does not copy the storage those descriptors borrow.
                    O::EstablishRecord { result, fields, .. } => fields
                        .iter()
                        .filter_map(|field| match &field.value {
                            terminal_psi::RecordFieldValue::Structural(argument) => {
                                Some((argument.place, result.place))
                            }
                            terminal_psi::RecordFieldValue::Scalar { .. } => None,
                        })
                        .collect(),
                    _ => Vec::new(),
                };
                let successors = if node.successors.is_empty() {
                    (node_index + 1 < block.nodes.len())
                        .then(|| DescriptorFlowEdge {
                            target: (block.id, position.1 + 1),
                            parameters: Vec::new(),
                            bindings: Vec::new(),
                        })
                        .into_iter()
                        .collect()
                } else {
                    node.successors
                        .iter()
                        .map(|edge| DescriptorFlowEdge {
                            target: (edge.target, 0),
                            parameters: function
                                .blocks
                                .iter()
                                .find(|block| block.id == edge.target)
                                .into_iter()
                                .flat_map(|block| &block.structural_parameters)
                                .map(|parameter| parameter.place)
                                .collect(),
                            bindings: edge
                                .structural_bindings
                                .iter()
                                .map(|binding| (binding.argument.place, binding.parameter))
                                .collect(),
                        })
                        .collect()
                };
                steps.insert(
                    position,
                    DescriptorFlowStep {
                        definitions: Self::result_definition(function, &node.operation)
                            .into_iter()
                            .collect(),
                        authority_derivations: if matches!(
                            node.operation,
                            O::EstablishElementView { .. }
                                | O::ElementViewSubslice { .. }
                                | O::ByteSequenceSubslice { .. }
                                | O::CallStructural { .. }
                                | O::StructuralLeafCopy { .. }
                        ) {
                            derivations.clone()
                        } else {
                            Vec::new()
                        },
                        derivations,
                        inputs: descriptor_inputs(function, types, &node.operation),
                        may_escape: !known_descriptor_use(
                            function,
                            types,
                            &node.operation,
                            borrowed_regions,
                            &mut closed_storage,
                        ),
                        successors,
                    },
                );
            }
        }
        Self { steps }
    }

    fn result_definition(function: &PsiOptimizationFunction, operation: &O) -> Option<PlaceId> {
        let contract =
            crate::unit_validation::operation_contracts::structural_operation_result_contract(
                function, operation,
            )?;
        let result = contract.operation_result()?;
        let producer = operation.psi_operation()?;
        // A coincident operation identity does not define this value: the
        // current node must actually publish the catalog's typed result.
        function
            .structural_places
            .iter()
            .any(|declaration| {
                declaration.id == result.place
                    && declaration.kind
                        == semantic_vocabulary::StructuralPlaceKind::OperationResult {
                            producer,
                            structural_type: result.structural_type,
                        }
            })
            .then_some(result.place)
    }

    fn live_positions(&self, birth: Position, descriptor: PlaceId) -> BTreeSet<Position> {
        terminal_semantics::descriptor_lifetimes::descriptor_live_positions(
            &self.steps,
            birth,
            descriptor,
        )
    }
}

#[cfg(test)]
mod authority_tests {
    use super::*;

    #[test]
    fn address_join_child_authority_requires_every_incoming_origin() {
        let [current, earlier, left, right, merged] =
            [1, 2, 3, 4, 5].map(|value| PlaceId::new(value).unwrap());
        let mut parents = BTreeMap::from([
            (left, BTreeSet::from([current])),
            (right, BTreeSet::from([current])),
            (merged, BTreeSet::from([left, right])),
        ]);
        assert!(super::super::entirely_descends_from(
            merged,
            current,
            &parents,
            &mut BTreeSet::new(),
        ));
        parents.get_mut(&right).unwrap().insert(earlier);
        assert!(!super::super::entirely_descends_from(
            merged,
            current,
            &parents,
            &mut BTreeSet::new(),
        ));
        parents.insert(right, BTreeSet::from([merged]));
        assert!(!super::super::entirely_descends_from(
            merged,
            current,
            &parents,
            &mut BTreeSet::new(),
        ));
    }
}

#[cfg(test)]
mod tests;

/// Carrier disposal keeps its affine debt, but an exact borrowed view has no
/// payload destructor and therefore does not observe its backing at that point.
fn descriptor_inputs(
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    operation: &O,
) -> Vec<PlaceId> {
    let mut inputs = crate::unit_validation::function_structure::operation_place_inputs(operation);
    let mut dispose = |place: PlaceId| {
        let descriptor = crate::unit_validation::operation_contracts::structural_source_contract(
            function, place, false,
        )
        .is_some_and(|source| {
            source.operation_result().is_some()
                && source.multiplicity == terminal_psi::StructuralMultiplicity::Affine
                && matches!(
                    source.access,
                    StructuralAccess::SharedBorrow | StructuralAccess::MutableBorrow
                )
                && types
                    .get(&source.structural_type)
                    .is_some_and(|declaration| {
                        matches!(
                            declaration.shape,
                            terminal_psi::StructuralTypeShape::ElementView { .. }
                        ) || declaration.shape.is_borrowed_byte_view()
                    })
        });
        // Cleanup inputs follow ordinary operands in the common inventory.
        // Remove only this occurrence, preserving a simultaneous actual use.
        if descriptor && let Some(position) = inputs.iter().rposition(|input| *input == place) {
            inputs.remove(position);
        }
    };
    match operation {
        O::Jump {
            trivial_affine_discards,
            ..
        }
        | O::ReturnStructural {
            trivial_affine_discards,
            ..
        } => {
            for place in trivial_affine_discards {
                dispose(*place);
            }
        }
        O::Return {
            cleanup_actions, ..
        }
        | O::ReturnUnit {
            cleanup_actions, ..
        } => {
            for cleanup in cleanup_actions {
                if let terminal_psi::TerminalAffineCleanupAction::DiscardRoot(place) = cleanup {
                    dispose(*place);
                }
            }
        }
        _ => {}
    }
    inputs
}

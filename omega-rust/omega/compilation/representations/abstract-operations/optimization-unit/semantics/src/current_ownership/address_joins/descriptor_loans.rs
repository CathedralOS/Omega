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

use super::super::storage_regions::storage_cannot_retain_loan;
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
}

#[derive(Default)]
pub(super) struct DescriptorLoans {
    loans: Vec<DescriptorLoan>,
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
                    O::EstablishElementView { result, source, .. } => Some((
                        result.place,
                        source.place,
                        Projection::new(&source.path),
                        source.access == StructuralAccess::SharedBorrow,
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
                        true,
                    )),
                    _ => None,
                };
                if let Some((result, source, projection, true)) = descriptor {
                    births.push((position, result, source, projection));
                }
            }
        }
        if births.is_empty() {
            return Self::default();
        }
        let flow = DescriptorFlow::new(function, types);
        let mut loans = Vec::with_capacity(births.len());
        for (birth, descriptor, source, projection) in births {
            loans.push(DescriptorLoan {
                regions: borrowed_regions.resolve(source, projection),
                live: flow.live_positions(birth, descriptor),
            });
        }
        Self { loans }
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
            for (place, regions) in borrowed_regions.writes_at(position).iter().chain(&moved) {
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
                    StructuralAccess::SharedBorrow => false,
                    StructuralAccess::Owned => {
                        parameter.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
                    }
                    StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow => true,
                })
                .map(|(binding, _)| {
                    (
                        binding.argument.place,
                        Projection::new(&binding.argument.path),
                    )
                })
                .chain(
                    edge.trivial_affine_discards
                        .iter()
                        .map(|place| (*place, Projection::new(&[]))),
                )
                .chain(
                    edge.residual_affine_discards
                        .iter()
                        .map(|discard| (discard.place, Projection::new(&[]))),
                );
            for (place, projection) in disturbed {
                if borrowed_regions
                    .resolve(place, projection)
                    .iter()
                    .any(|region| loan.regions.iter().any(|backing| backing.overlaps(region)))
                {
                    return Some(place);
                }
            }
        }
        None
    }
}

fn known_descriptor_use(
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    operation: &O,
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
        return matches!(
            result,
            abstract_operations::AbstractStructuralCallResult::Unit
                | abstract_operations::AbstractStructuralCallResult::Scalar(_)
        ) && structural_arguments.iter().all(|argument| {
            argument.access == StructuralAccess::SharedBorrow
                || crate::unit_validation::operation_contracts::structural_source_contract(
                    function,
                    argument.place,
                    false,
                )
                .and_then(|source| {
                    crate::unit_validation::structural_catalog::resolve_structural_path(
                        types,
                        source.structural_type,
                        &argument.path,
                    )
                })
                .is_some_and(|structural_type| {
                    storage_cannot_retain_loan(types, structural_type, closed_storage)
                })
        }) && dynamic_arguments.is_empty()
            && claim_transfers.is_empty();
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
                        ) {
                            derivations.clone()
                        } else {
                            Vec::new()
                        },
                        derivations,
                        inputs: crate::unit_validation::function_structure::operation_place_inputs(
                            &node.operation,
                        ),
                        may_escape: !known_descriptor_use(
                            function,
                            types,
                            &node.operation,
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

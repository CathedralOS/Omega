//! Shared descriptors freeze their projected backing for their observable life.
//!
//! A descriptor freshly reborrowed from an exclusive state arrival is usable;
//! that does not permit its parent to mutate the same storage while the child
//! remains observable. Reconstruct the interval from current operations and
//! exact descriptor forwarding, including every possible child at a join.
//! Instruction positions matter at both ends: a preceding write did not touch
//! a live child, and a following write cannot disturb a completed scalar read.
//! Value names are followed through actual definitions and simultaneous edge
//! bindings. A loop's redefinition ends that name's old value, not its aliases.
//! Unknown escapes conservatively retain the loan until the activation ends.
//! Possible origins establish interference only, never permission or equality.

use super::borrowed_regions::{BorrowedRegions, Position, Projection, Region};
use abstract_operations::AbstractOperation as O;
use optimization_unit::{OptimizationEdge, PsiOptimizationFunction};
use semantic_vocabulary::{BlockId, PlaceId, StructuralTypeId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{
    StructuralAccess, StructuralFieldDeclaration, StructuralFieldType, StructuralTypeDeclaration,
    StructuralTypeShape,
};

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
                    O::ElementViewSubslice { result, source, .. }
                    | O::ByteSequenceSubslice { result, source, .. } => Some((
                        result.place,
                        *source,
                        // Runtime slice endpoints cannot refine the parent footprint.
                        Projection {
                            path: Vec::new(),
                            exact: false,
                            indirect: false,
                        },
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
                if regions
                    .iter()
                    .any(|region| loan.regions.iter().any(|backing| backing.overlaps(region)))
                {
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

/// This is a storage property, not a constructor whitelist. Every case and
/// nested element must be closed owned data; missing or recursive declarations
/// cannot prove absence of an alias. Mark an in-progress declaration false so
/// cycles fail conservatively. Reuse the result across this function's calls;
/// storage declarations do not change while the descriptor flow is built.
fn storage_cannot_retain_loan(
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    structural_type: StructuralTypeId,
    established: &mut BTreeMap<StructuralTypeId, bool>,
) -> bool {
    if let Some(result) = established.get(&structural_type) {
        return *result;
    }
    established.insert(structural_type, false);
    let Some(declaration) = types.get(&structural_type) else {
        return false;
    };
    let result = match &declaration.shape {
        StructuralTypeShape::PrimitiveScalar(_) => true,
        StructuralTypeShape::ByteSequence(carrier) => !carrier.is_borrowed(),
        StructuralTypeShape::Record { fields } => {
            fields_cannot_retain_loan(types, fields, established)
        }
        StructuralTypeShape::FixedArray { element, .. } => {
            storage_cannot_retain_loan(types, *element, established)
        }
        StructuralTypeShape::Sum { cases } => cases
            .iter()
            .all(|case| fields_cannot_retain_loan(types, &case.fields, established)),
        StructuralTypeShape::Mixed { fields, cases } => {
            fields_cannot_retain_loan(types, fields, established)
                && cases
                    .iter()
                    .all(|case| fields_cannot_retain_loan(types, &case.fields, established))
        }
        StructuralTypeShape::FusedServiceBinding
        | StructuralTypeShape::Reference { .. }
        | StructuralTypeShape::ElementView { .. } => false,
    };
    established.insert(structural_type, result);
    result
}

fn fields_cannot_retain_loan(
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    fields: &[StructuralFieldDeclaration],
    established: &mut BTreeMap<StructuralTypeId, bool>,
) -> bool {
    fields.iter().all(|field| match &field.field_type {
        StructuralFieldType::Scalar(_)
        | StructuralFieldType::BoundedInteger(_)
        | StructuralFieldType::IeeeFloat(_) => true,
        StructuralFieldType::ByteSequence(carrier)
        | StructuralFieldType::PredicateByteSequence { carrier, .. } => !carrier.is_borrowed(),
        StructuralFieldType::Structural(child) => {
            storage_cannot_retain_loan(types, *child, established)
        }
        StructuralFieldType::Erased { .. } => field.relevance.is_erased(),
    })
}

/// Current-value transport, separate from the conservative backing-region
/// graph. Redefinition ends one name, not every alias of that earlier value.
struct DescriptorFlow {
    steps: BTreeMap<Position, DescriptorFlowStep>,
}

#[derive(Default)]
struct DescriptorFlowStep {
    definitions: Vec<PlaceId>,
    derivation: Option<(PlaceId, PlaceId)>,
    inputs: Vec<PlaceId>,
    may_escape: bool,
    successors: Vec<DescriptorFlowEdge>,
}

struct DescriptorFlowEdge {
    target: Position,
    parameters: Vec<PlaceId>,
    bindings: Vec<(PlaceId, PlaceId)>,
}

impl DescriptorFlowEdge {
    fn forward(&self, source: PlaceId) -> impl Iterator<Item = PlaceId> + '_ {
        // Every actual belongs to the pre-edge state. A swap must not read a
        // destination after another binding has replaced that destination.
        (!self.parameters.contains(&source))
            .then_some(source)
            .into_iter()
            .chain(
                self.bindings.iter().filter_map(move |(actual, parameter)| {
                    (*actual == source).then_some(*parameter)
                }),
            )
    }
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
                let derivation = match &node.operation {
                    O::EstablishElementView { result, source, .. } => {
                        Some((source.place, result.place))
                    }
                    O::ElementViewSubslice { result, source, .. }
                    | O::ByteSequenceSubslice { result, source, .. } => {
                        Some((*source, result.place))
                    }
                    _ => None,
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
                        derivation,
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
                operation,
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
        // Materialize only reachable (position, name) links. Returning to the
        // formation kills its old result name, not independently carried aliases.
        type Occurrence = (Position, PlaceId);
        let mut pending = Vec::new();
        if let Some(step) = self.steps.get(&birth) {
            for edge in &step.successors {
                pending.extend(edge.forward(descriptor).map(|name| (edge.target, name)));
            }
        }
        let mut visited = BTreeSet::new();
        let mut predecessors = BTreeMap::<Occurrence, BTreeSet<Occurrence>>::new();
        let mut observations = Vec::new();
        let mut escaped = Vec::new();
        while let Some(occurrence @ (position, name)) = pending.pop() {
            if !visited.insert(occurrence) {
                continue;
            }
            let Some(step) = self.steps.get(&position) else {
                continue;
            };
            if step.inputs.contains(&name) {
                observations.push(occurrence);
                if step.may_escape {
                    escaped.push(position);
                }
            }
            let mut outputs = Vec::new();
            if !step.definitions.contains(&name) {
                outputs.push(name);
            }
            if let Some((source, result)) = step.derivation
                && source == name
            {
                outputs.push(result);
            }
            for edge in &step.successors {
                for output in &outputs {
                    for destination in edge.forward(*output) {
                        let next = (edge.target, destination);
                        predecessors.entry(next).or_default().insert(occurrence);
                        pending.push(next);
                    }
                }
            }
        }
        let mut observed = BTreeSet::new();
        let mut live = BTreeSet::new();
        while let Some(occurrence @ (position, _)) = observations.pop() {
            if !observed.insert(occurrence) {
                continue;
            }
            live.insert(position);
            observations.extend(predecessors.get(&occurrence).into_iter().flatten().copied());
        }
        // A name kill cannot retire an untracked escape. Its possible alias
        // persists along every subsequent control edge until activation exit.
        let mut escaped_positions = BTreeSet::new();
        while let Some(position) = escaped.pop() {
            if !escaped_positions.insert(position) {
                continue;
            }
            live.insert(position);
            if let Some(step) = self.steps.get(&position) {
                escaped.extend(step.successors.iter().map(|edge| edge.target));
            }
        }
        live
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

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
use super::borrowed_regions::{Access, BorrowedRegions, Position, Projection, Region};
use abstract_operations::AbstractOperation as O;
use optimization_unit::{OptimizationEdge, PsiOptimizationFunction};
use semantic_vocabulary::{BlockId, PlaceId, StructuralTypeId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_semantics::descriptor_results::{
    DescriptorCallOrigin, DescriptorCompatibilityRequirement, DescriptorSourceOrigin,
};
mod compatibility;
pub(crate) use compatibility::validate_compatibility;
use terminal_psi::{StructuralAccess, StructuralTypeDeclaration};
#[cfg(test)]
use terminal_psi::{StructuralFieldDeclaration, StructuralFieldType, StructuralTypeShape};
use terminal_semantics::descriptor_lifetimes::{
    DescriptorFlowEdge, DescriptorFlowStep, DescriptorLocation,
};

struct DescriptorLoan {
    incoming: Option<DescriptorSourceOrigin>,
    regions: Vec<Region>,
    live: BTreeSet<Position>,
    access: StructuralAccess,
    authorized: BTreeMap<Position, BTreeSet<DescriptorLocation>>,
}

#[derive(Default)]
pub(super) struct DescriptorLoans {
    loans: Vec<DescriptorLoan>,
    observations: BTreeMap<Position, Vec<Access>>,
    exclusive_inputs: BTreeSet<PlaceId>,
    requirements: Vec<DescriptorCompatibilityRequirement>,
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
                if let O::StructuralByteViewFieldStore {
                    destination,
                    path,
                    field,
                    source,
                    ..
                } = &node.operation
                {
                    let selected = super::super::storage_regions::projected(
                        function,
                        types,
                        *destination,
                        path,
                        Some(*field),
                    )
                    .path;
                    if let Some(slot) =
                        borrowed_regions.location_at(position, *destination, &selected)
                        && let Some(origins) =
                            borrowed_regions.source_origins_at(position, *source, &[])
                    {
                        births.extend(origins.iter().filter_map(|origin| match origin {
                            terminal_semantics::descriptor_results::DescriptorCallOrigin::Argument { source, projection } => Some((position, slot.clone(), *source, projection.clone(), StructuralAccess::SharedBorrow)),
                            _ => None,
                        }));
                    }
                }
                for effect in borrowed_regions
                    .stored_effects(position)
                    .into_iter()
                    .flatten()
                {
                    births.extend(effect.origins.iter().filter_map(|origin| match origin {
                        terminal_semantics::descriptor_results::DescriptorCallOrigin::Argument { source, projection } => Some((position, effect.destination.clone(), *source, projection.clone(), StructuralAccess::SharedBorrow)),
                        _ => None,
                    }));
                }
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
                            terminal_semantics::descriptor_results::DescriptorCallOrigin::Argument { source, projection } => Some((position, DescriptorLocation::whole(result.place), *source, projection.clone(), StructuralAccess::SharedBorrow)),
                            terminal_semantics::descriptor_results::DescriptorCallOrigin::Empty
                            | terminal_semantics::descriptor_results::DescriptorCallOrigin::Literal { .. } => None,
                        }));
                    } else if matches!(node.operation, O::StructuralLeafCopy { .. })
                        && terminal_semantics::descriptor_results::is_shared_descriptor(
                            types.values().copied(),
                            result.structural_type,
                        )
                    {
                        if let Some(origins) = borrowed_regions.source_origins_at(
                            (position.0, position.1 + 1),
                            result.place,
                            &[],
                        ) {
                            births.extend(origins.iter().filter_map(|origin| match origin {
                                terminal_semantics::descriptor_results::DescriptorCallOrigin::Argument { source, projection } => Some((position, DescriptorLocation::whole(result.place), *source, projection.clone(), StructuralAccess::SharedBorrow)),
                                _ => None,
                            }));
                        } else if let Some(aliases) = borrowed_regions.direct_aliases(result.place)
                        {
                            births.extend(aliases.iter().map(|(source, projection)| {
                                (
                                    position,
                                    DescriptorLocation::whole(result.place),
                                    *source,
                                    projection.clone(),
                                    StructuralAccess::SharedBorrow,
                                )
                            }));
                        }
                    }
                }
                if let Some((result, source, projection, access)) = descriptor {
                    births.push((
                        position,
                        DescriptorLocation::whole(result),
                        source,
                        projection,
                        access,
                    ));
                }
            }
        }
        if births.is_empty() {
            return Self::default();
        }
        let flow = descriptor_flow(function, types, borrowed_regions);
        let mut loans = Vec::with_capacity(births.len());
        for (birth, descriptor, source, projection, access) in births {
            loans.push(DescriptorLoan {
                incoming: borrowed_regions.incoming_origin_at((birth.0, birth.1 + 1), &descriptor, source, &projection),
                regions: borrowed_regions.resolve(source, projection),
                live: terminal_semantics::descriptor_lifetimes::descriptor_location_live_positions(&flow, birth, &descriptor),
                access,
                authorized: terminal_semantics::descriptor_lifetimes::descriptor_location_authorized_locations(&flow, birth, &descriptor),
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
        let mut result = Self {
            loans,
            observations,
            exclusive_inputs: function
                .structural_parameters
                .iter()
                .filter(|parameter| {
                    matches!(
                        parameter.access,
                        StructuralAccess::Owned
                            | StructuralAccess::MutableBorrow
                            | StructuralAccess::WriteOnlyBorrow
                    )
                })
                .map(|parameter| parameter.place)
                .collect(),
            requirements: Vec::new(),
        };
        let mut requirements = Vec::new();
        let mut retain = |requirement| {
            if !requirements.contains(&requirement) {
                requirements.push(requirement);
            }
            true
        };
        for block in &function.blocks {
            for (index, node) in block.nodes.iter().enumerate() {
                result.conflicted_operation_replaying(
                    borrowed_regions,
                    block.id,
                    index as u32,
                    &node.operation,
                    &[],
                    &mut retain,
                );
                for edge in &node.successors {
                    if let Some(target) =
                        function.blocks.iter().find(|block| block.id == edge.target)
                    {
                        result.conflicted_edge_replaying(
                            borrowed_regions,
                            block.id,
                            index as u32,
                            edge,
                            &target.structural_parameters,
                            &mut retain,
                        );
                    }
                }
            }
        }
        result.requirements = requirements;
        result
    }

    fn compatibility_requirement(
        &self,
        loan: &DescriptorLoan,
        demand: &Region,
        position: Position,
    ) -> Option<DescriptorCompatibilityRequirement> {
        let incoming = loan.incoming.as_ref()?;
        let source = demand.root?;
        if loan.access != StructuralAccess::SharedBorrow
            || !self.exclusive_inputs.contains(&source)
            || !demand.projection.exact
            || demand.projection.indirect
            || demand.projection.window.is_some()
            || demand.projection.captured_at.is_some()
        {
            return None;
        }
        Some(DescriptorCompatibilityRequirement {
            position,
            loan: incoming.clone(),
            demand: DescriptorCallOrigin::Argument {
                source,
                projection: demand.projection.clone(),
            },
        })
    }

    pub(super) fn requirements(&self) -> Vec<DescriptorCompatibilityRequirement> {
        self.requirements.clone()
    }

    pub(super) fn conflicted_operation(
        &self,
        borrowed_regions: &BorrowedRegions,
        block: BlockId,
        node: u32,
        operation: &O,
        moved: &[PlaceId],
    ) -> Option<PlaceId> {
        self.conflicted_operation_replaying(
            borrowed_regions,
            block,
            node,
            operation,
            moved,
            |requirement| self.requirements.contains(&requirement),
        )
    }

    fn conflicted_operation_replaying(
        &self,
        borrowed_regions: &BorrowedRegions,
        block: BlockId,
        node: u32,
        operation: &O,
        moved: &[PlaceId],
        mut defer: impl FnMut(DescriptorCompatibilityRequirement) -> bool,
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
            // Publishing a structural result replaces that carrier's actual
            // home. Never follow its descriptor aliases here: forming a new
            // view writes its private carrier, not the referenced byte backing.
            if let Some((_, result)) = operation.structural_result() {
                let replacement = Region {
                    root: Some(result.place),
                    projection: Projection::new(&[]),
                };
                if loan
                    .regions
                    .iter()
                    .any(|backing| backing.overlaps(&replacement))
                {
                    return Some(result.place);
                }
            }
            let observations = (loan.access != StructuralAccess::SharedBorrow)
                .then(|| self.observations.get(&position))
                .flatten()
                .map(Vec::as_slice)
                .unwrap_or(&[]);
            for observation in borrowed_regions
                .writes_at(position)
                .iter()
                .chain(observations)
            {
                if !loan.authorizes(
                    position,
                    observation.location.as_ref(),
                    &observation.regions,
                ) && observation.regions.iter().any(|region| {
                    loan.regions.iter().any(|backing| {
                        borrowed_regions.regions_overlap_at(backing, region, position)
                            && !self
                                .compatibility_requirement(loan, region, position)
                                .is_some_and(&mut defer)
                    })
                }) {
                    return Some(observation.place);
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
        self.conflicted_edge_replaying(
            borrowed_regions,
            block,
            node,
            edge,
            destination,
            |requirement| self.requirements.contains(&requirement),
        )
    }

    fn conflicted_edge_replaying(
        &self,
        borrowed_regions: &BorrowedRegions,
        block: BlockId,
        node: u32,
        edge: &OptimizationEdge,
        destination: &[terminal_psi::StructuralParameterDeclaration],
        mut defer: impl FnMut(DescriptorCompatibilityRequirement) -> bool,
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
                let location =
                    borrowed_regions.access_location((block, node), place, &projection.path);
                let regions =
                    borrowed_regions.resolve_at((block, node), place, projection.at((block, node)));
                if (forced || !loan.authorizes((block, node), location, &regions))
                    && regions.iter().any(|region| {
                        loan.regions.iter().any(|backing| {
                            borrowed_regions.regions_overlap_at(backing, region, (block, node))
                                && (forced
                                    || !self
                                        .compatibility_requirement(loan, region, (block, node))
                                        .is_some_and(&mut defer))
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
    fn authorizes(
        &self,
        position: Position,
        location: Option<&DescriptorLocation>,
        regions: &[Region],
    ) -> bool {
        self.authorized.get(&position).is_some_and(|places| {
            location.is_some_and(|location| {
                places
                    .iter()
                    .any(|authorized| authorized.contains(location))
            })
        }) && !regions.is_empty()
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
) -> Vec<Access> {
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
        result.push(Access {
            place,
            location: borrowed_regions
                .access_location(position, place, &projection.path)
                .cloned(),
            regions: borrowed_regions.resolve_at(position, place, projection.at(position)),
        });
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
    let byte_field_projection = |place, path: &[StructuralPathSegment], field| {
        let mut projection = projected(place, path, Some(field));
        let owned = crate::unit_validation::operation_contracts::structural_source_contract(
            function, place, false,
        )
        .and_then(|root| {
            terminal_semantics::runtime_structural_path_tip(
                types.values().copied(),
                root.structural_type,
                path,
            )
        })
        .and_then(|declaration| match &declaration.shape {
            terminal_psi::StructuralTypeShape::Record { fields }
            | terminal_psi::StructuralTypeShape::Mixed { fields, .. } => fields
                .iter()
                .find(|row| row.id == field && !row.relevance.is_erased()),
            _ => None,
        })
        .and_then(|field| field.field_type.byte_sequence_carrier())
        .is_some_and(|carrier| {
            matches!(
                carrier,
                terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
            )
        });
        // A descriptor field reads its backing, not its containing slot. Only
        // declared inline owned bytes permit direct sibling disjointness.
        projection.indirect |= !owned;
        projection
    };
    let mut observe_byte_operand =
        |operand: &terminal_psi::control_flow::ByteSequenceOperand| match operand {
            terminal_psi::control_flow::ByteSequenceOperand::Whole { source, .. } => {
                retain(*source, Projection::new(&[]))
            }
            terminal_psi::control_flow::ByteSequenceOperand::Field { source, field, .. } => retain(
                source.place,
                byte_field_projection(source.place, &source.path, *field),
            ),
        };
    match operation {
        O::EstablishByteSequenceConcatenation { left, right, .. }
        | O::ByteSequenceEqual { left, right, .. } => {
            observe_byte_operand(left);
            observe_byte_operand(right);
        }
        O::ByteSequencePredicate { operand, .. } => observe_byte_operand(operand),
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
            retain(*source, byte_field_projection(*source, path, *field));
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
        result.push(Access {
            place,
            location: borrowed_regions.location_at(position, place, &[]).cloned(),
            regions: borrowed_regions.resolve_at(position, place, Projection::new(&[])),
        });
    }
    result
}

/// The shared graph supplies all slot/edge/call transport. This receiving
/// adapter only removes authenticated no-code disposal observations; ordinary
/// reads, nominal cleanup and residual payload cleanup remain observable.
fn descriptor_flow(
    function: &PsiOptimizationFunction,
    types: &BTreeMap<StructuralTypeId, &StructuralTypeDeclaration>,
    borrowed_regions: &BorrowedRegions,
) -> BTreeMap<Position, DescriptorFlowStep> {
    let mut steps = borrowed_regions
        .descriptor_flow()
        .cloned()
        .unwrap_or_else(|| unknown_flow(function));
    for block in &function.blocks {
        for (index, node) in block.nodes.iter().enumerate() {
            let position = (block.id, index as u32);
            let Some(step) = steps.get_mut(&position) else {
                continue;
            };
            let mut all =
                crate::unit_validation::function_structure::operation_place_inputs(&node.operation);
            for retained in descriptor_inputs(function, types, &node.operation) {
                if let Some(index) = all.iter().position(|place| *place == retained) {
                    all.remove(index);
                }
            }
            for disposed in all {
                let raw = DescriptorLocation::whole(disposed);
                let canonical = borrowed_regions.location_at(position, disposed, &[]);
                if let Some(index) = step
                    .inputs
                    .iter()
                    .rposition(|input| input == &raw || Some(input) == canonical)
                {
                    step.inputs.remove(index);
                }
            }
        }
    }
    steps
}

// Missing exact provenance cannot shorten a loan or grant descendant authority.
fn unknown_flow(function: &PsiOptimizationFunction) -> BTreeMap<Position, DescriptorFlowStep> {
    let mut steps = BTreeMap::new();
    for block in &function.blocks {
        for (index, node) in block.nodes.iter().enumerate() {
            let targets = if node.successors.is_empty() {
                (index + 1 < block.nodes.len())
                    .then_some((block.id, index as u32 + 1))
                    .into_iter()
                    .collect::<Vec<_>>()
            } else {
                node.successors
                    .iter()
                    .map(|edge| (edge.target, 0))
                    .collect()
            };
            steps.insert(
                (block.id, index as u32),
                DescriptorFlowStep {
                    inputs: function
                        .structural_places
                        .iter()
                        .map(|place| DescriptorLocation::whole(place.id))
                        .collect(),
                    may_escape: true,
                    successors: targets
                        .into_iter()
                        .map(|target| DescriptorFlowEdge {
                            target,
                            parameters: vec![],
                            bindings: vec![],
                        })
                        .collect(),
                    ..DescriptorFlowStep::default()
                },
            );
        }
    }
    steps
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

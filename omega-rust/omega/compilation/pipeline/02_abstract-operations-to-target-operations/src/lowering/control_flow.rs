//! Ordinary control graphs: definitions belong to dominating blocks, while
//! descriptor storage follows actual post-transfer, post-cleanup edges.
//! Incoming conditional slots retain the union of possible backing origins.
//! The fixed point computes storage metadata, not a second ownership judgment.
use super::unit::known_integers::KnownUnitInteger;
use crate::LoweringError;
use crate::lowering::selected_boundary_calls::SelectedBoundaryCall;
use crate::lowering::structural_type_lookup::StructuralTypeLookup;
use abstract_operations::{
    AbstractFunction, AbstractFunctionResult, AbstractOperation, AbstractStructuralCallResult,
};
use semantic_vocabulary::{
    BoundaryMachineId, MachineId, OperationId, PlaceId, StructuralTypeId, ValueId,
};
use std::collections::{BTreeMap, BTreeSet};
use target::NativeTarget;
use target_operations::{
    BoundarySettlementBinding, ScalarFunctionAbi, TargetFunction, TargetUnitScalarHomeRequirement,
    TerminalPsiProvenance,
};
use terminal_psi::{StructuralAccess, StructuralPathSegment, TerminalAffineCleanupAction};
mod aggregate_borrows;
pub(super) mod aggregate_results;
mod atomic_access;
mod borrowed_calls;
mod borrowed_windows;
mod byte_arguments;
mod byte_concatenation;
mod byte_write;
mod call_signature;
mod calls;
mod dominance;
mod element_view_write;
mod ieee_float_arithmetic;
mod leaf_copy;
mod observations;
mod operations;
mod owned_arguments;
mod placed_scalar_access;
mod primitive_storage;
mod records;
pub(super) mod references;
pub(super) mod scalar_arrays;
pub(super) mod scalar_sources;
mod selected_boundary_calls;
mod structural_case;
mod terminator;
mod transfers;
use operations::lower_operation;
use target_operations::{TargetControlBlock, TargetControlGraph, TargetScalarBlockParameter};
use terminator::lower_terminator;

/// Descriptor metadata after simultaneous edge transfer and cleanup.
#[derive(Clone)]
struct ReferenceEdgeStorage {
    target: semantic_vocabulary::BlockId,
    references: BTreeMap<(PlaceId, Vec<StructuralPathSegment>), references::ReferenceStorage>,
    retired_reference_roots: BTreeSet<PlaceId>,
}

#[derive(Clone)]
struct LiveDefinitions {
    borrowed_reference_sources: references::BorrowedSources,
    stored_reference_effects: references::StoredEffectCatalog,
    reference_position: terminal_semantics::descriptor_lifetimes::DescriptorPosition,
    /// Invocation obligations, never captured backing or permission leaves.
    placed_formals: BTreeMap<PlaceId, terminal_psi::TerminalPlacedViewInput>,
    // Dominating definitions only. Abstract-unit validation owns edge liveness.
    structural_homes: BTreeMap<PlaceId, target_operations::TargetStructuralHomeRequirement>,
    integers: BTreeMap<ValueId, KnownUnitInteger>,
    booleans: BTreeMap<ValueId, (OperationId, bool)>,
    scalar_homes: BTreeMap<ValueId, TargetUnitScalarHomeRequirement>,
    ieee_float_constants: BTreeMap<ValueId, (OperationId, semantic_vocabulary::IeeeFloatValue)>,
    scalar_block_parameters: BTreeMap<ValueId, target_operations::TargetScalarBlockValue>,
    views: BTreeMap<PlaceId, (OperationId, StructuralTypeId)>,
    block_views: BTreeSet<PlaceId>,
    /// Shared block parameters carried as their referent's address (see
    /// `transfers::is_address_join`). They own no storage and are never
    /// descriptor views, so byte and element operations cannot name them.
    address_joins: BTreeSet<PlaceId>,
    owned_arrivals: BTreeSet<PlaceId>,
    lengths: BTreeMap<ValueId, PlaceId>,
    /// Empty-record affine locals and the establishment that minted each.
    /// They occupy no bytes, so they have no structural home; a whole owned
    /// call argument names the establishment as its producer.
    trivial_affine_locals: BTreeMap<PlaceId, (OperationId, StructuralTypeId)>,
    /// Descriptor metadata keyed by current carrier location, never permission
    /// or referent storage. Case-qualified keys describe possible residents.
    references: BTreeMap<(PlaceId, Vec<StructuralPathSegment>), references::ReferenceStorage>,
    /// Carrier roots whose descriptor metadata was already retired on this path.
    retired_reference_roots: BTreeSet<PlaceId>,
    reference_edge_exits: Vec<ReferenceEdgeStorage>,
}

impl LiveDefinitions {
    /// Every element view resolves its collection backing over the same live
    /// home map, join set, and scalar sources.
    fn element_view_backings(&self) -> super::scalar::element_views::ElementViewBackings<'_> {
        super::scalar::element_views::ElementViewBackings::new(
            &self.structural_homes,
            &self.views,
            &self.address_joins,
            scalar_sources::ScalarSources::from(self),
        )
    }

    /// Rejoin a live view's exact producer result: the one structural result
    /// published at `place`, from the producer and with the type the view
    /// row recorded. A shared view keeps no home, yet its producer published
    /// the same operation-result contract a home retains.
    fn view_result<'function>(
        &self,
        function: &'function AbstractFunction,
        place: PlaceId,
    ) -> Option<&'function terminal_psi::StructuralOperationResult> {
        self.element_view_backings().view_result(function, place)
    }

    fn reference_edge_exit(&self, target: semantic_vocabulary::BlockId) -> ReferenceEdgeStorage {
        ReferenceEdgeStorage {
            target,
            references: self.references.clone(),
            retired_reference_roots: self.retired_reference_roots.clone(),
        }
    }

    /// Keep each outgoing edge intact before joining its possible residents.
    fn finish_reference_edges(&mut self, exits: Vec<ReferenceEdgeStorage>) {
        self.reference_edge_exits = exits;
    }

    /// Select post-transfer, post-cleanup storage from actual edges. A type
    /// declaration cannot supply the descriptor's runtime-selected origin.
    fn reference_arrival(
        &self,
        target: semantic_vocabulary::BlockId,
        function: &AbstractFunction,
        storage: &BTreeSet<PlaceId>,
    ) -> Result<ReferenceEdgeStorage, LoweringError> {
        let scoped = |mut exit: ReferenceEdgeStorage| {
            exit.references
                .retain(|(carrier, _), _| storage.contains(carrier));
            exit
        };
        if self.reference_edge_exits.is_empty() {
            return Ok(scoped(self.reference_edge_exit(target)));
        }
        let mut exits = self
            .reference_edge_exits
            .iter()
            .filter(|exit| exit.target == target);
        let mut arrival = scoped(
            exits
                .next()
                .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?
                .clone(),
        );
        for other in exits {
            let other = scoped(other.clone());
            references::merge_arrivals(function, &mut arrival.references, &other.references)?;
            arrival
                .retired_reference_roots
                .retain(|root| other.retired_reference_roots.contains(root));
        }
        Ok(arrival)
    }

    /// Resolve the declared type of one currently readable structural root.
    ///
    /// `AbstractFunction::arrival_parameter` authenticates entry and block
    /// coordinates and fails closed for duplicate place identities. A block
    /// declaration is observable only while its descriptor or referent address
    /// is live; the declaration alone never revives storage outside its block.
    fn structural_observation_type(
        &self,
        function: &AbstractFunction,
        place: PlaceId,
    ) -> Option<StructuralTypeId> {
        if let Some(home) = self.structural_homes.get(&place) {
            return (home.access() != StructuralAccess::WriteOnlyBorrow)
                .then(|| home.structural_type());
        }
        if let Some((_, structural_type)) = self.views.get(&place) {
            return Some(*structural_type);
        }
        let parameter = function
            .arrival_parameter(place)
            .filter(|parameter| parameter.access != StructuralAccess::WriteOnlyBorrow)?;
        let entry = function
            .structural_parameters
            .iter()
            .any(|entry| entry.place == place);
        (entry || self.address_joins.contains(&place) || self.block_views.contains(&place))
            .then_some(parameter.structural_type)
    }
}

pub(super) fn lower(
    function: &AbstractFunction,
    target: NativeTarget,
    functions: &BTreeMap<MachineId, &AbstractFunction>,
    dynamic_dispatch: &terminal_psi::TerminalDynamicDispatchCatalog,
    borrowed_reference_sources: references::BorrowedSources,
    stored_reference_effects: references::StoredEffectCatalog,
    structural_types: &StructuralTypeLookup<'_>,
    boundary_machines: &BTreeMap<BoundaryMachineId, &terminal_psi::BoundaryMachineDeclaration>,
    settlements: &BTreeMap<BoundaryMachineId, BoundarySettlementBinding>,
    selected_calls: &BTreeMap<(MachineId, OperationId, BoundaryMachineId), SelectedBoundaryCall>,
    scalar_abis: &BTreeMap<MachineId, ScalarFunctionAbi>,
    native_callbacks: &BTreeMap<OperationId, Vec<target_operations::TargetNativeCallbackArgument>>,
) -> Result<TargetFunction, LoweringError> {
    if function.block_entries.is_empty() {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    // Invocation borrows keep their original places throughout the graph.
    // Shared signature preparation owns referent layout and ABI placement;
    // only transferred block descriptors need the narrower edge classifier.
    // Whole-root domain qualifications are signature preconditions the caller or
    // root installation discharges at invocation; admission binds the roster and
    // checks it again wherever a call edge or structural return transfers it.
    // Projected memberships use the same source-checked contract and retain
    // their exact paths; neither roster changes the parameter's storage ABI.
    if !matches!(
        function.result,
        AbstractFunctionResult::Unit
            | AbstractFunctionResult::Scalar(_)
            | AbstractFunctionResult::Structural(_)
    ) || !function.structural_parameters.iter().all(|parameter| {
        (parameter.access != StructuralAccess::Owned
            && matches!(
                parameter.multiplicity,
                terminal_psi::nonlinear_multiplicity_pair_pattern!()
            ))
            || owned_arguments::is_owned_parameter(parameter, structural_types)
    }) {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    let prepared =
        super::function_signature::prepare_function_signature(function, target, structural_types)?;
    let parameters_by_place = super::function_signature::parameters_by_place(&prepared.parameters);
    let mut definitions = BTreeSet::new();
    for parameter in &function.parameters {
        if !definitions.insert(parameter.value) {
            return Err(LoweringError::DuplicateValue(parameter.value));
        }
    }
    transfers::validate_parameters(function, &mut definitions)?;
    for operation in &function.operations {
        let result = match operation {
            abstract_operations::constant_value_triple_pattern!(result, ..)
            | AbstractOperation::IeeeFloatCompare { result, .. }
            | AbstractOperation::FloatResultPolicy { result, .. }
            | abstract_operations::integer_widen_or_exact_cast_pattern!(result, ..)
            | AbstractOperation::ScalarRepresentationView { result, .. }
            | abstract_operations::integer_equal_less_pair_pattern!(result, ..)
            | AbstractOperation::IntegerLessOrEqual { result, .. }
            | abstract_operations::boolean_not_or_equal_pair_pattern!(result, ..) => Some(*result),
            AbstractOperation::IntegerArithmetic { result, .. }
            | AbstractOperation::TrappingInteger { result, .. }
            | AbstractOperation::IntegerBitwiseAnd { result, .. }
            | abstract_operations::bitwise_or_xor_pair_pattern!(result, ..)
            | AbstractOperation::IntegerBitwiseNot { result, .. }
            | AbstractOperation::IntegerShift { result, .. }
            | AbstractOperation::Call { result, .. } => Some(*result),
            AbstractOperation::ByteSequenceLength { result, .. }
            | AbstractOperation::ByteSequenceRead { result, .. }
            | AbstractOperation::PrimitiveScalarRead { result, .. }
            | AbstractOperation::StructuralScalarFieldRead { result, .. }
            | abstract_operations::byte_len_or_field_read_pattern!(result, ..)
            | AbstractOperation::StructuralCaseMembership { result, .. }
            | AbstractOperation::SelectImplementation { result, .. }
            | AbstractOperation::CallStructural {
                result: AbstractStructuralCallResult::Scalar(result),
                ..
            } => Some(result.value),
            _ => None,
        };
        if let Some(result) = result
            && !definitions.insert(result)
        {
            return Err(LoweringError::DuplicateValue(result));
        }
    }
    let mut places = function
        .structural_parameters
        .iter()
        .map(|parameter| parameter.place)
        .collect::<BTreeSet<_>>();
    if places.len() != function.structural_parameters.len() {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    let entry_references = references::entry(function, structural_types)?;
    for entry in &function.block_entries {
        for (position, parameter) in entry.structural_parameters.iter().enumerate() {
            if entry.block == function.entry
                || parameter.position as usize != position
                || (!super::scalar::byte_views::is_byte_parameter(parameter, structural_types)
                    && !super::scalar::byte_views::is_immutable_byte_carrier_parameter(
                        parameter,
                        structural_types,
                    )
                    && !super::scalar::element_views::is_element_view_parameter(
                        parameter,
                        structural_types,
                    )
                    && !transfers::is_address_join(parameter, structural_types)
                    && !super::scalar::byte_views::is_owned_byte_parameter(
                        parameter,
                        structural_types,
                    )
                    && !abstract_operations::control_flow::owner_loans::is_shared_view_carrier_loan(
                        parameter,
                        |identity| structural_types.get(&identity).copied(),
                        &structural_types
                            .values()
                            .map(|row| (*row).clone())
                            .collect::<Vec<_>>(),
                    )
                    && !owned_arguments::is_owned_parameter(parameter, structural_types))
                || !places.insert(parameter.place)
            {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
        }
    }
    for operation in &function.operations {
        let established = match operation {
            AbstractOperation::EstablishPrimitiveLocal { result, .. }
            | AbstractOperation::EstablishScalarArray { result, .. }
            | AbstractOperation::EstablishStructuralArray { result, .. }
            | AbstractOperation::EstablishByteSequenceConcatenation { result, .. }
            | AbstractOperation::EstablishRecord { result, .. }
            | AbstractOperation::EstablishZeroedStructural { result, .. }
            | AbstractOperation::EstablishCase { result, .. }
            | AbstractOperation::EstablishReference { result, .. }
            | AbstractOperation::EstablishPlacedReference { result, .. }
            | AbstractOperation::CallStructural {
                result: AbstractStructuralCallResult::Structural { result, .. },
                ..
            }
            | abstract_operations::element_view_or_byte_subslice_pattern!(result, ..)
            | AbstractOperation::BoundaryCall {
                result: abstract_operations::AbstractBoundaryResult::Structural(result),
                ..
            } => Some(result.place),
            AbstractOperation::EstablishElementView { destination, .. } => Some(*destination),
            AbstractOperation::EstablishByteSequenceLiteral { place, .. } => Some(place.id),
            _ => None,
        };
        if established.is_some_and(|place| !places.insert(place)) {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
    }
    let entries = &function.block_entries;
    if entries.is_empty()
        || entries[0].operation_offset != 0
        || entries.iter().any(|entry| {
            entry.block == function.entry
                && !entry.parameters.is_empty()
                && entry.parameters != function.parameters
        })
    {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    let mut ranges = Vec::new();
    let mut incoming = vec![Vec::new(); entries.len()];
    let mut outgoing = vec![Vec::new(); entries.len()];
    let mut entry_position = None;
    for (position, entry) in entries.iter().enumerate() {
        if entries[..position]
            .iter()
            .any(|earlier| earlier.block == entry.block)
        {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        if entry.block == function.entry {
            entry_position = Some(position);
        }
        let end = entries
            .get(position + 1)
            .map_or(function.operations.len(), |next| next.operation_offset);
        if entry.operation_offset >= end || end > function.operations.len() {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        ranges.push(entry.operation_offset..end);
        let targets = match &function.operations[end - 1] {
            AbstractOperation::ReturnStructural { .. }
            | AbstractOperation::ExternalCompletion { .. }
            | AbstractOperation::Crash { .. } => Vec::new(),
            abstract_operations::return_or_return_unit_pattern!(cleanup_actions, ..)
                if cleanup_actions.iter().all(|action| {
                    matches!(
                        action,
                        TerminalAffineCleanupAction::DiscardRoot(_)
                            | TerminalAffineCleanupAction::RetireHome(_)
                            | TerminalAffineCleanupAction::DiscardResidual(_)
                            | TerminalAffineCleanupAction::InvokeNominal(_)
                    )
                }) =>
            {
                Vec::new()
            }
            AbstractOperation::Jump { target, .. } => {
                vec![*target]
            }
            AbstractOperation::Conditional {
                when_true,
                when_false,
                ..
            } => {
                vec![when_true.target, when_false.target]
            }
            AbstractOperation::StructuralCase { cases, .. } => {
                cases.iter().map(|case| case.target).collect()
            }
            _ => return Err(LoweringError::unsupported_control_flow(function.machine)),
        };
        for target_block in targets {
            let target_position = entries
                .iter()
                .position(|candidate| candidate.block == target_block)
                .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
            if !outgoing[position].contains(&target_position) {
                outgoing[position].push(target_position);
                incoming[target_position].push(position);
            }
        }
    }
    let entry_position =
        entry_position.ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
    if !incoming[entry_position].is_empty() {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    let schedule = dominance::schedule(&incoming, &outgoing, entry_position)
        .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
    // Descriptor metadata follows SSA storage availability. Branch-local
    // observations are not incoming storage at a later join, even when their
    // unrestricted descriptors need no affine cleanup. Keep actual edge state
    // for available carriers; never intersect away a missing surviving loan.
    let mut reference_storage = vec![BTreeSet::new(); entries.len()];
    for &(position, dominator) in &schedule {
        let mut storage = match dominator {
            Some(dominator) => {
                let mut storage = reference_storage[dominator].clone();
                storage.extend(
                    function.operations[ranges[dominator].clone()]
                        .iter()
                        .filter_map(|operation| {
                            operation
                                .structural_result()
                                .map(|(_, result)| result.place)
                        }),
                );
                storage
            }
            None => function
                .structural_parameters
                .iter()
                .map(|parameter| parameter.place)
                .collect(),
        };
        storage.extend(
            entries[position]
                .structural_parameters
                .iter()
                .map(|parameter| parameter.place),
        );
        reference_storage[position] = storage;
    }
    let mut live_exits: Vec<Option<LiveDefinitions>> = vec![None; entries.len()];
    let mut reference_entries = vec![None; entries.len()];
    let mut drained_entries = vec![None; entries.len()];
    let mut lowered = vec![None; entries.len()];
    let mut block_provenance = vec![TerminalPsiProvenance::default(); entries.len()];
    let initial = LiveDefinitions {
        borrowed_reference_sources,
        stored_reference_effects,
        reference_position: (entries[entry_position].block, 0),
        placed_formals: references::placed_formals(function, structural_types)?,
        structural_homes: BTreeMap::new(),
        integers: super::function_signature::integer_parameters(
            function.machine,
            &prepared.scalar_parameters,
        )?,
        booleans: BTreeMap::new(),
        scalar_homes: BTreeMap::new(),
        ieee_float_constants: BTreeMap::new(),
        scalar_block_parameters: BTreeMap::new(),
        views: BTreeMap::new(),
        block_views: BTreeSet::new(),
        address_joins: BTreeSet::new(),
        owned_arrivals: function
            .structural_parameters
            .iter()
            .filter(|parameter| parameter.access == StructuralAccess::Owned)
            .map(|parameter| parameter.place)
            .collect(),
        lengths: BTreeMap::new(),
        trivial_affine_locals: BTreeMap::new(),
        references: entry_references.clone(),
        retired_reference_roots: BTreeSet::new(),
        reference_edge_exits: Vec::new(),
    };
    loop {
        let mut changed = false;
        let mut deferred = false;
        'blocks: for &(position, dominator) in &schedule {
            let mut live = match dominator {
                None => initial.clone(),
                Some(dominator) => match live_exits[dominator].as_ref() {
                    Some(exit) => exit.clone(),
                    None => {
                        deferred = true;
                        continue;
                    }
                },
            };
            if position != entry_position {
                let mut predecessors = incoming[position]
                    .iter()
                    .filter_map(|predecessor| live_exits[*predecessor].as_ref());
                let Some(predecessor) = predecessors.next() else {
                    deferred = true;
                    continue;
                };
                let mut arrival = match predecessor.reference_arrival(
                    entries[position].block,
                    function,
                    &reference_storage[position],
                ) {
                    Ok(arrival) => arrival,
                    Err(_) => {
                        deferred = true;
                        continue;
                    }
                };
                for predecessor in predecessors {
                    let other = match predecessor.reference_arrival(
                        entries[position].block,
                        function,
                        &reference_storage[position],
                    ) {
                        Ok(arrival) => arrival,
                        Err(_) => {
                            deferred = true;
                            continue 'blocks;
                        }
                    };
                    if references::merge_arrivals(
                        function,
                        &mut arrival.references,
                        &other.references,
                    )
                    .is_err()
                    {
                        deferred = true;
                        continue 'blocks;
                    }
                    arrival
                        .retired_reference_roots
                        .retain(|root| other.retired_reference_roots.contains(root));
                }
                // Homes and scalar definitions come from the dominator.
                // Descriptor metadata comes from the actual incoming edges,
                // not an overlay that could revive moved storage.
                live.references = arrival.references;
                live.retired_reference_roots = arrival.retired_reference_roots;
            }
            live.reference_edge_exits.clear();
            if reference_entries[position].as_ref() == Some(&live.references)
                && drained_entries[position].as_ref() == Some(&live.retired_reference_roots)
            {
                continue;
            }
            changed = true;
            reference_entries[position] = Some(live.references.clone());
            drained_entries[position] = Some(live.retired_reference_roots.clone());
            if position != entry_position {
                transfers::enter(&entries[position], &mut live);
                // Outgoing reference carriers were captured on the selected edge
                // before its cleanup; rebasing again here would corrupt swaps.
                for parameter in
                    entries[position]
                        .structural_parameters
                        .iter()
                        .filter(|parameter| {
                            matches!(
                                parameter.access,
                                StructuralAccess::SharedBorrow
                                    | StructuralAccess::MutableBorrow
                                    | StructuralAccess::WriteOnlyBorrow
                            )
                        })
                {
                    if transfers::is_address_join(parameter, structural_types) {
                        live.address_joins.insert(parameter.place);
                    } else {
                        live.block_views.insert(parameter.place);
                    }
                }
                // Each owned arrival has storage determined by its declaration.
                // Neighboring instructions cannot erase its transfer or cleanup
                // custody; optional dead-storage removal belongs to optimization.
                for parameter in entries[position]
                    .structural_parameters
                    .iter()
                    .filter(|parameter| parameter.access == StructuralAccess::Owned)
                {
                    let home = aggregate_results::block_home(
                        entries[position].block,
                        parameter,
                        structural_types,
                    )?;
                    if live
                        .structural_homes
                        .insert(parameter.place, home)
                        .is_some()
                    {
                        return Err(LoweringError::unsupported_control_flow(function.machine));
                    }
                }
                live.owned_arrivals.extend(
                    entries[position]
                        .structural_parameters
                        .iter()
                        .filter(|parameter| parameter.access == StructuralAccess::Owned)
                        .map(|parameter| parameter.place),
                );
            }
            let range = ranges[position].clone();
            let mut operations = Vec::new();
            block_provenance[position] = TerminalPsiProvenance::default();
            let provenance = &mut block_provenance[position];
            for (operation_index, operation) in function.operations[range.start..range.end - 1]
                .iter()
                .enumerate()
            {
                live.reference_position = (
                    entries[position].block,
                    u32::try_from(operation_index)
                        .map_err(|_| LoweringError::unsupported_control_flow(function.machine))?,
                );
                lower_operation(
                    operation,
                    function,
                    target,
                    functions,
                    dynamic_dispatch,
                    structural_types,
                    boundary_machines,
                    settlements,
                    selected_calls,
                    scalar_abis,
                    native_callbacks,
                    &prepared,
                    &parameters_by_place,
                    &mut live,
                    &mut operations,
                    provenance,
                )?;
            }
            live.reference_position = (
                entries[position].block,
                u32::try_from(range.end - range.start - 1)
                    .map_err(|_| LoweringError::unsupported_control_flow(function.machine))?,
            );
            transfers::validate_successors(
                &function.operations[range.end - 1],
                function,
                &live,
                structural_types,
            )?;
            calls::lower_cleanup_calls(
                &function.operations[range.end - 1],
                function,
                target,
                functions,
                structural_types,
                &prepared,
                &live,
                &mut operations,
            )?;
            let terminator = lower_terminator(
                &function.operations[range.end - 1],
                function,
                &prepared,
                &mut live,
                structural_types,
                target,
                boundary_machines,
                settlements,
                provenance,
            )?;
            lowered[position] = Some(TargetControlBlock {
                block: entries[position].block,
                structural_parameters: entries[position].structural_parameters.clone(),
                parameters: entries[position]
                    .parameters
                    .iter()
                    .map(|parameter| TargetScalarBlockParameter {
                        value: parameter.value,
                        scalar_type: parameter.scalar_type,
                    })
                    .collect(),
                operations,
                terminator,
            });
            live_exits[position] = Some(live);
        }
        if !changed {
            if deferred {
                return Err(LoweringError::unsupported_control_flow(function.machine));
            }
            break;
        }
    }
    // Initial arrivals seed cyclic storage propagation. Publication uses every
    // latest edge, including later arms and backedges.
    for (position, predecessors) in incoming.iter().enumerate() {
        let mut arrivals = predecessors.iter().map(|predecessor| {
            live_exits[*predecessor]
                .as_ref()
                .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?
                .reference_arrival(
                    entries[position].block,
                    function,
                    &reference_storage[position],
                )
        });
        let Some(first) = arrivals.next() else {
            continue;
        };
        let mut arrival = first?;
        for other in arrivals {
            references::merge_arrivals(function, &mut arrival.references, &other?.references)?;
        }
        if reference_entries[position].as_ref() != Some(&arrival.references) {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
    }
    let blocks = lowered
        .into_iter()
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
    let mut provenance = TerminalPsiProvenance::default();
    for block in block_provenance {
        provenance.operations.extend(block.operations);
        provenance.edges.extend(block.edges);
    }
    Ok(TargetFunction {
        machine: function.machine,
        attachment: function.attachment,
        nominal_cleanup_subject: function.nominal_cleanup_subject,
        scalar_abi: None,
        mixed_structural_scalar_abi: None,
        provenance,
        graph: TargetControlGraph {
            structural_types: structural_types.catalog().clone(),
            call_plan: prepared.call_plan,
            scalar_parameters: prepared.scalar_parameters,
            parameters: prepared.parameters,
            entry: function.entry,
            blocks,
        },
    })
}

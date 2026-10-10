//! Ordinary control graphs: definitions belong to dominating blocks, but
//! reference custody follows actual post-transfer, post-cleanup edges. Every
//! selected owned parameter retains its incoming formations, backing roots
//! and immediate parents. Other live leaves agree exactly. Replay reaches a
//! fixed point over current predecessor exits before publishing the graph.
use super::unit::known_integers::KnownUnitInteger;
use crate::LoweringError;
use crate::lowering::structural_type_lookup::StructuralTypeLookup;
use abstract_operations::{
    AbstractFunction, AbstractFunctionResult, AbstractOperation, AbstractStructuralCallResult,
};
use installation_evidence::InstalledProviderCallEvidence;
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
mod installed_calls;
mod leaf_copy;
mod observations;
mod operations;
mod owned_arguments;
mod primitive_storage;
mod records;
pub(super) mod references;
pub(super) mod scalar_arrays;
pub(super) mod scalar_sources;
mod structural_case;
mod terminator;
mod transfers;
use operations::lower_operation;
use target_operations::{TargetControlBlock, TargetControlGraph, TargetScalarBlockParameter};
use terminator::lower_terminator;

/// Exact custody after simultaneous edge transfer and cleanup. Descriptor
/// formation, backing and parents survive relocation. Selected occurrences
/// use their actual receiving parameter identity and contract.
#[derive(Clone)]
struct ReferenceEdgeCustody {
    target: semantic_vocabulary::BlockId,
    references: BTreeMap<(PlaceId, Vec<StructuralPathSegment>), references::ReferenceCustody>,
    drained_loan_roots: BTreeSet<PlaceId>,
}

#[derive(Clone)]
struct LiveDefinitions {
    borrowed_reference_sources: references::BorrowedSources,
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
    /// Live reference loans keyed by current carrier location. The map is the
    /// lowering-time mirror of the verified ownership replay: carriers own
    /// loan permission, never referent storage.
    references: BTreeMap<(PlaceId, Vec<StructuralPathSegment>), references::ReferenceCustody>,
    /// Roots whose complete loan roster already ended on this path.
    drained_loan_roots: BTreeSet<PlaceId>,
    reference_edge_exits: Vec<ReferenceEdgeCustody>,
}

impl LiveDefinitions {
    /// Every element view resolves its collection backing over the same live
    /// home map, join set, and scalar sources.
    fn element_view_backings(&self) -> super::scalar::element_views::ElementViewBackings<'_> {
        super::scalar::element_views::ElementViewBackings::new(
            &self.structural_homes,
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
        let (producer, structural_type) = self.views.get(&place)?;
        let mut results = function.operations.iter().filter_map(|operation| {
            operation
                .structural_result()
                .filter(|(_, result)| result.place == place)
        });
        let (operation, result) = results.next()?;
        (results.next().is_none()
            && operation == *producer
            && result.structural_type == *structural_type)
            .then_some(result)
    }

    fn reference_edge_exit(&self, target: semantic_vocabulary::BlockId) -> ReferenceEdgeCustody {
        ReferenceEdgeCustody {
            target,
            references: self.references.clone(),
            drained_loan_roots: self.drained_loan_roots.clone(),
        }
    }

    /// Keep each outgoing edge intact. An intersection would lose both
    /// transported permissions and the obligations of conditionally live loans.
    fn finish_reference_edges(&mut self, exits: Vec<ReferenceEdgeCustody>) {
        self.reference_edge_exits = exits;
    }

    /// Select post-transfer, post-cleanup custody from an actual edge. The
    /// checked ownership contract requires exact agreement at every arrival;
    /// a type declaration cannot restore a missing loan or choose its origin.
    fn reference_arrival(
        &self,
        target: semantic_vocabulary::BlockId,
        function: &AbstractFunction,
        storage: &BTreeSet<PlaceId>,
    ) -> Result<ReferenceEdgeCustody, LoweringError> {
        let scoped = |mut exit: ReferenceEdgeCustody| {
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
            references::merge_arrivals(
                function,
                target,
                &mut arrival.references,
                &other.references,
            )?;
            arrival
                .drained_loan_roots
                .retain(|root| other.drained_loan_roots.contains(root));
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
    structural_types: &StructuralTypeLookup<'_>,
    boundary_machines: &BTreeMap<BoundaryMachineId, &terminal_psi::BoundaryMachineDeclaration>,
    settlements: &BTreeMap<BoundaryMachineId, BoundarySettlementBinding>,
    installed_calls: &BTreeMap<
        (MachineId, OperationId, BoundaryMachineId),
        InstalledProviderCallEvidence,
    >,
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
        drained_loan_roots: BTreeSet::new(),
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
                        entries[position].block,
                        &mut arrival.references,
                        &other.references,
                    )
                    .is_err()
                    {
                        deferred = true;
                        continue 'blocks;
                    }
                    arrival
                        .drained_loan_roots
                        .retain(|root| other.drained_loan_roots.contains(root));
                }
                // Ordinary homes and scalar definitions still come from the
                // dominator. Loan membership must come from an incoming edge:
                // overlaying it would revive a child moved out by both arms.
                live.references = arrival.references;
                live.drained_loan_roots = arrival.drained_loan_roots;
            }
            live.reference_edge_exits.clear();
            if reference_entries[position].as_ref() == Some(&live.references)
                && drained_entries[position].as_ref() == Some(&live.drained_loan_roots)
            {
                continue;
            }
            changed = true;
            reference_entries[position] = Some(live.references.clone());
            drained_entries[position] = Some(live.drained_loan_roots.clone());
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
            for operation in &function.operations[range.start..range.end - 1] {
                lower_operation(
                    operation,
                    function,
                    target,
                    functions,
                    dynamic_dispatch,
                    structural_types,
                    boundary_machines,
                    settlements,
                    installed_calls,
                    scalar_abis,
                    native_callbacks,
                    &prepared,
                    &parameters_by_place,
                    &mut live,
                    &mut operations,
                    provenance,
                )?;
            }
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
    // Initial arrivals seed cyclic replay. Publication uses every latest edge,
    // including later arms and backedges: only selected owned leaves combine
    // provenance, while unrelated permission must agree exactly.
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
            references::merge_arrivals(
                function,
                entries[position].block,
                &mut arrival.references,
                &other?.references,
            )?;
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

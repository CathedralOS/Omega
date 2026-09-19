//! Structural ownership frontiers: the live claims, owned places and
//! partial custody verified at every block boundary of every machine.
//!
//! `validate_structural_frontier` walks the blocks in traversal order:
//! `block_entry` gives the frontier a block starts from, `operations`
//! applies each operation to it, and `terminators` closes the block and
//! hands the frontier to its successors; every arrival at an already
//! visited block must match its established entry snapshot.

use super::affine_cleanup::{
    bounded_nominal_cleanup_receiver_shape, valid_nominal_cleanup_requirements,
};
use super::{
    BTreeMap, BTreeSet, BlockId, CanonicalStructuralPathSegment, ClaimId, EdgeId, MachineId,
    ModuleError, OperationId, OperationKind, PlaceId, StructuralAccess, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralPathSegment, StructuralPlaceKind, StructuralTypeId,
    StructuralTypeShape, TerminalAffineCleanupAction, TerminalMachine, TerminalMachineResult,
    TerminalModule, Terminator, partial_affine_residuals, partial_affine_root_type,
};

mod block_entry;
mod block_parameters;
mod operations;
mod terminators;
mod traversal;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedLiveClaim {
    pub claim: ClaimId,
    pub input: Option<PlaceId>,
    pub path: Vec<StructuralPathSegment>,
    pub multiplicity: Option<StructuralMultiplicity>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VerifiedOwnedStructuralPlace {
    pub place: PlaceId,
    pub multiplicity: StructuralMultiplicity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedPartialStructuralCustody {
    pub place: PlaceId,
    pub moved_paths: Vec<Vec<StructuralPathSegment>>,
}

/// One open borrowed-storage restoration window: `path` is the exact
/// canonical hole beneath the borrowed `place` that a repair store must
/// reseat with a subtree of `structural_type` before any non-crash exit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedRestorationDebt {
    pub place: PlaceId,
    pub path: Vec<CanonicalStructuralPathSegment>,
    pub structural_type: StructuralTypeId,
}

/// Exact verifier-owned ownership state at one deterministic control site.
/// Plain unrestricted arrays, primitive locals, and borrowed views have no
/// by-value disposal debt here; their availability is validated separately.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedStructuralOwnershipFrontier {
    claims: Vec<VerifiedLiveClaim>,
    owned_places: Vec<VerifiedOwnedStructuralPlace>,
    partial_custody: Vec<VerifiedPartialStructuralCustody>,
    references: Vec<super::references::LiveReference>,
    restoration_debt: Vec<VerifiedRestorationDebt>,
}

impl VerifiedStructuralOwnershipFrontier {
    pub fn claims(&self) -> &[VerifiedLiveClaim] {
        &self.claims
    }

    pub fn owned_places(&self) -> &[VerifiedOwnedStructuralPlace] {
        &self.owned_places
    }

    pub fn partial_custody(&self) -> &[VerifiedPartialStructuralCustody] {
        &self.partial_custody
    }

    pub fn restoration_debt(&self) -> &[VerifiedRestorationDebt] {
        &self.restoration_debt
    }
}

/// Path-sensitive frontier snapshots for one verified Terminal-Psi machine.
/// Entries and exits are separately retained so a rewrite cannot treat a
/// transfer as a timeless membership fact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedMachineStructuralFrontiers {
    pub machine: MachineId,
    block_entries: BTreeMap<BlockId, VerifiedStructuralOwnershipFrontier>,
    operation_entries: BTreeMap<OperationId, VerifiedStructuralOwnershipFrontier>,
    operation_exits: BTreeMap<OperationId, VerifiedStructuralOwnershipFrontier>,
    edge_entries: BTreeMap<EdgeId, VerifiedStructuralOwnershipFrontier>,
    edge_exits: BTreeMap<EdgeId, VerifiedStructuralOwnershipFrontier>,
}

impl VerifiedMachineStructuralFrontiers {
    pub fn block_entry(&self, block: BlockId) -> Option<&VerifiedStructuralOwnershipFrontier> {
        self.block_entries.get(&block)
    }

    pub fn operation_entry(
        &self,
        operation: OperationId,
    ) -> Option<&VerifiedStructuralOwnershipFrontier> {
        self.operation_entries.get(&operation)
    }

    pub fn operation_exit(
        &self,
        operation: OperationId,
    ) -> Option<&VerifiedStructuralOwnershipFrontier> {
        self.operation_exits.get(&operation)
    }

    pub fn edge_exit(&self, edge: EdgeId) -> Option<&VerifiedStructuralOwnershipFrontier> {
        self.edge_exits.get(&edge)
    }

    pub fn edge_entry(&self, edge: EdgeId) -> Option<&VerifiedStructuralOwnershipFrontier> {
        self.edge_entries.get(&edge)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedTerminalStructuralFrontiers {
    pub(super) machines: Vec<VerifiedMachineStructuralFrontiers>,
}

impl VerifiedTerminalStructuralFrontiers {
    pub fn machines(&self) -> &[VerifiedMachineStructuralFrontiers] {
        &self.machines
    }

    pub fn machine(&self, machine: MachineId) -> Option<&VerifiedMachineStructuralFrontiers> {
        self.machines
            .iter()
            .find(|candidate| candidate.machine == machine)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct LiveClaim {
    pub(super) input: Option<PlaceId>,
    pub(super) path: Vec<StructuralPathSegment>,
    pub(super) multiplicity: Option<StructuralMultiplicity>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct StructuralOwnershipFrontier {
    pub(super) references: Vec<super::references::LiveReference>,
    // Claims carry proof-visible custody identity. Owned places independently
    // enforce by-value affine/linear use even when no linear claim row exists.
    pub(super) claims: BTreeMap<ClaimId, LiveClaim>,
    pub(super) owned_places: BTreeMap<PlaceId, StructuralMultiplicity>,
    /// Exact projected paths already transferred from an otherwise-live owned
    /// root. This is independent of the root's multiplicity: no whole-root use
    /// is legal while a hole remains. Affine roots close through explicit
    /// residual cleanup; a fixed-array root closes only after its complete
    /// dense sibling set has moved. Linear arrays retain their existing general
    /// rule, while affine arrays admit only the exact two-element/no-residual
    /// carrier so no cleanup order is inferred.
    partial_custody_paths: BTreeMap<PlaceId, BTreeSet<Vec<StructuralPathSegment>>>,
    /// Open borrowed-storage windows: `MoveStructuralField` keys the exact
    /// canonical hole under its borrowed root and records the declared type
    /// the matching `StoreStructuralField` must reseat. Unlike
    /// `partial_custody_paths` the root is borrowed, not owned — disjoint
    /// siblings stay usable and only the exact hole closes the debt.
    pub(super) restoration_debt:
        BTreeMap<PlaceId, BTreeMap<Vec<CanonicalStructuralPathSegment>, StructuralTypeId>>,
}

impl StructuralOwnershipFrontier {
    fn snapshot(&self) -> VerifiedStructuralOwnershipFrontier {
        VerifiedStructuralOwnershipFrontier {
            references: self.references.clone(),
            claims: self
                .claims
                .iter()
                .map(|(claim, live)| VerifiedLiveClaim {
                    claim: *claim,
                    input: live.input,
                    path: live.path.clone(),
                    multiplicity: live.multiplicity,
                })
                .collect(),
            owned_places: self
                .owned_places
                .iter()
                .map(|(place, multiplicity)| VerifiedOwnedStructuralPlace {
                    place: *place,
                    multiplicity: *multiplicity,
                })
                .collect(),
            partial_custody: self
                .partial_custody_paths
                .iter()
                .map(|(place, moved_paths)| VerifiedPartialStructuralCustody {
                    place: *place,
                    moved_paths: moved_paths.iter().cloned().collect(),
                })
                .collect(),
            restoration_debt: self
                .restoration_debt
                .iter()
                .flat_map(|(place, holes)| {
                    holes
                        .iter()
                        .map(move |(path, structural_type)| VerifiedRestorationDebt {
                            place: *place,
                            path: path.clone(),
                            structural_type: *structural_type,
                        })
                })
                .collect(),
        }
    }
}

/// What the frontier walk reads at every block: the module, the machine,
/// the machine table, the machine's blocks, its dominator tree and the
/// disposal order of its block parameters.
#[derive(Clone, Copy)]
pub(super) struct FrontierWalk<'a> {
    pub(super) module: &'a TerminalModule,
    pub(super) machine: &'a TerminalMachine,
    pub(super) machines: &'a BTreeMap<MachineId, &'a TerminalMachine>,
    pub(super) blocks: &'a BTreeMap<BlockId, &'a terminal_psi::Block>,
    pub(super) dominators: &'a crate::control_graph::DominatorTree,
    pub(super) parameter_order: &'a [&'a StructuralParameterDeclaration],
    /// Referent roots pinned by each block's shared-borrow parameters: the
    /// union over every incoming edge's shared arguments, transitively
    /// closed through shared parameters that reborrow earlier views.
    pub(super) shared_loans: &'a BTreeMap<BlockId, BTreeSet<PlaceId>>,
    /// The ultimate place each borrowed block parameter may alias, with the
    /// canonical prefix beneath that place. Machine parameters and
    /// non-parameter places alias only themselves, so they need no row.
    pub(super) window_aliases: &'a crate::validation::borrowed_windows::WindowAliases,
}

pub(super) fn validate_structural_frontier(
    module: &TerminalModule,
    machine: &TerminalMachine,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    blocks: &BTreeMap<BlockId, &terminal_psi::Block>,
    dominators: &crate::control_graph::DominatorTree,
) -> Result<VerifiedMachineStructuralFrontiers, ModuleError> {
    let mut snapshots = VerifiedMachineStructuralFrontiers {
        machine: machine.id,
        block_entries: BTreeMap::new(),
        operation_entries: BTreeMap::new(),
        operation_exits: BTreeMap::new(),
        edge_entries: BTreeMap::new(),
        edge_exits: BTreeMap::new(),
    };
    let entry = block_entry::entry_frontier(module, machine)?;
    let parameter_order = block_parameters::disposal_order(machine, dominators);
    let order = traversal::block_order(machine.entry, blocks);
    // Every shared successor argument pins its referent root for the whole
    // duration of the block that binds it: a joined view observes the exact
    // place the authored borrow named, so no operation inside the target may
    // move that root, take an exclusive subloan on it, or mutate through it.
    // A shared parameter bound from another shared parameter forwards the
    // places that view observes, so the closure below is transitive.
    let mut pinned = BTreeMap::<PlaceId, BTreeSet<PlaceId>>::new();
    for block in &machine.blocks {
        let mut successors = Vec::new();
        match &block.terminator {
            Terminator::Jump {
                target,
                structural_arguments,
                ..
            } => successors.push((*target, structural_arguments.as_slice())),
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => {
                successors.push((when_true.target, when_true.structural_arguments.as_slice()));
                successors.push((
                    when_false.target,
                    when_false.structural_arguments.as_slice(),
                ));
            }
            _ => {}
        }
        for (target, arguments) in successors {
            let Some(target_block) = blocks.get(&target) else {
                continue;
            };
            for (argument, parameter) in arguments.iter().zip(&target_block.structural_parameters) {
                if parameter.access == StructuralAccess::SharedBorrow {
                    pinned
                        .entry(parameter.place)
                        .or_default()
                        .insert(argument.place);
                }
            }
        }
    }
    loop {
        let mut expanded = pinned.clone();
        for (parameter, roots) in &pinned {
            for root in roots {
                if let Some(next) = pinned.get(root) {
                    expanded
                        .entry(*parameter)
                        .or_default()
                        .extend(next.iter().copied());
                }
            }
        }
        if expanded == pinned {
            break;
        }
        pinned = expanded;
    }
    let shared_loans: BTreeMap<BlockId, BTreeSet<PlaceId>> = machine
        .blocks
        .iter()
        .map(|block| {
            (
                block.id,
                block
                    .structural_parameters
                    .iter()
                    .filter(|parameter| parameter.access == StructuralAccess::SharedBorrow)
                    .flat_map(|parameter| {
                        pinned.get(&parameter.place).into_iter().flatten().copied()
                    })
                    .collect(),
            )
        })
        .collect();
    let mut incoming = BTreeMap::<BlockId, Vec<StructuralOwnershipFrontier>>::new();
    incoming.insert(machine.entry, vec![entry]);
    let window_aliases = crate::validation::borrowed_windows::window_aliases(module, machine);
    let walk = FrontierWalk {
        module,
        machine,
        machines,
        blocks,
        dominators,
        parameter_order: &parameter_order,
        shared_loans: &shared_loans,
        window_aliases: &window_aliases,
    };
    for block_id in order {
        let frontiers = incoming
            .remove(&block_id)
            .expect("control validation established reachability");
        let mut frontier = block_entry::joined_frontier(block_id, &frontiers)?;
        let block = blocks
            .get(&block_id)
            .expect("frontier traversal contains known blocks");
        snapshots
            .block_entries
            .insert(block.id, frontier.snapshot());
        for operation in &block.operations {
            snapshots
                .operation_entries
                .insert(operation.id, frontier.snapshot());
            operations::apply_operation(&walk, block.id, operation, &mut frontier)?;
            snapshots
                .operation_exits
                .insert(operation.id, frontier.snapshot());
        }
        for edge in block.terminator.edges() {
            snapshots.edge_entries.insert(edge, frontier.snapshot());
        }
        terminators::close_block(&walk, block, frontier, &mut snapshots, &mut incoming)?;
    }
    // Each block's transfer is deterministic for its incoming frontier. A
    // first arrival seeds that frontier; every later arrival must match it
    // exactly, including edges to blocks already visited. Equality establishes
    // the fixed point without repeatedly executing transfers or inventing a
    // merged ownership state. No authority escapes before all arrivals agree.
    for (block, frontiers) in incoming {
        let established = snapshots
            .block_entries
            .get(&block)
            .expect("frontier traversal visits every reachable block");
        for frontier in frontiers {
            require_snapshot_match(block, established, &frontier.snapshot())?;
        }
    }
    Ok(snapshots)
}

/// Borrowed parameters are supplied by the caller. A shared view of an owned
/// parameter or result, or a direct scalar-field observation, requires this
/// frame to still own that value.
/// Check every read before committing any outgoing move; repeated shared
/// arguments do not alter the frontier.
fn validate_owned_reads(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
    frontier: &StructuralOwnershipFrontier,
    dominators: &crate::control_graph::DominatorTree,
) -> Result<(), ModuleError> {
    if let OperationKind::EstablishRecord { fields } = &operation.kind {
        for field in fields {
            let terminal_psi::RecordFieldValue::Structural(argument) = &field.value else {
                continue;
            };
            let source =
                super::structural_result_contracts::source_signature(machine, argument.place)
                    .ok_or(ModuleError::RecordResultMismatch(operation.id))?;
            let unrestricted_parameter = source.multiplicity
                == StructuralMultiplicity::Unrestricted
                && (super::record::completed_source(module, machine, argument.place).is_some()
                    || machine.structural_parameters.iter().any(|parameter| {
                        parameter.place == argument.place
                            && parameter.access == StructuralAccess::Owned
                    })
                    || machine
                        .blocks
                        .iter()
                        .find(|block| {
                            block
                                .operations
                                .iter()
                                .any(|candidate| candidate.id == operation.id)
                        })
                        .is_some_and(|current| {
                            machine.blocks.iter().any(|definition| {
                                definition.structural_parameters.iter().any(|parameter| {
                                    parameter.place == argument.place
                                        && parameter.access == StructuralAccess::Owned
                                }) && dominators.dominates(definition.id, current.id)
                            })
                        }));
            if (!unrestricted_parameter
                && frontier.owned_places.get(&argument.place) != Some(&source.multiplicity))
                || frontier
                    .claims
                    .values()
                    .any(|claim| claim.input == Some(argument.place))
            {
                return Err(ModuleError::OwnedStructuralPlaceNotLiveAtOperation {
                    operation: operation.id,
                    place: argument.place,
                });
            }
            if frontier.partial_custody_paths.contains_key(&argument.place) {
                return Err(
                    ModuleError::PartiallyMovedStructuralPlaceUsedWholeAtOperation {
                        operation: operation.id,
                        place: argument.place,
                    },
                );
            }
        }
    }
    if let OperationKind::StructuralCaseMembership { source, .. } = operation.kind {
        let borrowed = machine
            .structural_parameters
            .iter()
            .chain(
                machine
                    .blocks
                    .iter()
                    .flat_map(|block| &block.structural_parameters),
            )
            .any(|parameter| {
                parameter.place == source && parameter.access != StructuralAccess::Owned
            });
        if !borrowed
            && let Some(signature) =
                super::structural_result_contracts::source_signature(machine, source)
            && signature.multiplicity != StructuralMultiplicity::Unrestricted
        {
            if frontier.owned_places.get(&source) != Some(&signature.multiplicity) {
                return Err(ModuleError::OwnedStructuralPlaceNotLiveAtOperation {
                    operation: operation.id,
                    place: source,
                });
            }
            if frontier.partial_custody_paths.contains_key(&source) {
                return Err(
                    ModuleError::PartiallyMovedStructuralPlaceUsedWholeAtOperation {
                        operation: operation.id,
                        place: source,
                    },
                );
            }
        }
    }
    let arguments = match &operation.kind {
        OperationKind::CallUnit {
            structural_arguments,
            ..
        }
        | OperationKind::CallStructuralScalar {
            structural_arguments,
            ..
        }
        | OperationKind::CallStructural {
            structural_arguments,
            ..
        }
        | OperationKind::CallStructuralWithScalarArguments {
            structural_arguments,
            ..
        }
        | OperationKind::BoundaryCall {
            structural_arguments,
            ..
        } => structural_arguments.as_slice(),
        _ => &[],
    };
    let observation = match operation.kind {
        OperationKind::PrimitiveScalarRead { source, ref path } if !path.is_empty() => Some(source),
        OperationKind::WriteOnlyPrimitiveStore {
            destination,
            ref path,
            ..
        } if !path.is_empty() => Some(destination),
        // The runtime index is itself a projection: the root is used even when
        // the static path to the array is empty.
        OperationKind::WriteOnlyIndexedPrimitiveStore { destination, .. } => Some(destination),
        OperationKind::IntegerStructuralField { source, .. }
        | OperationKind::BooleanStructuralField { source, .. } => Some(source),
        OperationKind::StructuralScalarFieldStore { destination, .. } => Some(destination),
        _ => None,
    };
    let reads = arguments
        .iter()
        .filter(|argument| {
            matches!(
                argument.access,
                StructuralAccess::SharedBorrow
                    | StructuralAccess::MutableBorrow
                    | StructuralAccess::WriteOnlyBorrow
            )
        })
        .map(|argument| argument.place)
        .chain(observation);
    for place in reads.filter(|place| {
        super::byte_sequence_subslice::borrowed_result(machine, *place).is_none()
            && super::primitive_storage::local_result(machine, *place).is_none()
            // Copyable case results have no affine frontier entry. Their exact
            // producer and dominance are checked by scalar_case::validate_uses;
            // Shared observations can accompany owned copies of the same
            // unrestricted value; neither grants mutable or projected access.
            && !(super::scalar_case::plain_return_source(module, machine, *place)
                && super::structural_result_contracts::source_signature(machine, *place)
                    .is_some_and(|source| source.multiplicity == StructuralMultiplicity::Unrestricted)
                && arguments.iter().any(|argument| argument.place == *place)
                && arguments.iter().filter(|argument| argument.place == *place).all(|argument|
                    matches!(argument.access, StructuralAccess::Owned | StructuralAccess::SharedBorrow)
                        && argument.path.is_empty()))
            && !super::record::result(machine, *place).is_some_and(|result| {
                result.multiplicity == StructuralMultiplicity::Unrestricted
                    && super::record::plain_return_source(module, machine, *place)
                    && result.qualifications.is_empty()
                    && result.projected_qualifications.is_empty()
                    && result.claims.is_empty()
            })
            && !(super::scalar_array::plain_return_source(module, machine, *place)
                && super::structural_result_contracts::source_signature(machine, *place)
                    .is_some_and(|source| source.multiplicity == StructuralMultiplicity::Unrestricted))
            && (machine.structural_places.iter().any(|declaration| {
                declaration.id == *place
                    && matches!(
                        declaration.kind,
                        StructuralPlaceKind::OperationResult { .. }
                    )
            }) || machine
                .structural_parameters
                .iter()
                .chain(
                    machine
                        .blocks
                        .iter()
                        .flat_map(|block| &block.structural_parameters),
                )
                .any(|parameter| {
                    parameter.place == *place
                        && parameter.access == StructuralAccess::Owned
                        && parameter.multiplicity == StructuralMultiplicity::Affine
                }))
    }) {
        if frontier.owned_places.get(&place) != Some(&StructuralMultiplicity::Affine) {
            return Err(ModuleError::OwnedStructuralPlaceNotLiveAtOperation {
                operation: operation.id,
                place,
            });
        }
        if frontier.partial_custody_paths.contains_key(&place) {
            return Err(
                ModuleError::PartiallyMovedStructuralPlaceUsedWholeAtOperation {
                    operation: operation.id,
                    place,
                },
            );
        }
    }
    Ok(())
}

fn release_reference_discards(
    module: &TerminalModule,
    machine: &TerminalMachine,
    references: &mut Vec<super::references::LiveReference>,
    discards: &[PlaceId],
) -> Result<(), ModuleError> {
    for place in discards {
        super::references::discard_owned(module, machine, references, *place)?;
    }
    Ok(())
}

fn require_no_references(
    machine: &TerminalMachine,
    references: &[super::references::LiveReference],
) -> Result<(), ModuleError> {
    if references.is_empty() {
        Ok(())
    } else {
        Err(super::references::invalid(
            machine,
            "normal completion leaves reference custody unaccounted for",
        ))
    }
}

fn require_snapshot_match(
    block: BlockId,
    expected: &VerifiedStructuralOwnershipFrontier,
    candidate: &VerifiedStructuralOwnershipFrontier,
) -> Result<(), ModuleError> {
    if candidate.claims != expected.claims {
        return Err(ModuleError::ClaimFrontierJoinMismatch(block));
    }
    if candidate.owned_places != expected.owned_places
        || candidate.partial_custody != expected.partial_custody
        || candidate.references != expected.references
        || candidate.restoration_debt != expected.restoration_debt
    {
        return Err(ModuleError::OwnedStructuralFrontierJoinMismatch(block));
    }
    Ok(())
}

fn projected_root_is_fully_consumed(
    module: &TerminalModule,
    machine: &TerminalMachine,
    frontier: &StructuralOwnershipFrontier,
    place: PlaceId,
) -> bool {
    if frontier
        .claims
        .values()
        .any(|claim| claim.input == Some(place))
    {
        return false;
    }
    if let Some(structural_type) = partial_affine_root_type(machine, place)
        && !machine
            .structural_parameters
            .iter()
            .any(|parameter| parameter.place == place && parameter.is_self)
    {
        return frontier
            .partial_custody_paths
            .get(&place)
            .is_some_and(|moved| {
                partial_affine_residuals(module, structural_type, moved, 0)
                    .is_some_and(|residuals| residuals.is_empty())
            });
    }
    let Some(parameter) = machine
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == place)
    else {
        return false;
    };
    let Some(StructuralTypeShape::FixedArray { length, .. }) = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == parameter.structural_type)
        .map(|declaration| &declaration.shape)
    else {
        return false;
    };
    let Some(length) = usize::try_from(*length).ok() else {
        return false;
    };
    if parameter.multiplicity != StructuralMultiplicity::Linear {
        return false;
    }
    let Some(moved) = frontier.partial_custody_paths.get(&place) else {
        return false;
    };
    moved.len() == length
        && (0..length).all(|index| {
            moved.contains(&vec![StructuralPathSegment::FixedIndex(
                u64::try_from(index).expect("a usize index fits u64"),
            )])
        })
}

fn validate_scalar_cleanup_actions(
    module: &TerminalModule,
    machine: &TerminalMachine,
    parameter_order: &[&StructuralParameterDeclaration],
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    block: BlockId,
    frontier: &StructuralOwnershipFrontier,
    actions: &[TerminalAffineCleanupAction],
) -> Result<(), ModuleError> {
    let mismatch = || ModuleError::ScalarReturnAffineDiscardsMismatch {
        machine: machine.id,
        block,
    };
    let mut frontier = frontier.clone();
    let max_residuals = actions.len();
    let mut actions = actions.iter();

    // Scalar-case temporaries follow the same reverse producer order as
    // ordinary edge and Unit-return disposal, before older named roots.
    for place in expected_trivial_affine_discards(machine, parameter_order, &frontier) {
        if !super::scalar_case::plain_return_source(module, machine, place)
            && super::record::completed_source(module, machine, place).is_none()
            && super::references::carrier_type(module, machine, place).is_none()
            && !frontier
                .references
                .iter()
                .any(|reference| reference.carrier == place)
        {
            continue;
        }
        if frontier.partial_custody_paths.contains_key(&place)
            || actions.next() != Some(&TerminalAffineCleanupAction::DiscardRoot(place))
        {
            return Err(mismatch());
        }
        super::references::discard_owned(module, machine, &mut frontier.references, place)?;
        frontier.owned_places.remove(&place);
    }

    let mut locals = machine
        .structural_places
        .iter()
        .filter_map(|place| match place.kind {
            StructuralPlaceKind::TrivialAffineLocal {
                declaration_ordinal,
                ..
            } if frontier.owned_places.contains_key(&place.id) => {
                Some((declaration_ordinal, place.id))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    locals.sort_by_key(|(ordinal, _)| std::cmp::Reverse(*ordinal));
    for (_, place) in locals {
        if actions.next() != Some(&TerminalAffineCleanupAction::DiscardRoot(place)) {
            return Err(mismatch());
        }
        frontier.owned_places.remove(&place);
    }

    for parameter in parameter_order.iter().rev() {
        if !frontier.owned_places.contains_key(&parameter.place) {
            continue;
        }
        if parameter.multiplicity != StructuralMultiplicity::Affine
            || frontier
                .claims
                .values()
                .any(|claim| claim.input == Some(parameter.place))
            || machine
                .content_entry_claims
                .iter()
                .any(|claim| claim.input.root == parameter.place)
        {
            return Err(mismatch());
        }
        if let Some(moved) = frontier.partial_custody_paths.remove(&parameter.place) {
            let Some(residuals) =
                partial_affine_residuals(module, parameter.structural_type, &moved, max_residuals)
            else {
                return Err(mismatch());
            };
            if moved.is_empty() || residuals.is_empty() {
                return Err(mismatch());
            }
            for (path, structural_type) in residuals {
                let expected = TerminalAffineCleanupAction::DiscardResidual(
                    terminal_psi::StructuralAffineDiscard {
                        place: parameter.place,
                        path,
                        structural_type,
                    },
                );
                if actions.next() != Some(&expected) {
                    return Err(mismatch());
                }
            }
        } else {
            let Some(action) = actions.next() else {
                return Err(mismatch());
            };
            match action {
                TerminalAffineCleanupAction::DiscardRoot(place) if *place == parameter.place => {}
                TerminalAffineCleanupAction::InvokeNominal(cleanup)
                    if cleanup.place == parameter.place
                        && cleanup.structural_type == parameter.structural_type
                        && valid_scalar_nominal_cleanup(module, machine, machines, cleanup) => {}
                _ => return Err(mismatch()),
            }
        }
        frontier.owned_places.remove(&parameter.place);
    }

    // The frontier also records available copies. They may remain after a
    // scalar return; only affine/linear custody requires explicit discharge.
    if actions.next().is_some()
        || !frontier.references.is_empty()
        || frontier
            .owned_places
            .values()
            .any(|multiplicity| *multiplicity != StructuralMultiplicity::Unrestricted)
        || !frontier.partial_custody_paths.is_empty()
    {
        return Err(mismatch());
    }
    Ok(())
}

fn valid_scalar_nominal_cleanup(
    module: &TerminalModule,
    caller: &TerminalMachine,
    machines: &BTreeMap<MachineId, &TerminalMachine>,
    cleanup: &terminal_psi::NominalAffineCleanup,
) -> bool {
    let Some(source) = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == cleanup.structural_type)
    else {
        return false;
    };
    let Some(target) = machines.get(&cleanup.cleanup_machine).copied() else {
        return false;
    };
    cleanup.cleanup_machine != caller.id
        && bounded_nominal_cleanup_receiver_shape(&source.shape)
        && target.attachment == Some(cleanup.structural_type)
        && target.result == TerminalMachineResult::Unit
        && target.parameters.is_empty()
        && target.structural_parameters.is_empty()
        && target.entry_claims.is_empty()
        && target.content_entry_claims.is_empty()
        && target.contract.ensures.is_empty()
        && target.contract.crash_routes.is_empty()
        && cleanup.requirement_obligations.len() == target.contract.requires.len()
        && valid_nominal_cleanup_requirements(module, target, cleanup)
}

fn expected_trivial_affine_discards(
    machine: &TerminalMachine,
    parameter_order: &[&StructuralParameterDeclaration],
    frontier: &StructuralOwnershipFrontier,
) -> Vec<PlaceId> {
    let mut operation_results = machine
        .structural_places
        .iter()
        .filter_map(|place| match place.kind {
            StructuralPlaceKind::OperationResult { producer, .. }
                if frontier.owned_places.get(&place.id)
                    == Some(&StructuralMultiplicity::Affine)
                    && !frontier
                        .claims
                        .values()
                        .any(|claim| claim.input == Some(place.id)) =>
            {
                Some((producer, place.id))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    operation_results.sort_by_key(|(producer, _)| std::cmp::Reverse(*producer));
    let mut output = operation_results
        .into_iter()
        .map(|(_, place)| place)
        .collect::<Vec<_>>();
    let mut locals = machine
        .structural_places
        .iter()
        .filter_map(|place| match place.kind {
            StructuralPlaceKind::TrivialAffineLocal {
                declaration_ordinal,
                ..
            } if frontier.owned_places.contains_key(&place.id) => {
                Some((declaration_ordinal, place.id))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    locals.sort_by_key(|(ordinal, _)| std::cmp::Reverse(*ordinal));
    output.extend(locals.into_iter().map(|(_, place)| place));
    output.extend(
        parameter_order
            .iter()
            .rev()
            .filter_map(|parameter| {
                (parameter.multiplicity == StructuralMultiplicity::Affine
                    && frontier.owned_places.contains_key(&parameter.place)
                    && !frontier
                        .claims
                        .values()
                        .any(|claim| claim.input == Some(parameter.place))
                    && !machine
                        .content_entry_claims
                        .iter()
                        .any(|claim| claim.input.root == parameter.place))
                .then_some(parameter.place)
            })
            .collect::<Vec<_>>(),
    );
    output
}

fn apply_edge_trivial_affine_discards(
    module: &TerminalModule,
    machine: &TerminalMachine,
    parameter_order: &[&StructuralParameterDeclaration],
    frontier: &mut StructuralOwnershipFrontier,
    edge: EdgeId,
    discards: &[PlaceId],
) -> Result<(), ModuleError> {
    if discards
        .iter()
        .any(|place| frontier.partial_custody_paths.contains_key(place))
    {
        return Err(ModuleError::EdgeAffineDiscardsInvalid { edge });
    }
    let eligible = expected_trivial_affine_discards(machine, parameter_order, frontier);
    let mut next = 0;
    for eligible_place in eligible {
        if discards.get(next) == Some(&eligible_place) {
            next += 1;
        }
    }
    if next != discards.len() {
        return Err(ModuleError::EdgeAffineDiscardsInvalid { edge });
    }
    for place in discards {
        super::references::discard_owned(module, machine, &mut frontier.references, *place)?;
        frontier.owned_places.remove(place);
    }
    Ok(())
}

fn apply_continuation_residual_discards(
    module: &TerminalModule,
    machine: &TerminalMachine,
    block: BlockId,
    frontier: &mut StructuralOwnershipFrontier,
    discards: &[terminal_psi::StructuralAffineDiscard],
) -> Result<(), ModuleError> {
    let invalid = || ModuleError::InvalidPartialAffineCleanup {
        machine: machine.id,
        block,
    };
    // Each dying root owns one contiguous run of residual rows — the shape
    // pass has already required the same per-place grouping and order. Every
    // run must be that place's exact complement before the root leaves the
    // frontier; a second run for one root cannot re-open closed custody.
    let mut offset = 0;
    while offset < discards.len() {
        let place = discards[offset].place;
        let moved = frontier
            .partial_custody_paths
            .get(&place)
            .ok_or_else(invalid)?;
        let expected = partial_affine_root_type(machine, place)
            .and_then(|root_type| {
                partial_affine_residuals(module, root_type, moved, discards.len() - offset)
            })
            .ok_or_else(invalid)?;
        if expected.is_empty()
            || offset + expected.len() > discards.len()
            || discards[offset..offset + expected.len()]
                .iter()
                .zip(&expected)
                .any(|(discard, (path, structural_type))| {
                    discard.place != place
                        || discard.path != *path
                        || discard.structural_type != *structural_type
                })
            || frontier.owned_places.get(&place) != Some(&StructuralMultiplicity::Affine)
        {
            return Err(invalid());
        }
        frontier.partial_custody_paths.remove(&place);
        frontier.owned_places.remove(&place);
        offset += expected.len();
    }
    // A dying partial root cannot leak across an unannotated continuation.
    // Fully transferred roots have already left both frontier maps at the call.
    if frontier
        .partial_custody_paths
        .keys()
        .any(|place| partial_affine_root_type(machine, *place).is_some())
    {
        return Err(invalid());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        BlockId, ClaimId, ModuleError, PlaceId, StructuralMultiplicity, StructuralPathSegment,
        VerifiedLiveClaim, VerifiedOwnedStructuralPlace, VerifiedPartialStructuralCustody,
        VerifiedStructuralOwnershipFrontier, require_snapshot_match,
    };

    fn empty_snapshot() -> VerifiedStructuralOwnershipFrontier {
        VerifiedStructuralOwnershipFrontier {
            references: Vec::new(),
            claims: Vec::new(),
            owned_places: Vec::new(),
            partial_custody: Vec::new(),
            restoration_debt: Vec::new(),
        }
    }

    #[test]
    fn ranked_preservation_compares_every_frontier_axis_in_diagnostic_order() {
        let block = BlockId::new(1).unwrap();
        let expected = empty_snapshot();
        assert_eq!(require_snapshot_match(block, &expected, &expected), Ok(()));

        let mut claim_drift = empty_snapshot();
        claim_drift.claims.push(VerifiedLiveClaim {
            claim: ClaimId::new(1).unwrap(),
            input: None,
            path: Vec::new(),
            multiplicity: None,
        });
        assert_eq!(
            require_snapshot_match(block, &expected, &claim_drift),
            Err(ModuleError::ClaimFrontierJoinMismatch(block))
        );

        let place = PlaceId::new(1).unwrap();
        let mut owned_drift = empty_snapshot();
        owned_drift.owned_places.push(VerifiedOwnedStructuralPlace {
            place,
            multiplicity: StructuralMultiplicity::Affine,
        });
        assert_eq!(
            require_snapshot_match(block, &expected, &owned_drift),
            Err(ModuleError::OwnedStructuralFrontierJoinMismatch(block))
        );

        let mut partial_drift = empty_snapshot();
        partial_drift
            .partial_custody
            .push(VerifiedPartialStructuralCustody {
                place,
                moved_paths: vec![vec![StructuralPathSegment::FixedIndex(0)]],
            });
        assert_eq!(
            require_snapshot_match(block, &expected, &partial_drift),
            Err(ModuleError::OwnedStructuralFrontierJoinMismatch(block))
        );
    }
}

//! Exclusive state loans of a local that a later (non-entry) state owns.
//!
//! A transition is a tail jump that never resumes the departed state
//! (wiki/spec/language/state_contracts.md, "Bindings and arrivals"), and a
//! transported reference's storage must outlive the target path
//! (wiki/spec/language/lifetimes.md). After `transition { _ -> bump(&mut
//! score) }` leaves `prepare`, nothing names `score` again: the loan is the
//! storage's only remaining name, so the storage must live exactly as long as
//! the chain of states that carries the loan.
//!
//! The activation-entry route (`loaned_local_homes`) keeps a lent home in its
//! original place and discards it once at every return. That cannot serve a
//! later lender: a return reachable on a path that never visited the lender
//! would discard a place that was never established. Instead, a chain is the
//! set of exclusive state parameters connected by whole forwarding
//! (`next(score)` or `next(&mut score)` of a `&mut` parameter), and its
//! storage dies where the chain ends: on every edge leaving a member state
//! that does not carry the loan to another member, and at every return from a
//! member state. Two backings realize that:
//!
//! - **Lender home.** Exactly one state's one local lends into the chain and
//!   that state is not itself a member. Every path into the chain passes the
//!   lender's establishment, so its home stays in place, members keep their
//!   exclusive views, and the chain's ends discard the home. Omega's
//!   abstract-operation ownership replay (`current_ownership/address_joins.rs`)
//!   today disposes of a mutably joined local home only at a return, so a
//!   chain that ends on an edge verifies here but is not yet realized natively.
//! - **Transferred storage.** Several lenders (or a member that lends a fresh
//!   local while it holds the chain) meet in one parameter, whose arrivals
//!   would otherwise join different live homes. Every member parameter is
//!   then declared as an owned block parameter over the moved storage; each
//!   arrival moves its home or the previous member's parameter into it, and
//!   the chain's ends discard that parameter. Terminal verification admits
//!   this; native realization additionally needs Omega to store into an owned
//!   block parameter (`00_terminal-psi-to-abstract-operations` admits stores
//!   only through exclusive parameters and owned operation results).
//!
//! Every arrival into a chain must lend a whole local or forward a member: a
//! caller-owned or projected loan cannot be disposed of at the chain's ends,
//! so that join stays refused, as does re-lending a member shared or by
//! projection into a successor, which would need the storage to outlive the
//! state holding it. An activation-entry local lent into a chain joins it as
//! one more lender, provided no other edge retains it as an entry home.

use crate::LoweringError;
use checked_trees::{
    CheckedComposedUnitControlMachinePlan, CheckedStructuralAccess,
    CheckedStructuralControlTransferSourcePlan, CheckedUnitStructuralArgumentSourcePlan,
};
use language_semantics::Multiplicity;
use semantic_vocabulary::{BlockId, OperationId, PlaceId, StructuralPlaceKind};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi::{
    StructuralAccess, StructuralArgument, StructuralMultiplicity, StructuralParameterDeclaration,
    TerminalAffineCleanupAction, TerminalMachine, Terminator,
};

/// The exclusive loan chains of one composed-control machine that a later
/// state's local backs.
#[derive(Debug, Default)]
pub(super) struct LoanChains {
    chains: Vec<LoanChain>,
}

#[derive(Debug)]
struct LoanChain {
    backing: ChainBacking,
    multiplicity: StructuralMultiplicity,
    /// Member parameters: state position, dense structural index, and the
    /// place the state's block declares.
    members: Vec<(usize, u32, PlaceId)>,
}

#[derive(Debug, PartialEq, Eq)]
enum ChainBacking {
    /// One lender's local stays in its own place. `home` is that place,
    /// recorded when a lending edge emits.
    LenderHome { home: Option<PlaceId> },
    /// Members are owned block parameters over the moved storage.
    Transferred,
}

impl LoanChain {
    fn contains(&self, position: usize, dense: u32) -> bool {
        self.members
            .iter()
            .any(|(member, index, _)| *member == position && *index == dense)
    }
}

/// How a successor edge lowers its argument into a chain member.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ChainArrival {
    /// An exclusive loan of the lender's home or of the previous member.
    Retained,
    /// An owned move of the storage itself.
    Transferred,
}

/// How one transition argument relates to a whole exclusive loan.
enum LoanSource {
    /// The emitting state's own local, by result binding ordinal.
    Local(u32),
    /// A whole forward of the emitting state's parameter at a dense index.
    Parameter(u32),
    /// A projection below the emitting state's parameter at a dense index.
    ProjectedParameter(u32),
    /// Anything else: a projected local, the receiver, a copy, a literal.
    Other,
}

/// How a whole exclusive loan reaches one candidate parameter.
enum Arrival {
    Local {
        lender: usize,
        binding: u32,
    },
    Forward,
    /// Caller-owned storage, a projected loan, or the reentered entry's
    /// invocation arrival: storage the chain's ends cannot dispose of.
    Foreign,
}

impl LoanChains {
    /// Find the chains a later state's local backs and choose their backing.
    /// `state_views` holds each state's block parameter declarations, in
    /// dense order, before emission.
    pub(super) fn plan(
        plan: &CheckedComposedUnitControlMachinePlan,
        live: &[bool],
        aliased: &[BTreeMap<u32, u32>],
        entry_reentered: bool,
        state_views: &[Vec<StructuralParameterDeclaration>],
    ) -> Result<Self, LoweringError> {
        let mut chains = Self::default();
        // A state whose parameters are block parameters; a plain entry's are
        // the invocation's own, caller-owned places.
        let block_owned = |position: usize| position != 0 || entry_reentered;
        let mut nodes = Vec::new();
        let mut node_of = Vec::with_capacity(plan.states.len());
        for (position, state) in plan.states.iter().enumerate() {
            let mut dense_nodes = Vec::with_capacity(state.structural_parameters.len());
            for (dense, parameter) in state.structural_parameters.iter().enumerate() {
                let candidate = live.get(position).copied().unwrap_or(false)
                    && block_owned(position)
                    && parameter.access == CheckedStructuralAccess::MutableBorrow
                    && !parameter.is_self
                    && !aliased
                        .get(position)
                        .is_some_and(|aliased| aliased.contains_key(&(dense as u32)));
                dense_nodes.push(candidate.then(|| {
                    nodes.push((position, dense as u32));
                    nodes.len() - 1
                }));
            }
            node_of.push(dense_nodes);
        }
        if nodes.is_empty() {
            return Ok(chains);
        }
        let node = |position: usize, dense: u32| -> Option<usize> {
            node_of
                .get(position)
                .and_then(|dense_nodes: &Vec<Option<usize>>| dense_nodes.get(dense as usize))
                .copied()
                .flatten()
        };
        let mut arrivals: Vec<Vec<Arrival>> = (0..nodes.len()).map(|_| Vec::new()).collect();
        let mut escapes = vec![false; nodes.len()];
        let mut links: Vec<usize> = (0..nodes.len()).collect();
        if entry_reentered {
            // The one-shot invocation block hands the caller's places in.
            for entry_node in node_of[0].iter().flatten() {
                arrivals[*entry_node].push(Arrival::Foreign);
            }
        }
        // Each plain-entry local's loans: the candidate a whole exclusive
        // loan enters, or none for a loan that retains an entry home.
        let mut entry_loans: BTreeMap<u32, Vec<Option<usize>>> = BTreeMap::new();
        for (source, state) in plan.states.iter().enumerate() {
            if !live.get(source).copied().unwrap_or(false) {
                continue;
            }
            for edge in state.successors() {
                let target = plan
                    .states
                    .iter()
                    .position(|candidate| candidate.state == edge.target_state)
                    .ok_or(LoweringError::Unsupported(
                        "Unit graph loan chain target state is absent",
                    ))?;
                for (dense, (parameter, transfer)) in plan.states[target]
                    .structural_parameters
                    .iter()
                    .zip(&edge.transfers)
                    .enumerate()
                {
                    let destination = node(target, dense as u32);
                    match loan_source(&transfer.source) {
                        LoanSource::Local(binding) => {
                            if let Some(destination) = destination {
                                arrivals[destination].push(Arrival::Local {
                                    lender: source,
                                    binding,
                                });
                            }
                            if !block_owned(source)
                                && matches!(
                                    parameter.access,
                                    CheckedStructuralAccess::MutableBorrow
                                        | CheckedStructuralAccess::SharedBorrow
                                )
                            {
                                entry_loans.entry(binding).or_default().push(destination);
                            }
                        }
                        LoanSource::Parameter(index) => match (node(source, index), destination) {
                            (Some(origin), Some(destination)) => {
                                link(&mut links, origin, destination);
                                arrivals[destination].push(Arrival::Forward);
                            }
                            (Some(origin), None) => escapes[origin] = true,
                            (None, Some(destination)) => {
                                arrivals[destination].push(Arrival::Foreign);
                            }
                            (None, None) => {}
                        },
                        LoanSource::ProjectedParameter(index) => {
                            if let Some(origin) = node(source, index) {
                                escapes[origin] = true;
                            }
                            if let Some(destination) = destination {
                                arrivals[destination].push(Arrival::Foreign);
                            }
                        }
                        LoanSource::Other => {
                            if let Some(destination) = destination {
                                arrivals[destination].push(Arrival::Foreign);
                            }
                            // A projected loan of an entry local retains its
                            // home exactly as a whole shared loan does.
                            if !block_owned(source)
                                && let Some(binding) = projected_local(&transfer.source)
                            {
                                entry_loans.entry(binding).or_default().push(None);
                            }
                        }
                    }
                }
            }
        }
        let roots = (0..nodes.len())
            .map(|node| representative(&mut links, node))
            .collect::<Vec<_>>();
        // A chain exists once any arrival lends a later state's local; an
        // entry-only chain keeps the activation-entry route unchanged.
        let seeded = arrivals
            .iter()
            .enumerate()
            .filter(|(_, arrivals)| {
                arrivals.iter().any(|arrival| {
                    matches!(arrival, Arrival::Local { lender, .. } if block_owned(*lender))
                })
            })
            .map(|(node, _)| roots[node])
            .collect::<BTreeSet<_>>();
        if seeded.is_empty() {
            return Ok(chains);
        }
        let moves = |destination: &Option<usize>| {
            destination.is_some_and(|node| seeded.contains(&roots[node]))
        };
        if entry_loans
            .values()
            .any(|destinations| destinations.iter().any(moves) && !destinations.iter().all(moves))
        {
            return Err(LoweringError::Unsupported(
                "Unit graph entry local both backs a later state loan chain and retains an entry home",
            ));
        }
        for root in seeded {
            let mut members = Vec::new();
            let mut lenders = BTreeSet::new();
            let mut multiplicity = None;
            for (index, &(position, dense)) in nodes.iter().enumerate() {
                if roots[index] != root {
                    continue;
                }
                if escapes[index] {
                    return Err(LoweringError::Unsupported(
                        "Unit graph state-local loan chain is re-lent beyond its exclusive members",
                    ));
                }
                let parameter = &plan.states[position].structural_parameters[dense as usize];
                for arrival in &arrivals[index] {
                    let (lender, binding) = match arrival {
                        Arrival::Local { lender, binding } => (*lender, *binding),
                        Arrival::Forward => continue,
                        Arrival::Foreign => {
                            return Err(LoweringError::Unsupported(
                                "Unit graph state loan joins a state-local loan chain with a caller-owned or projected loan",
                            ));
                        }
                    };
                    let (result, _) = plan.states[lender]
                        .operations
                        .iter()
                        .filter_map(|operation| operation.discardable_structural_result())
                        .find(|(result, _)| result.binding_ordinal == binding)
                        .ok_or(LoweringError::Unsupported(
                            "Unit graph state loan chain names no local result",
                        ))?;
                    if result.type_identity != parameter.type_identity {
                        return Err(LoweringError::Unsupported(
                            "Unit graph state loan chain changes its referent type",
                        ));
                    }
                    let home = match result.multiplicity {
                        Multiplicity::Affine => StructuralMultiplicity::Affine,
                        Multiplicity::Unrestricted => StructuralMultiplicity::Unrestricted,
                        Multiplicity::Linear => {
                            return Err(LoweringError::Unsupported(
                                "Unit graph state loan chain home is linear",
                            ));
                        }
                    };
                    if *multiplicity.get_or_insert(home) != home {
                        return Err(LoweringError::Unsupported(
                            "Unit graph state loan chain mixes home multiplicities",
                        ));
                    }
                    lenders.insert((lender, binding));
                }
                let place = state_views
                    .get(position)
                    .and_then(|views| views.get(dense as usize))
                    .ok_or(LoweringError::Unsupported(
                        "Unit graph state loan chain member has no block declaration",
                    ))?
                    .place;
                members.push((position, dense, place));
            }
            // One lender outside the chain dominates every member; anything
            // else would join different live homes, or re-establish a home
            // its own chain still holds.
            let backing = match lenders.iter().next() {
                Some((lender, _))
                    if lenders.len() == 1
                        && !members.iter().any(|(position, _, _)| position == lender) =>
                {
                    ChainBacking::LenderHome { home: None }
                }
                _ => ChainBacking::Transferred,
            };
            chains.chains.push(LoanChain {
                backing,
                multiplicity: multiplicity.ok_or(LoweringError::Unsupported(
                    "Unit graph state loan chain has no lender",
                ))?,
                members,
            });
        }
        Ok(chains)
    }

    /// How an edge's argument enters the state at `position`'s structural
    /// parameter at `dense`, when that parameter is a chain member.
    pub(super) fn arrival(&self, position: usize, dense: u32) -> Option<ChainArrival> {
        Some(match self.member(position, dense)?.backing {
            ChainBacking::LenderHome { .. } => ChainArrival::Retained,
            ChainBacking::Transferred => ChainArrival::Transferred,
        })
    }

    /// Record the home a lending edge lends into a lender-home chain. Every
    /// lending edge of the one lender names the same place.
    pub(super) fn lend(
        &mut self,
        position: usize,
        dense: u32,
        home: PlaceId,
    ) -> Result<(), LoweringError> {
        let Some(chain) = self
            .chains
            .iter_mut()
            .find(|chain| chain.contains(position, dense))
        else {
            return Ok(());
        };
        if let ChainBacking::LenderHome { home: recorded } = &mut chain.backing
            && *recorded.get_or_insert(home) != home
        {
            return Err(LoweringError::Unsupported(
                "Unit graph state loan chain changed its backing home",
            ));
        }
        Ok(())
    }

    /// Declare a transferred chain member as the owned storage it holds.
    /// Called when the state's block seals, after its body emitted against
    /// the authored exclusive view and before private-borrow transport, which
    /// forwards only exclusive loans: the owned block parameter dominates the
    /// state's private blocks instead.
    pub(super) fn declare(
        &self,
        position: usize,
        dense: u32,
        declaration: &mut StructuralParameterDeclaration,
    ) {
        if let Some(chain) = self.member(position, dense)
            && chain.backing == ChainBacking::Transferred
        {
            declaration.access = StructuralAccess::Owned;
            declaration.multiplicity = chain.multiplicity;
        }
    }

    fn member(&self, position: usize, dense: u32) -> Option<&LoanChain> {
        self.chains
            .iter()
            .find(|chain| chain.contains(position, dense))
    }

    /// Dispose of every affine chain's storage where the chain ends: on each
    /// edge leaving a member state that does not carry the loan to another
    /// member, and at each return. `state_ids` names every authored state's
    /// header block; edges between a state's own private blocks keep the
    /// storage live.
    pub(super) fn finish(
        &self,
        machine: &mut TerminalMachine,
        state_ids: &[BlockId],
        live: &[bool],
    ) -> Result<(), LoweringError> {
        if self.chains.is_empty() {
            return Ok(());
        }
        let headers = state_ids
            .iter()
            .zip(live)
            .filter(|(_, live)| **live)
            .map(|(header, _)| *header)
            .collect::<BTreeSet<_>>();
        let producers = machine
            .structural_places
            .iter()
            .filter_map(|place| match place.kind {
                StructuralPlaceKind::OperationResult { producer, .. } => Some((place.id, producer)),
                _ => None,
            })
            .collect::<BTreeMap<_, _>>();
        let machine_parameters = machine
            .structural_parameters
            .iter()
            .map(|parameter| parameter.place)
            .collect::<Vec<_>>();
        for chain in &self.chains {
            if chain.multiplicity != StructuralMultiplicity::Affine {
                continue;
            }
            let member_headers = chain
                .members
                .iter()
                .map(|(position, _, _)| state_ids[*position])
                .collect::<BTreeSet<_>>();
            // A lender home dies once per member state's exit; transferred
            // storage is each member's own parameter.
            let mut visited = BTreeSet::new();
            for &(position, _, parameter) in &chain.members {
                if matches!(chain.backing, ChainBacking::LenderHome { .. })
                    && !visited.insert(position)
                {
                    continue;
                }
                let header = state_ids[position];
                let exit = match chain.backing {
                    ChainBacking::LenderHome { home } => {
                        let home = home.ok_or(LoweringError::Unsupported(
                            "Unit graph state loan chain lost its lender's home",
                        ))?;
                        ChainExit {
                            headers: &headers,
                            storage: home,
                            forwarded: Forwarding::IntoMember(&member_headers),
                            position: DisposalPosition::OperationResult {
                                producer: *producers.get(&home).ok_or(
                                    LoweringError::Unsupported(
                                        "Unit graph state loan chain home has no producer",
                                    ),
                                )?,
                                producers: &producers,
                            },
                        }
                    }
                    ChainBacking::Transferred => ChainExit {
                        headers: &headers,
                        storage: parameter,
                        forwarded: Forwarding::ByArgument,
                        position: DisposalPosition::BlockParameter {
                            position: block_position(machine, header, parameter)?,
                            header_parameters: header_parameters(machine, header)?,
                            machine_parameters: &machine_parameters,
                        },
                    },
                };
                let region = private_region(machine, header, &headers);
                for block in machine
                    .blocks
                    .iter_mut()
                    .filter(|block| region.contains(&block.id))
                {
                    exit.dispose(&mut block.terminator)?;
                }
            }
        }
        Ok(())
    }
}

/// The successor argument's relation to a whole exclusive loan.
fn loan_source(source: &CheckedStructuralControlTransferSourcePlan) -> LoanSource {
    match source {
        CheckedStructuralControlTransferSourcePlan::StructuralResult { binding_ordinal } => {
            LoanSource::Local(*binding_ordinal)
        }
        CheckedStructuralControlTransferSourcePlan::Parameter { index } => {
            LoanSource::Parameter(*index)
        }
        CheckedStructuralControlTransferSourcePlan::ProjectedLoan { subject, .. } => {
            match (&subject.source, subject.path.is_empty()) {
                (
                    CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal },
                    true,
                ) => LoanSource::Local(*binding_ordinal),
                (CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index }, true) => {
                    LoanSource::Parameter(*parameter_index)
                }
                (CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index }, false) => {
                    LoanSource::ProjectedParameter(*parameter_index)
                }
                _ => LoanSource::Other,
            }
        }
        _ => LoanSource::Other,
    }
}

/// The local a projected loan below a state-local result lends.
fn projected_local(source: &CheckedStructuralControlTransferSourcePlan) -> Option<u32> {
    match source {
        CheckedStructuralControlTransferSourcePlan::ProjectedLoan { subject, .. } => {
            match subject.source {
                CheckedUnitStructuralArgumentSourcePlan::StructuralResult { binding_ordinal } => {
                    Some(binding_ordinal)
                }
                _ => None,
            }
        }
        _ => None,
    }
}

fn representative(links: &mut [usize], mut node: usize) -> usize {
    while links[node] != node {
        links[node] = links[links[node]];
        node = links[node];
    }
    node
}

fn link(links: &mut [usize], first: usize, second: usize) {
    let first = representative(links, first);
    let second = representative(links, second);
    if first != second {
        links[second.max(first)] = first.min(second);
    }
}

fn header_parameters(
    machine: &TerminalMachine,
    header: BlockId,
) -> Result<Vec<(PlaceId, u32)>, LoweringError> {
    Ok(machine
        .blocks
        .iter()
        .find(|block| block.id == header)
        .ok_or(LoweringError::Unsupported(
            "Unit graph state loan chain member block is absent",
        ))?
        .structural_parameters
        .iter()
        .map(|parameter| (parameter.place, parameter.position))
        .collect())
}

fn block_position(
    machine: &TerminalMachine,
    header: BlockId,
    place: PlaceId,
) -> Result<u32, LoweringError> {
    header_parameters(machine, header)?
        .into_iter()
        .find(|(candidate, _)| *candidate == place)
        .map(|(_, position)| position)
        .ok_or(LoweringError::Unsupported(
            "Unit graph state loan chain member left its state block",
        ))
}

/// The blocks a state's control reaches before arriving at any state header.
fn private_region(
    machine: &TerminalMachine,
    header: BlockId,
    headers: &BTreeSet<BlockId>,
) -> BTreeSet<BlockId> {
    let mut region = BTreeSet::from([header]);
    let mut pending = vec![header];
    while let Some(current) = pending.pop() {
        let Some(block) = machine.blocks.iter().find(|block| block.id == current) else {
            continue;
        };
        let targets = match &block.terminator {
            Terminator::Jump { target, .. } => vec![*target],
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => vec![when_true.target, when_false.target],
            Terminator::StructuralCase { cases, .. } => {
                cases.iter().map(|case| case.target).collect()
            }
            _ => Vec::new(),
        };
        for target in targets {
            if !headers.contains(&target) && region.insert(target) {
                pending.push(target);
            }
        }
    }
    region
}

/// Which state edges carry the chain onward instead of ending it.
enum Forwarding<'a> {
    /// Any edge into a member state: a lender-home chain's arrivals are all
    /// forwards once the lender is behind it, whatever private name carries
    /// the exclusive view.
    IntoMember(&'a BTreeSet<BlockId>),
    /// An edge naming the owned member parameter as an argument.
    ByArgument,
}

/// Where the storage's discard joins an exit's existing cleanup roster.
enum DisposalPosition<'a> {
    /// Operation results newest first, ahead of locals and parameters.
    OperationResult {
        producer: OperationId,
        producers: &'a BTreeMap<PlaceId, OperationId>,
    },
    /// After every operation result and local: block parameters in reverse
    /// declaration order, ahead of the invocation's own parameters.
    BlockParameter {
        position: u32,
        header_parameters: Vec<(PlaceId, u32)>,
        machine_parameters: &'a [PlaceId],
    },
}

/// One member state's exits for one chain.
struct ChainExit<'a> {
    headers: &'a BTreeSet<BlockId>,
    storage: PlaceId,
    forwarded: Forwarding<'a>,
    position: DisposalPosition<'a>,
}

impl ChainExit<'_> {
    fn dispose(&self, terminator: &mut Terminator) -> Result<(), LoweringError> {
        match terminator {
            Terminator::Jump {
                target,
                structural_arguments,
                trivial_affine_discards,
                ..
            } => self.leave(*target, structural_arguments, trivial_affine_discards),
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => {
                for arm in [when_true, when_false] {
                    self.leave(
                        arm.target,
                        &arm.structural_arguments,
                        &mut arm.trivial_affine_discards,
                    )?;
                }
                Ok(())
            }
            Terminator::StructuralCase { cases, .. } => {
                for arm in cases {
                    self.leave(arm.target, &[], &mut arm.trivial_affine_discards)?;
                }
                Ok(())
            }
            Terminator::ReturnUnit {
                trivial_affine_discards,
                ..
            } => self.insert(trivial_affine_discards),
            Terminator::ReturnStructural {
                source,
                trivial_affine_discards,
                ..
            } => {
                if *source == self.storage {
                    return Err(LoweringError::Unsupported(
                        "Unit graph state loan chain storage cannot escape as an owned return",
                    ));
                }
                self.insert(trivial_affine_discards)
            }
            Terminator::Return {
                cleanup_actions, ..
            } => {
                let mut discards = cleanup_actions
                    .iter()
                    .map(|action| match action {
                        TerminalAffineCleanupAction::DiscardRoot(place) => Ok(*place),
                        _ => Err(LoweringError::Unsupported(
                            "Unit graph state loan chain requires plain scalar return cleanup",
                        )),
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                self.insert(&mut discards)?;
                *cleanup_actions = discards
                    .into_iter()
                    .map(TerminalAffineCleanupAction::DiscardRoot)
                    .collect();
                Ok(())
            }
            Terminator::ReturnUnitPartialAffine { .. }
            | Terminator::ReturnUnitNominalAffine { .. } => Err(LoweringError::Unsupported(
                "Unit graph state loan chain requires plain return cleanup",
            )),
            Terminator::Crash { .. } => Ok(()),
        }
    }

    /// One edge out of a private block: a private successor keeps the
    /// storage, a forwarding state edge carries it, and any other state edge
    /// ends it.
    fn leave(
        &self,
        target: BlockId,
        arguments: &[StructuralArgument],
        discards: &mut Vec<PlaceId>,
    ) -> Result<(), LoweringError> {
        if !self.headers.contains(&target) {
            return Ok(());
        }
        match self.forwarded {
            Forwarding::IntoMember(members) if members.contains(&target) => return Ok(()),
            Forwarding::ByArgument => {
                if let Some(argument) = arguments
                    .iter()
                    .find(|argument| argument.place == self.storage)
                {
                    if !argument.path.is_empty() || argument.access != StructuralAccess::Owned {
                        return Err(LoweringError::Unsupported(
                            "Unit graph transferred loan leaves its state without moving its storage",
                        ));
                    }
                    return Ok(());
                }
            }
            Forwarding::IntoMember(_) => {}
        }
        self.insert(discards)
    }

    fn insert(&self, discards: &mut Vec<PlaceId>) -> Result<(), LoweringError> {
        if discards.contains(&self.storage) {
            return Err(LoweringError::Unsupported(
                "Unit graph state loan chain storage is already disposed on this exit",
            ));
        }
        let at = match &self.position {
            DisposalPosition::OperationResult {
                producer,
                producers,
            } => discards.iter().position(|discard| {
                producers
                    .get(discard)
                    .is_none_or(|existing| existing < producer)
            }),
            DisposalPosition::BlockParameter {
                position,
                header_parameters,
                machine_parameters,
            } => discards.iter().position(|discard| {
                machine_parameters.contains(discard)
                    || header_parameters
                        .iter()
                        .any(|(candidate, earlier)| candidate == discard && earlier < position)
            }),
        };
        discards.insert(at.unwrap_or(discards.len()), self.storage);
        Ok(())
    }
}

#[cfg(test)]
mod tests;

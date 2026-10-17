//! State loans backed by completed storage whose lifetime crosses an arrival.
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
//! set of state parameters connected by whole forwarding
//! (`next(score)` or `next(&mut score)` of a `&mut` parameter), and its
//! storage dies where the chain ends: on every edge leaving a member state
//! that does not carry the loan to another member, and at every return from a
//! member state. Two backings realize that:
//!
//! - **Lender home.** Exactly one state's one local lends into the chain and
//!   that state is not itself a member. Every path into the chain passes the
//!   lender's establishment, so its home stays in place, members keep their
//!   views, and the chain's ends discard the home. Omega's
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
//! Every exclusive member's arrival must lend a local or forward a member.
//! A projected local lends its selected path but retains its complete home,
//! including a reference descriptor whose referent is caller-owned. The home
//! is disposed at the chain's end, never the caller's referent. Such a chain
//! cannot use transferred storage: its parameters describe only the selection.
//! A caller-owned arrival from outside has no local home to dispose, so a join
//! of those different ownership relations stays refused. A member may re-lend
//! its storage onward, shared or exclusive, whole or by projection
//! (`report(&parser.output)`): the successor's parameter then joins the chain
//! as a derived member, every arrival into it must come from the chain, and
//! its state's exits end the chain like any member's. That needs the lender
//! home: the home outlives every name derived from it, whereas transferred
//! storage lives in one member parameter and a projection cannot move it.
//! A shared loan of the local itself (`scan(&stream)`) starts a chain the
//! same way when the storage is affine. Whole readonly arrivals can transport
//! that home just like whole exclusive arrivals; readonly body access stays
//! checked independently of the hidden owned block parameter. A projected
//! member instead retains its lender home. The home dies where the last state
//! holding it leaves. Completed edge operands are lenders too: their identity
//! includes the source state, selected edge, and target operand coordinate,
//! because sibling operand frames can reuse one result binding ordinal. Shared views of
//! unrestricted storage have nothing to dispose of and keep the plain view
//! route. An activation-entry local lent into a chain joins it as one more
//! lender, provided no other edge retains it as an entry home.

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

/// The state loan chains backed by completed owned storage.
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
    /// recorded when a lending edge emits; `lender` is the lending state's
    /// position.
    LenderHome {
        home: Option<PlaceId>,
        lender: usize,
    },
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
    /// A loan of the lender's home or of a previous member; the home stays in
    /// place.
    Retained,
    /// An owned move of the storage itself.
    Transferred,
}

/// How one transition argument relates to a whole exclusive loan.
enum LoanSource {
    /// The emitting state's own local, by result binding ordinal.
    Local { binding: u32, projected: bool },
    /// A completed value established on this exact edge. Its binding ordinal
    /// is local to the edge's operand frame, not the source state's locals.
    Completed(u32),
    /// A whole forward of the emitting state's parameter at a dense index.
    Parameter(u32),
    /// A projection below the emitting state's parameter at a dense index.
    ProjectedParameter(u32),
    /// Anything else: a projected local, the receiver, a copy, a literal.
    Other,
}

/// How a loan reaches one candidate parameter.
enum Arrival {
    Local {
        lender: usize,
        binding: u32,
        /// A selected subtree retains the entire lender home, never moves it
        /// into a parameter whose type describes only the borrowed selection.
        projected: bool,
        /// Exact edge and target dense coordinate of a completed producer.
        /// Several selected edges may reuse one scratch binding ordinal.
        completed: Option<(usize, usize)>,
    },
    /// A whole forward of another candidate.
    Forward,
    /// A projection below another candidate.
    Projected,
    /// Caller-owned storage, a projection of storage outside the chain, or
    /// the reentered entry's invocation arrival: storage the chain's ends
    /// cannot dispose of.
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
        presents: &dyn Fn(
            typed_trees::type_identity::TypeIdentityId,
            &checked_trees::CheckedUnitStructuralParameterPlan,
        ) -> bool,
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
                // Shared parameters join as whole or projected members; one seeds a chain
                // only when it lends affine storage (`seeded` below).
                let candidate = live.get(position).copied().unwrap_or(false)
                    && block_owned(position)
                    && matches!(
                        parameter.access,
                        checked_trees::checked_borrowed_access_pair_pattern!()
                    )
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
            for (edge_index, edge) in state.successors().into_iter().enumerate() {
                let target =
                    plan.state_position(edge.target_state)
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
                    match loan_source(&transfer.source, &state.operations) {
                        LoanSource::Local { binding, projected } => {
                            if let Some(destination) = destination {
                                arrivals[destination].push(Arrival::Local {
                                    lender: source,
                                    binding,
                                    projected,
                                    completed: None,
                                });
                            }
                            if !block_owned(source)
                                && matches!(
                                    parameter.access,
                                    checked_trees::checked_borrowed_access_pair_pattern!()
                                )
                            {
                                entry_loans.entry(binding).or_default().push(destination);
                            }
                        }
                        LoanSource::Completed(binding) => {
                            if let Some(destination) = destination {
                                arrivals[destination].push(Arrival::Local {
                                    lender: source,
                                    binding,
                                    projected: false,
                                    completed: Some((edge_index, dense)),
                                });
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
                            match (node(source, index), destination) {
                                (Some(origin), Some(destination)) => {
                                    link(&mut links, origin, destination);
                                    arrivals[destination].push(Arrival::Projected);
                                }
                                (Some(origin), None) => escapes[origin] = true,
                                (None, Some(destination)) => {
                                    arrivals[destination].push(Arrival::Foreign);
                                }
                                (None, None) => {}
                            }
                        }
                        LoanSource::Other => {
                            if let Some(destination) = destination {
                                arrivals[destination].push(Arrival::Foreign);
                            }
                            // A projected loan of an entry local retains its
                            // home exactly as a whole shared loan does.
                            if !block_owned(source)
                                && let Some(binding) =
                                    projected_local(&transfer.source, &state.operations)
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
        let exclusive = |index: usize| {
            let (position, dense) = nodes[index];
            plan.states[position].structural_parameters[dense as usize].access
                == CheckedStructuralAccess::MutableBorrow
        };
        let local_result = |lender: usize, binding: u32, completed: Option<(usize, usize)>| {
            let result = if let Some((edge, dense)) = completed {
                plan.states[lender]
                    .successors()
                    .into_iter()
                    .nth(edge)
                    .and_then(|edge| edge.transfers.get(dense))
                    .and_then(|transfer| match &transfer.source {
                        CheckedStructuralControlTransferSourcePlan::EstablishedValue {
                            establishment,
                            ..
                        } => establishment.structural_result_binding(),
                        _ => None,
                    })
                    .filter(|result| result.binding_ordinal == binding)
            } else {
                let mut matching = plan.states[lender]
                    .operations
                    .iter()
                    .filter_map(|operation| operation.discardable_structural_result())
                    .map(|(result, _)| result)
                    .filter(|result| result.binding_ordinal == binding);
                let result = matching.next();
                if matching.next().is_some() {
                    None
                } else {
                    result
                }
            };
            result.ok_or(LoweringError::Unsupported(
                "Unit graph state loan chain names no exact completed home",
            ))
        };
        // A chain exists once an arrival lends a later state's local or an
        // edge-completed home. Entry locals keep the activation-entry route. A
        // shared loan seeds one only over an affine structural home, whose
        // death the chain must place; a view or unrestricted local has none.
        let mut seeded = BTreeSet::new();
        for (index, arrivals) in arrivals.iter().enumerate() {
            for arrival in arrivals {
                if let Arrival::Local {
                    lender,
                    binding,
                    completed,
                    ..
                } = *arrival
                    && (block_owned(lender) || completed.is_some())
                    && (exclusive(index)
                        || local_result(lender, binding, completed)
                            .is_ok_and(|result| result.multiplicity == Multiplicity::Affine))
                {
                    seeded.insert(roots[index]);
                }
            }
        }
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
            // A projected member needs the lender home to outlive its selected path.
            let mut derived = false;
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
                    let (lender, binding, completed) = match arrival {
                        Arrival::Local {
                            lender,
                            binding,
                            completed,
                            projected,
                        } => {
                            // The checked loan already retains its exact selected
                            // type and path. Its backing has the containing type:
                            // compare whole homes only for whole-value arrivals.
                            // Projected arrivals force retained-home lifetime,
                            // so no subtree can become ownership of its parent.
                            derived |= projected;
                            if !projected
                                && !presents(
                                    local_result(*lender, *binding, *completed)?.type_identity,
                                    parameter,
                                )
                            {
                                return Err(LoweringError::Unsupported(
                                    "Unit graph state loan chain changes its referent type",
                                ));
                            }
                            (*lender, *binding, *completed)
                        }
                        Arrival::Forward => continue,
                        Arrival::Projected => {
                            derived = true;
                            continue;
                        }
                        Arrival::Foreign => {
                            return Err(LoweringError::Unsupported(
                                "Unit graph state loan joins a state-local loan chain with a caller-owned or projected loan",
                            ));
                        }
                    };
                    let result = local_result(lender, binding, completed)?;
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
                    lenders.insert((lender, binding, completed));
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
                Some((lender, _, _))
                    if lenders.len() == 1
                        && !members.iter().any(|(position, _, _)| position == lender) =>
                {
                    ChainBacking::LenderHome {
                        home: None,
                        lender: *lender,
                    }
                }
                _ if derived => {
                    return Err(LoweringError::Unsupported(
                        "Unit graph state-local loan chain is re-lent beyond its exclusive members",
                    ));
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

    /// Record the home a lending edge lends into a lender-home chain. The
    /// lender-home backing retains one place; a lender whose edges lend the
    /// local from different edge-current places (a value selection on one
    /// edge) needs transferred storage, which this chain was not planned as.
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
        if let ChainBacking::LenderHome { home: recorded, .. } = &mut chain.backing
            && *recorded.get_or_insert(home) != home
        {
            return Err(LoweringError::Unsupported(
                "Unit graph state loan chain lender lends from two homes",
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
            if let ChainBacking::LenderHome {
                home: Some(home),
                lender,
            } = chain.backing
            {
                retain_lent_home(&mut machine.blocks, home);
                end_on_lender_exits(machine, state_ids[lender], &headers, home, &producers)?;
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
                    ChainBacking::LenderHome { home, .. } => {
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

/// Drop a lender home's disposal from each edge that lends the home onward.
///
/// The lender is not a chain member, so `finish` never reaches its exits:
/// only the chain's ends own the home's disposal. Stage 04 still records the
/// lender state's whole dying complement on that exit, because the authored
/// local's last source use is the transition, so `cleanup::for_exit` lowers a
/// `DiscardRoot` of the home onto the very edge that hands out the loan. An
/// edge cannot both carry a borrow of a root and destroy it: the loan's
/// referent would be gone on arrival, and terminal validation refuses that
/// pairing (`block_views::mutable_records::validate_terminator`, "a loan
/// carried on this edge still needs its home"). Keyed on the argument rather
/// than on which state lends, so a lender that lends on one arm and lets the
/// local die on another keeps the dying arm's disposal.
fn retain_lent_home(blocks: &mut [terminal_psi::Block], home: PlaceId) {
    for block in blocks {
        match &mut block.terminator {
            Terminator::Jump {
                structural_arguments,
                cleanup_actions,
                ..
            } => retain_unless_lent(structural_arguments, cleanup_actions, home),
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => {
                for arm in [when_true, when_false] {
                    retain_unless_lent(&arm.structural_arguments, &mut arm.cleanup_actions, home);
                }
            }
            _ => {}
        }
    }
}

/// End a lender home on each exit of its own state that does not lend it.
///
/// `retain_lent_home` keeps whatever disposal stage 04 recorded on a sibling
/// arm, but stage 04 records cleanup only for owners it tracks. A view or
/// reference carrier the lender forms has no checked cleanup row, and
/// `affine_cleanup::complete` leaves it alone once the chain schedules its
/// disposal at the members' exits. A lender that lends its view on one arm and
/// lets it die on another therefore reached the other successor, and every
/// return after it, still owning the view. The home dies on that arm exactly
/// as it dies at a member's non-forwarding exit. Exits that the home's
/// establishment does not precede keep their roster.
fn end_on_lender_exits(
    machine: &mut TerminalMachine,
    header: BlockId,
    headers: &BTreeSet<BlockId>,
    home: PlaceId,
    producers: &BTreeMap<PlaceId, OperationId>,
) -> Result<(), LoweringError> {
    let producer = *producers.get(&home).ok_or(LoweringError::Unsupported(
        "Unit graph state loan chain home has no producer",
    ))?;
    let region = private_region(machine, header, headers);
    let birth = machine
        .blocks
        .iter()
        .find(|block| {
            region.contains(&block.id)
                && block
                    .operations
                    .iter()
                    .any(|operation| operation.id == producer)
        })
        .map(|block| block.id)
        .ok_or(LoweringError::Unsupported(
            "Unit graph state loan chain home is not established by its lender",
        ))?;
    let established = private_region(machine, birth, headers);
    let exit = ChainExit {
        headers,
        storage: home,
        forwarded: Forwarding::ByLoan,
        position: DisposalPosition::OperationResult {
            producer,
            producers,
        },
    };
    for block in machine
        .blocks
        .iter_mut()
        .filter(|block| established.contains(&block.id))
    {
        exit.dispose(&mut block.terminator)?;
    }
    Ok(())
}

/// Transferred chain storage moves as an `Owned` argument and still dies on
/// the edge that moves it; only a retained borrow keeps the home alive.
fn retain_unless_lent(
    arguments: &[StructuralArgument],
    actions: &mut Vec<TerminalAffineCleanupAction>,
    home: PlaceId,
) {
    if arguments
        .iter()
        .any(|argument| argument.place == home && argument.access != StructuralAccess::Owned)
    {
        actions.retain(|action| action.place() != home);
    }
}

/// The successor argument's relation to a retained whole home.
fn loan_source(
    source: &CheckedStructuralControlTransferSourcePlan,
    operations: &[checked_trees::CheckedUnitEffectOperationPlan],
) -> LoanSource {
    match source {
        CheckedStructuralControlTransferSourcePlan::EstablishedValue {
            establishment,
            presentation,
            ..
        } => {
            if !presentation.path.is_empty() {
                // A descriptor crossing lends external backing, not this
                // completed home's storage. Its carrier custody stays separate.
                return LoanSource::Other;
            }
            establishment
                .structural_result_binding()
                .map(|result| LoanSource::Completed(result.binding_ordinal))
                .unwrap_or(LoanSource::Other)
        }
        CheckedStructuralControlTransferSourcePlan::StructuralResult { binding_ordinal } => {
            LoanSource::Local {
                binding: *binding_ordinal,
                projected: false,
            }
        }
        CheckedStructuralControlTransferSourcePlan::Parameter { index } => {
            LoanSource::Parameter(*index)
        }
        CheckedStructuralControlTransferSourcePlan::ProjectedLoan { subject, .. } => {
            if let Some(binding) = subject.source.current_result_binding(operations) {
                return LoanSource::Local {
                    binding,
                    projected: !subject.path.is_empty(),
                };
            }
            match (&subject.source, subject.path.is_empty()) {
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
fn projected_local(
    source: &CheckedStructuralControlTransferSourcePlan,
    operations: &[checked_trees::CheckedUnitEffectOperationPlan],
) -> Option<u32> {
    match source {
        CheckedStructuralControlTransferSourcePlan::ProjectedLoan { subject, .. } => {
            subject.source.current_result_binding(operations)
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
        .block(&header)
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
        let Some(block) = machine.block(&current) else {
            continue;
        };
        let targets = block.terminator.successor_targets();
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
    /// The lender's own exits: an edge naming the home lends it onward, and
    /// a disposal the lender already scheduled stands.
    ByLoan,
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
                cleanup_actions,
                ..
            } => self.leave(*target, structural_arguments, cleanup_actions),
            Terminator::Conditional {
                when_true,
                when_false,
                ..
            } => {
                for arm in [when_true, when_false] {
                    self.leave(
                        arm.target,
                        &arm.structural_arguments,
                        &mut arm.cleanup_actions,
                    )?;
                }
                Ok(())
            }
            Terminator::StructuralCase { cases, .. } => {
                for arm in cases {
                    self.leave(arm.target, &[], &mut arm.cleanup_actions)?;
                }
                Ok(())
            }
            Terminator::ReturnStructural {
                source,
                cleanup_actions,
                ..
            } => {
                if *source == self.storage {
                    if matches!(self.forwarded, Forwarding::ByLoan) {
                        return Ok(());
                    }
                    return Err(LoweringError::Unsupported(
                        "Unit graph state loan chain storage cannot escape as an owned return",
                    ));
                }
                self.insert_actions(cleanup_actions)
            }
            terminal_psi::terminator_return_pair_pattern!(cleanup_actions, ..) => {
                self.insert_actions(cleanup_actions)
            }
            Terminator::Crash { .. } | Terminator::ExternalCompletion { .. } => Ok(()),
        }
    }

    /// One edge out of a private block: a private successor keeps the
    /// storage, a forwarding state edge carries it, and any other state edge
    /// ends it.
    fn leave(
        &self,
        target: BlockId,
        arguments: &[StructuralArgument],
        discards: &mut Vec<TerminalAffineCleanupAction>,
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
            Forwarding::ByLoan => {
                if arguments
                    .iter()
                    .any(|argument| argument.place == self.storage)
                {
                    return Ok(());
                }
            }
            Forwarding::IntoMember(_) => {}
        }
        self.insert_actions(discards)
    }

    fn insert_actions(
        &self,
        actions: &mut Vec<TerminalAffineCleanupAction>,
    ) -> Result<(), LoweringError> {
        if matches!(self.forwarded, Forwarding::ByLoan)
            && actions.iter().any(|action| action.place() == self.storage)
        {
            return Ok(());
        }
        let mut places = actions
            .iter()
            .map(TerminalAffineCleanupAction::place)
            .collect::<Vec<_>>();
        self.insert(&mut places)?;
        let at = places
            .iter()
            .position(|place| *place == self.storage)
            .expect("inserted loan storage");
        actions.insert(at, TerminalAffineCleanupAction::DiscardRoot(self.storage));
        Ok(())
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

//! Memberships describe current payloads, not the storage's arrival value.
//! Stores snapshot their actual source theory; writes retire overlapping rows;
//! reachable control-flow joins intersect evidence. Descriptor replay resolves
//! backing separately. Neither replay grants access, custody or an origin route.

mod entry_snapshots;
mod normal_calls;
mod premise_table;
mod reference_formations;

use normal_calls::NormalPayloadContext;
pub use normal_calls::{BoundaryLenderPreservation, require_boundary_lender_preservation};
use premise_table::{PremiseTable, SharedPremise};

use super::PayloadScalarObservation;
use super::contents::{
    PayloadMembershipKind, incoming_memberships, intersect_payload_memberships_with_kind,
    select_payload_memberships_with_kind,
};
use super::payload_predicates;
use super::payload_predicates::observation_type;
use super::payload_reachability;
use super::payload_reachability::incoming_flow;
use super::*;
use crate::descriptors::descriptor_lifetimes::descriptor_path;
use crate::qualification_path_carrier;
use std::borrow::Borrow;
use std::collections::VecDeque;
use std::sync::Arc;

/// Query-owned snapshots share immutable rows. Only a write detaches a row;
/// map order and value equality remain identical to the owned representation.
#[derive(Clone, Default, PartialEq, Eq)]
struct PayloadContracts(BTreeMap<PayloadSubject, Arc<PayloadMembershipContract>>);

/// The data behind a saved reference has its own identity. Replacing the
/// pointer later cannot change that identity or reveal the caller's address.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum PayloadSubject {
    Value(PlaceId),
    Incoming(DescriptorContentsQuery),
}

impl PayloadSubject {
    fn root(&self) -> PlaceId {
        match self {
            Self::Value(root) => *root,
            Self::Incoming(query) => query.root,
        }
    }
}

impl PayloadContracts {
    fn len(&self) -> usize {
        self.0.len()
    }
    fn get(&self, root: &PlaceId) -> Option<&PayloadMembershipContract> {
        self.get_subject(&PayloadSubject::Value(*root))
    }
    fn get_subject(&self, subject: &PayloadSubject) -> Option<&PayloadMembershipContract> {
        self.0.get(subject).map(Arc::as_ref)
    }
    fn get_mut(&mut self, root: &PlaceId) -> Option<&mut PayloadMembershipContract> {
        self.0
            .get_mut(&PayloadSubject::Value(*root))
            .map(Arc::make_mut)
    }
    fn insert(&mut self, root: PlaceId, contract: PayloadMembershipContract) {
        self.insert_subject(PayloadSubject::Value(root), contract);
    }
    fn insert_subject(&mut self, subject: PayloadSubject, contract: PayloadMembershipContract) {
        self.0.insert(subject, Arc::new(contract));
    }
    fn remove(&mut self, root: &PlaceId) -> Option<Arc<PayloadMembershipContract>> {
        self.0.remove(&PayloadSubject::Value(*root))
    }
    fn remove_subject(
        &mut self,
        subject: &PayloadSubject,
    ) -> Option<Arc<PayloadMembershipContract>> {
        self.0.remove(subject)
    }
    #[cfg(test)]
    fn shared(&self, root: &PlaceId) -> &Arc<PayloadMembershipContract> {
        &self.0[&PayloadSubject::Value(*root)]
    }
    fn values_mut(&mut self) -> impl Iterator<Item = &mut Arc<PayloadMembershipContract>> {
        self.0.values_mut()
    }
}

#[cfg(test)]
impl<const N: usize> From<[(PlaceId, PayloadMembershipContract); N]> for PayloadContracts {
    fn from(rows: [(PlaceId, PayloadMembershipContract); N]) -> Self {
        Self(
            rows.into_iter()
                .map(|(root, contract)| (PayloadSubject::Value(root), Arc::new(contract)))
                .collect(),
        )
    }
}

#[cfg(test)]
impl std::ops::Index<&PlaceId> for PayloadContracts {
    type Output = PayloadMembershipContract;
    fn index(&self, root: &PlaceId) -> &Self::Output {
        self.get(root).unwrap()
    }
}

#[derive(Clone, Default, PartialEq, Eq)]
struct State {
    values: PayloadContracts,
    /// After a write might have changed the data, entry facts can no longer
    /// describe its current contents. Replacing a pointer cannot restore them.
    incoming_entry_unavailable: bool,
    /// Observations may forget a fact; preservation cannot call an unknown
    /// duty roster empty. These exact roots remain unavailable until a whole
    /// authenticated replacement supplies a complete roster again.
    unavailable_preservation: Vec<PlaceId>,
    // Published snapshots keep all premise rosters sorted and unique. An
    // edge join only removes evidence, so ordered intersection is sufficient.
    scalar_requirements: Arc<Vec<SharedPremise>>,
    // Published payloads contain their local roster plus this shared roster.
    // Shared premises are always a subset of the ambient premises above.
    // Entry starts with none: an entry assumption need not occur in an input
    // payload's own contract. Ordinary publication attaches ambient premises
    // to every payload, so transfers need not expand that theory per SSA value.
    // Successive pre-operation snapshots usually keep the same complete theory.
    // Sharing only across payloads still copied its expression trees twice per
    // position. Keep the published roster immutable across positions as well;
    // writers detach at additions, removals or necessary normalization. Ambient
    // and payload retirement differ even when their input allocation agrees.
    // Each immutable proposition is shared too: detaching a changed roster
    // copies its handles, rather than every retained expression tree. Ordering
    // and equality still compare the complete proposition values.
    shared_requirements: Arc<Vec<SharedPremise>>,
}

impl State {
    fn preservation_status(&self, root: PlaceId) -> &'static str {
        if self.unavailable_preservation.contains(&root) {
            "withheld"
        } else if self.values.get(&root).is_none() {
            "absent-root"
        } else {
            "present"
        }
    }

    fn withhold_preservation(&mut self, root: PlaceId) {
        if !self.unavailable_preservation.contains(&root) {
            self.unavailable_preservation.push(root);
        }
    }

    fn withhold_all_preservation(&mut self) {
        for subject in self.values.0.keys() {
            let PayloadSubject::Value(root) = subject else {
                continue;
            };
            if !self.unavailable_preservation.contains(root) {
                self.unavailable_preservation.push(*root);
            }
        }
    }

    fn invalidate_incoming_observations(&mut self) {
        self.incoming_entry_unavailable = true;
        self.values
            .0
            .retain(|subject, _| matches!(subject, PayloadSubject::Value(_)));
    }

    /// Publish only after the transfer has normalized its ambient/local rows.
    /// Previous shared rows survive in the new ambient theory: retirement uses
    /// the payload filter on shared rows, and joins intersect both theories.
    /// Newly produced or rebound payloads remain raw until this publication;
    /// their transfer suffix has no premise reader before this point.
    fn publish_requirements(&mut self) {
        for contract in self.values.values_mut() {
            // Publication removes only local premises already in the ambient
            // roster. Do not detach a shared row when that intersection is empty.
            if !contract.scalar_requirements.iter().any(|premise| {
                self.scalar_requirements
                    .binary_search_by(|candidate| candidate.as_ref().cmp(premise))
                    .is_ok()
            }) {
                continue;
            }
            let contract = Arc::make_mut(contract);
            let mut common = self
                .scalar_requirements
                .iter()
                .map(Borrow::<semantic_vocabulary::Proposition>::borrow)
                .peekable();
            contract.scalar_requirements.retain(|premise| {
                while common.peek().is_some_and(|candidate| *candidate < premise) {
                    common.next();
                }
                !common.peek().is_some_and(|candidate| *candidate == premise)
            });
            // This roster is replay scratch: keep its capacity across publication
            // instead of reallocating after every intersection. Unused capacity
            // lives only with this query's snapshots; measure peak RSS alongside
            // the avoided allocator traffic when changing that lifetime.
        }
        self.shared_requirements = self.scalar_requirements.clone();
    }

    /// Materialize only an actual reader's complete theory. A predicate edge
    /// appends its whole predicate after the ordered local/shared union, just
    /// as the former eager expansion did before final edge normalization.
    fn contract(
        &self,
        root: PlaceId,
        predicate: Option<&semantic_vocabulary::Proposition>,
    ) -> Option<PayloadMembershipContract> {
        if self.unavailable_preservation.contains(&root) {
            return None;
        }
        self.subject_contract(&PayloadSubject::Value(root), predicate)
    }

    fn subject_contract(
        &self,
        subject: &PayloadSubject,
        predicate: Option<&semantic_vocabulary::Proposition>,
    ) -> Option<PayloadMembershipContract> {
        let mut contract = self.values.get_subject(subject)?.clone();
        extend_canonical_premises(&mut contract.scalar_requirements, &self.shared_requirements);
        if let Some(predicate) = predicate
            && !contract.scalar_requirements.contains(predicate)
        {
            contract.scalar_requirements.push(predicate.clone());
        }
        Some(contract)
    }

    fn selected_contract(
        &self,
        types: &[StructuralTypeDeclaration],
        domains: &[terminal_psi::StructuralDomainDeclaration],
        target: DescriptorPayloadTarget,
        memberships: PayloadMembershipKind,
        predicate: Option<&semantic_vocabulary::Proposition>,
    ) -> Option<PayloadMembershipContract> {
        match target {
            DescriptorPayloadTarget::Storage(location) => select_payload_memberships_with_kind(
                types,
                domains,
                &self.contract(location.root, predicate)?,
                &location.path,
                memberships,
            ),
            DescriptorPayloadTarget::Incoming(query) => {
                let subject = PayloadSubject::Incoming(query.clone());
                if self.values.get_subject(&subject).is_some() {
                    if memberships != PayloadMembershipKind::Observed {
                        return None;
                    }
                    return self.subject_contract(&subject, predicate);
                }
                if self.incoming_entry_unavailable {
                    return None;
                }
                let contract = self.contract(query.root, predicate)?;
                query.validate(types, contract.carrier)?;
                incoming_memberships::select(types, domains, contract, &query.steps, memberships)
            }
        }
    }

    fn meet(&mut self, mut arrival: State, memberships: PayloadMembershipKind) -> bool {
        let incoming_entry_changed =
            !self.incoming_entry_unavailable && arrival.incoming_entry_unavailable;
        self.incoming_entry_unavailable |= arrival.incoming_entry_unavailable;
        let previous_unavailable = self.unavailable_preservation.len();
        if memberships == PayloadMembershipKind::Preserved {
            for root in &arrival.unavailable_preservation {
                self.withhold_preservation(*root);
            }
            for (subject, contract) in &self.values.0 {
                let PayloadSubject::Value(root) = subject else {
                    continue;
                };
                if arrival.values.get(root).is_none_or(|incoming| {
                    contract.qualifications != incoming.qualifications
                        || contract.projected_qualifications != incoming.projected_qualifications
                }) && !self.unavailable_preservation.contains(root)
                {
                    self.unavailable_preservation.push(*root);
                }
            }
        }
        let previous_shared = std::mem::take(&mut self.shared_requirements);
        let mut shared = previous_shared.clone();
        retain_common_snapshot(&mut shared, &arrival.shared_requirements);
        let mut changed =
            retain_common_snapshot(&mut self.scalar_requirements, &arrival.scalar_requirements);
        let previous_values = self.values.len();
        self.values.0.retain(|place, contract| {
            let Some(incoming) = arrival.values.remove_subject(place) else {
                return false;
            };
            if contract.carrier != incoming.carrier {
                return false;
            }
            let contract = Arc::make_mut(contract);
            let previous_premises = contract.scalar_requirements.len() + previous_shared.len();
            // Keep every cross term of (old local + old shared) intersected
            // with (incoming local + incoming shared). A fact can be shared
            // on one path and local on the other. Only shared/shared becomes
            // the new shared roster; the other terms remain payload-local.
            retain_captured_premises(
                &mut contract.scalar_requirements,
                &incoming.scalar_requirements,
                &arrival.shared_requirements,
            );
            // Only this incoming cross-term is scratch; do not detach the
            // incoming row's observations and membership vectors to filter it.
            let mut incoming_premises = incoming.scalar_requirements.clone();
            retain_common_premises(&mut incoming_premises, &previous_shared);
            extend_canonical_premises(&mut contract.scalar_requirements, &incoming_premises);
            changed |= contract.scalar_requirements.len() + shared.len() != previous_premises;
            let previous_counts = (
                contract.qualifications.len(),
                contract.projected_qualifications.len(),
            );
            // Executable expressions and entry identities meet independently.
            // Losing one component can change a row without changing its count.
            changed |= PayloadScalarObservation::intersect(
                &mut contract.scalar_observations,
                &incoming.scalar_observations,
            );
            retain_common_memberships(&mut contract.qualifications, &incoming.qualifications);
            retain_common_memberships(
                &mut contract.projected_qualifications,
                &incoming.projected_qualifications,
            );
            changed |= previous_counts
                != (
                    contract.qualifications.len(),
                    contract.projected_qualifications.len(),
                );
            true
        });
        self.shared_requirements = shared;
        // All logical changes are losses. Moving a fact between local and
        // shared storage alone must not enqueue another worklist iteration.
        changed
            || incoming_entry_changed
            || self.values.len() != previous_values
            || self.unavailable_preservation.len() != previous_unavailable
    }
}

struct CachedHistory {
    relevant: payload_reachability::Reachability,
    snapshots: BTreeMap<DescriptorPosition, State>,
    // One queried post-state serves every formal root at that exact exit.
    // Any pending replay work invalidates it before another query can reuse it.
    normal_completion: Option<(DescriptorPosition, State)>,
}

/// One history's interned premises and its latest replay. A later query
/// extends that replay when its predecessor graph is compatible; otherwise it
/// starts again from entry. Both replays intern into the same table.
#[derive(Default)]
struct HistoryReplay {
    memberships: PayloadMembershipKind,
    premises: PremiseTable,
    cached: Option<CachedHistory>,
}

/// Exact declaration data in one current executable representation. Prepared
/// rows are data, not predicates or callbacks; the history still derives every
/// value observation from the supplied current graph and actual operations.
enum PayloadSources<'input> {
    Terminal(&'input TerminalMachine),
    Current {
        identity: MachineId,
        parameters: &'input [terminal_psi::StructuralParameterDeclaration],
        contracts: &'input [(PlaceId, PayloadMembershipContract)],
        contract_positions: Vec<usize>,
    },
}
impl PayloadSources<'_> {
    fn identity(&self) -> MachineId {
        match self {
            Self::Terminal(machine) => machine.id,
            Self::Current { identity, .. } => *identity,
        }
    }
    fn parameters(&self) -> &[terminal_psi::StructuralParameterDeclaration] {
        match self {
            Self::Terminal(machine) => &machine.structural_parameters,
            Self::Current { parameters, .. } => parameters,
        }
    }
    fn contract(&self, root: PlaceId) -> Option<PayloadMembershipContract> {
        match self {
            Self::Terminal(machine) => super::terminal_payload_contract(machine, root),
            Self::Current {
                contracts,
                contract_positions,
                ..
            } => {
                let index = contract_positions
                    .binary_search_by_key(&root, |&index| contracts[index].0)
                    .ok()?;
                Some(contracts[contract_positions[index]].1.clone())
            }
        }
    }
}

/// Query-local reuse bound to one immutable Terminal machine and its exact
/// catalog/descriptor graph. A producer creates this inside its read sweep and
/// drops it before publishing changed rosters. Arbitrary source callbacks are
/// deliberately excluded from this reusable interface.
pub(crate) struct PreservedNormalExit {
    pub position: DescriptorPosition,
    pub contracts: Vec<(PlaceId, PayloadMembershipContract)>,
}

pub struct PayloadMembershipHistory<'input> {
    types: &'input [StructuralTypeDeclaration],
    domains: &'input [terminal_psi::StructuralDomainDeclaration],
    graph: &'input [DescriptorMachine],
    descriptor: Option<&'input DescriptorMachine>,
    sources: PayloadSources<'input>,
    // Queries and payload transfers share this immutable pre-operation replay.
    // Normal-completion queries use their separate borrowed-slot replay below.
    incoming: Option<Option<payload_reachability::IncomingFlow>>,
    analysis: Option<DescriptorAnalysis<'input>>,
    normal: Option<NormalPayloadContext<'input>>,
    replay: HistoryReplay,
    predicate_observations: HistoryReplay,
    /// Materialized only for an exact scalar-snapshot query. Ordinary numeric
    /// membership sweeps keep their existing input and allocation profile.
    entry_snapshots: Option<Vec<(PlaceId, Vec<PayloadScalarObservation>)>>,
}

impl<'input> PayloadMembershipHistory<'input> {
    pub fn new(
        types: &'input [StructuralTypeDeclaration],
        domains: &'input [terminal_psi::StructuralDomainDeclaration],
        graph: &'input [DescriptorMachine],
        machine: &'input TerminalMachine,
    ) -> Self {
        Self {
            types,
            domains,
            graph,
            descriptor: descriptor_machine(graph, machine.id),
            sources: PayloadSources::Terminal(machine),
            incoming: None,
            analysis: None,
            normal: None,
            replay: HistoryReplay::default(),
            predicate_observations: HistoryReplay::default(),
            entry_snapshots: None,
        }
    }

    /// Current calls share the same immutable body catalog. Their normal
    /// effects can corroborate an existing source contract at its real cut.
    pub fn for_module(
        module: &'input TerminalModule,
        graph: &'input [DescriptorMachine],
        machine: &'input TerminalMachine,
    ) -> Self {
        let mut history = Self::new(
            &module.structural_types,
            &module.structural_domains,
            graph,
            machine,
        );
        history.normal = Some(NormalPayloadContext::new(module, graph));
        history
    }

    pub fn for_module_preservation(
        module: &'input TerminalModule,
        graph: &'input [DescriptorMachine],
        machine: &'input TerminalMachine,
    ) -> Self {
        let mut history = Self::for_preservation(
            &module.structural_types,
            &module.structural_domains,
            graph,
            machine,
        );
        history.normal = Some(NormalPayloadContext::new(module, graph));
        history
    }
    /// Keep the promises carried by actual producers, copies and stores.
    /// Current scalar truths remain observations and introduce no new duty on
    /// a mutable reference. This history owns its separate replay snapshots.
    /// Its graph includes ordinary borrowed-storage slots, independently of
    /// the view-only inventory used for ordinary observations.
    pub fn for_preservation(
        types: &'input [StructuralTypeDeclaration],
        domains: &'input [terminal_psi::StructuralDomainDeclaration],
        graph: &'input [DescriptorMachine],
        machine: &'input TerminalMachine,
    ) -> Self {
        let mut history = Self::new(types, domains, graph, machine);
        history.replay.memberships = PayloadMembershipKind::Preserved;
        history.analysis = Some(DescriptorAnalysis::with_borrowed_storage_slots(
            types, graph,
        ));
        history
    }

    /// The complete source-authored roster behind this exact reference.
    /// An unresolved history is a missing preservation proof, not an empty
    /// declaration to fall back to. Every physical alternative still passes
    /// the common carrier, ancestor and exclusive-roster checks.
    pub fn reference_preservation_memberships(
        &mut self,
        identities: &terminal_psi::TerminalIdentityTable,
        analysis: &mut DescriptorAnalysis<'_>,
        position: DescriptorPosition,
        source: &terminal_psi::StructuralArgument,
        storage: Option<&terminal_psi::ReferenceStorageView>,
        result: &terminal_psi::StructuralOperationResult,
    ) -> Result<Vec<terminal_psi::StructuralPathQualification>, &'static str> {
        if self.replay.memberships != PayloadMembershipKind::Preserved {
            return Err("reference preservation requires its source-authored history");
        }
        let (types, domains, machine) = (self.types, self.domains, self.sources.identity());
        let mut unavailable = None;
        let result = super::reference_backing_memberships(
            types,
            domains,
            identities,
            analysis,
            machine,
            position,
            source,
            storage,
            result,
            |place| {
                self.current_payload_contract(position, place)
                    .map_err(|reason| unavailable = Some(reason))
                    .ok()
            },
        );
        result.map_err(|reason| unavailable.unwrap_or(reason))
    }

    /// Reuse this same history over normalized current-body declarations.
    /// Missing declarations stay unavailable when queried; an absent contract
    /// never becomes an established empty payload or initialized local.
    pub fn for_current_contracts(
        types: &'input [StructuralTypeDeclaration],
        domains: &'input [terminal_psi::StructuralDomainDeclaration],
        graph: &'input [DescriptorMachine],
        identity: MachineId,
        parameters: &'input [terminal_psi::StructuralParameterDeclaration],
        contracts: &'input [(PlaceId, PayloadMembershipContract)],
    ) -> Result<Self, &'static str> {
        let current =
            descriptor_machine(graph, identity).ok_or("current payload machine is absent")?;
        let mut contract_positions = (0..contracts.len()).collect::<Vec<_>>();
        contract_positions.sort_by_key(|&index| contracts[index].0);
        if contract_positions
            .windows(2)
            .any(|pair| contracts[pair[0]].0 == contracts[pair[1]].0)
        {
            return Err("current payload declaration disagrees with its exact graph carrier");
        }
        for (root, contract) in contracts {
            if current.place_types.get(root) != Some(&contract.carrier) {
                return Err("current payload declaration disagrees with its exact graph carrier");
            }
        }
        Ok(Self {
            types,
            domains,
            graph,
            descriptor: Some(current),
            incoming: None,
            analysis: None,
            normal: None,
            sources: PayloadSources::Current {
                identity,
                parameters,
                contracts,
                contract_positions,
            },
            replay: HistoryReplay::default(),
            predicate_observations: HistoryReplay::default(),
            entry_snapshots: None,
        })
    }

    /// Replay the complete old duties over these exact current declarations.
    /// New formation predicates still require the separate current-storage
    /// observation; neither the source roster nor the requested result proves it.
    pub fn for_current_preservation_contracts(
        types: &'input [StructuralTypeDeclaration],
        domains: &'input [terminal_psi::StructuralDomainDeclaration],
        graph: &'input [DescriptorMachine],
        identity: MachineId,
        parameters: &'input [terminal_psi::StructuralParameterDeclaration],
        contracts: &'input [(PlaceId, PayloadMembershipContract)],
    ) -> Result<Self, &'static str> {
        let mut history =
            Self::for_current_contracts(types, domains, graph, identity, parameters, contracts)?;
        history.replay.memberships = PayloadMembershipKind::Preserved;
        history.analysis = Some(DescriptorAnalysis::with_borrowed_storage_slots(
            types, graph,
        ));
        Ok(history)
    }
    pub fn current_selected_payload_contract(
        &mut self,
        position: DescriptorPosition,
        root: PlaceId,
        path: &[StructuralPathSegment],
    ) -> Result<PayloadMembershipContract, &'static str> {
        let mut analysis = self
            .analysis
            .take()
            .unwrap_or_else(|| DescriptorAnalysis::new(self.types, self.graph));
        let result = self.current_selected_payload_contract_in(&mut analysis, position, root, path);
        self.analysis = Some(analysis);
        result
    }

    pub fn current_selected_payload_contract_in(
        &mut self,
        analysis: &mut DescriptorAnalysis<'_>,
        position: DescriptorPosition,
        root: PlaceId,
        path: &[StructuralPathSegment],
    ) -> Result<PayloadMembershipContract, &'static str> {
        let descriptor = self.descriptor;
        let incoming = self
            .incoming
            .get_or_insert_with(|| descriptor.and_then(|machine| incoming_flow(&machine.flow)))
            .as_ref();
        selected_payload_contract_reusing(
            self.types,
            self.domains,
            self.sources.identity(),
            descriptor,
            incoming,
            analysis,
            position,
            root,
            path,
            &|place| self.sources.contract(place),
            self.entry_snapshots.as_deref().unwrap_or(&[]),
            &mut self.replay,
            self.normal.as_mut(),
        )
    }

    /// Observe this owned value occurrence before the operation, without
    /// following a descriptor to its backing. The caller's shared immutable
    /// analysis and this history use the same exact current source catalog.
    pub fn current_payload_contract(
        &mut self,
        position: DescriptorPosition,
        root: PlaceId,
    ) -> Result<PayloadMembershipContract, &'static str> {
        let mut analysis = self
            .analysis
            .take()
            .unwrap_or_else(|| DescriptorAnalysis::new(self.types, self.graph));
        let result = self.current_payload_contract_in(&mut analysis, position, root);
        self.analysis = Some(analysis);
        result
    }

    pub fn current_payload_contract_in(
        &mut self,
        analysis: &mut DescriptorAnalysis<'_>,
        position: DescriptorPosition,
        root: PlaceId,
    ) -> Result<PayloadMembershipContract, &'static str> {
        let sources = &self.sources;
        let descriptor = self.descriptor;
        let incoming = self
            .incoming
            .get_or_insert_with(|| descriptor.and_then(|machine| incoming_flow(&machine.flow)))
            .as_ref();
        current_payload_contract_reusing(
            self.types,
            self.domains,
            descriptor,
            incoming,
            analysis,
            position,
            root,
            &|place| sources.contract(place),
            self.entry_snapshots.as_deref().unwrap_or(&[]),
            &mut self.replay,
            self.normal.as_mut(),
        )
    }
    /// Read this value after the exact operation's effects, before edge
    /// bindings. Reuse the preservation replay and external borrowed-storage
    /// analysis; never seed this contract from the guarantee being requested.
    fn preserved_completion_contract_in(
        &mut self,
        analysis: &mut DescriptorAnalysis<'_>,
        position: DescriptorPosition,
        root: PlaceId,
        normal: Option<&mut NormalPayloadContext<'_>>,
    ) -> Result<PayloadMembershipContract, &'static str> {
        if self.replay.memberships != PayloadMembershipKind::Preserved {
            return Err("normal-return duties require a preservation history");
        }
        let sources = &self.sources;
        let descriptor = self.descriptor;
        let incoming = self
            .incoming
            .get_or_insert_with(|| descriptor.and_then(|machine| incoming_flow(&machine.flow)))
            .as_ref();
        payload_contract_at_cut(
            self.types,
            self.domains,
            descriptor,
            incoming,
            analysis,
            position,
            root,
            &|place| sources.contract(place),
            self.entry_snapshots.as_deref().unwrap_or(&[]),
            &mut self.replay,
            PayloadMembershipCut::NormalCompletion,
            normal,
        )
    }

    /// Each possible normal exit must retain the promised payload. Return
    /// every complete roster so the summary owner meets guarantees and checks
    /// the caller's full original duties, rather than unioning alternatives.
    /// This query itself publishes nothing into the caller's withheld state.
    ///
    /// The invocation owns the shared normal-summary recursion context. This
    /// method creates no new context or per-exit history. When integrating the
    /// call hook, forward that same context through the completion/cut query.
    fn preserved_normal_exit_contracts_in(
        &mut self,
        analysis: &mut DescriptorAnalysis<'_>,
        root: PlaceId,
        normal: Option<&mut NormalPayloadContext<'_>>,
    ) -> Result<Vec<(DescriptorPosition, PayloadMembershipContract)>, &'static str> {
        self.preserved_normal_exit_rosters_in(analysis, &[root], normal)?
            .into_iter()
            .map(|exit| {
                let (_, contract) = exit
                    .contracts
                    .into_iter()
                    .next()
                    .ok_or("the single-root normal exit lost its requested contract")?;
                Ok((exit.position, contract))
            })
            .collect()
    }

    /// Batch roots inside each exit so its performed post-state is evaluated
    /// once. No guarantee is unioned across exits, and unavailable roots refuse
    /// the complete batch rather than disappearing from the required roster.
    fn preserved_normal_exit_rosters_in(
        &mut self,
        analysis: &mut DescriptorAnalysis<'_>,
        roots: &[PlaceId],
        mut normal: Option<&mut NormalPayloadContext<'_>>,
    ) -> Result<Vec<PreservedNormalExit>, &'static str> {
        if self.replay.memberships != PayloadMembershipKind::Preserved {
            return Err("normal-return duties require a preservation history");
        }
        if roots
            .iter()
            .enumerate()
            .any(|(index, root)| roots[..index].contains(root))
        {
            return Err("the normal-return requested root roster is ambiguous");
        }
        let descriptor = self
            .descriptor
            .ok_or("the normal-return body graph is absent")?;
        let feasible = analysis.flow_feasibility(self.sources.identity());
        let mut exits = Vec::new();
        for position in &descriptor.normal_exits {
            match feasible.get(position) {
                Some(FlowFeasibility::ProvedImpossible) => continue,
                Some(FlowFeasibility::Possible) => {}
                Some(FlowFeasibility::Unknown) | None => {
                    return Err("a normal exit has no complete reachability evidence");
                }
            }
            let mut contracts = Vec::with_capacity(roots.len());
            for root in roots {
                contracts.push((
                    *root,
                    self.preserved_completion_contract_in(
                        analysis,
                        *position,
                        *root,
                        normal.as_deref_mut(),
                    )?,
                ));
            }
            exits.push(PreservedNormalExit {
                position: *position,
                contracts,
            });
        }
        if exits.is_empty() {
            return Err("the callee has no established reachable normal completion");
        }
        Ok(exits)
    }

    /// Observe a primitive value snapshot, without asserting a predicate about
    /// it. Unknown entry contents acquire only a typed entry identity; existing
    /// history transports that identity through actual copies and intersects it
    /// across every predecessor. No synthetic SSA or executable read is added.
    pub fn scalar_snapshot(
        &mut self,
        position: DescriptorPosition,
        root: PlaceId,
        path: &[crate::descriptors::descriptor_lifetimes::DescriptorPathSegment],
    ) -> Result<PayloadScalarObservation, &'static str> {
        if self.entry_snapshots.is_none() {
            self.entry_snapshots = Some(entry_snapshots::collect(
                self.types,
                self.sources.parameters(),
            ));
            self.replay.cached = None;
            self.predicate_observations.cached = None;
        }
        let contract = self.current_payload_contract(position, root)?;
        let expected = observation_type(self.types, contract.carrier, path)
            .ok_or("snapshot subject is not an exact primitive value")?;
        let mut observations = contract
            .scalar_observations
            .iter()
            .filter(|row| row.path == path);
        let observed = observations
            .next()
            .ok_or("snapshot subject has no retained value identity")?;
        if observations.next().is_some()
            || observed
                .value
                .as_ref()
                .is_some_and(|value| value.scalar_type() != expected)
            || observed
                .entry
                .as_ref()
                .is_some_and(|entry| entry.scalar_type != expected)
            || (observed.value.is_none() && observed.entry.is_none())
        {
            return Err("snapshot subject has ambiguous or mismatched scalar identity");
        }
        Ok(observed.clone())
    }
}

/// Observe the current payload selected by an operand, following descriptors
/// to every possible backing. This proves facts about the selected bytes; it
/// grants neither a copy nor a loan and never reads the receiving contract.
pub fn current_selected_payload_contract(
    types: &[StructuralTypeDeclaration],
    domains: &[terminal_psi::StructuralDomainDeclaration],
    graph: &[DescriptorMachine],
    identity: MachineId,
    position: DescriptorPosition,
    root: PlaceId,
    path: &[StructuralPathSegment],
    source: &impl Fn(PlaceId) -> Option<PayloadMembershipContract>,
) -> Result<PayloadMembershipContract, &'static str> {
    let mut analysis = DescriptorAnalysis::new(types, graph);
    let descriptor = descriptor_machine(graph, identity);
    let incoming = descriptor.and_then(|machine| incoming_flow(&machine.flow));
    let mut replay = HistoryReplay::default();
    selected_payload_contract_reusing(
        types,
        domains,
        identity,
        descriptor,
        incoming.as_ref(),
        &mut analysis,
        position,
        root,
        path,
        source,
        &[],
        &mut replay,
        None,
    )
}

/// Reconstruct memberships at an exact pre-operation position. Entry supplies
/// only actual input contracts; producer rosters enter at establishment. A
/// receiving store or requested copy annotation never seeds the fixed point.
/// The bounded worklist fails closed if the current graph cannot be resolved.
pub fn current_payload_contract(
    types: &[StructuralTypeDeclaration],
    domains: &[terminal_psi::StructuralDomainDeclaration],
    graph: &[DescriptorMachine],
    identity: MachineId,
    position: DescriptorPosition,
    root: PlaceId,
    source: &impl Fn(PlaceId) -> Option<PayloadMembershipContract>,
) -> Result<PayloadMembershipContract, &'static str> {
    let mut analysis = DescriptorAnalysis::new(types, graph);
    let descriptor = descriptor_machine(graph, identity);
    let incoming = descriptor.and_then(|machine| incoming_flow(&machine.flow));
    current_payload_contract_reusing(
        types,
        domains,
        descriptor,
        incoming.as_ref(),
        &mut analysis,
        position,
        root,
        source,
        &[],
        &mut HistoryReplay::default(),
        None,
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PayloadMembershipCut {
    BeforeOperation,
    NormalCompletion,
}

/// Memberships after this operation's effects and normal postconditions, before
/// successor bindings. The caller captures an input's pointer identity before
/// moves, then asks about that backing at this cut. This never reads the new
/// result's annotation as evidence for an input fact. Borrowed-storage replay
/// retains pointer-reference and view contents through the same typed graph.
pub(crate) fn completed_payload_contract(
    types: &[StructuralTypeDeclaration],
    domains: &[terminal_psi::StructuralDomainDeclaration],
    graph: &[DescriptorMachine],
    identity: MachineId,
    position: DescriptorPosition,
    root: PlaceId,
    source: &impl Fn(PlaceId) -> Option<PayloadMembershipContract>,
) -> Result<PayloadMembershipContract, &'static str> {
    let mut analysis = DescriptorAnalysis::with_borrowed_storage_slots(types, graph);
    let descriptor = descriptor_machine(graph, identity);
    let incoming = descriptor.and_then(|machine| incoming_flow(&machine.flow));
    payload_contract_at_cut(
        types,
        domains,
        descriptor,
        incoming.as_ref(),
        &mut analysis,
        position,
        root,
        source,
        &[],
        &mut HistoryReplay::default(),
        PayloadMembershipCut::NormalCompletion,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn selected_payload_contract_reusing(
    types: &[StructuralTypeDeclaration],
    domains: &[terminal_psi::StructuralDomainDeclaration],
    identity: MachineId,
    descriptor: Option<&DescriptorMachine>,
    incoming: Option<&payload_reachability::IncomingFlow>,
    analysis: &mut DescriptorAnalysis<'_>,
    position: DescriptorPosition,
    root: PlaceId,
    path: &[StructuralPathSegment],
    source: &impl Fn(PlaceId) -> Option<PayloadMembershipContract>,
    entry_snapshots: &[(PlaceId, Vec<PayloadScalarObservation>)],
    replay: &mut HistoryReplay,
    mut normal: Option<&mut NormalPayloadContext<'_>>,
) -> Result<PayloadMembershipContract, &'static str> {
    let root_contract = source(root).ok_or("selected payload root is absent")?;
    let query = descriptor_contents_query(types, root, root_contract.carrier, path, None)
        .ok_or("selected payload path has no typed coordinate")?;
    let targets = analysis
        .payload_targets(identity, position, &query)
        .ok_or("selected payload backing is unresolved")?;
    let mut failure = None;
    let kind = replay.memberships;
    intersect_payload_memberships_with_kind(
        types,
        domains,
        targets,
        |target| {
            let result = match target {
                DescriptorPayloadTarget::Storage(location) => current_payload_contract_reusing(
                    types,
                    domains,
                    descriptor,
                    incoming,
                    analysis,
                    position,
                    location.root,
                    source,
                    entry_snapshots,
                    replay,
                    normal.as_deref_mut(),
                )
                .and_then(|contract| {
                    select_payload_memberships_with_kind(
                        types,
                        domains,
                        &contract,
                        &location.path,
                        kind,
                    )
                    .ok_or("selected storage payload has no typed contract")
                }),
                DescriptorPayloadTarget::Incoming(query) => payload_subject_contract_at_cut(
                    types,
                    domains,
                    descriptor,
                    incoming,
                    analysis,
                    position,
                    PayloadSubject::Incoming(query),
                    source,
                    entry_snapshots,
                    replay,
                    PayloadMembershipCut::BeforeOperation,
                    normal.as_deref_mut(),
                ),
            };
            result.map_err(|reason| failure = Some(reason)).ok()
        },
        kind,
    )
    .ok_or(failure.unwrap_or("selected payload memberships are unresolved"))
}

fn current_payload_contract_reusing(
    types: &[StructuralTypeDeclaration],
    domains: &[terminal_psi::StructuralDomainDeclaration],
    descriptor: Option<&DescriptorMachine>,
    incoming: Option<&payload_reachability::IncomingFlow>,
    analysis: &mut DescriptorAnalysis<'_>,
    position: DescriptorPosition,
    root: PlaceId,
    source: &impl Fn(PlaceId) -> Option<PayloadMembershipContract>,
    entry_snapshots: &[(PlaceId, Vec<PayloadScalarObservation>)],
    replay: &mut HistoryReplay,
    normal: Option<&mut NormalPayloadContext<'_>>,
) -> Result<PayloadMembershipContract, &'static str> {
    payload_contract_at_cut(
        types,
        domains,
        descriptor,
        incoming,
        analysis,
        position,
        root,
        source,
        entry_snapshots,
        replay,
        PayloadMembershipCut::BeforeOperation,
        normal,
    )
}

#[allow(clippy::too_many_arguments)]
fn payload_contract_at_cut(
    types: &[StructuralTypeDeclaration],
    domains: &[terminal_psi::StructuralDomainDeclaration],
    descriptor: Option<&DescriptorMachine>,
    incoming: Option<&payload_reachability::IncomingFlow>,
    analysis: &mut DescriptorAnalysis<'_>,
    position: DescriptorPosition,
    root: PlaceId,
    source: &impl Fn(PlaceId) -> Option<PayloadMembershipContract>,
    entry_snapshots: &[(PlaceId, Vec<PayloadScalarObservation>)],
    replay: &mut HistoryReplay,
    cut: PayloadMembershipCut,
    normal: Option<&mut NormalPayloadContext<'_>>,
) -> Result<PayloadMembershipContract, &'static str> {
    payload_subject_contract_at_cut(
        types,
        domains,
        descriptor,
        incoming,
        analysis,
        position,
        PayloadSubject::Value(root),
        source,
        entry_snapshots,
        replay,
        cut,
        normal,
    )
}

#[allow(clippy::too_many_arguments)]
fn payload_subject_contract_at_cut(
    types: &[StructuralTypeDeclaration],
    domains: &[terminal_psi::StructuralDomainDeclaration],
    descriptor: Option<&DescriptorMachine>,
    incoming: Option<&payload_reachability::IncomingFlow>,
    analysis: &mut DescriptorAnalysis<'_>,
    position: DescriptorPosition,
    subject: PayloadSubject,
    source: &impl Fn(PlaceId) -> Option<PayloadMembershipContract>,
    entry_snapshots: &[(PlaceId, Vec<PayloadScalarObservation>)],
    replay: &mut HistoryReplay,
    cut: PayloadMembershipCut,
    mut normal: Option<&mut NormalPayloadContext<'_>>,
) -> Result<PayloadMembershipContract, &'static str> {
    let root = subject.root();
    let memberships = replay.memberships;
    let trace_preservation = memberships == PayloadMembershipKind::Preserved
        && std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some();
    let trace_observations = memberships == PayloadMembershipKind::Observed
        && normal.is_some()
        && std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some();
    let mut failure = "descriptor machine is absent";
    let result = (|| {
        let machine = descriptor?;
        let identity = machine.id;
        failure = "payload query reachability is invalid or exceeds its finite budget";
        let mut relevant = match incoming {
            Some(incoming) => {
                payload_reachability::predecessors_with_incoming(machine, incoming, position)?
            }
            None => payload_reachability::predecessors(machine, position)?,
        };
        let premises = &mut replay.premises;
        let mut previous = replay.cached.take();
        let frontier = previous
            .as_ref()
            .and_then(|previous| relevant.compatible_frontier(&previous.relevant, &machine.flow));
        let mut normal_completion = if frontier.as_ref().is_some_and(|pending| pending.is_empty()) {
            previous
                .as_mut()
                .and_then(|history| history.normal_completion.take())
        } else {
            None
        };
        let (mut snapshots, mut pending, previous_relevant) = if let Some(frontier) = frontier {
            let previous = previous.expect("frontier owns the preceding immutable history");
            (previous.snapshots, frontier, Some(previous.relevant))
        } else {
            // Release an incompatible prefix before constructing its replacement.
            drop(previous);
            let mut initial = State::default();
            if let Some((_, requirements)) = &machine.payload_entry_predicates {
                initial.scalar_requirements = Arc::new(
                    super::scalar_contracts::scalar_conjuncts(requirements)
                        .into_iter()
                        .map(|premise| premises.intern(premise))
                        .collect(),
                );
            }
            canonicalize_snapshot(&mut initial.scalar_requirements);
            for place in &machine.places {
                if matches!(place.definition, DescriptorDefinition::Parameter { .. }) {
                    failure = "entry payload contract is absent";
                    let mut contract = source(place.place)?;
                    if let Some((_, snapshots)) = entry_snapshots
                        .iter()
                        .find(|(root, _)| *root == place.place)
                    {
                        contract
                            .scalar_observations
                            .extend(snapshots.iter().cloned());
                    }
                    if memberships == PayloadMembershipKind::Observed {
                        super::payload_entry::close(
                            types,
                            domains,
                            machine,
                            place.place,
                            &mut contract,
                        );
                    }
                    contract.scalar_requirements.sort();
                    contract.scalar_requirements.dedup();
                    initial.values.insert(place.place, contract);
                }
            }
            (
                BTreeMap::from([(machine.entry, initial)]),
                VecDeque::from([machine.entry]),
                None,
            )
        };
        // Reuse a completed cut only while no replay work changed its input.
        // A different queried cut can still reuse its exact cached BEFORE
        // state by replaying that one authoritative transfer in this context.
        if !pending.is_empty() {
            normal_completion = None;
        }
        if cut == PayloadMembershipCut::NormalCompletion
            && normal_completion
                .as_ref()
                .is_none_or(|(at, _)| *at != position)
            && snapshots.contains_key(&position)
            && !pending.contains(&position)
        {
            pending.push_back(position);
        }
        // The transfer and edge batches must retain all simultaneous RHSs until
        // their reads finish. Draining moves those rows without dropping the
        // scratch allocation between worklist positions or successor edges.
        let mut stores = Vec::new();
        let mut incoming_stores = Vec::new();
        let mut bindings = Vec::new();
        let mut remaining = 65_536usize;
        while let Some(current) = pending.pop_front() {
            failure = "payload history worklist exceeded its finite budget";
            remaining = remaining.checked_sub(1)?;
            if !relevant.contains(current) {
                continue;
            }
            failure = "payload history flow position is absent";
            // Reading before a terminal cut does not execute that operation.
            // Its transfer is needed only when a real loop edge feeds this cut
            // again, or when the caller explicitly requested normal completion.
            let step = machine.flow.get(&current)?;
            if cut == PayloadMembershipCut::BeforeOperation
                && current == position
                && !step
                    .successors
                    .iter()
                    .enumerate()
                    .any(|(ordinal, _)| relevant.includes_edge(current, ordinal))
            {
                continue;
            }
            let mut before = snapshots.get(&current)?.clone();
            if let Some(definitions) = machine.payload_scalar_definitions.get(&current) {
                retire_scalar_instances(&mut before, definitions);
            }
            // Only derivation actuals read the pre-write payloads below. Keep
            // one immutable copy of each such root, not a second full state.
            // Selectors and fallible backing checks still run in their original
            // order, so simultaneous stores retain both snapshot semantics and
            // diagnostic precedence. Unused roots move directly into `after`.
            let mut source_values = BTreeMap::new();
            for (actual, _) in &step.derivations {
                if !before.unavailable_preservation.contains(&actual.root)
                    && let Some(contract) = before.values.get(&actual.root)
                {
                    source_values.entry(actual.root).or_insert_with(|| {
                        let mut contract = contract.clone();
                        extend_canonical_premises(
                            &mut contract.scalar_requirements,
                            &before.shared_requirements,
                        );
                        contract
                    });
                }
            }
            let before_unavailable =
                trace_preservation.then(|| before.unavailable_preservation.clone());
            let precise_writes = if machine.calls.contains_key(&current) {
                normal.as_deref_mut().and_then(|normal| {
                    normal.call_writes(analysis, identity, current)
                        .inspect_err(|reason| {
                            if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
                                eprintln!("CHECKSITE normal-payload-write-unavailable machine={identity:?} position={current:?} reason={reason}");
                            }
                        })
                        .ok()
                })
            } else {
                None
            };
            let precise_writes =
                precise_writes.map(|writes| writes.into_iter().map(Some).collect::<Vec<_>>());
            let normal_posts = if machine.calls.contains_key(&current) {
                normal.as_deref_mut().and_then(|normal| {
                    normal.call_posts(analysis, identity, current)
                        .inspect_err(|reason| {
                            if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
                                eprintln!("CHECKSITE normal-payload-post-unavailable machine={identity:?} position={current:?} reason={reason}");
                            }
                        }).ok()
                })
            } else {
                None
            };
            let public_facts = if memberships == PayloadMembershipKind::Observed
                && machine.calls.contains_key(&current)
            {
                normal.as_deref_mut().and_then(|normal|
                    normal.captured_provider_facts(analysis, identity, current, &before)
                        .inspect_err(|reason| {
                            if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
                                eprintln!("CHECKSITE public-normal-fact-unavailable machine={identity:?} position={current:?} reason={reason}");
                            }
                        }).ok())
            } else {
                None
            };
            let conserved_payloads = if machine.calls.contains_key(&current) {
                normal.as_deref_mut().and_then(|normal| {
                    normal.conserved_call_payloads(analysis, identity, current, &before)
                        .inspect_err(|reason| {
                            if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
                                eprintln!("CHECKSITE normal-call-conservation-unavailable machine={identity:?} position={current:?} reason={reason}");
                            }
                        }).ok()
                })
            } else {
                None
            };
            let completed_outputs = if memberships == PayloadMembershipKind::Preserved
                && machine.calls.contains_key(&current)
            {
                normal.as_deref_mut().and_then(|normal| {
                    normal.completed_call_outputs(analysis, identity, current, &before)
                        .inspect_err(|reason| {
                            if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
                                eprintln!("CHECKSITE completed-output-unavailable machine={identity:?} position={current:?} reason={reason}");
                            }
                        }).ok()
                })
            } else {
                None
            };
            let before_post = normal_posts.as_ref().map(|_| before.clone());
            let mut after = before;
            if precise_writes
                .as_ref()
                .or_else(|| machine.payload_invalidations.get(&current))
                .is_some_and(|rows| !rows.is_empty())
                || machine.unresolved_contents_writes.contains(&current)
                || step.derivations.iter().any(|(actual, destination)| {
                    machine.payload_producers.get(&current) != Some(&destination.root)
                        && !machine.storage_derivations.contains(&(
                            current,
                            actual.clone(),
                            destination.clone(),
                        ))
                })
            {
                // A later store cannot borrow facts from an old entry pointer
                // or an earlier symbolic write without a proved alias relation.
                after.invalidate_incoming_observations();
            }
            if machine.unresolved_contents_writes.contains(&current) {
                if memberships == PayloadMembershipKind::Preserved {
                    after.withhold_all_preservation();
                }
                clear(&mut after.values);
            }
            if let Some(writes) = precise_writes
                .as_ref()
                .or_else(|| machine.payload_invalidations.get(&current))
            {
                for query in writes {
                    let targets = query
                        .as_ref()
                        .and_then(|query| analysis.payload_targets(identity, current, query));
                    let primitive_write = memberships == PayloadMembershipKind::Preserved
                        && query.as_ref().is_some_and(|query| {
                            analysis
                                .unchanged_primitive_payload_write(identity, current, query)
                                .is_ok()
                        });
                    match targets {
                        Some(targets) => {
                            for target in targets {
                                if trace_observations {
                                    eprintln!(
                                        "CHECKSITE payload-observation-invalidation machine={identity:?} request={position:?} requested_root={root:?} at={current:?} query={query:?} target={target:?}"
                                    );
                                }
                                if trace_preservation {
                                    eprintln!(
                                        "CHECKSITE payload-preservation-invalidation machine={identity:?} request={position:?} requested_root={root:?} at={current:?} query={query:?} target={target:?}",
                                    );
                                }
                                match target {
                                    DescriptorPayloadTarget::Storage(location) => {
                                        // Keep an already complete empty duty
                                        // list through a proved scalar mutation.
                                        // Unknown or previously withheld lists
                                        // stay unknown; value facts still retire.
                                        let empty_roster = after
                                            .values
                                            .get(&location.root)
                                            .is_some_and(|contract| {
                                                contract.qualifications.is_empty()
                                                    && contract.projected_qualifications.is_empty()
                                            });
                                        if memberships == PayloadMembershipKind::Preserved
                                            && !(primitive_write && empty_roster)
                                        {
                                            after.withhold_preservation(location.root);
                                        }
                                        if let Some(contract) = after.values.get_mut(&location.root)
                                        {
                                            failure = "payload write path has no typed coordinate";
                                            let path = descriptor_path(
                                                types,
                                                contract.carrier,
                                                &location.path,
                                            )?;
                                            retire(contract, &path, &location.path);
                                        }
                                    }
                                    DescriptorPayloadTarget::Incoming(_) => {
                                        if memberships == PayloadMembershipKind::Preserved {
                                            after.withhold_all_preservation();
                                        }
                                        clear(&mut after.values)
                                    }
                                }
                            }
                        }
                        None => {
                            if trace_observations {
                                eprintln!(
                                    "CHECKSITE payload-observation-invalidation machine={identity:?} request={position:?} requested_root={root:?} at={current:?} query={query:?} target=unavailable"
                                );
                            }
                            if trace_preservation {
                                eprintln!(
                                    "CHECKSITE payload-preservation-invalidation machine={identity:?} request={position:?} requested_root={root:?} at={current:?} query={query:?} target=unavailable",
                                );
                            }
                            if memberships == PayloadMembershipKind::Preserved {
                                after.withhold_all_preservation();
                            }
                            clear(&mut after.values)
                        }
                    }
                }
            }
            // Snapshot all RHSs before applying any destination update. A repeated
            // or overlapping destination cannot acquire the last writer's theory.
            stores.clear();
            incoming_stores.clear();
            for (actual, destination) in &step.derivations {
                if machine.payload_producers.get(&current) == Some(&destination.root)
                    || machine.storage_derivations.contains(&(
                        current,
                        actual.clone(),
                        destination.clone(),
                    ))
                {
                    continue;
                }
                let mut incoming = source_values.get(&actual.root).and_then(|contract| {
                    select_payload_memberships_with_kind(
                        types,
                        domains,
                        contract,
                        &actual.path,
                        memberships,
                    )
                });
                failure = "structural store backing is unresolved";
                let destinations = if machine.payload_stores.contains_key(&current) {
                    // The store's receiving path selects the variable being
                    // replaced, independently of its old pointer value. A
                    // failed typed selection cannot fall back to that pointee.
                    let (targets, carrier) = match analysis.unchanged_payload_store_targets(
                        identity,
                        current,
                        actual,
                        destination,
                    ) {
                        Ok(target) => target,
                        Err(reason) => {
                            failure = reason;
                            return None;
                        }
                    };
                    if let [DescriptorPayloadTarget::Incoming(query)] = targets.as_slice() {
                        // Current observations can describe a symbolic write;
                        // they cannot settle the caller's preservation debt.
                        if memberships == PayloadMembershipKind::Preserved {
                            return None;
                        }
                        clear(&mut after.values);
                        if let Some(incoming) = incoming {
                            failure = "incoming store carrier does not match its actual source";
                            incoming_stores.push((
                                query.clone(),
                                incoming.presented(types, domains, carrier)?,
                            ));
                        }
                        continue;
                    }
                    let mut destinations = Vec::with_capacity(targets.len());
                    for target in targets {
                        let DescriptorPayloadTarget::Storage(target) = target else {
                            failure = "a payload store mixes local and incoming targets";
                            return None;
                        };
                        destinations.push(target);
                    }
                    destinations
                } else {
                    // Synthetic transfers have no operation-owned destination
                    // query. Keep their existing physical-only judgment.
                    analysis.storage_origins(identity, current, destination)?
                };
                let strong = destinations.len() == 1;
                let mut destinations = destinations.into_iter().peekable();
                while let Some(destination) = destinations.next() {
                    // Earlier aliases need independent snapshots; the final
                    // destination owns the already selected contract directly.
                    let selected = if destinations.peek().is_some() {
                        incoming.clone()
                    } else {
                        incoming.take()
                    };
                    stores.push((destination, selected, strong));
                }
            }
            // Decide simultaneous overlap before consuming the snapshots. The
            // exact-carrier path can then move its theory without cloning it.
            for index in 0..stores.len() {
                let destination = &stores[index].0;
                if stores.iter().enumerate().any(|(other, (candidate, _, _))| {
                    index != other
                        && candidate.root == destination.root
                        && (candidate.path.starts_with(&destination.path)
                            || destination.path.starts_with(&candidate.path))
                }) {
                    stores[index].2 = false;
                }
            }
            for (destination, incoming, strong) in stores.drain(..) {
                if memberships == PayloadMembershipKind::Preserved
                    && after.values.get(&destination.root).is_none()
                    && strong
                    && destination.path.is_empty()
                    && let Some(incoming) = incoming.as_ref()
                {
                    let carrier = *machine.place_types.get(&destination.root)?;
                    let restored = incoming.clone().presented(types, domains, carrier)?;
                    after.values.insert(destination.root, restored);
                    after
                        .unavailable_preservation
                        .retain(|root| *root != destination.root);
                    continue;
                }
                let Some(contract) = after.values.get_mut(&destination.root) else {
                    continue;
                };
                failure = "structural store path has no typed coordinate";
                let path = descriptor_path(types, contract.carrier, &destination.path)?;
                let loses_ancestor_duty = memberships == PayloadMembershipKind::Preserved
                    && !path.is_empty()
                    && (!contract.qualifications.is_empty()
                        || contract
                            .projected_qualifications
                            .iter()
                            .any(|row| row.path.len() < path.len() && path.starts_with(&row.path)));
                retire(contract, &path, &destination.path);
                if !strong {
                    if memberships == PayloadMembershipKind::Preserved {
                        after.withhold_preservation(destination.root);
                    }
                    continue;
                }
                let Some(incoming) = incoming else {
                    if memberships == PayloadMembershipKind::Preserved {
                        after.withhold_preservation(destination.root);
                    }
                    continue;
                };
                failure = "structural store carrier does not match its actual source";
                let destination_carrier =
                    qualification_path_carrier(types.iter(), contract.carrier, &path)?;
                let incoming = incoming.presented(types, domains, destination_carrier)?;
                if path.is_empty() {
                    *contract = incoming;
                    after
                        .unavailable_preservation
                        .retain(|root| *root != destination.root);
                } else {
                    append_scalar_observations(contract, &incoming, &destination.path);
                    terminal_psi::StructuralPathQualification::append_under(
                        &incoming.qualifications,
                        &incoming.projected_qualifications,
                        &path,
                        &mut contract.projected_qualifications,
                    );
                }
                if loses_ancestor_duty {
                    after.withhold_preservation(destination.root);
                }
            }
            for (query, contract) in incoming_stores.drain(..) {
                after
                    .values
                    .insert_subject(PayloadSubject::Incoming(query), contract);
            }
            if let Some(producer) = machine.payload_producers.get(&current) {
                failure = "established payload contract is absent";
                let mut produced = source(*producer)?;
                // Source callbacks need not order their premises. Normalize at
                // this boundary once; child composition and stores preserve it.
                produced.scalar_requirements.sort();
                produced.scalar_requirements.dedup();
                // Constructors and copies snapshot their actual children before
                // consuming them. A result annotation supplies no scalar content.
                let mut complete_children = memberships != PayloadMembershipKind::Preserved
                    || !machine.opaque_payload_results.contains(&current);
                for (actual, destination) in &step.derivations {
                    if destination.root != *producer {
                        continue;
                    }
                    if machine.storage_derivations.contains(&(
                        current,
                        actual.clone(),
                        destination.clone(),
                    )) {
                        if memberships == PayloadMembershipKind::Preserved {
                            // A new shared wrapper keeps every actual source
                            // duty, even when its annotation promises fewer.
                            // Unknown duties cannot become empty by borrowing.
                            let captured = source_values.get(&actual.root).and_then(|source| {
                                select_payload_memberships_with_kind(
                                    types,
                                    domains,
                                    source,
                                    &actual.path,
                                    memberships,
                                )
                            });
                            let captured = captured.and_then(|source| {
                                let StructuralTypeShape::Reference { referent, .. } =
                                    terminal_psi::find_structural_type(types, produced.carrier)?
                                        .shape
                                else {
                                    return None;
                                };
                                if machine
                                    .projections
                                    .get(&(current, actual.clone(), destination.clone()))
                                    .is_some_and(|projection| *projection != Projection::new(&[]))
                                {
                                    return None;
                                }
                                source.presented(types, domains, referent)
                            });
                            if let Some(captured) = captured {
                                if matches!(
                                    terminal_psi::find_structural_type(types, produced.carrier)
                                        .map(|declaration| &declaration.shape),
                                    Some(StructuralTypeShape::Reference {
                                        access: StructuralAccess::SharedBorrow,
                                        ..
                                    })
                                ) {
                                    terminal_psi::StructuralPathQualification::append_under(
                                        &captured.qualifications,
                                        &captured.projected_qualifications,
                                        &[StructuralPathSegment::Referent],
                                        &mut produced.projected_qualifications,
                                    );
                                    produced.projected_qualifications.sort();
                                    produced.projected_qualifications.dedup();
                                }
                            } else {
                                complete_children = false;
                            }
                        }
                        continue;
                    }
                    let incomplete = |reason| {
                        if trace_preservation {
                            eprintln!(
                                "CHECKSITE payload-preservation-child-unavailable machine={identity:?} request={position:?} at={current:?} producer={producer:?} actual={actual:?} destination={destination:?} reason={reason} source={:?}",
                                source_values.get(&actual.root),
                            );
                        }
                    };
                    let Some(source) = source_values.get(&actual.root) else {
                        incomplete("source-unavailable");
                        complete_children = false;
                        continue;
                    };
                    let Some(incoming) = select_payload_memberships_with_kind(
                        types,
                        domains,
                        source,
                        &actual.path,
                        memberships,
                    ) else {
                        incomplete("selected-child-unavailable");
                        complete_children = false;
                        continue;
                    };
                    let path = descriptor_path(types, produced.carrier, &destination.path)?;
                    let destination_carrier =
                        qualification_path_carrier(types.iter(), produced.carrier, &path)?;
                    let Some(incoming) = incoming.presented(types, domains, destination_carrier)
                    else {
                        incomplete("presented-child-unavailable");
                        complete_children = false;
                        continue;
                    };
                    append_scalar_observations(&mut produced, &incoming, &destination.path);
                    if path.is_empty() {
                        produced.qualifications.extend(&incoming.qualifications);
                        produced
                            .projected_qualifications
                            .extend(incoming.projected_qualifications.clone());
                    } else {
                        terminal_psi::StructuralPathQualification::append_under(
                            &incoming.qualifications,
                            &incoming.projected_qualifications,
                            &path,
                            &mut produced.projected_qualifications,
                        );
                    }
                    produced.qualifications.sort();
                    produced.qualifications.dedup();
                    produced.projected_qualifications.sort();
                    produced.projected_qualifications.dedup();
                }
                // No later suffix operation reads this raw producer's
                // premises. Scalar writes/reads inspect observations only;
                // publication below attaches the current ambient theory.
                after.values.insert(*producer, produced);
                after
                    .unavailable_preservation
                    .retain(|root| root != producer);
                if memberships == PayloadMembershipKind::Preserved && !complete_children {
                    after.withhold_preservation(*producer);
                }
            }
            drop(source_values);
            if let Some(writes) = machine.scalar_payload_writes.get(&current) {
                for (destination, value) in writes {
                    // New result storage has no pre-operation backing. Every
                    // other observation, including a borrowed call actual,
                    // must resolve to one exact current storage coordinate.
                    let resolved;
                    let destination =
                        if machine.payload_producers.get(&current) == Some(&destination.root) {
                            destination
                        } else {
                            let Some(destinations) =
                                analysis.storage_origins(identity, current, destination)
                            else {
                                continue;
                            };
                            if destinations.len() != 1 {
                                continue;
                            }
                            resolved = destinations.into_iter().next()?;
                            &resolved
                        };
                    let Some(contract) = after.values.get_mut(&destination.root) else {
                        continue;
                    };
                    if observation_type(types, contract.carrier, &destination.path)
                        != Some(value.scalar_type())
                    {
                        continue;
                    }
                    contract.scalar_observations.retain(|observation| {
                        !observation.path.starts_with(&destination.path)
                            && !destination.path.starts_with(&observation.path)
                    });
                    contract.scalar_observations.push(PayloadScalarObservation {
                        path: destination.path.clone(),
                        value: Some(value.clone()),
                        entry: None,
                    });
                }
            }
            if let Some(reads) = machine.scalar_payload_reads.get(&current) {
                for (location, value) in reads {
                    if let Some(contract) = after.values.get_mut(&location.root)
                        && let Some(equality) = observe_scalar(contract, &location.path, value)
                    {
                        Arc::make_mut(&mut after.scalar_requirements)
                            .push(premises.intern(equality));
                    }
                }
            }
            if let Some(requirements) = machine.payload_scalar_postconditions.get(&current)
                && !requirements.is_empty()
            {
                Arc::make_mut(&mut after.scalar_requirements).extend(
                    requirements
                        .iter()
                        .map(|premise| premises.intern_ref(premise)),
                );
            }
            if let (Some(proofs), Some(before)) = (normal_posts, before_post) {
                for proof in proofs {
                    proof.restore(types, domains, &before, &mut after);
                }
            }
            if let Some(proofs) = conserved_payloads {
                for proof in proofs {
                    proof.restore(&mut after);
                }
            }
            if let Some(facts) = public_facts {
                for fact in facts {
                    fact.publish(types, &mut after);
                }
            }
            if let Some(outputs) = completed_outputs {
                for output in outputs {
                    output.publish(&mut after);
                }
            }
            // Reads can add equations even when this step has no postcondition.
            // Normalize at publication, after all local append operations.
            canonicalize_snapshot(&mut after.scalar_requirements);
            after.publish_requirements();
            for contract in after.values.values_mut() {
                if memberships == PayloadMembershipKind::Preserved
                    || !payload_predicates::closure_may_change(types, domains, contract)
                {
                    continue;
                }
                payload_predicates::close_with_shared_requirements(
                    types,
                    domains,
                    Arc::make_mut(contract),
                    &after.shared_requirements,
                    None,
                );
            }
            if let Some(before) = before_unavailable {
                let withheld = after
                    .unavailable_preservation
                    .iter()
                    .filter(|root| !before.contains(root))
                    .collect::<Vec<_>>();
                let restored = before
                    .iter()
                    .filter(|root| !after.unavailable_preservation.contains(root))
                    .collect::<Vec<_>>();
                if !withheld.is_empty() || !restored.is_empty() {
                    eprintln!(
                        "CHECKSITE payload-preservation-step machine={identity:?} request={position:?} requested_root={root:?} at={current:?} withheld={withheld:?} restored={restored:?} invalidations={:?} derivations={:?} producer={:?}",
                        machine.payload_invalidations.get(&current),
                        step.derivations,
                        machine.payload_producers.get(&current),
                    );
                }
            }
            if cut == PayloadMembershipCut::NormalCompletion && current == position {
                // The worklist may revisit a loop cut. Replace this observation
                // only after all incoming rows have met and this transfer has
                // run again; the final value is the fixed-point completion.
                normal_completion = Some((position, after.clone()));
            }
            // Only distinct arrivals need independent state. Move the completed
            // transfer into the last relevant edge instead of cloning every
            // payload's full scalar theory even on a straight-line step.
            let mut successors = step
                .successors
                .iter()
                .enumerate()
                .filter(|(ordinal, edge)| {
                    relevant.includes_edge(current, *ordinal)
                        && !previous_relevant
                            .as_ref()
                            .is_some_and(|previous| previous.contains(edge.target))
                })
                .peekable();
            while let Some((ordinal, edge)) = successors.next() {
                let mut arrival = if successors.peek().is_some() {
                    after.clone()
                } else {
                    std::mem::take(&mut after)
                };
                let edge_predicate = machine
                    .payload_edge_predicates
                    .get(&current)
                    .and_then(|rows| rows.get(ordinal))
                    .and_then(Option::as_ref);
                if let Some(predicate) = edge_predicate {
                    Arc::make_mut(&mut arrival.scalar_requirements).extend(
                        super::scalar_contracts::scalar_conjuncts(std::slice::from_ref(predicate))
                            .into_iter()
                            .map(|premise| premises.intern(premise)),
                    );
                    canonicalize_snapshot(&mut arrival.scalar_requirements);
                    for contract in arrival.values.values_mut() {
                        if memberships == PayloadMembershipKind::Preserved
                            || !payload_predicates::closure_may_change(types, domains, contract)
                        {
                            continue;
                        }
                        payload_predicates::close_with_shared_requirements(
                            types,
                            domains,
                            Arc::make_mut(contract),
                            &arrival.shared_requirements,
                            Some(predicate),
                        );
                    }
                }
                // Snapshot only actual binding results before retiring any
                // formal. Loop edges can read and rebind the same root, so the
                // entire binding batch must observe the pre-arrival state.
                // The other payloads need no second full-state snapshot.
                bindings.clear();
                for (actual, parameter) in edge.derivations() {
                    failure = "edge formal is not a whole payload root";
                    if !parameter.path.is_empty() {
                        return None;
                    }
                    if let Some(contract) =
                        arrival
                            .contract(actual.root, edge_predicate)
                            .and_then(|contract| {
                                select_payload_memberships_with_kind(
                                    types,
                                    domains,
                                    &contract,
                                    &actual.path,
                                    memberships,
                                )
                            })
                    {
                        // Exact descriptor bindings also carry reference access
                        // conversions. Memberships are carrier-specific: an
                        // incompatible row supplies no evidence for that formal,
                        // but cannot invalidate other payloads on the edge.
                        let Some(contract) = contract.presented(
                            types,
                            domains,
                            *machine.place_types.get(&parameter.root)?,
                        ) else {
                            continue;
                        };
                        bindings.push((parameter.root, contract));
                    }
                }
                // A selected contents family supplies only facts justified by
                // every candidate; its shared typed row never claims an address.
                for binding in &edge.bindings {
                    let crate::descriptors::descriptor_lifetimes::DescriptorFlowSource::Contents(
                        query,
                    ) = &binding.source
                    else {
                        continue;
                    };
                    let contract = analysis
                        .arrival_payload_targets(identity, current, query)
                        .and_then(|targets| {
                            intersect_payload_memberships_with_kind(
                                types,
                                domains,
                                targets,
                                |target| {
                                    arrival.selected_contract(
                                        types,
                                        domains,
                                        target,
                                        memberships,
                                        edge_predicate,
                                    )
                                },
                                memberships,
                            )
                        });
                    if let Some(contract) = contract {
                        failure = "selected edge actual and formal payload carriers differ";
                        let contract = contract.presented(
                            types,
                            domains,
                            *machine.place_types.get(&binding.parameter.root)?,
                        )?;
                        bindings.push((binding.parameter.root, contract));
                    }
                }
                // Readers above see exactly the old ordered union plus its
                // unsorted predicate tail. Retain only that tail in old locals,
                // before replacing formals, so subsequent retirement still
                // removes old instances without copying the shared theory into
                // every unrelated payload. Captured RHS contracts already own
                // their complete pre-rebinding theory.
                if let Some(predicate) = edge_predicate {
                    for contract in arrival.values.values_mut() {
                        if !contract.scalar_requirements.contains(predicate)
                            && !arrival
                                .shared_requirements
                                .iter()
                                .any(|premise| premise.as_ref() == predicate)
                        {
                            Arc::make_mut(contract)
                                .scalar_requirements
                                .push(predicate.clone());
                        }
                    }
                }
                for parameter in &edge.parameters {
                    arrival.values.remove(&parameter.root);
                }
                for (parameter, contract) in bindings.drain(..) {
                    // All RHS readers have finished. The rebound formals are
                    // retired and published before another premise read.
                    arrival.values.insert(parameter, contract);
                    arrival
                        .unavailable_preservation
                        .retain(|root| *root != parameter);
                }
                // Only an actual block arrival rebinds its formals. Ordinary
                // operation steps within the block keep that same SSA instance.
                if edge.target.1 == 0
                    && let Some(parameters) = machine.payload_scalar_parameters.get(&edge.target.0)
                {
                    let incoming_requirements = arrival.scalar_requirements.clone();
                    retire_scalar_instances(&mut arrival, parameters);
                    if let Some(bindings) = machine
                        .payload_scalar_bindings
                        .get(&current)
                        .and_then(|edges| edges.get(ordinal))
                    {
                        let rebound = premises.arrival_requirements(
                            &incoming_requirements,
                            bindings,
                            parameters,
                        );
                        // Rebinding can return the complete incoming theory
                        // unchanged. Appending it would detach the snapshot
                        // only to deduplicate it immediately below.
                        if !contains_rebound_premises(&arrival.scalar_requirements, &rebound) {
                            Arc::make_mut(&mut arrival.scalar_requirements).extend(rebound);
                        }
                        canonicalize_snapshot(&mut arrival.scalar_requirements);
                    }
                }
                for contract in arrival.values.values_mut() {
                    // Only the edge predicate can append to a payload roster
                    // since the canonical post-step publication. Selection,
                    // presentation, intersection and retirement preserve its
                    // order. Keep predicate normalization after the existing
                    // intermediate closure checks, but do not rescan unchanged
                    // rosters on ordinary edges.
                    if edge_predicate.is_some()
                        && contract
                            .scalar_requirements
                            .windows(2)
                            .any(|pair| pair[0] >= pair[1])
                    {
                        let contract = Arc::make_mut(contract);
                        contract.scalar_requirements.sort();
                        contract.scalar_requirements.dedup();
                    }
                }
                arrival.publish_requirements();
                for contract in arrival.values.values_mut() {
                    if memberships == PayloadMembershipKind::Preserved
                        || !payload_predicates::closure_may_change(types, domains, contract)
                    {
                        continue;
                    }
                    payload_predicates::close_with_shared_requirements(
                        types,
                        domains,
                        Arc::make_mut(contract),
                        &arrival.shared_requirements,
                        None,
                    );
                }
                let changed = if let Some(snapshot) = snapshots.get_mut(&edge.target) {
                    let trace = trace_preservation.then(|| (snapshot.clone(), arrival.clone()));
                    let changed = snapshot.meet(arrival, memberships);
                    if let Some((previous, incoming)) = trace {
                        for root in &snapshot.unavailable_preservation {
                            if !previous.unavailable_preservation.contains(root) {
                                eprintln!(
                                    "CHECKSITE payload-preservation-meet machine={identity:?} request={position:?} root={root:?} from={current:?} target={:?} previous_status={} incoming_status={} previous={:?} incoming={:?}",
                                    edge.target,
                                    previous.preservation_status(*root),
                                    incoming.preservation_status(*root),
                                    previous.values.get(root),
                                    incoming.values.get(root),
                                );
                            }
                        }
                    }
                    changed
                } else {
                    snapshots.insert(edge.target, arrival);
                    true
                };
                if changed && !pending.contains(&edge.target) {
                    pending.push_back(edge.target);
                }
            }
        }
        if let Some(previous) = previous_relevant {
            relevant.merge(previous);
        }
        replay.cached = Some(CachedHistory {
            relevant,
            snapshots,
            normal_completion,
        });
        failure = "requested payload root has no reached incoming membership history";
        let at = match cut {
            PayloadMembershipCut::BeforeOperation => replay
                .cached
                .as_ref()
                .and_then(|history| history.snapshots.get(&position)),
            PayloadMembershipCut::NormalCompletion => replay
                .cached
                .as_ref()
                .and_then(|history| history.normal_completion.as_ref())
                .filter(|(at, _)| *at == position)
                .map(|(_, state)| state),
        };
        if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_some() {
            let status = at.map_or("no-snapshot", |state| state.preservation_status(root));
            if status != "present" {
                eprintln!(
                    "CHECKSITE payload-history-unavailable machine={identity:?} position={position:?} root={root:?} cut={} status={status} type={:?} producers={:?}",
                    match cut {
                        PayloadMembershipCut::BeforeOperation => "before-operation",
                        PayloadMembershipCut::NormalCompletion => "normal-completion",
                    },
                    machine.place_types.get(&root),
                    machine
                        .payload_producers
                        .iter()
                        .filter_map(|(position, producer)| (*producer == root).then_some(*position))
                        .collect::<Vec<_>>(),
                );
            }
        }
        let at = at?;
        if let PayloadSubject::Incoming(query) = &subject {
            failure = "current incoming payload has no retained membership history";
            return at.selected_contract(
                types,
                domains,
                DescriptorPayloadTarget::Incoming(query.clone()),
                memberships,
                None,
            );
        }
        let mut current = at.contract(root, None)?;
        if cut == PayloadMembershipCut::NormalCompletion {
            // The completed state already applied writes and closed its actual
            // postconditions. Do not re-seed it from an earlier declaration.
            return Some(current);
        }
        failure = "requested source has no declared payload contract";
        let declared = source(root)?;
        failure = "current and declared source payload carriers differ";
        if current.carrier != declared.carrier {
            return None;
        }
        // Pure byte predicates in an actual source's retained contract are
        // standing validity obligations. Independent arrival, primitive-write,
        // replacement and writable-call checks preserve that floor. History owns
        // additional transient/routed memberships; the receiving copy contributes
        // none. This query must never discharge those independent checks itself.
        let byte_domain = |carrier, domain| {
            crate::byte_predicate_carrier(types.iter(), carrier)
                && crate::byte_qualification_predicates_in(domains.iter(), carrier, &[domain])
                    .is_some()
        };
        current.qualifications.extend(
            declared
                .qualifications
                .into_iter()
                .filter(|domain| byte_domain(declared.carrier, *domain)),
        );
        current.projected_qualifications.extend(
            declared.projected_qualifications.into_iter().filter(|row| {
                qualification_path_carrier(types.iter(), declared.carrier, &row.path)
                    .is_some_and(|carrier| byte_domain(carrier, row.domain))
            }),
        );
        current.qualifications.sort();
        current.qualifications.dedup();
        current.projected_qualifications.sort();
        current.projected_qualifications.dedup();
        Some(current)
    })();
    result.ok_or(failure)
}

/// Rebinding usually preserves the incoming roster's order. Check that order
/// before testing its complete theory by a single ordered scan. Normalization
/// can reorder IEEE/math propositions, so unordered results retain the exact
/// binary-search path. Equal duplicates reuse one retained row; no input moves.
fn contains_rebound_premises(retained: &[SharedPremise], rebound: &[SharedPremise]) -> bool {
    if rebound.windows(2).any(|pair| pair[0] > pair[1]) {
        return rebound
            .iter()
            .all(|premise| retained.binary_search(premise).is_ok());
    }
    let mut remaining = retained.iter().peekable();
    rebound
        .iter()
        .all(|premise| next_premise_is(&mut remaining, premise))
}

/// Advance one sorted premise cursor to `premise`, reporting whether it lands on
/// it. Rows ordered before `premise` occur in neither roster; the cursor moves
/// forward only, so a shared-prefix join needs one pass per caller.
fn next_premise_is<'a, Premise: PartialOrd + 'a>(
    remaining: &mut std::iter::Peekable<impl Iterator<Item = &'a Premise>>,
    premise: &Premise,
) -> bool {
    while matches!(remaining.peek(), Some(candidate) if *candidate < premise) {
        remaining.next();
    }
    matches!(remaining.peek(), Some(candidate) if *candidate == premise)
}

/// Union two sorted, unique premise rosters without cloning and then discarding
/// every ambient premise already present in each live payload. Callers normalize
/// newly produced or rebound contracts before this operation; retained snapshots
/// already satisfy that invariant. Preserve the existing proposition on equality,
/// just as stable append/sort/dedup did. No new proposition is established here.
pub(super) fn extend_canonical_premises<Additional: Borrow<semantic_vocabulary::Proposition>>(
    retained: &mut Vec<semantic_vocabulary::Proposition>,
    additional: &[Additional],
) {
    let mut existing = retained.iter().peekable();
    let introduces_premise = additional
        .iter()
        .map(Borrow::<semantic_vocabulary::Proposition>::borrow)
        .any(|premise| !next_premise_is(&mut existing, premise));
    if !introduces_premise {
        return;
    }
    let mut combined = Vec::with_capacity(retained.len() + additional.len());
    let mut existing = std::mem::take(retained).into_iter().peekable();
    let mut additional = additional
        .iter()
        .map(Borrow::<semantic_vocabulary::Proposition>::borrow)
        .peekable();
    while let (Some(left), Some(right)) = (existing.peek(), additional.peek()) {
        match left.cmp(right) {
            std::cmp::Ordering::Less => combined.extend(existing.next()),
            std::cmp::Ordering::Greater => combined.extend(additional.next().cloned()),
            std::cmp::Ordering::Equal => {
                combined.extend(existing.next());
                additional.next();
            }
        }
    }
    combined.extend(existing);
    combined.extend(additional.cloned());
    *retained = combined;
}

/// Intersect canonical premise rosters without searching the whole arrival for
/// every retained premise. The caller publishes sorted, unique snapshots.
fn retain_common_snapshot(
    retained: &mut Arc<Vec<SharedPremise>>,
    incoming: &[SharedPremise],
) -> bool {
    let mut remaining = incoming.iter().peekable();
    let unchanged = retained
        .iter()
        .all(|premise| next_premise_is(&mut remaining, premise));
    if unchanged {
        return false;
    }
    let mut remaining = incoming.iter().peekable();
    Arc::make_mut(retained).retain(|premise| next_premise_is(&mut remaining, premise));
    true
}

/// Preserve a published allocation when normalization would change no row.
/// The check uses the same complete proposition ordering as sort/dedup.
fn canonicalize_snapshot(premises: &mut Arc<Vec<SharedPremise>>) {
    if premises.windows(2).any(|pair| pair[0] >= pair[1]) {
        let premises = Arc::make_mut(premises);
        premises.sort();
        premises.dedup();
    }
}

fn retain_common_premises<
    Retained: Borrow<semantic_vocabulary::Proposition>,
    Incoming: Borrow<semantic_vocabulary::Proposition>,
>(
    retained: &mut Vec<Retained>,
    incoming: &[Incoming],
) -> bool {
    let previous = retained.len();
    let mut remaining = incoming
        .iter()
        .map(Borrow::<semantic_vocabulary::Proposition>::borrow)
        .peekable();
    retained.retain(|premise| {
        let premise: &semantic_vocabulary::Proposition = premise.borrow();
        next_premise_is(&mut remaining, premise)
    });
    retained.len() != previous
}

/// Intersect an owned incoming roster with a snapshot's disjoint local and
/// common rosters, without expanding that common theory into every payload.
/// Equal propositions have the same complete value, including scalar identity.
fn retain_captured_premises<
    Local: Borrow<semantic_vocabulary::Proposition>,
    Common: Borrow<semantic_vocabulary::Proposition>,
>(
    incoming: &mut Vec<semantic_vocabulary::Proposition>,
    local: &[Local],
    common: &[Common],
) {
    let mut local = local
        .iter()
        .map(Borrow::<semantic_vocabulary::Proposition>::borrow)
        .peekable();
    let mut common = common
        .iter()
        .map(Borrow::<semantic_vocabulary::Proposition>::borrow)
        .peekable();
    incoming.retain(|premise| {
        while local.peek().is_some_and(|candidate| *candidate < premise) {
            local.next();
        }
        while common.peek().is_some_and(|candidate| *candidate < premise) {
            common.next();
        }
        local.peek().is_some_and(|candidate| *candidate == premise)
            || common.peek().is_some_and(|candidate| *candidate == premise)
    });
}

/// A large qualified aggregate can carry thousands of projected memberships.
/// Searching its complete incoming roster for every retained row makes every
/// join quadratic. Source callbacks need not publish sorted rosters, so sort
/// borrowed references once and leave both contracts' order and duplicates
/// untouched. This computes exactly the old membership filter, not a stronger
/// meet or a new canonicalization requirement on callers.
fn retain_common_memberships<Membership: Ord>(
    retained: &mut Vec<Membership>,
    incoming: &[Membership],
) {
    if retained.is_empty() {
        return;
    }
    if incoming.is_empty() {
        retained.clear();
        return;
    }
    // This operation only filters by membership; it never normalizes the
    // retained order or duplicates. Identical rosters therefore need no work,
    // including raw unsorted inputs (unlike scalar-observation intersection).
    if retained.as_slice() == incoming {
        return;
    }
    let mut incoming_memberships: Vec<_> = incoming.iter().collect();
    incoming_memberships.sort_unstable();
    retained.retain(|membership| incoming_memberships.binary_search(&membership).is_ok());
}

fn retire(
    contract: &mut PayloadMembershipContract,
    path: &[StructuralPathSegment],
    scalar_path: &[crate::descriptors::descriptor_lifetimes::DescriptorPathSegment],
) {
    contract.scalar_observations.retain(|observation| {
        !observation.path.starts_with(scalar_path) && !scalar_path.starts_with(&observation.path)
    });
    contract.qualifications.clear();
    contract
        .projected_qualifications
        .retain(|row| !crate::structural_paths_may_overlap(&row.path, path));
}

/// An unchanged location still contains its earlier observed value. Retain
/// that common observation across branch-local rereads and connect the new SSA
/// result to it. Writes and dynamic-instance retirement remove observations
/// before this operation, so neither can resurrect a stale field equation.
fn observe_scalar(
    contract: &mut PayloadMembershipContract,
    path: &[crate::descriptors::descriptor_lifetimes::DescriptorPathSegment],
    value: &semantic_vocabulary::ScalarTerm,
) -> Option<semantic_vocabulary::Proposition> {
    if let Some(previous) = contract.scalar_observations.iter_mut().find(|observation| {
        observation.path == path
            && matches!(
                value.scalar_type(),
                semantic_vocabulary::ScalarType::Integer(_)
                    | semantic_vocabulary::ScalarType::Boolean
            )
    }) {
        if let Some(observed) = &previous.value {
            if observed.scalar_type() == value.scalar_type() {
                return (observed != value).then(|| {
                    semantic_vocabulary::Proposition::Equal(value.clone(), observed.clone())
                });
            }
        } else if previous
            .entry
            .as_ref()
            .is_some_and(|entry| entry.scalar_type == value.scalar_type())
        {
            previous.value = Some(value.clone());
            return None;
        }
    }
    contract
        .scalar_observations
        .retain(|observation| observation.path != path);
    contract.scalar_observations.push(PayloadScalarObservation {
        path: path.to_vec(),
        value: Some(value.clone()),
        entry: None,
    });
    None
}

fn clear(state: &mut PayloadContracts) {
    for contract in state.values_mut() {
        if contract.qualifications.is_empty()
            && contract.projected_qualifications.is_empty()
            && contract.scalar_observations.is_empty()
        {
            continue;
        }
        let contract = Arc::make_mut(contract);
        contract.qualifications.clear();
        contract.projected_qualifications.clear();
        contract.scalar_observations.clear();
    }
}

fn append_scalar_observations(
    destination: &mut PayloadMembershipContract,
    source: &PayloadMembershipContract,
    prefix: &[crate::descriptors::descriptor_lifetimes::DescriptorPathSegment],
) {
    // Both sides have canonical premises: actual selection and presentation
    // preserve snapshots, and a fresh destination is normalized at its source
    // callback boundary. Composition needs neither repeated normalization nor
    // a quadratic per-premise membership search.
    extend_canonical_premises(
        &mut destination.scalar_requirements,
        &source.scalar_requirements,
    );
    for observation in &source.scalar_observations {
        let mut path = prefix.to_vec();
        path.extend_from_slice(&observation.path);
        destination
            .scalar_observations
            .retain(|previous| previous.path != path);
        destination
            .scalar_observations
            .push(PayloadScalarObservation {
                path,
                value: observation.value.clone(),
                entry: observation.entry.clone(),
            });
    }
}

/// SSA names acquire new dynamic instances on repeated execution. Completed
/// payload memberships survive; observations and premises naming old instances
/// must not classify newly read values or rebound block formals.
fn retire_scalar_instances(state: &mut State, definitions: &[semantic_vocabulary::ValueId]) {
    if definitions.is_empty() {
        return;
    }
    if state
        .scalar_requirements
        .iter()
        .any(|premise| premise.mentions_any(definitions))
    {
        Arc::make_mut(&mut state.scalar_requirements)
            .retain(|premise| !premise.mentions_any(definitions));
    }
    if state
        .shared_requirements
        .iter()
        .any(|premise| !premise.survives_payload_retirement(definitions))
    {
        Arc::make_mut(&mut state.shared_requirements)
            .retain(|premise| premise.survives_payload_retirement(definitions));
    }
    for contract in state.values.values_mut() {
        // Match both mutation cases below, including already-empty observations
        // and opaque premises (whose retirement differs from ambient premises).
        if !contract.scalar_observations.iter().any(|observation| {
            (observation.value.is_none() && observation.entry.is_none())
                || observation
                    .value
                    .as_ref()
                    .is_some_and(|value| value.any_value_id(|value| definitions.contains(&value)))
        }) && !contract
            .scalar_requirements
            .iter()
            .any(|premise| !survives_payload_retirement(premise, definitions))
        {
            continue;
        }
        let contract = Arc::make_mut(contract);
        contract.scalar_observations.retain_mut(|observation| {
            if observation
                .value
                .as_ref()
                .is_some_and(|value| value.any_value_id(|value| definitions.contains(&value)))
            {
                observation.value = None;
            }
            observation.value.is_some() || observation.entry.is_some()
        });
        retire_payload_premises(&mut contract.scalar_requirements, definitions);
    }
}

fn retire_payload_premises<Premise: Borrow<semantic_vocabulary::Proposition>>(
    premises: &mut Vec<Premise>,
    definitions: &[semantic_vocabulary::ValueId],
) {
    // Payloads also forget unknown/unclosed premises. The ambient filter is
    // deliberately different; applying it here would preserve stale theory.
    premises.retain(|premise| survives_payload_retirement(premise.borrow(), definitions));
}

fn survives_payload_retirement(
    premise: &semantic_vocabulary::Proposition,
    definitions: &[semantic_vocabulary::ValueId],
) -> bool {
    let mut changed = false;
    let closed = premise.visit_value_ids(|value| {
        changed |= definitions.contains(&value);
    });
    closed && !changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptors::descriptor_lifetimes::{
        DescriptorFlowBinding, DescriptorFlowEdge, DescriptorFlowSource, DescriptorPathSegment,
    };
    use semantic_vocabulary::{
        BlockId, IntegerSign, IntegerType, Proposition, ScalarTerm, ScalarType, ValueId,
    };

    fn position(block: u64) -> DescriptorPosition {
        (BlockId::new(block).unwrap(), 0)
    }

    fn condition(value: u64) -> Proposition {
        Proposition::Equal(
            ScalarTerm::Boolean(true),
            ScalarTerm::value(ValueId::new(value).unwrap(), ScalarType::Boolean),
        )
    }

    fn forked_history(branches: &[u64]) -> (DescriptorMachine, PayloadMembershipContract) {
        let root = PlaceId::new(1).unwrap();
        let carrier = StructuralTypeId::new(1).unwrap();
        let step = |targets: &[u64]| DescriptorFlowStep {
            successors: targets
                .iter()
                .map(|target| DescriptorFlowEdge {
                    target: position(*target),
                    parameters: Vec::new(),
                    bindings: Vec::new(),
                })
                .collect(),
            ..Default::default()
        };
        let machine = DescriptorMachine {
            id: MachineId::new(1).unwrap(),
            returns: None,
            places: vec![DescriptorPlace {
                place: root,
                definition: DescriptorDefinition::Parameter {
                    position: 0,
                    access: StructuralAccess::Owned,
                    structural_type: carrier,
                },
            }],
            place_types: BTreeMap::from([(root, carrier)]),
            entry: position(1),
            flow: BTreeMap::from([
                (position(1), step(branches)),
                (position(2), step(&[4])),
                (position(3), step(&[4])),
                (position(4), step(&[])),
            ]),
            originations: BTreeMap::new(),
            projections: BTreeMap::new(),
            contents_queries: BTreeMap::new(),
            contents_transfers: BTreeMap::new(),
            payload_producers: BTreeMap::new(),
            payload_stores: BTreeMap::new(),
            primitive_payload_writes: Default::default(),
            opaque_payload_results: Default::default(),
            payload_entry_predicates: Some((
                semantic_vocabulary::PropositionContext::from_value_types([
                    (ValueId::new(1).unwrap(), ScalarType::Boolean),
                    (ValueId::new(2).unwrap(), ScalarType::Boolean),
                ])
                .unwrap(),
                vec![condition(2)],
            )),
            payload_scalar_definitions: BTreeMap::new(),
            payload_scalar_postconditions: BTreeMap::from([(position(2), vec![condition(1)])]),
            payload_scalar_parameters: BTreeMap::new(),
            scalar_payload_reads: BTreeMap::new(),
            payload_edge_predicates: BTreeMap::new(),
            payload_scalar_bindings: BTreeMap::new(),
            scalar_payload_writes: BTreeMap::new(),
            payload_invalidations: BTreeMap::new(),
            case_constructors: BTreeMap::new(),
            case_successors: BTreeMap::new(),
            unresolved_contents_writes: BTreeSet::new(),
            storage_derivations: BTreeSet::new(),
            calls: BTreeMap::new(),
            normal_exits: vec![position(4)],
        };
        let contract = PayloadMembershipContract {
            carrier,
            scalar_requirements: vec![condition(1)],
            scalar_observations: Vec::new(),
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        };
        (machine, contract)
    }

    #[test]
    fn preservation_history_transports_stored_promises_without_inferred_predicates() {
        use semantic_vocabulary::{DomainSemanticId, StructuralDomainId};
        use terminal_psi::{
            StructuralDomainDeclaration, StructuralDomainPredicate, StructuralTypeShape,
            TerminalIdentityIndex,
        };
        let root = PlaceId::new(1).unwrap();
        let home = PlaceId::new(2).unwrap();
        let promised = StructuralDomainId::new(1).unwrap();
        let inferred = StructuralDomainId::new(2).unwrap();
        for case in ["stored", "joined", "unknown"] {
            let joined = case == "joined";
            let unknown = case == "unknown";
            let (mut machine, mut source) = forked_history(if joined { &[2, 3] } else { &[2] });
            if unknown {
                machine.flow.get_mut(&position(2)).unwrap().successors[0].target = position(3);
                machine
                    .payload_invalidations
                    .insert(position(3), vec![None]);
            } else if !joined {
                machine.flow.remove(&position(3));
            }
            source.qualifications = vec![promised];
            let types = [StructuralTypeDeclaration {
                quotient_carrier: None,
                id: source.carrier,
                identity: TerminalIdentityIndex(1),
                owns_linear_claim: false,
                no_code_affine_cleanup: true,
                copyable: true,
                descriptor_cleanup: false,
                scalar_restriction: None,
                shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
            }];
            let domain = |id, predicate| StructuralDomainDeclaration {
                required_memberships: Some(Vec::new()),
                complete_predicate_theory: true,
                predicate,
                id,
                semantic_domain: DomainSemanticId::new(id.get()).unwrap(),
                identity: TerminalIdentityIndex(id.get() as u32 + 1),
                carrier: source.carrier,
                content_projection: None,
                byte_predicate: None,
                establishment_routes: Vec::new(),
            };
            let domains = [
                domain(promised, None),
                domain(
                    inferred,
                    Some(StructuralDomainPredicate {
                        subject: root,
                        proposition: Proposition::Truth,
                    }),
                ),
            ];
            machine.places.push(DescriptorPlace {
                place: home,
                definition: DescriptorDefinition::Local,
            });
            machine.place_types.insert(home, source.carrier);
            machine.payload_producers.insert(position(1), home);
            machine
                .flow
                .get_mut(&position(1))
                .unwrap()
                .definitions
                .push(DescriptorLocation::whole(home));
            machine.originations.insert(
                position(1),
                vec![(
                    DescriptorLocation::whole(home),
                    DescriptorCallOrigin::Argument {
                        source: home,
                        projection: Projection::new(&[]),
                    },
                )],
            );
            let step = machine.flow.get_mut(&position(2)).unwrap();
            step.derivations.push((
                DescriptorLocation::whole(root),
                DescriptorLocation::whole(home),
            ));
            step.definitions.push(DescriptorLocation::whole(home));
            let mut empty = source.clone();
            empty.qualifications.clear();
            let contracts = |place| {
                Some(if place == root {
                    source.clone()
                } else {
                    empty.clone()
                })
            };
            for kind in [
                PayloadMembershipKind::Observed,
                PayloadMembershipKind::Preserved,
            ] {
                let graph = std::slice::from_ref(&machine);
                let mut analysis = DescriptorAnalysis::new(&types, graph);
                let mut replay = HistoryReplay {
                    memberships: kind,
                    ..Default::default()
                };
                let result = current_payload_contract_reusing(
                    &types,
                    &domains,
                    Some(&machine),
                    None,
                    &mut analysis,
                    position(4),
                    home,
                    &contracts,
                    &[],
                    &mut replay,
                    None,
                );
                if kind == PayloadMembershipKind::Observed {
                    assert!(result.unwrap().qualifications.contains(&inferred));
                } else if joined || unknown {
                    assert!(
                        result.is_err(),
                        "an incomplete arrival/write cannot erase a preservation duty"
                    );
                } else {
                    assert_eq!(result.unwrap().qualifications, vec![promised]);
                }
            }
        }
    }

    #[test]
    fn entry_observation_does_not_inherit_an_unpublished_ambient_premise() {
        let (machine, source) = forked_history(&[2, 3]);
        let observed = current_payload_contract(
            &[],
            &[],
            std::slice::from_ref(&machine),
            machine.id,
            machine.entry,
            PlaceId::new(1).unwrap(),
            &|_| Some(source.clone()),
        )
        .unwrap();
        assert_eq!(observed.scalar_requirements, vec![condition(1)]);
    }

    #[test]
    fn joined_payload_keeps_both_paths_facts_but_not_a_retired_instance() {
        for branches in [[2, 3], [3, 2]] {
            for retire in [false, true] {
                let (mut machine, source) = forked_history(&branches);
                if retire {
                    machine
                        .payload_scalar_definitions
                        .insert(position(3), vec![ValueId::new(1).unwrap()]);
                }
                let observed = current_payload_contract(
                    &[],
                    &[],
                    std::slice::from_ref(&machine),
                    machine.id,
                    position(4),
                    PlaceId::new(1).unwrap(),
                    &|_| Some(source.clone()),
                )
                .unwrap();
                let mut expected = vec![condition(2)];
                if !retire {
                    expected.push(condition(1));
                }
                expected.sort();
                assert_eq!(observed.scalar_requirements, expected);
            }
        }
    }

    #[test]
    fn predicate_edge_captures_both_arrival_routes_before_instance_retirement() {
        use terminal_psi::{StructuralTypeShape, TerminalIdentityIndex};

        let root = PlaceId::new(1).unwrap();
        let selected = PlaceId::new(2).unwrap();
        let predicate = Proposition::Conjunction(vec![condition(3), condition(4)]);
        for retire in [false, true] {
            let (mut machine, source) = forked_history(&[2]);
            // This single-arm input has no route through the other fork.
            // Descriptor replay requires the whole supplied graph to be reachable.
            let _ = machine.flow.remove(&position(3));
            let types = [StructuralTypeDeclaration {
                quotient_carrier: None,
                id: source.carrier,
                identity: TerminalIdentityIndex(1),
                owns_linear_claim: false,
                no_code_affine_cleanup: true,
                copyable: false,
                descriptor_cleanup: true,
                shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
                scalar_restriction: None,
            }];
            machine.places.push(DescriptorPlace {
                place: selected,
                definition: DescriptorDefinition::Local,
            });
            machine.place_types.insert(selected, source.carrier);
            let edge = &mut machine.flow.get_mut(&position(2)).unwrap().successors[0];
            edge.parameters = vec![
                DescriptorLocation::whole(root),
                DescriptorLocation::whole(selected),
            ];
            edge.bindings = vec![
                DescriptorFlowBinding::new(
                    DescriptorLocation::whole(root),
                    DescriptorLocation::whole(root),
                    StructuralAccess::Owned,
                    StructuralAccess::Owned,
                )
                .unwrap(),
                DescriptorFlowBinding::new(
                    DescriptorFlowSource::Contents(DescriptorContentsQuery {
                        root,
                        steps: Vec::new(),
                    }),
                    DescriptorLocation::whole(selected),
                    StructuralAccess::Owned,
                    StructuralAccess::Owned,
                )
                .unwrap(),
            ];
            machine
                .payload_edge_predicates
                .insert(position(2), vec![Some(predicate.clone())]);
            if retire {
                machine
                    .payload_scalar_parameters
                    .insert(position(4).0, vec![ValueId::new(3).unwrap()]);
            }
            let mut expected = vec![condition(1), condition(2), condition(4)];
            if !retire {
                expected.extend([condition(3), predicate.clone()]);
            }
            expected.sort();
            for payload in [root, selected] {
                let observed = current_payload_contract(
                    &types,
                    &[],
                    std::slice::from_ref(&machine),
                    machine.id,
                    position(4),
                    payload,
                    &|_| Some(source.clone()),
                )
                .unwrap();
                assert_eq!(observed.scalar_requirements, expected);
            }
        }
    }

    #[test]
    fn published_payload_retains_capacity_and_its_complete_theory() {
        let root = PlaceId::new(1).unwrap();
        let carrier = StructuralTypeId::new(1).unwrap();
        let mut requirements = Vec::with_capacity(32);
        requirements.extend([condition(1), condition(2)]);
        requirements.sort();
        let retained_capacity = requirements.capacity();
        let contract = PayloadMembershipContract {
            carrier,
            scalar_observations: Vec::new(),
            scalar_requirements: requirements,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        };
        let mut state = State {
            values: PayloadContracts::from([(root, contract)]),
            scalar_requirements: Arc::new(vec![SharedPremise::new(condition(1))]),
            ..Default::default()
        };
        let before = state.contract(root, None).unwrap();
        state.publish_requirements();
        assert_eq!(state.contract(root, None).unwrap(), before);
        let local = &state.values.shared(&root).scalar_requirements;
        assert_eq!(local, &[condition(2)]);
        assert_eq!(local.capacity(), retained_capacity);
        state.publish_requirements();
        assert_eq!(state.contract(root, None).unwrap(), before);
        assert_eq!(
            state.values[&root].scalar_requirements.capacity(),
            retained_capacity
        );
    }

    #[test]
    fn equal_raw_observations_still_normalize_at_meet() {
        let root = PlaceId::new(1).unwrap();
        let observed = super::super::PayloadScalarObservation {
            path: Vec::new(),
            value: Some(ScalarTerm::Boolean(true)),
            entry: None,
        };
        // Raw ingress can contain ambiguous duplicate paths or empty rows.
        // Equality of the states (or just their contracts) does not prove that
        // intersect is a no-op: it also removes these unavailable observations.
        for observations in [
            vec![observed.clone(), observed],
            vec![super::super::PayloadScalarObservation {
                path: Vec::new(),
                value: None,
                entry: None,
            }],
        ] {
            for different_ambient in [false, true] {
                let mut retained = State {
                    values: PayloadContracts::from([(
                        root,
                        PayloadMembershipContract {
                            carrier: StructuralTypeId::new(1).unwrap(),
                            scalar_observations: observations.clone(),
                            scalar_requirements: Vec::new(),
                            qualifications: Vec::new(),
                            projected_qualifications: Vec::new(),
                        },
                    )]),
                    ..Default::default()
                };
                let mut arrival = retained.clone();
                if different_ambient {
                    // The payload rows and shared theory remain equal, but the
                    // whole states differ. This independently pins row meet.
                    arrival.scalar_requirements = Arc::new(vec![SharedPremise::new(condition(1))]);
                }
                let changed = retained.meet(arrival, PayloadMembershipKind::Observed);
                assert!(
                    retained
                        .values
                        .get(&root)
                        .unwrap()
                        .scalar_observations
                        .is_empty()
                );
                // Duplicate paths set intersect's changed flag. The original
                // intersection drops an already-empty row without setting it.
                assert_eq!(changed, observations.len() == 2);
            }
        }
    }

    #[test]
    fn untouched_payload_rows_stay_shared_and_changed_rows_detach() {
        let root = PlaceId::new(1).unwrap();
        let other = PlaceId::new(2).unwrap();
        let contract = PayloadMembershipContract {
            carrier: StructuralTypeId::new(1).unwrap(),
            scalar_observations: Vec::new(),
            scalar_requirements: vec![condition(1)],
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        };
        let original = State {
            values: PayloadContracts::from([(root, contract.clone()), (other, contract)]),
            ..Default::default()
        };
        let mut later = original.clone();
        later.publish_requirements();
        retire_scalar_instances(&mut later, &[ValueId::new(2).unwrap()]);
        clear(&mut later.values);
        assert!(!super::super::payload_predicates::closure_may_change(
            &[],
            &[],
            later.values.get(&root).unwrap()
        ));
        for root in [root, other] {
            assert!(Arc::ptr_eq(
                original.values.shared(&root),
                later.values.shared(&root)
            ));
        }
        later
            .values
            .get_mut(&root)
            .unwrap()
            .scalar_requirements
            .push(condition(2));
        assert!(!Arc::ptr_eq(
            original.values.shared(&root),
            later.values.shared(&root)
        ));
        assert!(Arc::ptr_eq(
            original.values.shared(&other),
            later.values.shared(&other)
        ));
        assert_eq!(
            original.values.get(&root).unwrap().scalar_requirements,
            vec![condition(1)]
        );
        assert_eq!(
            later.values.get(&root).unwrap().scalar_requirements,
            vec![condition(1), condition(2)]
        );
    }

    #[test]
    fn publication_and_retirement_detach_only_affected_payload_rows() {
        let root = PlaceId::new(1).unwrap();
        let other = PlaceId::new(2).unwrap();
        let contract = |premise| PayloadMembershipContract {
            carrier: StructuralTypeId::new(1).unwrap(),
            scalar_observations: Vec::new(),
            scalar_requirements: vec![condition(premise)],
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        };
        let original = State {
            values: PayloadContracts::from([(root, contract(1)), (other, contract(2))]),
            ..Default::default()
        };
        let mut published = original.clone();
        published.scalar_requirements = Arc::new(vec![SharedPremise::new(condition(1))]);
        published.publish_requirements();
        assert!(!Arc::ptr_eq(
            original.values.shared(&root),
            published.values.shared(&root)
        ));
        assert!(Arc::ptr_eq(
            original.values.shared(&other),
            published.values.shared(&other)
        ));
        assert!(
            published
                .values
                .get(&root)
                .unwrap()
                .scalar_requirements
                .is_empty()
        );
        let mut retired = original.clone();
        retire_scalar_instances(&mut retired, &[ValueId::new(1).unwrap()]);
        assert!(!Arc::ptr_eq(
            original.values.shared(&root),
            retired.values.shared(&root)
        ));
        assert!(Arc::ptr_eq(
            original.values.shared(&other),
            retired.values.shared(&other)
        ));
        assert!(
            retired
                .values
                .get(&root)
                .unwrap()
                .scalar_requirements
                .is_empty()
        );
        assert_eq!(
            original.values.get(&root).unwrap().scalar_requirements,
            vec![condition(1)]
        );
    }

    #[test]
    fn retained_payload_snapshot_survives_later_writes_retirement_and_closure() {
        use semantic_vocabulary::{PropositionId, StructuralDomainId};

        let root = PlaceId::new(1).unwrap();
        let path = vec![DescriptorPathSegment::FixedIndex(0)];
        let value = |id| ScalarTerm::value(ValueId::new(id).unwrap(), ScalarType::Boolean);
        let domain = |id| StructuralDomainId::new(id).unwrap();
        let mut requirements = vec![
            condition(1),
            Proposition::Atom(PropositionId::new(1).unwrap()),
            Proposition::Conjunction(vec![
                Proposition::Atom(PropositionId::new(1).unwrap()),
                condition(2),
            ]),
        ];
        requirements.sort();
        let shared = Arc::new(
            requirements
                .iter()
                .cloned()
                .map(SharedPremise::new)
                .collect(),
        );
        let original = PayloadMembershipContract {
            carrier: StructuralTypeId::new(1).unwrap(),
            scalar_observations: vec![super::super::PayloadScalarObservation {
                path: path.clone(),
                value: Some(value(1)),
                entry: None,
            }],
            scalar_requirements: requirements,
            qualifications: vec![domain(2), domain(1), domain(2)],
            projected_qualifications: Vec::new(),
        };
        let retained = State {
            values: PayloadContracts::from([(root, original.clone())]),
            scalar_requirements: Arc::clone(&shared),
            shared_requirements: shared,
            ..Default::default()
        };
        enum Change {
            Store,
            UnknownWrite,
            RetireInstance,
            Close,
        }
        for change in [
            Change::Store,
            Change::UnknownWrite,
            Change::RetireInstance,
            Change::Close,
        ] {
            let mut later = retained.clone();
            let mut expected = original.clone();
            match change {
                Change::Store => {
                    let contract = later.values.get_mut(&root).unwrap();
                    retire(contract, &[StructuralPathSegment::FixedIndex(0)], &path);
                    assert_eq!(observe_scalar(contract, &path, &value(2)), None);
                    expected.scalar_observations[0].value = Some(value(2));
                    expected.qualifications.clear();
                }
                Change::UnknownWrite => {
                    clear(&mut later.values);
                    expected.scalar_observations.clear();
                    expected.qualifications.clear();
                }
                Change::RetireInstance => {
                    retire_scalar_instances(&mut later, &[ValueId::new(1).unwrap()]);
                    expected.scalar_observations.clear();
                    // Opaque local premises also retire; the ambient filter is
                    // deliberately insufficient for this payload-local theory.
                    expected.scalar_requirements.clear();
                    assert!(later.shared_requirements.is_empty());
                    assert_eq!(later.scalar_requirements.len(), 2);
                    assert!(later.scalar_requirements.iter().all(|premise| {
                        !premise
                            .as_ref()
                            .any_value_id(|value| value == ValueId::new(1).unwrap())
                    }));
                }
                Change::Close => {
                    super::super::payload_predicates::close_with_shared_requirements::<Proposition>(
                        &[],
                        &[],
                        later.values.get_mut(&root).unwrap(),
                        &[],
                        None,
                    );
                    expected.qualifications = vec![domain(1), domain(2)];
                }
            }
            assert!(!Arc::ptr_eq(
                retained.values.shared(&root),
                later.values.shared(&root)
            ));
            assert_eq!(retained.contract(root, None).unwrap(), original);
            assert_eq!(later.contract(root, None).unwrap(), expected);
        }
        // Opaque classification must not hide a value dependency inside it.
        // Ambient facts keep the independent atom, but withdraw the conjunction
        // when its value retires. Payload theory also withdraws the opaque atom.
        let mut later = retained.clone();
        retire_scalar_instances(&mut later, &[ValueId::new(2).unwrap()]);
        assert_eq!(later.scalar_requirements.len(), 2);
        assert!(later.scalar_requirements.iter().all(|premise| {
            !premise
                .as_ref()
                .any_value_id(|value| value == ValueId::new(2).unwrap())
        }));
        assert_eq!(later.shared_requirements.len(), 1);
        assert_eq!(later.shared_requirements[0].as_ref(), &condition(1));
    }

    #[test]
    fn membership_filter_preserves_equal_raw_rosters_and_filters_different_ones() {
        for original in [vec![], vec![1], vec![3, 1, 3, 2, 1]] {
            let mut retained = original.clone();
            retain_common_memberships(&mut retained, &original);
            assert_eq!(retained, original);
        }
        for (original, incoming) in [
            (vec![3, 1, 3, 2, 1], vec![2, 3, 2]),
            (vec![3, 1, 3, 2, 1], vec![1, 2, 3]),
            (vec![3, 1], vec![]),
            (vec![], vec![3, 1]),
            (vec![3, 1], vec![2]),
        ] {
            let expected: Vec<_> = original
                .iter()
                .copied()
                .filter(|value| incoming.contains(value))
                .collect();
            let mut retained = original;
            retain_common_memberships(&mut retained, &incoming);
            assert_eq!(retained, expected);
        }
    }

    #[test]
    fn rereads_preserve_common_observation_but_writes_and_instances_retire_it() {
        let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
        let value = |identity| {
            ScalarTerm::value(
                ValueId::new(identity).unwrap(),
                ScalarType::Integer(integer),
            )
        };
        let path = vec![DescriptorPathSegment::FixedIndex(0)];
        let mut contract = PayloadMembershipContract {
            carrier: semantic_vocabulary::StructuralTypeId::new(1).unwrap(),
            scalar_observations: Vec::new(),
            scalar_requirements: Vec::new(),
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        };
        assert_eq!(observe_scalar(&mut contract, &path, &value(1)), None);
        let mut other_branch = contract.clone();
        assert_eq!(
            observe_scalar(&mut contract, &path, &value(2)),
            Some(Proposition::Equal(value(2), value(1)))
        );
        assert_eq!(
            observe_scalar(&mut other_branch, &path, &value(3)),
            Some(Proposition::Equal(value(3), value(1)))
        );
        assert_eq!(
            contract.scalar_observations,
            other_branch.scalar_observations
        );
        retire(
            &mut contract,
            &[StructuralPathSegment::FixedIndex(0)],
            &path,
        );
        assert_eq!(observe_scalar(&mut contract, &path, &value(4)), None);
        assert_eq!(contract.scalar_observations[0].value, Some(value(4)));
        let root = PlaceId::new(1).unwrap();
        let mut state = State {
            values: PayloadContracts::from([(root, other_branch)]),
            scalar_requirements: Arc::default(),
            shared_requirements: Arc::default(),
            ..Default::default()
        };
        retire_scalar_instances(&mut state, &[ValueId::new(1).unwrap()]);
        assert_eq!(
            observe_scalar(state.values.get_mut(&root).unwrap(), &path, &value(5)),
            None
        );
    }
    #[test]
    fn before_store_cut_does_not_execute_unresolved_incoming_destination() {
        use terminal_psi::TerminalIdentityIndex;
        let root = PlaceId::new(1).unwrap();
        let pointer = PlaceId::new(2).unwrap();
        let boolean = StructuralTypeId::new(1).unwrap();
        let reference = StructuralTypeId::new(2).unwrap();
        let (mut machine, mut payload) = forked_history(&[2]);
        machine.flow.remove(&position(3));
        machine.payload_entry_predicates.as_mut().unwrap().1.clear();
        machine.payload_scalar_postconditions.clear();
        payload.scalar_requirements.clear();
        payload.scalar_observations.push(PayloadScalarObservation {
            path: Vec::new(),
            value: Some(ScalarTerm::Boolean(true)),
            entry: None,
        });
        machine.places.push(DescriptorPlace {
            place: pointer,
            definition: DescriptorDefinition::Parameter {
                position: 1,
                access: StructuralAccess::Owned,
                structural_type: reference,
            },
        });
        machine.place_types.insert(pointer, reference);
        let destination = DescriptorLocation {
            root: pointer,
            path: vec![DescriptorPathSegment::Referent],
        };
        machine
            .flow
            .get_mut(&position(2))
            .unwrap()
            .definitions
            .push(destination.clone());
        machine
            .flow
            .get_mut(&position(2))
            .unwrap()
            .derivations
            .push((DescriptorLocation::whole(root), destination.clone()));
        machine.payload_invalidations.insert(
            position(2),
            vec![Some(DescriptorContentsQuery::from_location(&destination))],
        );
        let types = [
            (
                boolean,
                StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
            ),
            (
                reference,
                StructuralTypeShape::Reference {
                    referent: boolean,
                    access: StructuralAccess::MutableBorrow,
                },
            ),
        ]
        .map(|(id, shape)| StructuralTypeDeclaration {
            quotient_carrier: None,
            id,
            identity: TerminalIdentityIndex(id.get() as u32),
            owns_linear_claim: false,
            no_code_affine_cleanup: true,
            copyable: true,
            descriptor_cleanup: false,
            scalar_restriction: None,
            shape,
        });
        let pointer_payload = PayloadMembershipContract {
            carrier: reference,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            scalar_requirements: Vec::new(),
            scalar_observations: Vec::new(),
        };
        let source = |place| {
            if place == root {
                Some(payload.clone())
            } else if place == pointer {
                Some(pointer_payload.clone())
            } else {
                None
            }
        };
        let graph = [machine];
        let incoming = incoming_flow(&graph[0].flow).unwrap();
        let mut analysis = DescriptorAnalysis::with_borrowed_storage_slots(&types, &graph);
        assert!(matches!(
            analysis
                .payload_targets(
                    graph[0].id,
                    position(2),
                    &DescriptorContentsQuery::from_location(&destination)
                )
                .unwrap()
                .as_slice(),
            [DescriptorPayloadTarget::Incoming(_)]
        ));
        let mut replay = HistoryReplay {
            memberships: PayloadMembershipKind::Preserved,
            ..HistoryReplay::default()
        };
        let mut query = |at, cut| {
            payload_contract_at_cut(
                &types,
                &[],
                Some(&graph[0]),
                Some(&incoming),
                &mut analysis,
                at,
                root,
                &source,
                &[],
                &mut replay,
                cut,
                None,
            )
        };
        let before = query(position(2), PayloadMembershipCut::BeforeOperation).unwrap();
        assert_eq!(before.scalar_observations, payload.scalar_observations);
        assert!(query(position(2), PayloadMembershipCut::NormalCompletion).is_err());
        assert!(query(position(4), PayloadMembershipCut::BeforeOperation).is_err());
        let before_again = query(position(2), PayloadMembershipCut::BeforeOperation).unwrap();
        assert_eq!(
            before_again.scalar_observations,
            payload.scalar_observations
        );
    }

    #[test]
    fn normal_completion_cache_keeps_distinct_roots_and_query_cuts() {
        use terminal_psi::TerminalIdentityIndex;
        let root = PlaceId::new(1).unwrap();
        let other = PlaceId::new(2).unwrap();
        let (mut machine, mut contract) = forked_history(&[2]);
        // One normal postcondition is followed by an unknown write. Earlier
        // completed snapshots must survive; the later current payload must refuse.
        machine.flow.get_mut(&position(2)).unwrap().successors[0].target = position(3);
        machine
            .payload_invalidations
            .insert(position(3), vec![None]);
        machine.payload_entry_predicates.as_mut().unwrap().1.clear();
        contract.scalar_requirements.clear();
        machine.places.push(DescriptorPlace {
            place: other,
            definition: DescriptorDefinition::Parameter {
                position: 1,
                access: StructuralAccess::Owned,
                structural_type: contract.carrier,
            },
        });
        machine.place_types.insert(other, contract.carrier);
        let types = [StructuralTypeDeclaration {
            quotient_carrier: None,
            id: contract.carrier,
            identity: TerminalIdentityIndex(1),
            owns_linear_claim: false,
            no_code_affine_cleanup: true,
            copyable: true,
            descriptor_cleanup: false,
            scalar_restriction: None,
            shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
        }];
        let graph = [machine];
        let mut analysis = DescriptorAnalysis::with_borrowed_storage_slots(&types, &graph);
        let mut replay = HistoryReplay {
            memberships: PayloadMembershipKind::Preserved,
            ..HistoryReplay::default()
        };
        let source = |place| (place == root || place == other).then(|| contract.clone());
        let incoming = incoming_flow(&graph[0].flow).unwrap();
        let mut at_cut = |position, root, cut| {
            payload_contract_at_cut(
                &types,
                &[],
                Some(&graph[0]),
                Some(&incoming),
                &mut analysis,
                position,
                root,
                &source,
                &[],
                &mut replay,
                cut,
                None,
            )
        };
        let first = at_cut(position(2), root, PayloadMembershipCut::NormalCompletion).unwrap();
        assert!(first.scalar_requirements.contains(&condition(1)));
        let second = at_cut(position(2), other, PayloadMembershipCut::NormalCompletion).unwrap();
        assert!(second.scalar_requirements.contains(&condition(1)));
        let before = at_cut(position(2), root, PayloadMembershipCut::BeforeOperation).unwrap();
        assert!(!before.scalar_requirements.contains(&condition(1)));
        let repeated = at_cut(position(2), root, PayloadMembershipCut::NormalCompletion).unwrap();
        assert!(repeated.scalar_requirements.contains(&condition(1)));
        assert!(at_cut(position(4), root, PayloadMembershipCut::NormalCompletion).is_err());
        assert!(at_cut(position(4), other, PayloadMembershipCut::NormalCompletion).is_err());
        let earlier = at_cut(position(2), other, PayloadMembershipCut::NormalCompletion).unwrap();
        assert!(earlier.scalar_requirements.contains(&condition(1)));
    }
    #[test]
    fn requested_reference_predicate_uses_current_payload_without_growing_old_duties() {
        use semantic_vocabulary::{DomainSemanticId, StructuralDomainId};
        use terminal_psi::{
            StructuralDomainDeclaration, StructuralDomainPredicate, StructuralMultiplicity,
            StructuralOperationResult, StructuralParameterDeclaration, StructuralPathQualification,
            TerminalIdentityTableBuilder,
        };

        let source_root = PlaceId::new(1).unwrap();
        let written_value = PlaceId::new(2).unwrap();
        let reference_result = PlaceId::new(3).unwrap();
        let boolean_type = StructuralTypeId::new(1).unwrap();
        let reference_type = StructuralTypeId::new(2).unwrap();
        let requested = StructuralDomainId::new(1).unwrap();
        let unrequested = StructuralDomainId::new(2).unwrap();
        // A domain subject has its own namespace; it is not the source place.
        let predicate_subject = PlaceId::new(77).unwrap();
        let mut identities = TerminalIdentityTableBuilder::default();
        let boolean_identity = identities.intern("formation-test::Boolean");
        let reference_identity = identities.intern("formation-test::MutableBooleanReference");
        let requested_identity = identities.intern("formation-test::RequestedTrue");
        let unrequested_identity = identities.intern("formation-test::UnrequestedTrue");
        let identities = identities.build();
        let declaration = |id, identity, shape| StructuralTypeDeclaration {
            quotient_carrier: None,
            id,
            identity,
            owns_linear_claim: false,
            no_code_affine_cleanup: true,
            copyable: matches!(&shape, StructuralTypeShape::PrimitiveScalar(_)),
            descriptor_cleanup: matches!(&shape, StructuralTypeShape::Reference { .. }),
            shape,
            scalar_restriction: None,
        };
        let types = [
            declaration(
                boolean_type,
                boolean_identity,
                StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
            ),
            declaration(
                reference_type,
                reference_identity,
                StructuralTypeShape::Reference {
                    referent: boolean_type,
                    access: StructuralAccess::MutableBorrow,
                },
            ),
        ];
        let domain = |id: StructuralDomainId, identity| StructuralDomainDeclaration {
            required_memberships: Some(Vec::new()),
            complete_predicate_theory: true,
            predicate: Some(StructuralDomainPredicate {
                subject: predicate_subject,
                proposition: Proposition::Equal(
                    ScalarTerm::Boolean(true),
                    ScalarTerm::boolean_field_path(predicate_subject, Vec::new()),
                ),
            }),
            id,
            semantic_domain: DomainSemanticId::new(id.get()).unwrap(),
            identity,
            carrier: boolean_type,
            content_projection: None,
            byte_predicate: None,
            establishment_routes: Vec::new(),
        };
        let domains = [
            domain(requested, requested_identity),
            domain(unrequested, unrequested_identity),
        ];
        let requested_row = StructuralPathQualification {
            path: vec![StructuralPathSegment::Referent],
            domain: requested,
        };
        let source = StructuralArgument {
            place: source_root,
            path: Vec::new(),
            access: StructuralAccess::MutableBorrow,
        };
        let result = StructuralOperationResult {
            place: reference_result,
            structural_type: reference_type,
            multiplicity: StructuralMultiplicity::Affine,
            qualifications: Vec::new(),
            projected_qualifications: vec![requested_row.clone()],
            qualification_establishments: Vec::new(),
            claims: Vec::new(),
        };
        let parameters = [StructuralParameterDeclaration {
            place: source_root,
            position: 0,
            structural_type: boolean_type,
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::Owned,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        }];
        let refusal =
            "reference predicate introduction is not proved by every current source alternative";

        for (case, value, introduced) in [
            ("true current payload", ScalarTerm::Boolean(true), true),
            ("false current payload", ScalarTerm::Boolean(false), false),
            (
                "unknown current payload",
                ScalarTerm::value(ValueId::new(1).unwrap(), ScalarType::Boolean),
                false,
            ),
        ] {
            let (mut descriptor, mut empty) = forked_history(&[2]);
            descriptor.flow.remove(&position(3));
            // The helper normally has unrelated sample premises. This fixture has
            // no premise proving the unknown Boolean and no receiving qualification.
            descriptor
                .payload_entry_predicates
                .as_mut()
                .unwrap()
                .1
                .clear();
            descriptor.payload_scalar_postconditions.clear();
            empty.scalar_requirements.clear();
            empty.scalar_observations.clear();
            empty.qualifications.clear();
            empty.projected_qualifications.clear();

            let written = DescriptorLocation::whole(written_value);
            let destination = DescriptorLocation::whole(source_root);
            descriptor.places.push(DescriptorPlace {
                place: written_value,
                definition: DescriptorDefinition::Local,
            });
            descriptor.place_types.insert(written_value, boolean_type);
            // This is the same authenticated producer/home setup as the existing
            // preservation_history_transports_stored_promises... fixture.
            descriptor
                .payload_producers
                .insert(position(1), written_value);
            descriptor
                .flow
                .get_mut(&position(1))
                .unwrap()
                .definitions
                .push(written.clone());
            descriptor.originations.insert(
                position(1),
                vec![(
                    written.clone(),
                    DescriptorCallOrigin::Argument {
                        source: written_value,
                        projection: Projection::new(&[]),
                    },
                )],
            );
            descriptor
                .scalar_payload_writes
                .insert(position(1), vec![(written.clone(), value)]);
            // A real strong whole-value store changes the original parameter's
            // current payload at position(2). It is not a source-contract reseed.
            let store = descriptor.flow.get_mut(&position(2)).unwrap();
            store.derivations.push((written, destination.clone()));
            store.definitions.push(destination.clone());
            descriptor.payload_invalidations.insert(
                position(2),
                vec![Some(DescriptorContentsQuery::from_location(&destination))],
            );

            let source_contracts = [(source_root, empty.clone()), (written_value, empty)];
            let identity = descriptor.id;
            let graph = [descriptor];
            let mut history = PayloadMembershipHistory::for_current_contracts(
                &types,
                &domains,
                &graph,
                identity,
                &parameters,
                &source_contracts,
            )
            .unwrap();
            // Configure the fresh normalized-source fixture exactly as
            // for_preservation configures its fresh Terminal-source history.
            // No query/cache exists yet; predicate_observations stays Observed.
            history.replay.memberships = PayloadMembershipKind::Preserved;
            history.analysis = Some(DescriptorAnalysis::with_borrowed_storage_slots(
                &types, &graph,
            ));
            let mut analysis = DescriptorAnalysis::with_borrowed_storage_slots(&types, &graph);

            let old_before = history
                .reference_preservation_memberships(
                    &identities,
                    &mut analysis,
                    position(4),
                    &source,
                    None,
                    &result,
                )
                .unwrap();
            assert!(old_before.is_empty(), "{case}");
            let actual = history.reference_formation_memberships(
                &identities,
                &mut analysis,
                position(4),
                &source,
                None,
                &result,
            );
            if introduced {
                assert_eq!(actual.unwrap(), vec![requested_row.clone()], "{case}");
                // Prove the second row really is inferable; its exclusion from
                // the mutable roster must follow requested-only publication.
                let mut observed = PayloadMembershipHistory::for_current_contracts(
                    &types,
                    &domains,
                    &graph,
                    identity,
                    &parameters,
                    &source_contracts,
                )
                .unwrap();
                let current = observed
                    .current_selected_payload_contract_in(
                        &mut analysis,
                        position(4),
                        source_root,
                        &[],
                    )
                    .unwrap();
                assert_eq!(current.qualifications, vec![requested, unrequested]);
            } else {
                assert_eq!(actual.unwrap_err(), refusal, "{case}");
            }

            // Reuse the SAME history after the introduction attempt. An unrequested
            // inferred truth must not have contaminated source preservation/cache.
            let mut no_request = result.clone();
            no_request.projected_qualifications.clear();
            assert!(
                history
                    .reference_formation_memberships(
                        &identities,
                        &mut analysis,
                        position(4),
                        &source,
                        None,
                        &no_request,
                    )
                    .unwrap()
                    .is_empty(),
                "{case}"
            );
            assert_eq!(
                history
                    .reference_preservation_memberships(
                        &identities,
                        &mut analysis,
                        position(4),
                        &source,
                        None,
                        &result,
                    )
                    .unwrap(),
                old_before,
                "{case}"
            );

            // position(2) is BEFORE its store. Even a known-true RHS already
            // constructed at position(1) cannot prove the destination yet. Querying
            // backwards after position(4) also checks exact cut/cache separation.
            assert_eq!(
                history
                    .reference_formation_memberships(
                        &identities,
                        &mut analysis,
                        position(2),
                        &source,
                        None,
                        &result,
                    )
                    .unwrap_err(),
                refusal,
                "{case}"
            );
            let after_again = history.reference_formation_memberships(
                &identities,
                &mut analysis,
                position(4),
                &source,
                None,
                &result,
            );
            if introduced {
                assert_eq!(after_again.unwrap(), vec![requested_row.clone()], "{case}");
            } else {
                assert_eq!(after_again.unwrap_err(), refusal, "{case}");
            }
        }
    }
}

//! STR4 checked plans (machine_taxonomy.md): the normalized MACHINE
//! SEMANTIC CONTRACT, independent of syntax and lowering -- component
//! manifests, proof artifacts, provider admission, and hot-swap checks
//! reference this identity, never re-derived booleans. The checked public
//! surface carries supply mode, canonical service reach, operational ceilings,
//! normalized crash-route buckets, and the termination guarantee plus a
//! deterministic fingerprint over them.
//! Prover-independence (acceptance 8: a stronger prover cannot change an
//! exported contract ID) holds BY CONSTRUCTION: only declared/published
//! halves enter the fingerprint, never inferred rows or witnesses.

use psi_language_semantics::{
    BlockingInterface, BlockingPlan, MachineSupplyMode, ServiceReachPlan, SuspensionInterface,
    SuspensionPlan, SynchronousInvocationInterface, SynchronousInvocationPlan,
    TerminationGuarantee, TerminationInterface,
};
use psi_symbols::SymbolHandle;

pub use psi_language_semantics::crash::{
    ACTIVATION_CRASH_SCOPE, EXECUTION_DOMAIN_CRASH_SCOPE,
    scope_covers_minimum as crash_scope_covers_minimum,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CrashCause {
    Trap,
    Abort,
}

impl CrashCause {
    /// The first checked damage-minimum slice follows the language's intrinsic
    /// cause law. Later invariant/custody analysis may widen `Trap`; it can
    /// never narrow either cause below this seed.
    pub const fn intrinsic_damage_minimum(self) -> &'static str {
        match self {
            Self::Trap => ACTIVATION_CRASH_SCOPE,
            Self::Abort => EXECUTION_DOMAIN_CRASH_SCOPE,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CrashInterface {
    #[default]
    InternalInferred,
    PublishedCeiling,
}

/// Source-independent identity of one guarded crash route. The bytes are the
/// canonical, position-normalized proof-expression encoding; they are identity
/// material rather than executable source-tree handles.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CrashPredicateIdentity(Vec<u8>);

impl CrashPredicateIdentity {
    pub fn from_canonical_bytes(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CrashRouteGuard {
    /// The canonical route contributed by a route-less clause or an authored
    /// `true` route. It subsumes every guarded alternative in its bucket.
    Truth,
    Predicate(CrashPredicateIdentity),
}

/// Dense, one-based identity of a canonical published route bucket within one
/// machine's crash plan. Bucket normalization happens before these identities
/// are assigned, so clause regrouping and duplicate routes cannot renumber
/// them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CrashRouteBucketId(u32);

impl CrashRouteBucketId {
    fn from_index(index: usize) -> Self {
        Self(
            u32::try_from(index)
                .expect("published crash bucket count exceeds checked identity range")
                .checked_add(1)
                .expect("published crash bucket identity is one-based"),
        )
    }

    pub const fn get(self) -> u32 {
        self.0
    }

    fn index(self) -> Option<usize> {
        usize::try_from(self.0.checked_sub(1)?).ok()
    }
}

/// Source-handle-free location of one crash transition within a checked
/// machine body. State identity plus the statement's state-local ordinal is
/// stable against unrelated statement-arena insertions and is sufficient for
/// checked-tree consumers to join the derived site back to its body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CrashSiteLocation {
    state: SymbolHandle,
    statement_ordinal: u32,
}

/// Source-handle-free identity of one invocation within a checked machine
/// body. This deliberately reuses the flow layer's state/statement/call
/// coordinates so later crash propagation never has to rediscover a source
/// expression.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CrashCallSiteLocation {
    state: SymbolHandle,
    statement_ordinal: u32,
    call_ordinal: u32,
}

impl CrashCallSiteLocation {
    pub const fn new(state: SymbolHandle, statement_ordinal: u32, call_ordinal: u32) -> Self {
        Self {
            state,
            statement_ordinal,
            call_ordinal,
        }
    }

    pub const fn state(self) -> SymbolHandle {
        self.state
    }

    pub const fn statement_ordinal(self) -> u32 {
        self.statement_ordinal
    }

    pub const fn call_ordinal(self) -> u32 {
        self.call_ordinal
    }
}

impl CrashSiteLocation {
    pub const fn new(state: SymbolHandle, statement_ordinal: u32) -> Self {
        Self {
            state,
            statement_ordinal,
        }
    }

    pub const fn state(self) -> SymbolHandle {
        self.state
    }

    pub const fn statement_ordinal(self) -> u32 {
        self.statement_ordinal
    }
}

/// Body-derived seed for a crash-terminator plan. Intrinsic cause minima and
/// structurally unconditional guard coverage are attached immediately;
/// path-conditioned entailment, invariant/custody widening, and frontier
/// reconstruction remain independent later passes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedCrashSite {
    location: CrashSiteLocation,
    cause: CrashCause,
    /// Smallest nominal termination scope currently proved necessary to keep
    /// surviving state sound. This is body evidence, not public identity.
    damage_minimum: String,
    /// Exact canonical predicates known to hold on every path into this site.
    /// Their conjunction is the retained derived path guard; implication
    /// consequences remain separate coverage evidence.
    path_guard_conjuncts: Vec<CrashPredicateIdentity>,
    /// Published buckets whose guard implication is already established for
    /// this site. This is not yet complete crash coverage: damage-minimum and
    /// containment-demand comparison remains an independent check.
    guard_covering_buckets: Vec<CrashRouteBucketId>,
    /// Stable identities of claims proved live at this exact machine-local
    /// crash site. This is deliberately a lower bound: conditionally live sum
    /// payloads and obligations outside this activation are absent until a
    /// later analysis can prove their membership.
    frontier_lower_bound: Vec<psi_language_semantics::PermissionClaimIdentity>,
}

impl CheckedCrashSite {
    pub fn new(
        location: CrashSiteLocation,
        cause: CrashCause,
        mut guard_covering_buckets: Vec<CrashRouteBucketId>,
        mut frontier_lower_bound: Vec<psi_language_semantics::PermissionClaimIdentity>,
    ) -> Self {
        guard_covering_buckets.sort_unstable();
        guard_covering_buckets.dedup();
        frontier_lower_bound.sort_by_key(|identity| crash_frontier_claim_sort_key(*identity));
        frontier_lower_bound.dedup();
        Self {
            location,
            cause,
            damage_minimum: cause.intrinsic_damage_minimum().to_owned(),
            path_guard_conjuncts: Vec::new(),
            guard_covering_buckets,
            frontier_lower_bound,
        }
    }

    pub const fn location(&self) -> CrashSiteLocation {
        self.location
    }

    pub const fn cause(&self) -> CrashCause {
        self.cause
    }

    pub fn damage_minimum(&self) -> &str {
        &self.damage_minimum
    }

    pub fn with_damage_minimum(mut self, damage_minimum: impl Into<String>) -> Option<Self> {
        let damage_minimum = damage_minimum.into();
        if !crash_scope_covers_minimum(self.cause.intrinsic_damage_minimum(), &damage_minimum) {
            return None;
        }
        self.damage_minimum = damage_minimum;
        Some(self)
    }

    pub fn with_guard_covering_buckets(
        mut self,
        mut guard_covering_buckets: Vec<CrashRouteBucketId>,
    ) -> Self {
        guard_covering_buckets.sort_unstable();
        guard_covering_buckets.dedup();
        self.guard_covering_buckets = guard_covering_buckets;
        self
    }

    pub fn with_path_guard_conjuncts(
        mut self,
        mut path_guard_conjuncts: Vec<CrashPredicateIdentity>,
    ) -> Self {
        path_guard_conjuncts.sort();
        path_guard_conjuncts.dedup();
        self.path_guard_conjuncts = path_guard_conjuncts;
        self
    }

    pub fn with_frontier_lower_bound(
        mut self,
        mut frontier_lower_bound: Vec<psi_language_semantics::PermissionClaimIdentity>,
    ) -> Self {
        frontier_lower_bound.sort_by_key(|identity| crash_frontier_claim_sort_key(*identity));
        frontier_lower_bound.dedup();
        self.frontier_lower_bound = frontier_lower_bound;
        self
    }

    pub fn guard_covering_buckets(&self) -> &[CrashRouteBucketId] {
        &self.guard_covering_buckets
    }

    pub fn path_guard_conjuncts(&self) -> &[CrashPredicateIdentity] {
        &self.path_guard_conjuncts
    }

    pub fn frontier_lower_bound(&self) -> &[psi_language_semantics::PermissionClaimIdentity] {
        &self.frontier_lower_bound
    }
}

/// Invocation-specific refinement of a selected callee crash summary. The
/// summary may be a published ceiling or conservative same-unit checked-body
/// evidence. `surviving_buckets` are already expressed in the caller's
/// canonical parameter namespace. An empty set is meaningful evidence that
/// the selected summary is crash-free at this invocation, so such records are
/// retained rather than elided.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedCrashCallSite {
    location: CrashCallSiteLocation,
    target_machine: SymbolHandle,
    target_state: SymbolHandle,
    target_contract_fingerprint: u64,
    path_guard_conjuncts: Vec<CrashPredicateIdentity>,
    surviving_buckets: Vec<CrashRouteBucket>,
}

/// Source-independent crash-contract projection for a callable requirement
/// that has no local `MachineContractPlan`. The fingerprint pins the complete
/// normalized callable contract; `published_buckets` is the crash ceiling that
/// call-site selection may refine without reopening the authored signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrashContractCapsule {
    target_machine: SymbolHandle,
    target_state: SymbolHandle,
    target_contract_fingerprint: u64,
    published_buckets: Vec<CrashRouteBucket>,
}

impl CrashContractCapsule {
    pub fn new(
        target_machine: SymbolHandle,
        target_state: SymbolHandle,
        target_contract_fingerprint: u64,
        mut published_buckets: Vec<CrashRouteBucket>,
    ) -> Self {
        published_buckets.sort();
        published_buckets.dedup();
        Self {
            target_machine,
            target_state,
            target_contract_fingerprint,
            published_buckets,
        }
    }

    pub const fn target_machine(&self) -> SymbolHandle {
        self.target_machine
    }

    pub const fn target_state(&self) -> SymbolHandle {
        self.target_state
    }

    pub const fn target_contract_fingerprint(&self) -> u64 {
        self.target_contract_fingerprint
    }

    pub fn published_buckets(&self) -> &[CrashRouteBucket] {
        &self.published_buckets
    }
}

impl CheckedCrashCallSite {
    pub fn new(
        location: CrashCallSiteLocation,
        target_machine: SymbolHandle,
        target_state: SymbolHandle,
        target_contract_fingerprint: u64,
        mut surviving_buckets: Vec<CrashRouteBucket>,
    ) -> Self {
        surviving_buckets.sort();
        surviving_buckets.dedup();
        Self {
            location,
            target_machine,
            target_state,
            target_contract_fingerprint,
            path_guard_conjuncts: Vec::new(),
            surviving_buckets,
        }
    }

    pub const fn location(&self) -> CrashCallSiteLocation {
        self.location
    }

    pub const fn target_machine(&self) -> SymbolHandle {
        self.target_machine
    }

    pub const fn target_state(&self) -> SymbolHandle {
        self.target_state
    }

    pub const fn target_contract_fingerprint(&self) -> u64 {
        self.target_contract_fingerprint
    }

    pub fn path_guard_conjuncts(&self) -> &[CrashPredicateIdentity] {
        &self.path_guard_conjuncts
    }

    pub fn surviving_buckets(&self) -> &[CrashRouteBucket] {
        &self.surviving_buckets
    }

    pub fn with_path_guard_conjuncts(
        mut self,
        mut path_guard_conjuncts: Vec<CrashPredicateIdentity>,
    ) -> Self {
        path_guard_conjuncts.sort();
        path_guard_conjuncts.dedup();
        self.path_guard_conjuncts = path_guard_conjuncts;
        self
    }
}

fn crash_frontier_claim_sort_key(
    identity: psi_language_semantics::PermissionClaimIdentity,
) -> [u64; 11] {
    use psi_language_semantics::{PermissionClaimIdentity, PermissionEventSource};

    let PermissionClaimIdentity::Established {
        machine_symbol,
        state_symbol,
        source,
        ordinal,
    } = identity
    else {
        return [0; 11];
    };
    let mut key = [0; 11];
    key[0] = 1;
    key[1] = u64::from(machine_symbol.arena_index());
    key[2] = u64::from(machine_symbol.generation());
    key[3] = u64::from(state_symbol.arena_index());
    key[4] = u64::from(state_symbol.generation());
    match source {
        PermissionEventSource::StateEntry => key[5] = 0,
        PermissionEventSource::Statement { statement_index } => {
            key[5] = 1;
            key[6] = u64::try_from(statement_index).unwrap_or(u64::MAX);
        }
        PermissionEventSource::Call {
            statement_index,
            call_ordinal,
            target_symbol,
        } => {
            key[5] = 2;
            key[6] = u64::try_from(statement_index).unwrap_or(u64::MAX);
            key[7] = u64::try_from(call_ordinal).unwrap_or(u64::MAX);
            key[8] = u64::from(target_symbol.arena_index());
            key[9] = u64::from(target_symbol.generation());
        }
        PermissionEventSource::StateExit => key[5] = 3,
    }
    key[10] = u64::from(ordinal);
    key
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CrashRouteBucket {
    cause: CrashCause,
    containment_demand: String,
    /// Canonical nonempty set. `Truth` is always the sole entry when present.
    alternative_guards: Vec<CrashRouteGuard>,
}

impl CrashRouteBucket {
    pub fn new(
        cause: CrashCause,
        containment_demand: impl Into<String>,
        mut alternative_guards: Vec<CrashRouteGuard>,
    ) -> Option<Self> {
        if alternative_guards.contains(&CrashRouteGuard::Truth) {
            alternative_guards = vec![CrashRouteGuard::Truth];
        } else {
            alternative_guards.sort();
            alternative_guards.dedup();
        }
        (!alternative_guards.is_empty()).then(|| Self {
            cause,
            containment_demand: containment_demand.into(),
            alternative_guards,
        })
    }

    pub fn unconditional(cause: CrashCause, containment_demand: impl Into<String>) -> Self {
        Self::new(cause, containment_demand, vec![CrashRouteGuard::Truth])
            .expect("the unconditional crash bucket has one canonical guard")
    }

    pub fn cause(&self) -> CrashCause {
        self.cause
    }

    pub fn containment_demand(&self) -> &str {
        &self.containment_demand
    }

    pub fn alternative_guards(&self) -> &[CrashRouteGuard] {
        &self.alternative_guards
    }

    pub fn is_unconditional(&self) -> bool {
        self.alternative_guards == [CrashRouteGuard::Truth]
    }
}

/// The published and body-derived halves of CRASH-CONTRACT remain independent:
/// published route buckets are contract identity, while checked sites are
/// implementation evidence and never enter that fingerprint. Damage minima,
/// path guards, complete covering buckets, and frontier lower bounds enrich
/// the site layer without changing the published interface.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CrashPlan {
    interface: CrashInterface,
    published: Vec<CrashRouteBucket>,
    checked_sites: Vec<CheckedCrashSite>,
    checked_calls: Vec<CheckedCrashCallSite>,
}

impl CrashPlan {
    pub fn published_ceiling(mut published: Vec<CrashRouteBucket>) -> Self {
        published.sort();
        published.dedup();
        Self {
            interface: CrashInterface::PublishedCeiling,
            published,
            checked_sites: Vec::new(),
            checked_calls: Vec::new(),
        }
    }

    pub fn with_checked_sites(mut self, mut checked_sites: Vec<CheckedCrashSite>) -> Option<Self> {
        checked_sites.sort_by_key(|site| {
            (
                site.location.state.arena_index(),
                site.location.state.generation(),
                site.location.statement_ordinal,
                site.cause,
            )
        });
        checked_sites.dedup();
        if checked_sites.windows(2).any(|sites| {
            sites[0].location.state == sites[1].location.state
                && sites[0].location.statement_ordinal == sites[1].location.statement_ordinal
        }) {
            return None;
        }
        if checked_sites.iter().any(|site| {
            !crash_scope_covers_minimum(site.cause.intrinsic_damage_minimum(), &site.damage_minimum)
                || site.guard_covering_buckets.iter().any(|bucket| {
                    self.published_bucket(*bucket)
                        .is_none_or(|published| published.cause != site.cause)
                })
                || site.frontier_lower_bound.iter().any(|identity| {
                    *identity == psi_language_semantics::PermissionClaimIdentity::Unknown
                })
        }) {
            return None;
        }
        self.checked_sites = checked_sites;
        Some(self)
    }

    pub fn interface(&self) -> CrashInterface {
        self.interface
    }

    pub fn published(&self) -> &[CrashRouteBucket] {
        &self.published
    }

    pub fn published_with_ids(
        &self,
    ) -> impl Iterator<Item = (CrashRouteBucketId, &CrashRouteBucket)> {
        self.published
            .iter()
            .enumerate()
            .map(|(index, bucket)| (CrashRouteBucketId::from_index(index), bucket))
    }

    pub fn published_bucket(&self, id: CrashRouteBucketId) -> Option<&CrashRouteBucket> {
        self.published.get(id.index()?)
    }

    pub fn checked_sites(&self) -> &[CheckedCrashSite] {
        &self.checked_sites
    }

    pub fn with_checked_calls(
        mut self,
        mut checked_calls: Vec<CheckedCrashCallSite>,
    ) -> Option<Self> {
        checked_calls.sort_by_key(|call| {
            (
                call.location.state.arena_index(),
                call.location.state.generation(),
                call.location.statement_ordinal,
                call.location.call_ordinal,
            )
        });
        checked_calls.dedup();
        if checked_calls
            .windows(2)
            .any(|calls| calls[0].location == calls[1].location)
        {
            return None;
        }
        self.checked_calls = checked_calls;
        Some(self)
    }

    pub fn checked_calls(&self) -> &[CheckedCrashCallSite] {
        &self.checked_calls
    }

    pub fn checked_call_at(
        &self,
        state: SymbolHandle,
        statement_ordinal: u32,
        call_ordinal: u32,
    ) -> Option<&CheckedCrashCallSite> {
        self.checked_calls.iter().find(|call| {
            call.location.state == state
                && call.location.statement_ordinal == statement_ordinal
                && call.location.call_ordinal == call_ordinal
        })
    }

    /// Published buckets whose guards and containment demands both cover this
    /// checked body site.
    pub fn covering_buckets_for_site<'plan>(
        &'plan self,
        site: &'plan CheckedCrashSite,
    ) -> impl Iterator<Item = (CrashRouteBucketId, &'plan CrashRouteBucket)> + 'plan {
        site.guard_covering_buckets.iter().filter_map(move |id| {
            self.published_bucket(*id).and_then(|bucket| {
                (bucket.cause == site.cause
                    && crash_scope_covers_minimum(
                        site.damage_minimum(),
                        bucket.containment_demand(),
                    ))
                .then_some((*id, bucket))
            })
        })
    }

    pub fn checked_site_at(
        &self,
        state: SymbolHandle,
        statement_ordinal: u32,
    ) -> Option<&CheckedCrashSite> {
        self.checked_sites.iter().find(|site| {
            site.location.state == state && site.location.statement_ordinal == statement_ordinal
        })
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MachineContractPlans {
    /// One entry per machine, in machine order.
    pub machines: Vec<MachineContractPlan>,
    /// Trait requirements and compile-time machine-parameter contracts do not
    /// own local machine plans. Their normalized callable identity and crash
    /// projection live here for modular call-site selection.
    pub crash_capsules: Vec<CrashContractCapsule>,
}

impl MachineContractPlans {
    pub fn for_machine(&self, machine: SymbolHandle) -> Option<&MachineContractPlan> {
        self.machines.iter().find(|plan| plan.machine == machine)
    }

    pub fn crash_capsule(
        &self,
        target_machine: SymbolHandle,
        target_state: SymbolHandle,
    ) -> Option<&CrashContractCapsule> {
        self.crash_capsules.iter().find(|capsule| {
            capsule.target_machine == target_machine && capsule.target_state == target_state
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineContractPlan {
    pub machine: SymbolHandle,
    /// How the machine is supplied (checked body / requirement / boundary).
    pub supply_mode: MachineSupplyMode,
    /// EFX: the durable symbol-resolved service contract.
    pub service_reach: ServiceReachPlan,
    /// Direct synchronous boundary edges, kept separate from service reach.
    pub synchronous_invocation: SynchronousInvocationPlan,
    /// Independent authored/inferred operational axes.
    pub suspension: SuspensionPlan,
    pub blocking: BlockingPlan,
    /// Canonical published crash ceiling plus independent checked body sites.
    /// Clause grouping, ordering, duplicate predicates, and `true` spelling do
    /// not survive into the published carrier; sites do not enter identity.
    pub crash: CrashPlan,
    /// Public omission and private derivation stay distinct. The ranking
    /// witness remains outside this interface carrier.
    pub termination: TerminationInterface,
    /// Body-derived, state-relative write frames. These are implementation
    /// evidence, not authored contract material, and therefore never enter
    /// `fingerprint` or specialization identity.
    pub inferred_write_frames: Vec<StateWriteFramePlan>,
    /// The deterministic identity over the published halves above. Stable
    /// across prover-strength changes and body edits that keep the declared
    /// surface.
    pub fingerprint: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StateWriteFramePlan {
    pub state: SymbolHandle,
    pub frame: psi_facts::NormalizedWriteFrame,
}

/// The slice-1 fingerprint: an FNV-1a fold over the published halves'
/// normalized encodings. Deterministic across programs for the same
/// declared surface (canonical service names are sorted/deduplicated; the
/// termination guarantee and supply mode are closed enums).
pub fn contract_fingerprint(
    supply_mode: MachineSupplyMode,
    published_service_names: &[String],
    invocation_interface: SynchronousInvocationInterface,
    published_invocations: &[String],
    suspension_interface: SuspensionInterface,
    blocking_interface: BlockingInterface,
    crash: &CrashPlan,
    termination: &TerminationInterface,
    canonical_facts: &[Vec<u8>],
) -> u64 {
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;
    let mut hash = OFFSET;
    let mut fold = |byte: u8| {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(PRIME);
    };
    fold(match supply_mode {
        MachineSupplyMode::CheckedBody => 1,
        MachineSupplyMode::Requirement => 2,
        MachineSupplyMode::Boundary => 3,
        MachineSupplyMode::Accepted => 4,
        // PRV4: the leaf's supply tag; the binding identity folds separately
        // below so two leaves with different bindings differ.
        MachineSupplyMode::ExternalRealization { .. } => 5,
    });
    if let MachineSupplyMode::ExternalRealization { binding } = supply_mode {
        for byte in binding.0.to_le_bytes() {
            fold(byte);
        }
    }
    // Boundary-service declaration identity, rendered canonically rather than
    // folding per-program row or service-table indices.
    fold(0xfb);
    let mut canonical_service_names = published_service_names.iter().collect::<Vec<_>>();
    canonical_service_names.sort_unstable();
    canonical_service_names.dedup();
    for name in canonical_service_names {
        for byte in name.as_bytes() {
            fold(*byte);
        }
        fold(0xfa);
    }
    fold(match invocation_interface {
        SynchronousInvocationInterface::InternalInferred => 1,
        SynchronousInvocationInterface::PublishedCeiling => 2,
    });
    let mut canonical_invocations = published_invocations.iter().collect::<Vec<_>>();
    canonical_invocations.sort_unstable();
    canonical_invocations.dedup();
    for invocation in canonical_invocations {
        for byte in invocation.as_bytes() {
            fold(*byte);
        }
        fold(0xf9);
    }
    fold(match suspension_interface {
        SuspensionInterface::InternalInferred => 1,
        SuspensionInterface::PublishedMaySuspend(false) => 2,
        SuspensionInterface::PublishedMaySuspend(true) => 3,
    });
    fold(match blocking_interface {
        BlockingInterface::InternalInferred => 1,
        BlockingInterface::PublishedMayBlock(false) => 2,
        BlockingInterface::PublishedMayBlock(true) => 3,
    });
    fold(0xf8);
    fold(match crash.interface {
        CrashInterface::InternalInferred => 1,
        CrashInterface::PublishedCeiling => 2,
    });
    let mut crash_buckets = crash.published.clone();
    crash_buckets.sort();
    crash_buckets.dedup();
    for bucket in crash_buckets {
        fold(match bucket.cause {
            CrashCause::Trap => 1,
            CrashCause::Abort => 2,
        });
        for byte in u32::try_from(bucket.containment_demand.len())
            .expect("crash containment-demand name exceeds the canonical encoding limit")
            .to_le_bytes()
        {
            fold(byte);
        }
        for byte in bucket.containment_demand.as_bytes() {
            fold(*byte);
        }
        for guard in bucket.alternative_guards {
            match guard {
                CrashRouteGuard::Truth => fold(0),
                CrashRouteGuard::Predicate(predicate) => {
                    fold(1);
                    for byte in predicate.canonical_bytes() {
                        fold(*byte);
                    }
                }
            }
            fold(0xf7);
        }
        fold(0xf6);
    }
    fold(0xff);
    match termination {
        TerminationInterface::InternalDerived => fold(0),
        TerminationInterface::Published(TerminationGuarantee::NoGuarantee) => fold(1),
        TerminationInterface::Published(TerminationGuarantee::Terminates { premises }) => {
            fold(2);
            for premise in premises {
                for byte in premise.0.to_le_bytes() {
                    fold(byte);
                }
            }
        }
    }
    // Slice 2: the declared requires/ensures facts, pre-sorted by the
    // caller (clause order never enters the identity).
    fold(0xfd);
    for fact in canonical_facts {
        for byte in fact {
            fold(*byte);
        }
        fold(0xfc);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crash_route_carriers_enforce_canonical_nonempty_sets() {
        assert!(CrashRouteBucket::new(CrashCause::Trap, "Activation", Vec::new()).is_none());

        let predicate = CrashPredicateIdentity::from_canonical_bytes(vec![1, 2, 3]);
        let guarded = CrashRouteBucket::new(
            CrashCause::Trap,
            "Activation",
            vec![
                CrashRouteGuard::Predicate(predicate.clone()),
                CrashRouteGuard::Predicate(predicate),
            ],
        )
        .expect("a guarded bucket is nonempty");
        assert_eq!(guarded.alternative_guards().len(), 1);

        let unconditional = CrashRouteBucket::new(
            CrashCause::Trap,
            "Activation",
            vec![
                CrashRouteGuard::Predicate(CrashPredicateIdentity::from_canonical_bytes(vec![4])),
                CrashRouteGuard::Truth,
            ],
        )
        .expect("truth contributes a route");
        assert!(unconditional.is_unconditional());

        let plan = CrashPlan::published_ceiling(vec![unconditional.clone(), unconditional]);
        assert_eq!(plan.published().len(), 1);
    }

    #[test]
    fn crash_sites_are_canonical_implementation_evidence() {
        let first_state = SymbolHandle::from_arena_index(4);
        let second_state = SymbolHandle::from_arena_index(9);
        let first_claim = psi_language_semantics::PermissionClaimIdentity::Established {
            machine_symbol: SymbolHandle::from_arena_index(2),
            state_symbol: first_state,
            source: psi_language_semantics::PermissionEventSource::StateEntry,
            ordinal: 0,
        };
        let second_claim = psi_language_semantics::PermissionClaimIdentity::Established {
            machine_symbol: SymbolHandle::from_arena_index(2),
            state_symbol: first_state,
            source: psi_language_semantics::PermissionEventSource::Statement { statement_index: 1 },
            ordinal: 1,
        };
        let path_guard = CrashPredicateIdentity::from_canonical_bytes(vec![1, 9, 0, 0, 0, 0]);
        let first = CheckedCrashSite::new(
            CrashSiteLocation::new(first_state, 2),
            CrashCause::Abort,
            Vec::new(),
            vec![second_claim, first_claim, second_claim],
        )
        .with_path_guard_conjuncts(vec![path_guard.clone(), path_guard.clone()]);
        let second = CheckedCrashSite::new(
            CrashSiteLocation::new(second_state, 0),
            CrashCause::Trap,
            Vec::new(),
            Vec::new(),
        );
        let plan = CrashPlan::default()
            .with_checked_sites(vec![second.clone(), first.clone(), first.clone()])
            .expect("one crash cause occupies each source site");

        assert_eq!(plan.checked_sites(), &[first.clone(), second]);
        assert_eq!(
            plan.checked_sites()[0].frontier_lower_bound(),
            &[first_claim, second_claim],
            "frontier identity is canonical and duplicate-free"
        );
        assert_eq!(
            plan.checked_sites()[0].path_guard_conjuncts(),
            &[path_guard]
        );
        assert_eq!(
            plan.checked_site_at(first_state, 2)
                .map(|site| site.cause()),
            Some(CrashCause::Abort)
        );
        assert_eq!(plan.interface(), CrashInterface::InternalInferred);

        assert!(
            CrashPlan::default()
                .with_checked_sites(vec![
                    first.clone(),
                    CheckedCrashSite::new(
                        first.location(),
                        CrashCause::Trap,
                        Vec::new(),
                        Vec::new(),
                    ),
                ])
                .is_none()
        );
        assert!(
            CrashPlan::default()
                .with_checked_sites(vec![CheckedCrashSite::new(
                    CrashSiteLocation::new(first_state, 3),
                    CrashCause::Abort,
                    Vec::new(),
                    vec![psi_language_semantics::PermissionClaimIdentity::Unknown],
                )])
                .is_none(),
            "an unknown claim identity cannot enter checked crash evidence"
        );
    }

    #[test]
    fn crash_calls_retain_empty_refinements_and_reject_coordinate_collisions() {
        let machine = SymbolHandle::from_arena_index(2);
        let state = SymbolHandle::from_arena_index(3);
        let location = CrashCallSiteLocation::new(state, 4, 1);
        let call = CheckedCrashCallSite::new(location, machine, state, 17, Vec::new());
        let plan = CrashPlan::default()
            .with_checked_calls(vec![call.clone(), call.clone()])
            .expect("an identical duplicate canonicalizes away");
        assert_eq!(plan.checked_calls(), &[call.clone()]);
        assert!(plan.checked_calls()[0].surviving_buckets().is_empty());
        assert!(plan.checked_call_at(state, 4, 1).is_some());

        let conflicting = CheckedCrashCallSite::new(
            location,
            SymbolHandle::from_arena_index(8),
            state,
            18,
            vec![CrashRouteBucket::unconditional(
                CrashCause::Abort,
                EXECUTION_DOMAIN_CRASH_SCOPE,
            )],
        );
        assert!(
            CrashPlan::default()
                .with_checked_calls(vec![call, conflicting])
                .is_none(),
            "one invocation coordinate cannot name two checked crash refinements"
        );
    }

    #[test]
    fn crash_contract_capsules_are_canonical_and_addressable() {
        let target_machine = SymbolHandle::from_arena_index(11);
        let target_state = SymbolHandle::from_arena_index(12);
        let capsule = CrashContractCapsule::new(
            target_machine,
            target_state,
            0xfeed,
            vec![
                CrashRouteBucket::unconditional(CrashCause::Abort, EXECUTION_DOMAIN_CRASH_SCOPE),
                CrashRouteBucket::unconditional(CrashCause::Trap, ACTIVATION_CRASH_SCOPE),
                CrashRouteBucket::unconditional(CrashCause::Abort, EXECUTION_DOMAIN_CRASH_SCOPE),
            ],
        );
        assert_eq!(capsule.published_buckets().len(), 2);
        let plans = MachineContractPlans {
            machines: Vec::new(),
            crash_capsules: vec![capsule],
        };
        assert_eq!(
            plans
                .crash_capsule(target_machine, target_state)
                .map(CrashContractCapsule::target_contract_fingerprint),
            Some(0xfeed)
        );
    }

    #[test]
    fn crash_bucket_ids_join_checked_sites_to_their_published_contract() {
        let plan = CrashPlan::published_ceiling(vec![
            CrashRouteBucket::unconditional(CrashCause::Abort, "ExecutionDomain"),
            CrashRouteBucket::unconditional(CrashCause::Trap, "Activation"),
        ]);
        let ids = plan
            .published_with_ids()
            .map(|(id, _)| id)
            .collect::<Vec<_>>();
        assert_eq!(ids.iter().map(|id| id.get()).collect::<Vec<_>>(), [1, 2]);
        for (id, bucket) in plan.published_with_ids() {
            assert_eq!(plan.published_bucket(id), Some(bucket));
        }

        let abort_id = plan
            .published_with_ids()
            .find_map(|(id, bucket)| (bucket.cause() == CrashCause::Abort).then_some(id))
            .expect("published abort bucket");
        let site = CheckedCrashSite::new(
            CrashSiteLocation::new(SymbolHandle::from_arena_index(4), 0),
            CrashCause::Abort,
            vec![abort_id, abort_id],
            Vec::new(),
        );
        let plan = plan
            .with_checked_sites(vec![site])
            .expect("site coverage cites a same-cause bucket");
        assert_eq!(
            plan.checked_sites()[0].guard_covering_buckets(),
            &[abort_id]
        );
    }

    #[test]
    fn crash_damage_minima_filter_guard_covering_buckets_independently() {
        let plan = CrashPlan::published_ceiling(vec![
            CrashRouteBucket::unconditional(CrashCause::Abort, ACTIVATION_CRASH_SCOPE),
            CrashRouteBucket::unconditional(CrashCause::Abort, EXECUTION_DOMAIN_CRASH_SCOPE),
        ]);
        let ids = plan
            .published_with_ids()
            .map(|(id, _)| id)
            .collect::<Vec<_>>();
        let site = CheckedCrashSite::new(
            CrashSiteLocation::new(SymbolHandle::from_arena_index(4), 0),
            CrashCause::Abort,
            ids,
            Vec::new(),
        );
        assert_eq!(site.damage_minimum(), EXECUTION_DOMAIN_CRASH_SCOPE);

        let covering = plan
            .covering_buckets_for_site(&site)
            .map(|(_, bucket)| bucket.containment_demand())
            .collect::<Vec<_>>();
        assert_eq!(covering, [EXECUTION_DOMAIN_CRASH_SCOPE]);
        assert!(crash_scope_covers_minimum(
            ACTIVATION_CRASH_SCOPE,
            EXECUTION_DOMAIN_CRASH_SCOPE
        ));
        assert!(!crash_scope_covers_minimum(
            EXECUTION_DOMAIN_CRASH_SCOPE,
            ACTIVATION_CRASH_SCOPE
        ));

        let trap = CheckedCrashSite::new(
            CrashSiteLocation::new(SymbolHandle::from_arena_index(5), 0),
            CrashCause::Trap,
            Vec::new(),
            Vec::new(),
        );
        assert_eq!(trap.damage_minimum(), ACTIVATION_CRASH_SCOPE);
        assert_eq!(
            trap.clone()
                .with_damage_minimum(EXECUTION_DOMAIN_CRASH_SCOPE)
                .expect("a trap minimum may widen to the portable top")
                .damage_minimum(),
            EXECUTION_DOMAIN_CRASH_SCOPE
        );
        assert!(
            site.with_damage_minimum(ACTIVATION_CRASH_SCOPE).is_none(),
            "an abort minimum cannot narrow below ExecutionDomain"
        );
    }

    #[test]
    fn operational_interfaces_participate_independently_in_contract_identity() {
        let fingerprint = |suspension, blocking| {
            contract_fingerprint(
                MachineSupplyMode::Boundary,
                &[],
                SynchronousInvocationInterface::PublishedCeiling,
                &[],
                suspension,
                blocking,
                &CrashPlan::default(),
                &TerminationInterface::Published(TerminationGuarantee::NoGuarantee),
                &[],
            )
        };
        let neither = fingerprint(
            SuspensionInterface::PublishedMaySuspend(false),
            BlockingInterface::PublishedMayBlock(false),
        );
        let suspending = fingerprint(
            SuspensionInterface::PublishedMaySuspend(true),
            BlockingInterface::PublishedMayBlock(false),
        );
        let blocking = fingerprint(
            SuspensionInterface::PublishedMaySuspend(false),
            BlockingInterface::PublishedMayBlock(true),
        );
        assert_ne!(neither, suspending);
        assert_ne!(neither, blocking);
        assert_ne!(suspending, blocking);
    }

    #[test]
    fn symbol_resolved_service_names_participate_in_contract_identity() {
        let fingerprint = |services: &[String]| {
            contract_fingerprint(
                MachineSupplyMode::Boundary,
                services,
                SynchronousInvocationInterface::PublishedCeiling,
                &[],
                SuspensionInterface::PublishedMaySuspend(false),
                BlockingInterface::PublishedMayBlock(false),
                &CrashPlan::default(),
                &TerminationInterface::Published(TerminationGuarantee::NoGuarantee),
                &[],
            )
        };
        let empty = fingerprint(&[]);
        let readable = fingerprint(&["Readable".to_owned()]);
        let queryable = fingerprint(&["Queryable".to_owned()]);
        let composite = fingerprint(&["Readable".to_owned(), "Queryable".to_owned()]);
        let reordered = fingerprint(&["Queryable".to_owned(), "Readable".to_owned()]);
        assert_ne!(empty, readable);
        assert_ne!(readable, queryable);
        assert_eq!(composite, reordered);
    }

    #[test]
    fn synchronous_invocation_ceiling_participates_in_contract_identity() {
        let fingerprint = |interface, invocations: &[String]| {
            contract_fingerprint(
                MachineSupplyMode::Boundary,
                &[],
                interface,
                invocations,
                SuspensionInterface::PublishedMaySuspend(false),
                BlockingInterface::PublishedMayBlock(false),
                &CrashPlan::default(),
                &TerminationInterface::Published(TerminationGuarantee::NoGuarantee),
                &[],
            )
        };
        let omitted = fingerprint(SynchronousInvocationInterface::PublishedCeiling, &[]);
        let handler = fingerprint(
            SynchronousInvocationInterface::PublishedCeiling,
            &["parameter:0".to_owned()],
        );
        let composite = fingerprint(
            SynchronousInvocationInterface::PublishedCeiling,
            &["service:Clock".to_owned(), "parameter:0".to_owned()],
        );
        let reordered = fingerprint(
            SynchronousInvocationInterface::PublishedCeiling,
            &["parameter:0".to_owned(), "service:Clock".to_owned()],
        );
        let private = fingerprint(SynchronousInvocationInterface::InternalInferred, &[]);
        assert_ne!(omitted, handler);
        assert_ne!(omitted, private);
        assert_eq!(composite, reordered);
    }

    #[test]
    fn internal_derivation_differs_from_published_omission() {
        let fingerprint = |termination| {
            contract_fingerprint(
                MachineSupplyMode::CheckedBody,
                &[],
                SynchronousInvocationInterface::InternalInferred,
                &[],
                SuspensionInterface::InternalInferred,
                BlockingInterface::InternalInferred,
                &CrashPlan::default(),
                termination,
                &[],
            )
        };
        assert_ne!(
            fingerprint(&TerminationInterface::InternalDerived),
            fingerprint(&TerminationInterface::Published(
                TerminationGuarantee::NoGuarantee
            ))
        );
    }
}

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CrashCause {
    Trap,
    Abort,
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

/// Source-handle-free location of one crash transition within a checked
/// machine body. State identity plus the statement's state-local ordinal is
/// stable against unrelated statement-arena insertions and is sufficient for
/// checked-tree consumers to join the derived site back to its body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CrashSiteLocation {
    state: SymbolHandle,
    statement_ordinal: u32,
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

/// Body-derived seed for a crash-terminator plan. This row deliberately does
/// not pretend that path-conditioned guard, damage minimum, coverage, or
/// frontier reconstruction has happened; those independent checked fields are
/// added by later CRASH-CONTRACT passes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckedCrashSite {
    location: CrashSiteLocation,
    cause: CrashCause,
}

impl CheckedCrashSite {
    pub const fn new(location: CrashSiteLocation, cause: CrashCause) -> Self {
        Self { location, cause }
    }

    pub const fn location(self) -> CrashSiteLocation {
        self.location
    }

    pub const fn cause(self) -> CrashCause {
        self.cause
    }
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
/// path guards, covering buckets, and frontier lower bounds enrich the site
/// layer without changing the published interface.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CrashPlan {
    interface: CrashInterface,
    published: Vec<CrashRouteBucket>,
    checked_sites: Vec<CheckedCrashSite>,
}

impl CrashPlan {
    pub fn published_ceiling(mut published: Vec<CrashRouteBucket>) -> Self {
        published.sort();
        published.dedup();
        Self {
            interface: CrashInterface::PublishedCeiling,
            published,
            checked_sites: Vec::new(),
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
        self.checked_sites = checked_sites;
        Some(self)
    }

    pub fn interface(&self) -> CrashInterface {
        self.interface
    }

    pub fn published(&self) -> &[CrashRouteBucket] {
        &self.published
    }

    pub fn checked_sites(&self) -> &[CheckedCrashSite] {
        &self.checked_sites
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
}

impl MachineContractPlans {
    pub fn for_machine(&self, machine: SymbolHandle) -> Option<&MachineContractPlan> {
        self.machines.iter().find(|plan| plan.machine == machine)
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
        let first =
            CheckedCrashSite::new(CrashSiteLocation::new(first_state, 2), CrashCause::Abort);
        let second =
            CheckedCrashSite::new(CrashSiteLocation::new(second_state, 0), CrashCause::Trap);
        let plan = CrashPlan::default()
            .with_checked_sites(vec![second, first, first])
            .expect("one crash cause occupies each source site");

        assert_eq!(plan.checked_sites(), &[first, second]);
        assert_eq!(
            plan.checked_site_at(first_state, 2)
                .map(|site| site.cause()),
            Some(CrashCause::Abort)
        );
        assert_eq!(plan.interface(), CrashInterface::InternalInferred);

        assert!(
            CrashPlan::default()
                .with_checked_sites(vec![
                    first,
                    CheckedCrashSite::new(first.location(), CrashCause::Trap),
                ])
                .is_none()
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

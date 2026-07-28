//! STR2 — the core semantic vocabulary (semantic-taxonomy migration rung 2;
//! record: wiki/architecture/semantic_taxonomy_representation.md).
//!
//! These are the settled distinctions the old shapes LOSE (the STR1 pins in
//! omega-typed-trees witness the loss): first-class multiplicity, the
//! machine supply mode, decision 23's termination guarantee/ranking-witness
//! firewall, and decision 22's kinded effect members. Landed here, in the
//! lowest dependency-safe crate, with NO consumer yet — rungs STR3+
//! propagate them through the trees and plans. Nothing in this module may
//! grow behavior: it is vocabulary, identity handles, and the invariants
//! spelled next to them.

/// First-class usage multiplicity (record §Multiplicity). Replaces `copy`
/// as the whole usage model: `[copy]` maps to `Unrestricted`, ordinary data
/// defaults to `Affine`, `[linear]` maps to `Linear`. Zero establishment is
/// derived independently, and carry policy remains an orthogonal property.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Multiplicity {
    /// Freely duplicable and discardable (`[copy]`).
    Unrestricted,
    /// Use at most once; silent discard is legal (ordinary data).
    #[default]
    Affine,
    /// Use exactly once; discard is an error (`[linear]`).
    Linear,
}

/// How a data declaration obtains its representation. A checked shape exposes
/// fields/cases for structural derivation. A boundary-opaque carrier exposes no
/// representation: it can be named in contracts, but only boundary providers
/// may establish values and any permissive property claim requires admission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DataSupplyMode {
    #[default]
    CheckedShape,
    BoundaryOpaque,
}

/// Whether a live value may cross a suspension point. This is checked locally
/// against `Suspend` reach; it is intentionally independent from migration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CarrySuspension {
    #[default]
    Forbidden,
    Allowed,
}

/// CPU affinity of a live value relative to the CPU recorded at mint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CarryCpu {
    #[default]
    Origin,
    Any,
}

/// Host-thread affinity of a live value relative to the thread recorded at mint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CarryHostThread {
    #[default]
    Origin,
    Any,
}

/// Whether a live value may move to a different storage address.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CarryAddress {
    #[default]
    Stable,
    Movable,
}

/// Normalized four-axis carry policy. The default is deliberately strict so
/// missing evidence fails closed. Transparent values may derive a more
/// permissive policy; opaque declarations require proof/admission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct CarryPolicy {
    pub suspension: CarrySuspension,
    pub cpu: CarryCpu,
    pub host_thread: CarryHostThread,
    pub address: CarryAddress,
}

impl CarryPolicy {
    pub const STRICT: Self = Self {
        suspension: CarrySuspension::Forbidden,
        cpu: CarryCpu::Origin,
        host_thread: CarryHostThread::Origin,
        address: CarryAddress::Stable,
    };

    pub const PERMISSIVE: Self = Self {
        suspension: CarrySuspension::Allowed,
        cpu: CarryCpu::Any,
        host_thread: CarryHostThread::Any,
        address: CarryAddress::Movable,
    };

    /// True when `self` permits every transition promised by `required`.
    pub const fn permits(self, required: Self) -> bool {
        (matches!(required.suspension, CarrySuspension::Forbidden)
            || matches!(self.suspension, CarrySuspension::Allowed))
            && (matches!(required.cpu, CarryCpu::Origin) || matches!(self.cpu, CarryCpu::Any))
            && (matches!(required.host_thread, CarryHostThread::Origin)
                || matches!(self.host_thread, CarryHostThread::Any))
            && (matches!(required.address, CarryAddress::Stable)
                || matches!(self.address, CarryAddress::Movable))
    }

    /// Structural composition for aggregate fields: each axis takes the most
    /// restrictive demand contributed by either live field.
    pub const fn intersect(self, other: Self) -> Self {
        Self {
            suspension: if matches!(self.suspension, CarrySuspension::Allowed)
                && matches!(other.suspension, CarrySuspension::Allowed)
            {
                CarrySuspension::Allowed
            } else {
                CarrySuspension::Forbidden
            },
            cpu: if matches!(self.cpu, CarryCpu::Any) && matches!(other.cpu, CarryCpu::Any) {
                CarryCpu::Any
            } else {
                CarryCpu::Origin
            },
            host_thread: if matches!(self.host_thread, CarryHostThread::Any)
                && matches!(other.host_thread, CarryHostThread::Any)
            {
                CarryHostThread::Any
            } else {
                CarryHostThread::Origin
            },
            address: if matches!(self.address, CarryAddress::Movable)
                && matches!(other.address, CarryAddress::Movable)
            {
                CarryAddress::Movable
            } else {
                CarryAddress::Stable
            },
        }
    }
}

impl std::fmt::Display for CarryPolicy {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let suspension = match self.suspension {
            CarrySuspension::Forbidden => "forbidden",
            CarrySuspension::Allowed => "allowed",
        };
        let cpu = match self.cpu {
            CarryCpu::Origin => "same",
            CarryCpu::Any => "any",
        };
        let thread = match self.host_thread {
            CarryHostThread::Origin => "same",
            CarryHostThread::Any => "any",
        };
        let address = match self.address {
            CarryAddress::Stable => "stable",
            CarryAddress::Movable => "movable",
        };
        write!(
            formatter,
            "carry(suspension: {suspension}, cpu: {cpu}, thread: {thread}, address: {address})"
        )
    }
}

/// Semantic ownership-event roles. Shared by checked flow and every lowered
/// semantic summary so no stage can reinterpret a generic move/drop marker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionEventKind {
    Establish,
    Transfer,
    Consume,
    AffineDrop,
}

/// Access carried by one permission-context entry. Ownership events use
/// `Owned`; borrow loans use `Shared` or `Exclusive`. Keeping this axis
/// separate from multiplicity prevents a shared loan from being mistaken for
/// a copyable owned value (or an exclusive loan for a linear value).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PermissionAccess {
    #[default]
    Owned,
    Shared,
    Exclusive,
}

impl Default for PermissionEventKind {
    fn default() -> Self {
        Self::Transfer
    }
}

/// Stable source identity for a permission event across IR stages.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionEventSource {
    StateEntry,
    Statement {
        statement_index: usize,
    },
    Call {
        statement_index: usize,
        call_ordinal: usize,
        target_symbol: crate::symbols::SymbolHandle,
    },
    StateExit,
}

impl Default for PermissionEventSource {
    fn default() -> Self {
        Self::StateEntry
    }
}

/// Stable origin of the semantic value/obligation carried by a permission
/// event. Transfers preserve this value; they do not mint a fresh origin.
/// `Unknown` is retained only while a legacy compatibility producer cannot
/// identify where an affine value was established.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PermissionProvenance {
    #[default]
    Unknown,
    Established {
        machine_symbol: crate::symbols::SymbolHandle,
        state_symbol: crate::symbols::SymbolHandle,
        source: PermissionEventSource,
    },
}

/// How a machine is supplied to its consumers (record §Machines). The old
/// `boundary: bool` conflates all four; provider admission, proof
/// artifacts, manifests, and lowering must consume THIS, not re-derive
/// supply from syntax and lookup context.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MachineSupplyMode {
    /// An ordinary checked body compiled in this program (the ZII default).
    #[default]
    CheckedBody,
    /// A requirement slot: the signature is the contract; a provider is
    /// admitted against it.
    Requirement,
    /// A boundary declaration: supplied by the host/component seam, claims
    /// gated by grants.
    Boundary,
    /// An accepted (axiom-tier) declaration: trusted without proof, shown
    /// in the trust report.
    Accepted,
    /// PRV4: an irreducible external leaf -- `satisfies Requirement via
    /// <Binding>;` on a bodyless machine. The satisfied requirement supplies
    /// the public contract/effect ceiling; the normalized binding is the
    /// realization the lowering consumes. Composite lowerings are ordinary
    /// CheckedBody machines and never carry a binding.
    ExternalRealization { binding: ExternalBindingId },
}

/// Decision 23's PUBLIC half: the eventual-terminal guarantee that
/// participates in published machine-contract and import-slot identity.
/// The premises are explicit; an exported omission normalizes to
/// `NoGuarantee` (never to an implied promise).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum TerminationGuarantee {
    #[default]
    NoGuarantee,
    EventualTerminal {
        /// Progress-profile premises the guarantee is conditional on
        /// (sealed semantic commitments with grant/receipt identity).
        premises: Vec<ProgressProfileId>,
    },
}

/// Decision 23's PRIVATE half: the ranking witness proving one body. It
/// feeds checker legality, proof-cache identity, diagnostics, and
/// provider-local revalidation — and NEVER enters published contract
/// identity (the record's ordering constraint: swapping one valid witness
/// for another revalidates only the provider/proof artifact).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RankingWitness {
    /// The ranked subjects (parameter/field names, in rank order).
    pub subjects: Vec<String>,
    /// The canonical ranking view. Stable defaults elaborate IMMEDIATELY
    /// to an explicit view — the checker never selects a noncanonical view
    /// heuristically. `NULL` when the view is a user-declared measure (its
    /// normalized identity lands with the TPR3 checker migration) or while
    /// a single-subject short form awaits its type-directed elaboration.
    pub ranking_view: RankingViewId,
    /// The explicit, elaborated view SPELLING (`Nat::Descending`,
    /// `Card::PowerOrder`) — the witness is private, so a rendered path is
    /// its honest identity carrier for diagnostics and proof-cache keys.
    /// Empty ONLY while a single-subject short form awaits type-directed
    /// elaboration (the one canonical-default case that needs the subject's
    /// carrier type; TPR3 completes it inside the migrated checker).
    pub view_path: String,
    /// An ARGUMENTED view's arguments (`Nat::IncreasingTo(limit)` carries
    /// `["limit"]`), rendered source-like in order; empty for plain views.
    /// The bound is part of the view — an unbounded increasing view is not
    /// a valid ranking.
    pub view_arguments: Vec<String>,
    /// The optional `in <range>` constraint on the RANK the view produces
    /// (TPR3, decision 23): a termination FACT, no storage. Authored
    /// material like the subjects — the checker verifies it structurally
    /// and FAILS compilation otherwise, so a compiled artifact never
    /// carries an unverified range.
    pub rank_range: Option<RankRange>,
}

/// The rank-range fact (`in 0..=capacity`), rendered source-like. Its floor
/// establishes the well-founded floor; the ceiling bounds the produced rank.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RankRange {
    pub floor: String,
    pub ceiling: String,
    pub ceiling_inclusive: bool,
}

/// The interface/implementation split for one machine's termination story
/// (record §Machines): the published guarantee is contract identity, the
/// checked summary serves local consumers, the witness stays private.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum TerminationInterface {
    /// A private checked body publishes no external progress promise. Local
    /// checked consumers may still use its derived summary.
    #[default]
    InternalDerived,
    /// A requirement/export/provider-facing machine publishes this exact
    /// promise. Omission on that public surface is `Published(NoGuarantee)`.
    Published(TerminationGuarantee),
}

impl TerminationInterface {
    pub fn published(&self) -> Option<&TerminationGuarantee> {
        match self {
            Self::InternalDerived => None,
            Self::Published(guarantee) => Some(guarantee),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MachineTerminationPlan {
    pub interface: TerminationInterface,
    /// What the checker established for THIS body (local consumers only).
    pub checked_summary: TerminationGuarantee,
    /// The private proof material, if a ranked body carried one.
    pub implementation_witness: Option<RankingWitness>,
}

macro_rules! semantic_id {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
        pub struct $name(pub u32);

        impl $name {
            /// The ZII-inert null identity (index 0 is reserved).
            pub const NULL: Self = Self(0);

            pub fn is_valid(self) -> bool {
                self.0 != 0
            }
        }
    };
}

semantic_id!(
    /// Normalized semantic-domain identity (record §Domain theory): the
    /// deterministic normalizer owns it; checked types/bindings carry it;
    /// layout keeps using the carrier ABI (semantic interface identity and
    /// physical ABI identity are DISTINCT and both queryable).
    SemanticDomainId
);
semantic_id!(
    /// Normalized identity of one boundary-service trait. Ordinary traits and
    /// operational may-clauses never receive this identity.
    ServiceReachId
);
semantic_id!(
    /// Normalized service-reach row identity (service set + parent closure).
    /// Suspension and blocking are deliberately absent from this identity.
    ServiceReachRowId
);
semantic_id!(
    /// A sealed boundary progress profile (grant/receipt identity);
    /// participates in provider admission, outside the ordinary proof-fact
    /// catalog in v1.
    ProgressProfileId
);
semantic_id!(
    /// A normalized EXTERNAL-BINDING identity (PRV4 step 1): the rendered,
    /// compile-time-evaluable `via <Binding>` expression of an
    /// ExternalRealization leaf, interned so supply modes stay Copy and two
    /// spellings of one binding share one identity.
    ExternalBindingId
);
semantic_id!(
    /// A canonical ranking view (e.g. `Nat::Descending`); the witness names
    /// it explicitly, defaults elaborate at once.
    RankingViewId
);

/// Whether a machine's service reach is inferred privately or published as a
/// stable caller/provider ceiling. Published omission is represented by an
/// explicit empty row, never by `InternalInferred`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ServiceReachInterface {
    #[default]
    InternalInferred,
    PublishedCeiling(ServiceReachRowId),
}

/// The service-reach contract and checked body summary. The checked summary
/// may refine a published ceiling but may never widen it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ServiceReachPlan {
    pub interface: ServiceReachInterface,
    pub checked_inferred: ServiceReachRowId,
}

/// Whether suspension is inferred privately or published as an independent
/// may-ceiling. `PublishedMaySuspend(false)` is the public negative guarantee
/// produced by omitting `suspends;` on an export or requirement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SuspensionInterface {
    #[default]
    InternalInferred,
    PublishedMaySuspend(bool),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SuspensionPlan {
    pub interface: SuspensionInterface,
    pub checked_may_suspend: bool,
}

/// Whether worker blocking is inferred privately or published as an
/// independent may-ceiling. `PublishedMayBlock(false)` is the public negative
/// guarantee produced by omitting `blocks;` on an export or requirement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BlockingInterface {
    #[default]
    InternalInferred,
    PublishedMayBlock(bool),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BlockingPlan {
    pub interface: BlockingInterface,
    pub checked_may_block: bool,
}

/// Canonical service reach attached to one flow/graph scope. Rows index the
/// representation root's shared `ServiceReachRowTable`; no spelling or numeric
/// compatibility bit is stored on individual nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ServiceReachSummary {
    pub direct: ServiceReachRowId,
    pub transitive: ServiceReachRowId,
}

/// Independent operational possibilities attached to one flow/graph scope.
/// These booleans are never reconstructed from service rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OperationalMaySummary {
    pub direct_may_suspend: bool,
    pub transitive_may_suspend: bool,
    pub direct_may_block: bool,
    pub transitive_may_block: bool,
}

/// The BUILTIN canonical ranking-view catalog (decision 23, TPR2). The ids
/// are FIXED (deterministic across programs — they may enter proof-cache
/// keys); user-declared measures are NOT here (they get per-program
/// normalized identity with the TPR3 checker migration and carry
/// `RankingViewId::NULL` until then).
impl RankingViewId {
    /// `Nat::Descending` — an unsigned/bounded scalar counting down.
    pub const NAT_DESCENDING: Self = Self(1);
    /// `Nat::BoundedDistance` — a `(lower, upper)` pair ranked by the
    /// distance from `lower` up to the fixed `upper`; the only builtin
    /// two-subject view (and therefore the two-subject short-form default).
    pub const NAT_BOUNDED_DISTANCE: Self = Self(2);
    /// `Slice::Length` — a slice decreasing by its length.
    pub const SLICE_LENGTH: Self = Self(3);
    /// `Nat::IncreasingTo(limit)` — a cursor climbing toward the bound the
    /// view NAMES (the bound is part of the view: this is well-founded
    /// because the distance to `limit` descends; an unbounded `Increasing`
    /// is not a valid ranking).
    pub const NAT_INCREASING_TO: Self = Self(4);

    /// Look up a builtin canonical view by its explicit spelling (the BASE
    /// path — an argumented view's arguments live beside it).
    pub fn canonical(path: &str) -> Option<Self> {
        match path {
            "Nat::Descending" => Some(Self::NAT_DESCENDING),
            "Nat::BoundedDistance" => Some(Self::NAT_BOUNDED_DISTANCE),
            "Slice::Length" => Some(Self::SLICE_LENGTH),
            "Nat::IncreasingTo" => Some(Self::NAT_INCREASING_TO),
            _ => None,
        }
    }

    /// The explicit spelling of a builtin canonical view.
    pub fn canonical_path(self) -> Option<&'static str> {
        match self {
            Self::NAT_DESCENDING => Some("Nat::Descending"),
            Self::NAT_BOUNDED_DISTANCE => Some("Nat::BoundedDistance"),
            Self::SLICE_LENGTH => Some("Slice::Length"),
            Self::NAT_INCREASING_TO => Some("Nat::IncreasingTo"),
            _ => None,
        }
    }
}

/// Deterministic normalizer for service-only rows. Boundary-trait identity is
/// minted before rows are interned; this table owns set normalization and
/// preserves the empty published ceiling as the fixed row id 1.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ServiceReachRowTable {
    rows: Vec<Vec<ServiceReachId>>,
}

impl ServiceReachRowTable {
    pub const EMPTY_ROW: ServiceReachRowId = ServiceReachRowId(1);

    pub fn intern(&mut self, mut services: Vec<ServiceReachId>) -> ServiceReachRowId {
        services.sort_by_key(|service| service.0);
        services.dedup();
        if self.rows.is_empty() {
            self.rows.push(Vec::new());
        }
        if let Some(position) = self.rows.iter().position(|row| *row == services) {
            return ServiceReachRowId(u32::try_from(position + 1).expect("row table fits u32"));
        }
        self.rows.push(services);
        ServiceReachRowId(u32::try_from(self.rows.len()).expect("row table fits u32"))
    }

    pub fn services(&self, row: ServiceReachRowId) -> &[ServiceReachId] {
        row.0
            .checked_sub(1)
            .and_then(|index| self.rows.get(index as usize))
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }
}

/// One normalized boundary-service declaration. `symbol` is the resolved
/// declaration identity used inside a compilation; `name` is retained for
/// diagnostics and artifact rendering. Parent closure is normalized once from
/// resolved boundary-trait composition and never reconstructed from spelling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceReachDefinition {
    pub symbol: crate::symbols::SymbolHandle,
    pub name: String,
    pub parents: Vec<ServiceReachId>,
}

/// Deterministic registry of boundary-service identities. The resolved-tree
/// normalizer interns declarations in canonical name order after symbol
/// assignment, so unrelated source ordering does not perturb row order.
/// Ordinary traits never enter this table.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ServiceReachTable {
    definitions: Vec<ServiceReachDefinition>,
}

impl ServiceReachTable {
    pub fn intern(&mut self, symbol: crate::symbols::SymbolHandle, name: &str) -> ServiceReachId {
        if let Some((index, _)) = self
            .definitions
            .iter()
            .enumerate()
            .find(|(_, definition)| definition.symbol == symbol)
        {
            return ServiceReachId(u32::try_from(index + 1).expect("service table fits u32"));
        }
        self.definitions.push(ServiceReachDefinition {
            symbol,
            name: name.to_owned(),
            parents: Vec::new(),
        });
        ServiceReachId(u32::try_from(self.definitions.len()).expect("service table fits u32"))
    }

    pub fn id_for_symbol(&self, symbol: crate::symbols::SymbolHandle) -> Option<ServiceReachId> {
        self.definitions
            .iter()
            .position(|definition| definition.symbol == symbol)
            .map(|index| ServiceReachId(u32::try_from(index + 1).expect("service table fits u32")))
    }

    /// Resolve a canonical authored service name to its symbol-backed
    /// identity. This is intentionally an exact, case-sensitive lookup: the
    /// table contains declarations, not the retired global effect catalog.
    pub fn id_for_name(&self, name: &str) -> Option<ServiceReachId> {
        self.definitions
            .iter()
            .position(|definition| definition.name == name)
            .map(|index| ServiceReachId(u32::try_from(index + 1).expect("service table fits u32")))
    }

    pub fn definition(&self, id: ServiceReachId) -> Option<&ServiceReachDefinition> {
        id.0.checked_sub(1)
            .and_then(|index| self.definitions.get(index as usize))
    }

    pub fn definitions(&self) -> &[ServiceReachDefinition] {
        &self.definitions
    }

    pub fn set_parents(&mut self, id: ServiceReachId, mut parents: Vec<ServiceReachId>) {
        parents.sort_by_key(|parent| parent.0);
        parents.dedup();
        if let Some(definition) =
            id.0.checked_sub(1)
                .and_then(|index| self.definitions.get_mut(index as usize))
        {
            definition.parents = parents;
        }
    }

    /// Append `service` and its already-normalized parent closure.
    pub fn extend_closure(&self, service: ServiceReachId, services: &mut Vec<ServiceReachId>) {
        if services.contains(&service) {
            return;
        }
        services.push(service);
        if let Some(definition) = self.definition(service) {
            for parent in &definition.parents {
                self.extend_closure(*parent, services);
            }
        }
    }
}

/// PRV4 step 1: the deterministic EXTERNAL-BINDING interner -- normalized
/// rendered `via` bindings, minted in declaration order. `NULL`/0 stays
/// "not computed"; ids start at 1.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExternalBindingTable {
    renderings: Vec<String>,
}

impl ExternalBindingTable {
    pub fn intern(&mut self, rendering: &str) -> ExternalBindingId {
        if let Some(index) = self
            .renderings
            .iter()
            .position(|existing| existing == rendering)
        {
            return ExternalBindingId(index as u32 + 1);
        }
        self.renderings.push(rendering.to_owned());
        ExternalBindingId(self.renderings.len() as u32)
    }

    pub fn rendering(&self, id: ExternalBindingId) -> Option<&str> {
        id.0.checked_sub(1)
            .and_then(|index| self.renderings.get(index as usize))
            .map(String::as_str)
    }
}

/// Decision 19/22 (STR4 checked plans, slice 1): the deterministic
/// SEMANTIC-DOMAIN interner -- normalized domain identity is the declared
/// NAME, minted in declaration order (deterministic because lowering order
/// is; presentation is excluded from identity per the facets brief).
/// `NULL`/0 stays "not computed"; ids start at 1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticDomainTable {
    names: Vec<String>,
}

impl Default for SemanticDomainTable {
    fn default() -> Self {
        // The compiler-blessed arithmetic policies (the closed semantic-facet
        // subset, decision 17/19) PRE-SEED with FIXED identities -- ids 1-3
        // are deterministic across programs (proof-cache-safe); declared
        // domains follow in declaration order.
        Self {
            names: vec![
                "Wrapping".to_owned(),
                "Saturating".to_owned(),
                "Trapping".to_owned(),
            ],
        }
    }
}

impl SemanticDomainTable {
    /// The fixed identity of the `Wrapping` arithmetic policy.
    pub const WRAPPING: SemanticDomainId = SemanticDomainId(1);
    /// The fixed identity of the `Saturating` arithmetic policy.
    pub const SATURATING: SemanticDomainId = SemanticDomainId(2);
    /// The fixed identity of the `Trapping` arithmetic policy.
    pub const TRAPPING: SemanticDomainId = SemanticDomainId(3);

    /// Intern a declared domain name and return its identity (idempotent).
    pub fn intern(&mut self, name: &str) -> SemanticDomainId {
        if let Some(position) = self.names.iter().position(|candidate| candidate == name) {
            return SemanticDomainId(u32::try_from(position + 1).expect("domain table fits u32"));
        }
        self.names.push(name.to_owned());
        SemanticDomainId(u32::try_from(self.names.len()).expect("domain table fits u32"))
    }

    /// The declared name of an interned identity (`None` for NULL/unknown).
    pub fn name(&self, id: SemanticDomainId) -> Option<&str> {
        id.0.checked_sub(1)
            .and_then(|index| self.names.get(index as usize))
            .map(String::as_str)
    }

    /// Look up an existing identity without minting.
    pub fn lookup(&self, name: &str) -> Option<SemanticDomainId> {
        self.names
            .iter()
            .position(|candidate| candidate == name)
            .map(|position| {
                SemanticDomainId(u32::try_from(position + 1).expect("domain table fits u32"))
            })
    }
}

/// The domain-theory facet PAIR (record §Domain theory): optional facets,
/// NOT a mutually exclusive enum — hybrids are first-class. The facet
/// bodies land with STR3+ (they need tree vocabulary); the skeleton lands
/// now so no checked-stage query ever infers predicate-vs-semantic behavior
/// by testing whether a domain happens to have facts or operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DomainFacets {
    pub predicate: bool,
    pub semantic: Option<SemanticDomainId>,
}

/// Whether a declared domain carries a predicate body.
///
/// This is explicit domain-theory metadata rather than something consumers
/// may reconstruct from the current number of lowered facts. A semicolon
/// declaration and an empty braced declaration are both `Bodyless`; an
/// explicitly universal `{ true; }` declaration is `Present`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DomainPredicateBody {
    #[default]
    Bodyless,
    Present,
}

impl DomainPredicateBody {
    pub const fn is_present(self) -> bool {
        matches!(self, Self::Present)
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Bodyless => "bodyless",
            Self::Present => "present",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_domain_table_is_deterministic_and_idempotent() {
        // Identity = declaration order; re-interning the same name returns
        // the same id; NULL never resolves to a name.
        let mut table = SemanticDomainTable::default();
        // Policies pre-seed with FIXED ids; declared domains follow.
        assert_eq!(
            table.lookup("Wrapping"),
            Some(SemanticDomainTable::WRAPPING)
        );
        assert_eq!(
            table.lookup("Saturating"),
            Some(SemanticDomainTable::SATURATING)
        );
        assert_eq!(
            table.lookup("Trapping"),
            Some(SemanticDomainTable::TRAPPING)
        );
        let kilometres = table.intern("Km");
        assert_eq!(kilometres, SemanticDomainId(4));
        assert_eq!(table.intern("Km"), kilometres);
        assert_eq!(table.name(kilometres), Some("Km"));
        // Re-interning a policy returns its fixed id, never a duplicate.
        assert_eq!(table.intern("Wrapping"), SemanticDomainTable::WRAPPING);
        assert_eq!(table.name(SemanticDomainId::NULL), None);
        assert_eq!(table.lookup("Miles"), None);
    }

    #[test]
    fn domain_predicate_body_distinguishes_bodyless_from_explicit_predicates() {
        assert_eq!(
            DomainPredicateBody::default(),
            DomainPredicateBody::Bodyless
        );
        assert!(!DomainPredicateBody::Bodyless.is_present());
        assert!(DomainPredicateBody::Present.is_present());
        assert_eq!(DomainPredicateBody::Bodyless.as_str(), "bodyless");
        assert_eq!(DomainPredicateBody::Present.as_str(), "present");
    }

    #[test]
    fn multiplicity_default_is_affine() {
        // Ordinary data defaults to Affine (the record's mapping); `[copy]`
        // opts into Unrestricted, `[linear]` into Linear.
        assert_eq!(Multiplicity::default(), Multiplicity::Affine);
    }

    #[test]
    fn termination_guarantee_default_is_no_guarantee() {
        // Exported omission normalizes to NoGuarantee — never an implied
        // promise.
        assert_eq!(
            TerminationGuarantee::default(),
            TerminationGuarantee::NoGuarantee
        );
    }

    #[test]
    fn witness_stays_out_of_the_published_half() {
        // The plan SHAPE enforces the firewall: the witness lives beside
        // the published guarantee, never inside it — equality of two plans'
        // published halves is witness-blind by construction.
        let with_witness = MachineTerminationPlan {
            interface: TerminationInterface::Published(TerminationGuarantee::NoGuarantee),
            checked_summary: TerminationGuarantee::NoGuarantee,
            implementation_witness: Some(RankingWitness::default()),
        };
        let without_witness = MachineTerminationPlan {
            implementation_witness: None,
            ..with_witness.clone()
        };
        assert_eq!(with_witness.interface, without_witness.interface);
    }

    #[test]
    fn semantic_ids_are_zii_inert() {
        assert!(!SemanticDomainId::default().is_valid());
        assert!(!ServiceReachId::default().is_valid());
        assert!(!ServiceReachRowId::default().is_valid());
    }

    #[test]
    fn service_rows_exclude_operational_axes_and_normalize_as_sets() {
        let readable = ServiceReachId(2);
        let queryable = ServiceReachId(3);
        let mut rows = ServiceReachRowTable::default();
        assert_eq!(rows.intern(Vec::new()), ServiceReachRowTable::EMPTY_ROW);
        let combined = rows.intern(vec![queryable, readable]);
        assert_eq!(rows.intern(vec![readable, queryable, readable]), combined);
        assert_eq!(rows.services(combined), &[readable, queryable]);
        assert_eq!(
            rows.services(ServiceReachRowId::NULL),
            &[] as &[ServiceReachId]
        );
    }

    #[test]
    fn service_table_resolves_exact_canonical_names() {
        let mut services = ServiceReachTable::default();
        let machine_control = services.intern(
            crate::symbols::SymbolHandle::from_parts(7, 1),
            "MachineControl",
        );
        assert_eq!(
            services.id_for_name("MachineControl"),
            Some(machine_control)
        );
        assert_eq!(services.id_for_name("machine_control"), None);
        assert_eq!(services.id_for_name("PortIo"), None);
    }

    #[test]
    fn operational_plans_distinguish_private_inference_from_public_omission() {
        assert_eq!(
            SuspensionPlan::default().interface,
            SuspensionInterface::InternalInferred
        );
        assert_eq!(
            BlockingPlan::default().interface,
            BlockingInterface::InternalInferred
        );

        let public_non_suspending = SuspensionPlan {
            interface: SuspensionInterface::PublishedMaySuspend(false),
            checked_may_suspend: false,
        };
        let public_non_blocking = BlockingPlan {
            interface: BlockingInterface::PublishedMayBlock(false),
            checked_may_block: false,
        };
        assert_ne!(public_non_suspending, SuspensionPlan::default());
        assert_ne!(public_non_blocking, BlockingPlan::default());

        let independently_suspending = SuspensionPlan {
            interface: SuspensionInterface::PublishedMaySuspend(true),
            checked_may_suspend: true,
        };
        assert!(independently_suspending.checked_may_suspend);
        assert!(!public_non_blocking.checked_may_block);
    }

    #[test]
    fn canonical_view_catalog_round_trips() {
        // Fixed, deterministic ids: the catalog may enter proof-cache keys,
        // so a builtin's id and spelling must round-trip exactly.
        for id in [
            RankingViewId::NAT_DESCENDING,
            RankingViewId::NAT_BOUNDED_DISTANCE,
            RankingViewId::SLICE_LENGTH,
            RankingViewId::NAT_INCREASING_TO,
        ] {
            assert!(id.is_valid());
            let path = id.canonical_path().expect("builtin has a spelling");
            assert_eq!(RankingViewId::canonical(path), Some(id));
        }
        // Declared measures are NOT canonical builtins.
        assert_eq!(RankingViewId::canonical("Card::PowerOrder"), None);
        assert_eq!(RankingViewId::NULL.canonical_path(), None);
    }

    #[test]
    fn carry_axes_compose_independently_and_fail_closed() {
        let cpu_local = CarryPolicy {
            suspension: CarrySuspension::Allowed,
            cpu: CarryCpu::Origin,
            host_thread: CarryHostThread::Any,
            address: CarryAddress::Movable,
        };
        let pinned = CarryPolicy {
            suspension: CarrySuspension::Allowed,
            cpu: CarryCpu::Any,
            host_thread: CarryHostThread::Any,
            address: CarryAddress::Stable,
        };

        assert_eq!(
            cpu_local.intersect(pinned),
            CarryPolicy {
                suspension: CarrySuspension::Allowed,
                cpu: CarryCpu::Origin,
                host_thread: CarryHostThread::Any,
                address: CarryAddress::Stable,
            }
        );
        assert!(CarryPolicy::PERMISSIVE.permits(cpu_local));
        assert!(!CarryPolicy::STRICT.permits(cpu_local));
        assert_eq!(CarryPolicy::default(), CarryPolicy::STRICT);
    }
}

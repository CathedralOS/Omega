#![forbid(unsafe_code)]

//! Target-neutral resolved-language semantic identities, tables, and plans.
//!
//! This foundation vocabulary is shared by symbol-resolved and later Psi
//! representations. It contains no checking pass, target realization, or
//! backend policy.

pub mod byte_predicates;
pub mod const_value;
pub mod content;
pub mod declaration_selection;
pub mod quotient_correspondence;
pub mod type_identity;
pub mod value_domain;
pub mod wire;

pub use psi_language_core::{
    CallOperationalAcknowledgement, CallOperationalAcknowledgementOrigin, CarryAddress, CarryCpu,
    CarryHostThread, CarryPermission, CarryPolicy, CarrySuspension, DataSupplyMode,
    DomainClassification, DomainPredicateBody, Multiplicity, ReferenceAccess,
};

/// Semantic ownership-event roles. Shared by checked flow and every lowered
/// semantic summary so no stage can reinterpret a generic move/drop marker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PermissionEventKind {
    Establish,
    #[default]
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

/// Stable source identity for a permission event across IR stages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PermissionEventSource {
    #[default]
    StateEntry,
    Statement {
        statement_index: usize,
    },
    Call {
        statement_index: usize,
        call_ordinal: usize,
        target_symbol: psi_symbols::SymbolHandle,
    },
    StateExit,
}

/// Stable origin of the semantic value/obligation carried by a permission
/// event. Transfers preserve this value; they do not mint a fresh origin.
/// `Unknown` is retained for permission events whose producer cannot yet
/// identify where an affine value was established.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PermissionProvenance {
    #[default]
    Unknown,
    Established {
        machine_symbol: psi_symbols::SymbolHandle,
        state_symbol: psi_symbols::SymbolHandle,
        source: PermissionEventSource,
    },
}

/// Identity of one permission/resource claim, independent of its current place
/// and root-lineage provenance. Transfers preserve this identity. A resource
/// transformation may establish fresh child identities while retaining the
/// same [`PermissionProvenance`] lineage.
///
/// The ordinal distinguishes claims established at the same semantic source
/// (for example, multiple linear fields entering one state). It is allocated
/// deterministically by the checked ownership pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PermissionClaimIdentity {
    #[default]
    Unknown,
    Established {
        machine_symbol: psi_symbols::SymbolHandle,
        state_symbol: psi_symbols::SymbolHandle,
        source: PermissionEventSource,
        ordinal: u32,
    },
}

/// Closed mechanism tag for one irreducible external realization. This is
/// retained independently from the transitional binding interner so semantic
/// consumers never classify a rendered `Binding::Case(...)` string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalBindingMechanism {
    Import,
    Syscall,
    CompilerIntrinsic,
    VtableSlot,
    VtableField,
    TableFunction,
}

impl ExternalBindingMechanism {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Import => "import",
            Self::Syscall => "syscall",
            Self::CompilerIntrinsic => "compiler_intrinsic",
            Self::VtableSlot => "vtable_slot",
            Self::VtableField => "vtable_field",
            Self::TableFunction => "table_function",
        }
    }

    pub const fn identity_tag(self) -> u8 {
        match self {
            Self::Import => 1,
            Self::Syscall => 2,
            Self::CompilerIntrinsic => 3,
            Self::VtableSlot => 4,
            Self::VtableField => 5,
            Self::TableFunction => 6,
        }
    }
}

/// Closed, structural identity for one irreducible external binding. These
/// values are interned directly; no display rendering is parsed or compared.
/// Foreign library/symbol fields remain bootstrap strings until their nominal
/// ids move into the target package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExternalBindingIdentity {
    Import { library: String, symbol: String },
    Syscall { number: i64 },
    CompilerIntrinsic,
    VtableSlot { index: i64 },
    VtableField { field: String },
    TableFunction { field: String },
}

impl ExternalBindingIdentity {
    pub const fn mechanism(&self) -> ExternalBindingMechanism {
        match self {
            Self::Import { .. } => ExternalBindingMechanism::Import,
            Self::Syscall { .. } => ExternalBindingMechanism::Syscall,
            Self::CompilerIntrinsic => ExternalBindingMechanism::CompilerIntrinsic,
            Self::VtableSlot { .. } => ExternalBindingMechanism::VtableSlot,
            Self::VtableField { .. } => ExternalBindingMechanism::VtableField,
            Self::TableFunction { .. } => ExternalBindingMechanism::TableFunction,
        }
    }
}

/// How a machine is supplied to its consumers (record §Machines). Provider
/// admission, proof artifacts, manifests, and lowering consume this directly;
/// resolved and typed trees do not retain a parallel source-spelling flag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MachineSupplyMode {
    /// An ordinary checked body compiled in this program (the ZII default).
    #[default]
    CheckedBody,
    /// A requirement slot: the signature is the contract; a provider is
    /// admitted against it.
    Requirement,
    /// A package-qualified carrier-owned provider requirement. The declaration
    /// publishes an interface slot and contains no executable or admitted body.
    TopLevelRequirement,
    /// A boundary declaration: supplied by the host/component seam, claims
    /// gated by grants.
    Boundary,
    /// An accepted (axiom-tier) declaration: trusted without proof, shown
    /// in the trust report.
    AdmissionClaim,
    /// PRV4: an irreducible external leaf -- `satisfies Requirement via
    /// <Binding>;` on a bodyless machine. The satisfied requirement supplies
    /// the public contract and service/operational ceilings. The optional
    /// bootstrap identity remains only for unmigrated source; an ordinary
    /// producer expression is retained on the conformance until Omega installs
    /// the normalized evaluated binding. Composite lowerings are ordinary
    /// CheckedBody machines and never carry a binding.
    ExternalRealization {
        /// Present only for the segregated pre-evaluation bootstrap syntax.
        /// Ordinary `via` source retains its producer expression on the exact
        /// conformance until Omega installs the normalized evaluated binding.
        binding: Option<ExternalBindingId>,
        mechanism: Option<ExternalBindingMechanism>,
    },
}

impl MachineSupplyMode {
    pub const fn is_checked_body(self) -> bool {
        matches!(self, Self::CheckedBody)
    }

    /// Whether the source declaration uses the boundary seam. Admission claims
    /// declarations are a distinct trust tier but share that source-facing
    /// entry/storage shape.
    pub const fn is_boundary_declaration(self) -> bool {
        matches!(
            self,
            Self::TopLevelRequirement | Self::Boundary | Self::AdmissionClaim
        )
    }
}

/// Decision 23's PUBLIC half: the termination guarantee that
/// participates in published machine-contract and import-slot identity.
/// The premises are explicit; an exported omission normalizes to
/// `NoGuarantee` (never to an implied promise).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum TerminationGuarantee {
    #[default]
    NoGuarantee,
    Terminates {
        /// Exact subject-bearing progress-profile premises the guarantee is
        /// conditional on. Public contracts retain parameter-rooted schemas;
        /// checked call edges substitute those roots with caller occurrences.
        premises: Vec<ProgressPremise>,
    },
}

impl TerminationGuarantee {
    pub const fn promises_termination(&self) -> bool {
        matches!(self, Self::Terminates { .. })
    }
}

/// One exact progress premise. The profile identifies the closed semantic
/// domain; the subject prevents a grant for one capability occurrence from
/// discharging a premise about another.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgressPremise {
    pub profile: SemanticDomainId,
    pub subject: ProgressSubject,
}

/// Identity-preserving subject path rooted at a declared parameter or local.
/// Projections are semantic field symbols, never rendered names or offsets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgressSubject {
    pub root: psi_symbols::SymbolHandle,
    pub projections: Vec<psi_symbols::SymbolHandle>,
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
    /// A normalized EXTERNAL-BINDING identity (PRV4 step 1): the structural
    /// `via <Binding>` value of an ExternalRealization leaf, interned so supply
    /// modes stay Copy and equal bindings share one identity without rendering.
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

/// Whether the direct synchronous invocation set is private inference or a
/// published ceiling. Published omission is an explicit empty edge set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SynchronousInvocationInterface {
    #[default]
    InternalInferred,
    PublishedCeiling,
}

/// Erased direct-edge metadata retained in checked artifacts. Targets use
/// canonical positional identities (`parameter:N`) or canonical boundary
/// service names (`service:Name`); they are never replaced by reach closure.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SynchronousInvocationPlan {
    pub interface: SynchronousInvocationInterface,
    pub published: Vec<String>,
    pub checked_inferred: Vec<String>,
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

/// Suspension possibility attached to one flow/graph scope. Kept separate
/// from blocking so downstream consumers cannot accidentally treat parking an
/// activation as occupying its worker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SuspensionSummary {
    pub direct_may_suspend: bool,
    pub transitive_may_suspend: bool,
}

/// Worker-blocking possibility attached to one flow/graph scope. Kept
/// separate from suspension because the two public may-ceilings compose and
/// admit independently.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BlockingSummary {
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
    pub symbol: psi_symbols::SymbolHandle,
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
    pub fn intern(&mut self, symbol: psi_symbols::SymbolHandle, name: &str) -> ServiceReachId {
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

    pub fn id_for_symbol(&self, symbol: psi_symbols::SymbolHandle) -> Option<ServiceReachId> {
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

/// Deterministic EXTERNAL-BINDING interner. `NULL`/0 stays "not computed";
/// ids start at 1.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExternalBindingTable {
    identities: Vec<ExternalBindingIdentity>,
}

impl ExternalBindingTable {
    pub fn intern(&mut self, identity: ExternalBindingIdentity) -> ExternalBindingId {
        if let Some(index) = self
            .identities
            .iter()
            .position(|existing| existing == &identity)
        {
            return ExternalBindingId(index as u32 + 1);
        }
        self.identities.push(identity);
        ExternalBindingId(self.identities.len() as u32)
    }

    /// Recover the exact structured identity retained by an interned binding.
    /// Invalid/zero and out-of-table ids fail closed instead of exposing an
    /// implementation index or inviting a syntax-tree fallback.
    pub fn identity(&self, binding: ExternalBindingId) -> Option<&ExternalBindingIdentity> {
        let index = usize::try_from(binding.0).ok()?.checked_sub(1)?;
        self.identities.get(index)
    }

    pub fn identities(
        &self,
    ) -> impl ExactSizeIterator<Item = (ExternalBindingId, &ExternalBindingIdentity)> {
        self.identities
            .iter()
            .enumerate()
            .map(|(index, identity)| (ExternalBindingId(index as u32 + 1), identity))
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

/// One compiler-owned semantic contribution role.
///
/// Roles are closed because their consumers and composition laws are
/// compiler semantics. Packages contribute theories within these roles; they
/// cannot mint new role kinds by name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DomainSemanticRole {
    DenotationDimension,
    ArithmeticPolicy,
}

impl DomainSemanticRole {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DenotationDimension => "denotation_dimension",
            Self::ArithmeticPolicy => "arithmetic_policy",
        }
    }
}

/// Role-keyed semantic contributions of one declared domain.
///
/// Predicate membership is deliberately absent: it lives in
/// [`DomainPredicateBody`] and the proof-fact lattice. Fixed fields make the
/// initial closed vocabulary explicit while allowing a hybrid domain to
/// contribute independently on each axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DomainSemanticRoles {
    pub denotation_dimension: Option<SemanticDomainId>,
    pub arithmetic_policy: Option<SemanticDomainId>,
}

impl DomainSemanticRoles {
    pub const fn is_empty(self) -> bool {
        self.denotation_dimension.is_none() && self.arithmetic_policy.is_none()
    }

    pub const fn contribution(self, role: DomainSemanticRole) -> Option<SemanticDomainId> {
        match role {
            DomainSemanticRole::DenotationDimension => self.denotation_dimension,
            DomainSemanticRole::ArithmeticPolicy => self.arithmetic_policy,
        }
    }
}

/// One normalized relationship authorized to introduce domain membership.
///
/// These are declaration identities, not evidence origins. A checked fact
/// still records whether membership arrived through proof, propagation,
/// transformation, or a receipt; this record answers which authored
/// relationship was allowed to introduce it in the first place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DomainEstablishmentRoute {
    /// An exact ordinary trait requirement authored by `established by`.
    CheckedRequirement {
        trait_definition: psi_symbols::SymbolHandle,
        requirement: psi_symbols::SymbolHandle,
    },
    /// An exact result guarantee on an owner-authored boundary requirement.
    BoundaryRequirement {
        boundary_trait: psi_symbols::SymbolHandle,
        requirement: psi_symbols::SymbolHandle,
    },
}

impl DomainEstablishmentRoute {
    pub const fn kind_name(self) -> &'static str {
        match self {
            Self::CheckedRequirement { .. } => "checked_requirement",
            Self::BoundaryRequirement { .. } => "boundary_requirement",
        }
    }

    pub const fn source_symbol(self) -> psi_symbols::SymbolHandle {
        match self {
            Self::CheckedRequirement {
                trait_definition, ..
            } => trait_definition,
            Self::BoundaryRequirement { boundary_trait, .. } => boundary_trait,
        }
    }

    pub const fn requirement_symbol(self) -> psi_symbols::SymbolHandle {
        match self {
            Self::CheckedRequirement { requirement, .. }
            | Self::BoundaryRequirement { requirement, .. } => requirement,
        }
    }
}

/// Why one checked membership fact may qualify its exact runtime subject.
///
/// This is deliberately independent from the fact's program-point origin:
/// `CallEnsures` says where a fact entered the caller, while this enum says
/// which semantic route makes that fact trustworthy. `None` is used for
/// declarations and obligations that do not themselves establish membership.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum QualificationEvidenceOrigin {
    #[default]
    None,
    /// A nonempty predicate body was discharged by checked proof.
    Prover,
    /// A checked runtime validator established a predicate body.
    CheckedValidation,
    /// A checked conformance returned through an exact requirement route
    /// authored by the domain declaration.
    AuthorizedRouteEstablishment,
    /// Existing evidence was conserved through a checked transformation.
    CheckedTransformation,
    /// The fact crossed an admitted boundary under a public contract.
    AdmittedReceipt,
    /// Existing evidence was carried without changing its subject.
    Propagated,
    /// Explicit `as` introduced a domain with no predicates or routes.
    VacuousQualification,
}

impl QualificationEvidenceOrigin {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Prover => "prover",
            Self::CheckedValidation => "checked_validation",
            Self::AuthorizedRouteEstablishment => "authorized_route_establishment",
            Self::CheckedTransformation => "checked_transformation",
            Self::AdmittedReceipt => "admitted_receipt",
            Self::Propagated => "propagated",
            Self::VacuousQualification => "vacuous_qualification",
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
    fn establishment_routes_keep_source_and_requirement_identity_independent() {
        let checked_trait = psi_symbols::SymbolHandle::from_arena_index(9);
        let checked_requirement = psi_symbols::SymbolHandle::from_arena_index(10);
        let checked = DomainEstablishmentRoute::CheckedRequirement {
            trait_definition: checked_trait,
            requirement: checked_requirement,
        };
        assert_eq!(checked.kind_name(), "checked_requirement");
        assert_eq!(checked.source_symbol(), checked_trait);
        assert_eq!(checked.requirement_symbol(), checked_requirement);

        let boundary_trait = psi_symbols::SymbolHandle::from_arena_index(11);
        let requirement = psi_symbols::SymbolHandle::from_arena_index(12);
        let route = DomainEstablishmentRoute::BoundaryRequirement {
            boundary_trait,
            requirement,
        };
        assert_eq!(route.kind_name(), "boundary_requirement");
        assert_eq!(route.source_symbol(), boundary_trait);
        assert_eq!(route.requirement_symbol(), requirement);
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
    fn qualification_evidence_origins_have_stable_public_names() {
        use QualificationEvidenceOrigin as Origin;

        assert_eq!(Origin::default(), Origin::None);
        assert_eq!(Origin::Prover.as_str(), "prover");
        assert_eq!(Origin::CheckedValidation.as_str(), "checked_validation");
        assert_eq!(
            Origin::AuthorizedRouteEstablishment.as_str(),
            "authorized_route_establishment"
        );
        assert_eq!(
            Origin::CheckedTransformation.as_str(),
            "checked_transformation"
        );
        assert_eq!(Origin::AdmittedReceipt.as_str(), "admitted_receipt");
        assert_eq!(Origin::Propagated.as_str(), "propagated");
        assert_eq!(
            Origin::VacuousQualification.as_str(),
            "vacuous_qualification"
        );
    }

    #[test]
    fn multiplicity_default_is_affine() {
        // Ordinary data defaults to Affine (the record's mapping); `[copy]`
        // opts into Unrestricted, `[linear]` into Linear.
        assert_eq!(Multiplicity::default(), Multiplicity::Affine);
    }

    #[test]
    fn machine_supply_queries_keep_checked_and_boundary_tiers_distinct() {
        assert!(MachineSupplyMode::CheckedBody.is_checked_body());
        assert!(!MachineSupplyMode::CheckedBody.is_boundary_declaration());

        assert!(MachineSupplyMode::Boundary.is_boundary_declaration());
        assert!(MachineSupplyMode::AdmissionClaim.is_boundary_declaration());
        assert!(!MachineSupplyMode::AdmissionClaim.is_checked_body());
        assert!(MachineSupplyMode::TopLevelRequirement.is_boundary_declaration());
        assert!(!MachineSupplyMode::TopLevelRequirement.is_checked_body());

        for mode in [
            MachineSupplyMode::Requirement,
            MachineSupplyMode::ExternalRealization {
                binding: Some(ExternalBindingId(1)),
                mechanism: Some(ExternalBindingMechanism::CompilerIntrinsic),
            },
        ] {
            assert!(!mode.is_checked_body());
            assert!(!mode.is_boundary_declaration());
        }
    }

    #[test]
    fn external_binding_interner_uses_structural_identity() {
        let mut bindings = ExternalBindingTable::default();
        let first = bindings.intern(ExternalBindingIdentity::Import {
            library: "a,b".to_owned(),
            symbol: "c".to_owned(),
        });
        assert_eq!(
            bindings.intern(ExternalBindingIdentity::Import {
                library: "a,b".to_owned(),
                symbol: "c".to_owned(),
            }),
            first,
            "equal structural bindings must share one identity"
        );
        assert_ne!(
            bindings.intern(ExternalBindingIdentity::Import {
                library: "a".to_owned(),
                symbol: "b,c".to_owned(),
            }),
            first,
            "field boundaries must remain identity-bearing"
        );
        let intrinsic = bindings.intern(ExternalBindingIdentity::CompilerIntrinsic);
        assert_ne!(
            intrinsic, first,
            "mechanism tags must remain identity-bearing"
        );
        assert_eq!(
            bindings.intern(ExternalBindingIdentity::CompilerIntrinsic),
            intrinsic,
            "payloadless intrinsic values must share one structural binding identity"
        );
        assert_eq!(
            bindings.identity(first),
            Some(&ExternalBindingIdentity::Import {
                library: "a,b".to_owned(),
                symbol: "c".to_owned(),
            })
        );
        assert_eq!(bindings.identity(ExternalBindingId(0)), None);
        assert_eq!(bindings.identity(ExternalBindingId(u32::MAX)), None);
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
            psi_symbols::SymbolHandle::from_parts(7, 1),
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

    #[test]
    fn carry_permissions_are_closed_named_positive_relaxations() {
        let mut policy = CarryPolicy::STRICT;
        for permission in CarryPermission::ALL {
            assert_eq!(
                CarryPermission::from_name(permission.name()),
                Some(permission)
            );
            policy = permission.relax(policy);
        }
        assert_eq!(policy, CarryPolicy::PERMISSIVE);
        assert_eq!(CarryPermission::from_name("Carry::Portable"), None);
        assert_eq!(CarryPermission::from_name("Carry::Anywhere"), None);
    }
}

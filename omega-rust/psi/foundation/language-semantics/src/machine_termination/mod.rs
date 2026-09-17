//! Machine supply and termination: how a machine is supplied, what it
//! guarantees about progress, the ranking witnesses and views behind that
//! guarantee, and the termination plan.

use crate::RankingViewId;
use crate::{ExternalBindingId, ExternalBindingMechanism, SemanticDomainId};

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
    pub root: symbols::SymbolHandle,
    pub projections: Vec<symbols::SymbolHandle>,
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

/// Sealed toolchain source (relative to the core package root) that declares
/// the ranking views carrying a catalog declaration row.
pub const RANKING_VIEW_CORE_SOURCE: &str = "nat.omg";

/// One canonical view's sealed declaration: the exact source-visible path
/// and normalized signature the Toolchain-origin core file must declare for
/// the bare bodyless `machine Nat::Descending(value: u64) -> u64;` to be
/// supplied from the catalog. Identity only; the view's proof meaning stays
/// in the termination checker keyed on [`RankingViewId`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RankingViewDeclaration {
    pub view: RankingViewId,
    /// Relative path of the sealed declaring source under the core root.
    pub source: &'static str,
    pub namespace: &'static str,
    pub name: &'static str,
    /// The single parameter's type spelling.
    pub parameter: &'static str,
    /// The result type spelling.
    pub result: &'static str,
}

impl RankingViewDeclaration {
    pub fn path(&self) -> String {
        format!("{}::{}", self.namespace, self.name)
    }
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

    /// The sealed toolchain declaration a canonical view is browsable at, if
    /// the core library declares one. This is the catalog's declaration
    /// custody row: the executable-supply contract supplies an exact
    /// compiler-owned bodyless machine from the closed catalog only when the
    /// Toolchain-origin source named here declares it with exactly this path
    /// and signature, and "merely naming a declaration `Nat::Descending`
    /// grants no primitive". Views without a row (`Nat::BoundedDistance`,
    /// `Slice::Length`, `Nat::IncreasingTo`) are spelling-only builtins today.
    pub fn catalog_declaration(self) -> Option<RankingViewDeclaration> {
        match self {
            Self::NAT_DESCENDING => Some(RankingViewDeclaration {
                view: self,
                source: RANKING_VIEW_CORE_SOURCE,
                namespace: "Nat",
                name: "Descending",
                parameter: "u64",
                result: "u64",
            }),
            _ => None,
        }
    }

    /// Select the canonical view whose catalog declaration has exactly this
    /// path. The leaf spelling alone selects nothing: `("Card", "Descending")`
    /// and `("Nat", "Ranking")` are not rows.
    pub fn from_catalog_declaration(namespace: &str, name: &str) -> Option<RankingViewDeclaration> {
        [
            Self::NAT_DESCENDING,
            Self::NAT_BOUNDED_DISTANCE,
            Self::SLICE_LENGTH,
            Self::NAT_INCREASING_TO,
        ]
        .into_iter()
        .filter_map(Self::catalog_declaration)
        .find(|declaration| declaration.namespace == namespace && declaration.name == name)
    }
}

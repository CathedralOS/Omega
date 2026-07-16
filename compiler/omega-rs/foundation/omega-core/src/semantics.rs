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
/// defaults to `Affine`, `[linear]` maps to `Linear`. `zero_init` and
/// `send` remain orthogonal properties, never folded in.
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
}

/// The interface/implementation split for one machine's termination story
/// (record §Machines): the published guarantee is contract identity, the
/// checked summary serves local consumers, the witness stays private.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MachineTerminationPlan {
    /// `None` = internal/derived (not serialized as an authored external
    /// promise); `Some` = published, participating in contract identity.
    pub published: Option<TerminationGuarantee>,
    /// What the checker established for THIS body (local consumers only).
    pub checked_summary: TerminationGuarantee,
    /// The private proof material, if a ranked body carried one.
    pub implementation_witness: Option<RankingWitness>,
}

/// Decision 22's member kinds: the qualitative effect row is KINDED, never
/// one undifferentiated name list. A provider carrying an `OperationalMay`
/// member (e.g. `Block`) cannot satisfy a slot pinned without it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectMemberKind {
    /// Reach to a boundary service (minted by boundary-trait declarations).
    ServiceReach,
    /// An operational possibility the caller must tolerate (core-minted v1
    /// set: `Suspend`, `Block`).
    OperationalMay,
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
    /// Normalized declaration/core identity of one effect member.
    EffectMemberId
);
semantic_id!(
    /// Normalized effect-row identity (member set + parent closure). Must
    /// never depend on prover strength, provider selection, or the legacy
    /// numeric bit assigned to a name.
    EffectRowId
);
semantic_id!(
    /// A sealed boundary progress profile (grant/receipt identity);
    /// participates in provider admission, outside the ordinary proof-fact
    /// catalog in v1.
    ProgressProfileId
);
semantic_id!(
    /// A canonical ranking view (e.g. `Nat::Descending`); the witness names
    /// it explicitly, defaults elaborate at once.
    RankingViewId
);

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

#[cfg(test)]
mod tests {
    use super::*;

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
            published: Some(TerminationGuarantee::NoGuarantee),
            checked_summary: TerminationGuarantee::NoGuarantee,
            implementation_witness: Some(RankingWitness::default()),
        };
        let without_witness = MachineTerminationPlan {
            implementation_witness: None,
            ..with_witness.clone()
        };
        assert_eq!(with_witness.published, without_witness.published);
    }

    #[test]
    fn semantic_ids_are_zii_inert() {
        assert!(!SemanticDomainId::default().is_valid());
        assert!(EffectRowId(1).is_valid());
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
}

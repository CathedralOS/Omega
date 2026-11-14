#![forbid(unsafe_code)]

/*
The shared tables of the checker that runs on the lowered form: the explicit,
unsealed Terminal Psi module that structure-only elaboration produces
(`wiki/drafts/designs/check_on_lowered_form.md`). They hold what the checker
knows about one machine, as tables over dense identities: values with
their knowledge rows, places with the forward write log, obligations, and
edges with invocations.

Semantic rows never identify a fact by a name, printed expression, path string
or formatted handle: a place is a dense identity
whose row names its parent and one typed segment, and a fact is a row over
value identities. The complete program packet also retains diagnostic names
and source spans, separately from these semantic rows; no proof reads their
spelling. A checking step
returns a row or a `Refusal`; there is no "nothing" result.

Identities are reused from `semantic-vocabulary` wherever the lowered form
already names the same thing (values, blocks, edges, operations, machines,
obligations, root places), so the checker and the emitted product refer to
"the same stable values, places, operations, and edges"
(`wiki/spec/terminal-psi/product.md`). Every table is an `IdTable`, a flat
`Vec` indexed by identity. Elaboration mints each identity by `IdTable::push`,
so the identities are dense by construction and a lookup is one index, never
a scan or a map. The crate's own identities exist only where the lowered form
has no identity for the concept.
*/

pub mod bindings;
pub mod compilation;
pub mod edges;
pub mod names;
pub mod obligations;
pub mod program;
pub mod values;

use std::num::NonZeroU64;

pub use semantic_vocabulary::{
    BlockId, EdgeId, IdTable, InvocationSite, MachineId, ObligationId, OperationId, PlaceId,
    PropositionId, PsiSemanticId, ValueId,
};

macro_rules! check_id {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(NonZeroU64);

        impl PsiSemanticId for $name {
            fn new(raw: u64) -> Option<Self> {
                NonZeroU64::new(raw).map(Self)
            }

            fn get(self) -> u64 {
                self.0.get()
            }
        }
    };
}

check_id!(
    ExactPlaceId,
    "One exact projected place: a root place or a parent place plus one segment \
     (`bindings::PlaceRow`). The lowered form's `PlaceId` names roots only."
);
check_id!(
    KnowledgeId,
    "One knowledge row over value identities (`values::KnowledgeRow`)."
);
check_id!(
    WriteId,
    "One entry of the forward write log (`bindings::WriteRow`)."
);
check_id!(
    InvocationId,
    "One invocation entry, ordinary or terminal (`edges::InvocationRow`). \
     `InvocationSite` is its coordinate in the lowered form, not a dense identity."
);

/// A point in one machine's lowered graph at which a row becomes available.
///
/// `wiki/spec/terminal-psi/fact_validity.md`: "A premise must dominate its use
/// and remain valid on every path to it." Availability is stated at the point
/// the lowered form makes explicit, so dominance is a graph question over
/// these identities and never a question about source order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProgramPoint {
    /// On entry to a block, after its parameters are bound.
    BlockEntry(BlockId),
    /// On the normal continuation of an operation. fact_validity.md: "A
    /// partial operation establishes its result equation only on its normal
    /// successor, after its safety obligation is discharged."
    AfterOperation(OperationId),
    /// On one edge, after its transfers and cleanup and before the
    /// destination binds. fact_validity.md: "Each successor applies its own
    /// argument-to-parameter substitution."
    Edge(EdgeId),
}

/// The lowered construct a refusal concerns.
///
/// Source spans are not repeated here: `LoweredSourceCustody` is the single
/// source-to-lowered correspondence, and a diagnostic resolves this construct
/// through it once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Construct {
    Machine,
    Block(BlockId),
    Operation(OperationId),
    Edge(EdgeId),
    Value(ValueId),
    Place(ExactPlaceId),
    Obligation(ObligationId),
    Invocation(InvocationId),
}

/// Why a checking step refused. Closed: a lane that needs a new reason adds a
/// variant here, never a free-form code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefusalReason {
    /// The construct is outside the construct families this checker serves.
    /// Fail closed: an unserved construct is never accepted by default.
    Unsupported,
    /// An obligation was reconstructed and its evidence did not check. The
    /// cause is typed so a diagnostic reports it without reading `detail`.
    Unproved(UnprovedCause),
    /// Storage is used on a path where nothing established it: a compile
    /// error of the program, neither an unserved construct nor an unproved
    /// proposition. For plain storage that is today the zeroed storage of a
    /// gated type. A scalar or data local declared without an initializer
    /// is not otherwise such storage: it is zero-constructed, and
    /// established whenever zero satisfies its type
    /// (`local-without-initializer`), as it does for every builtin scalar. `wiki/spec/language/dependent_values.md`: "If
    /// zero satisfies the default domain, the value is zero-constructible.
    /// Otherwise the type is gated: construction or checked qualification
    /// must establish the domain before observation." It is a structural
    /// judgment over the write log and the lowered graph, so it has no
    /// obligation row and no certificate.
    ///
    /// The same reason covers the two structural judgments over a sum. A
    /// case payload named where its case is not established on every path:
    /// structural_access.md, "Each `Case` step requires source admission to
    /// establish the selected case for the same root and prefix at the
    /// read's observation epoch." And a dispatch with no edge for a declared
    /// case: `wiki/spec/language/patterns.md`, "Every dispatch must cover
    /// the admitted subject. Missing runtime coverage is a compile error,
    /// not an implicit trap or fallthrough."
    Unestablished,
    /// An edge did not account for an incoming conserved obligation exactly
    /// once. ownership.md: "every incoming owned obligation occurs exactly once
    /// in the edge's transfer map, explicit terminal consumption, eligible
    /// automatic cleanup, or validated no-code affine discard."
    NotConserved,
    /// Two live obligations or accesses conflict (overlap, aliasing, access).
    Conflict,
    /// An identity is past the end of the table it was looked up in. This
    /// catches only out-of-range identities; see `row`.
    ForeignIdentity,
}

/// Why an obligation's evidence did not check. Closed like `RefusalReason`:
/// a prover that finds a new way to fail names it here.
///
/// None of these claims the obligation is false.
/// `wiki/spec/build/compiler_request.md`, "Outcomes": "Failure to find a
/// proof is not proof of falsehood", and "Diagnostics distinguish failed
/// source checking, invalid proof, unavailable evidence and receiving-policy
/// refusal." Every cause but `NoRuleForGoalForm` publishes `Reject` with no
/// artifact (`wiki/spec/packages/review.md`: "Unproved source contracts
/// reject in Psi checking"). `NoRuleForGoalForm` publishes `Incomplete`: the
/// program may be valid and the prover cannot judge it, which is the
/// outcome table's "unsupported valid case" (owner ruling on
/// `unproved-goal-form-outcome`). There is no exhaustion cause because the
/// lowered-form prover has no search or resource bound. A bound added later
/// needs its own cause, published as `Incomplete` with the resource
/// diagnostic (`wiki/spec/build/configuration.md`: "Exhaustion leaves an
/// unproved obligation and a resource diagnostic; it grants no ... runtime
/// fallback").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnprovedCause {
    /// The prover has rules for the goal's form and found no derivation of
    /// it from the knowledge available at the obligation's point.
    NoDerivation,
    /// The prover has no rule for the goal's form (an order or equality over
    /// non-integers, a proposition form it does not reason about), so it
    /// cannot judge the obligation either way.
    NoRuleForGoalForm,
    /// The prover built a certificate and the proof kernel refused it: an
    /// invalid proof ("an invalid certificate is `Reject`").
    KernelRefused,
    /// A normal return does not establish a published `ensures` clause: the
    /// guarantee owed on that return edge is unproved or was never owed.
    ReturnDoesNotEstablish,
}

impl UnprovedCause {
    /// The cause as a program diagnostic states it. Stable wording: corpus
    /// expectations match it. It says what was not shown, never that the
    /// obligation is false.
    pub const fn describe(self) -> &'static str {
        match self {
            Self::NoDerivation => "no proof found from the facts available here",
            Self::NoRuleForGoalForm => "the prover has no rule for this form of goal",
            Self::KernelRefused => "invalid proof: the proof kernel refused the certificate",
            Self::ReturnDoesNotEstablish => "not shown to hold on every normal return",
        }
    }

    /// True when the outcome is `Incomplete` rather than `Reject`.
    pub const fn is_incomplete(self) -> bool {
        matches!(self, Self::NoRuleForGoalForm)
    }
}

/// The only negative result of a checking step: a reason, the machine, and
/// the lowered construct it concerns.
#[must_use]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    pub reason: RefusalReason,
    pub machine: MachineId,
    pub concerns: Construct,
    /// Diagnostic text only. Nothing may branch on it or compare it.
    pub detail: String,
}

/// Every later checking step returns `Checked<T>`: a row or a refusal.
pub type Checked<T> = Result<T, Refusal>;

/// Looks up a row, refusing an identity past the end of the table.
///
/// It cannot tell an identity minted by another machine's table: identities
/// are machine-local integers with no machine tag, so a foreign identity that
/// is in range resolves to the wrong row. The rule that prevents this is
/// structural, not checked here: identities never cross `MachineTables`, and
/// every checking API takes exactly one `MachineTables` and only identities
/// read from it. Crossing machines goes through an `InvocationRow`, which
/// holds a `MachineId` and positions, never the callee's local identities.
pub fn row<Id: PsiSemanticId, T>(
    table: &IdTable<Id, T>,
    identity: Id,
    machine: MachineId,
    concerns: Construct,
) -> Checked<&T> {
    table.get(identity).ok_or_else(|| Refusal {
        reason: RefusalReason::ForeignIdentity,
        machine,
        concerns,
        detail: String::from("identity is past the end of this machine's table"),
    })
}

/// Everything the checker holds for one machine. Value, place, claim and
/// edge identities are machine-local in the lowered form (ownership.md:
/// "Claim identities stay machine-local"), so these tables are too.
#[derive(Debug, Clone, Default)]
pub struct MachineTables {
    pub values: IdTable<ValueId, values::ValueRow>,
    pub knowledge: IdTable<KnowledgeId, values::KnowledgeRow>,
    /// The propositions knowledge rows state, minted by `push`. Machine-local
    /// like every other identity here: Terminal Psi keeps contract
    /// propositions inline and has no proposition table of its own.
    pub propositions: IdTable<PropositionId, semantic_vocabulary::Proposition>,
    pub places: IdTable<ExactPlaceId, bindings::PlaceRow>,
    /// The `ExactPlaceId` of each root `PlaceId`. Elaboration mints root
    /// `PlaceId`s densely too, so interning a root is one index.
    pub roots: IdTable<PlaceId, ExactPlaceId>,
    pub writes: IdTable<WriteId, bindings::WriteRow>,
    pub obligations: IdTable<ObligationId, obligations::ObligationRow>,
    pub edges: IdTable<EdgeId, edges::EdgeRow>,
    pub invocations: IdTable<InvocationId, edges::InvocationRow>,
}

/// The module: one `MachineTables` per machine.
#[derive(Debug, Clone, Default)]
pub struct CheckTables {
    pub machines: IdTable<MachineId, MachineTables>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pushed_identities_are_dense_from_one() {
        let mut table: IdTable<KnowledgeId, char> = IdTable::default();
        let first = table.push('a');
        let second = table.push('b');
        assert_eq!((first.get(), second.get()), (1, 2));
        assert_eq!(table.get(second), Some(&'b'));
    }

    #[test]
    fn an_identity_past_the_end_is_refused() {
        let machine = MachineId::new(1).unwrap();
        let mut small: IdTable<ExactPlaceId, ()> = IdTable::default();
        let mut large: IdTable<ExactPlaceId, ()> = IdTable::default();
        small.push(());
        large.push(());
        let foreign = large.push(());
        let refusal = row(&small, foreign, machine, Construct::Place(foreign)).unwrap_err();
        assert_eq!(refusal.reason, RefusalReason::ForeignIdentity);
        assert_eq!(refusal.concerns, Construct::Place(foreign));
    }
}

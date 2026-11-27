/*
The checker tables for the lowered form: the explicit,
unsealed Terminal Psi module that structure-only elaboration produces
(`wiki/drafts/designs/check_on_lowered_form.md`). They hold what the checker
knows about one machine, as tables over dense identities: values with
their knowledge rows, places, obligations, and
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
(`wiki/spec/terminal-psi/product.md`). The tables assign consecutive identities as elaboration appends rows,
so the identities are dense by construction and a lookup is one index, never
a scan or a map. The crate's own identities exist only where the lowered form
has no identity for the concept.
*/

mod machine_values;
pub use machine_values::MachineValues;
mod shared_rows;
pub use shared_rows::{CheckingRows, LoopCheckingRows};

pub mod bindings;
pub mod contracts;
pub mod edges;
pub mod obligations;
pub mod values;

use std::num::NonZeroU64;

pub use arena::{IdTable, NumericId};
pub use semantic_vocabulary::{
    BlockId, EdgeId, InvocationSite, MachineId, ObligationId, OperationId, PlaceId, PropositionId,
    ValueId,
};

macro_rules! check_id {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(NonZeroU64);

        impl NumericId for $name {
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
    /// The selected edge has captured successor operands, and all preceding
    /// cleanup actions have completed. The named action has not run yet.
    BeforeCleanup { edge: EdgeId, action_ordinal: u32 },
    /// Normal completion of this exact action, before its successor action.
    AfterCleanup { edge: EdgeId, action_ordinal: u32 },
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

/// Why a requested borrow conflicts with existing access.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoanConflict {
    /// Both loans are exclusive.
    TwiceExclusive,
    /// One is exclusive and the other shared.
    ExclusiveAndShared,
    /// An exclusive loan of storage this machine holds only shared.
    ExclusiveOfShared,
    /// Observation bypasses a live exclusive descriptor.
    ObserveWhileExclusive,
    /// A write or move conflicts with a retained loan.
    ChangeWhileBorrowed,
    /// A parent is disposed before its children close.
    ReleaseBeforeChildren,
    /// The actual projected source does not grant the requested access.
    AccessExceedsParent,
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
    /// judgment over the checker's write log and the lowered graph, so it has
    /// no obligation row and no certificate.
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
    /// A semantic access conflict at a successor, which forwards existing
    /// storage without necessarily creating a new loan obligation.
    ConflictingLoan(LoanConflict),
    /// An identity is past the end of the table it was looked up in. This
    /// catches only out-of-range identities; see `row`.
    ForeignIdentity,
    /// The program breaks a rule of the language: a value whose type
    /// carries other domains than its destination declares, or a cast that
    /// lacks both existing membership and an authorized establishment route.
    /// The source reader or lowered-form checker judges the rule from its
    /// own input representation. It is a compile
    /// error of the program and is published as a rejection, never as "not
    /// supported". `detail` states the rule. A step
    /// gives this reason only where the specification defines the error for
    /// every program of that form: a form the step merely cannot judge is
    /// `Unsupported`.
    Violation,
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
/// `unproved-goal-form-outcome`). `GoalStepsExhausted` publishes `Incomplete`
/// at its resource code (`wiki/spec/build/configuration.md`: "Exhaustion
/// leaves an unproved obligation and a resource diagnostic; it grants no ...
/// runtime fallback").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnprovedCause {
    /// The prover has rules for the goal's form and found no derivation of
    /// it from the knowledge available at the obligation's point.
    NoDerivation,
    /// Proof search or kernel conversion reached its configured work limit.
    BudgetExhausted,
    /// One goal's proof search ran out of its step budget, `limit` steps
    /// (`proof_search_goal_steps`, code 37). It says nothing about the goal.
    GoalStepsExhausted { limit: u32 },
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
            Self::BudgetExhausted => "mathematical proof budget exhausted",
            Self::GoalStepsExhausted { .. } => "proof search step budget exhausted",
            Self::NoDerivation => "no proof found from the facts available here",
            Self::NoRuleForGoalForm => "the prover has no rule for this form of goal",
            Self::KernelRefused => "invalid proof: the proof kernel refused the certificate",
            Self::ReturnDoesNotEstablish => "not shown to hold on every normal return",
        }
    }

    /// True when the outcome is `Incomplete` rather than `Reject`.
    pub const fn is_incomplete(self) -> bool {
        matches!(
            self,
            Self::NoRuleForGoalForm | Self::BudgetExhausted | Self::GoalStepsExhausted { .. }
        )
    }
}

/// The source of a checking question. Static declarations and whole-program
/// judgments can exist in a Product that has no executable machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefusalSite {
    Machine(MachineId, Construct),
    Declaration(symbols::SymbolHandle),
    Program,
}

impl RefusalSite {
    pub fn in_machine(self) -> Option<(MachineId, Construct)> {
        match self {
            Self::Machine(machine, concerns) => Some((machine, concerns)),
            Self::Declaration(_) | Self::Program => None,
        }
    }
}

impl From<MachineId> for RefusalSite {
    fn from(machine: MachineId) -> Self {
        Self::Machine(machine, Construct::Machine)
    }
}

/// The negative result of a checking step, with its reason and exact site. A retained authored source span can accompany any reason.
#[must_use]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    pub reason: RefusalReason,
    pub site: RefusalSite,
    /// The retained source span, when the authored construct carries one.
    pub source_span: Option<source_text::SourceSpan>,
    /// Diagnostic text only. Nothing may branch on it or compare it.
    pub detail: String,
    /// The obligation left unproved, for an `Unproved` refusal whose site
    /// names its owner rather than the obligation (a contract implication).
    pub obligation: Option<ObligationId>,
}

impl Refusal {
    pub fn at(reason: RefusalReason, site: RefusalSite, detail: impl Into<String>) -> Self {
        Self {
            reason,
            site,
            source_span: None,
            detail: detail.into(),
            obligation: None,
        }
    }

    /// The obligation an `Unproved` refusal leaves unproved: the one its
    /// site names, else the one it carries.
    pub fn unproved_obligation(&self) -> Option<ObligationId> {
        match self.site {
            RefusalSite::Machine(_, Construct::Obligation(obligation)) => Some(obligation),
            _ => self.obligation,
        }
    }

    pub fn with_source_span(mut self, source_span: Option<source_text::SourceSpan>) -> Self {
        self.source_span = source_span;
        self
    }
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
pub fn row<Id: NumericId, T>(
    table: &IdTable<Id, T>,
    identity: Id,
    machine: MachineId,
    concerns: Construct,
) -> Checked<&T> {
    checked_row(table.get(identity), machine, concerns)
}

fn checked_row<T>(value: Option<&T>, machine: MachineId, concerns: Construct) -> Checked<&T> {
    value.ok_or_else(|| Refusal {
        reason: RefusalReason::ForeignIdentity,
        site: crate::RefusalSite::Machine(machine, concerns),
        source_span: None,
        detail: String::from("identity is past the end of this machine's table"),
        obligation: None,
    })
}

/// The term a record or case literal supplies for one erased scalar field.
/// Terminal Psi omits the field from runtime construction, so the checker
/// reads its value from here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErasedFieldTerm {
    pub operation: semantic_vocabulary::OperationId,
    pub field: semantic_vocabulary::StructuralFieldId,
    pub term: semantic_vocabulary::ScalarTerm,
}

/// An Identity of two classes of a quotient of data that has storage, and
/// the quotient's relation read at their two representatives. The checker
/// may prove the Identity from a proof of the relation by the quotient's
/// soundness (wiki/spec/proofs/quotients.md, "Set-quotient foundation":
/// `sound`), an assumption its receipt counts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuotientSoundness {
    pub classes: semantic_vocabulary::ClauseIdentity,
    pub related: semantic_vocabulary::ClauseIdentity,
}

/// What a call of one machine returns, stated over that machine's own
/// formals: its result is its selected operation applied to its inputs, as
/// `Identity<bool>(CoinByValue::equals(left, right), result)` for a selected
/// comparison row. The machine's clauses name that operation; every call of
/// `callee` this machine makes returns the operation's value at the call's
/// unchanged arguments, so a call is congruent with the clause that names
/// the same operation on the same values (wiki/spec/proofs/contracts.md,
/// "Equality bridges and comparison facts": "Repeated admitted pure
/// comparisons with the same selected operation and exact unchanged
/// arguments have the same Boolean denotation"). The callee's body never
/// owes it: the statement is what the operation means, not a guarantee.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedOperation {
    pub callee: semantic_vocabulary::MachineId,
    pub statement: semantic_vocabulary::Proposition,
}

/// One `ensures` clause a member of an implementation writes for itself,
/// beyond its requirement's, over its formals and result. A call this
/// machine makes of `callee` establishes it on normal return when the two
/// are members of the same implementation: a law of the implementation is
/// about "the final selected implementation of that member", whose
/// consequences are exposed "through checked contracts"
/// (wiki/spec/proofs/contracts.md, "Equality bridges and comparison
/// facts"). The member's own body owes the clause through the
/// implementation's conformance witness. Calls from anywhere else keep the
/// requirement's contract: "Private strengthening does not become a public
/// guarantee".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RealizedGuarantee {
    pub callee: semantic_vocabulary::MachineId,
    /// The clause's position among the member's own `ensures`.
    pub clause: u32,
    pub statement: semantic_vocabulary::Proposition,
}

/// Everything the checker holds for one machine. Value, place, claim and
/// edge identities are machine-local in the lowered form (ownership.md:
/// "Claim identities stay machine-local"), so these tables are too.
#[derive(Debug, Clone, Default)]
pub struct MachineTables {
    pub values: MachineValues,
    pub knowledge: IdTable<KnowledgeId, values::KnowledgeRow>,
    /// The propositions knowledge rows state, minted by `push`. Machine-local
    /// like every other identity here: Terminal Psi keeps contract
    /// propositions inline and has no proposition table of its own.
    pub propositions: IdTable<PropositionId, semantic_vocabulary::Proposition>,
    pub places: CheckingRows<ExactPlaceId, bindings::PlaceRow>,
    /// The `ExactPlaceId` of each root `PlaceId`. Elaboration mints root
    /// `PlaceId`s densely too, so interning a root is one index.
    pub roots: IdTable<PlaceId, ExactPlaceId>,
    pub obligations: IdTable<ObligationId, obligations::ObligationRow>,
    pub edges: IdTable<EdgeId, edges::EdgeRow>,
    pub invocations: IdTable<InvocationId, edges::InvocationRow>,
    pub erased_field_terms: Vec<ErasedFieldTerm>,
    /// Each Identity of two classes this machine's contracts state over a
    /// quotient of data that has storage, with what relates them.
    pub quotient_soundness: Vec<QuotientSoundness>,
    /// The machines whose calls this machine's clauses name by their
    /// selected operation, with what each call returns.
    pub applied_operations: Vec<AppliedOperation>,
    /// The other members of this machine's implementation, with what each
    /// one's own written contract ensures (`RealizedGuarantee`).
    pub realized_guarantees: Vec<RealizedGuarantee>,
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
        assert_eq!(
            refusal.site.in_machine().unwrap().1,
            Construct::Place(foreign)
        );
    }
}

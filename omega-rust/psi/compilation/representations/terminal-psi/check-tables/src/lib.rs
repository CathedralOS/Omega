#![forbid(unsafe_code)]

/*
The shared tables of the checker that runs on the lowered form: the explicit,
unsealed Terminal Psi module that structure-only elaboration produces
(`wiki/drafts/designs/check_on_lowered_form.md`). They hold what the checker
knows about one machine, as five tables over dense identities: values with
their knowledge rows, places with the forward write log, may-sets,
obligations, and edges with invocations. Callee contracts are a sixth,
module-wide table of summaries, one term per axis.

The point of the crate is what it cannot say. No row holds a name, a printed
expression, a path string or a formatted handle: a place is a dense identity
whose row names its parent and one typed segment, and a fact is a row over
value identities. Text appears only in `Refusal::detail`. A checking step
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
pub mod edges;
pub mod may_sets;
pub mod obligations;
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
    ReferentSetId,
    "The places one installed reference may denote (`may_sets::ReferentSet`)."
);
check_id!(
    SelectionSetId,
    "The machines one callable selector value may select (`may_sets::SelectionSet`)."
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
    /// An obligation was reconstructed and its evidence did not check.
    Unproved,
    /// A place is used on a path on which nothing was stored into it: a
    /// compile error of the program, neither an unserved construct nor an
    /// unproved proposition. `wiki/spec/terminal-psi/structural_access.md`:
    /// "An unestablished local cannot be read: a whole read of the home, a
    /// loan of it to an ordinary or boundary call, and its forwarding on a
    /// state edge each require a whole store into it on every path reaching
    /// that use." `wiki/spec/language/state_contracts.md`: "Uninitialized
    /// storage has no assumed zero." It is a structural judgment over the
    /// write log and the lowered graph, so it has no obligation row and no
    /// certificate.
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
/// read from it. Crossing machines goes through an `InvocationRow` and the
/// callee's `Summary`, which hold `MachineId`s and positions, never the
/// callee's local identities.
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
    pub referent_sets: IdTable<ReferentSetId, may_sets::ReferentSet>,
    pub selection_sets: IdTable<SelectionSetId, may_sets::SelectionSet>,
    pub obligations: IdTable<ObligationId, obligations::ObligationRow>,
    pub edges: IdTable<EdgeId, edges::EdgeRow>,
    pub invocations: IdTable<InvocationId, edges::InvocationRow>,
}

/// The module: one `MachineTables` and one published `Summary` per machine.
#[derive(Debug, Clone, Default)]
pub struct CheckTables {
    pub machines: IdTable<MachineId, MachineTables>,
    pub summaries: IdTable<MachineId, edges::Summary>,
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

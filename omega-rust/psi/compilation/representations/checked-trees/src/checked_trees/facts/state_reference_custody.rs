//! Each declared state's reference-result custody, recorded once.
//!
//! Which expression produces a state's returned value, whether its normal
//! completions carry a complete roster of caller-relative reference origins,
//! and whether that authored tail names an ingress parameter or selects one
//! of its declared leaves are questions about the frozen typed program
//! alone. Checking answers them for every declared state while it validates
//! the same completions; lowering used to re-enter `validation` and ask the
//! same questions again, once per call site, which put the classifier's
//! admitted completion shapes behind a checking-stage entry point that
//! lowering has no business calling.
//!
//! These rows are that answer, recorded. The table is dense by the state
//! symbol's `Handle::arena_index`, because the state symbol already *is* the
//! key every reader resolves a state through: lowering's own
//! `state_declaration` finds a state by scanning the declared machines for
//! that symbol and refuses a symbol naming more than one. A map keyed by it
//! would only reproduce the symbol arena's own addressing with a hash. Row 0
//! is the invalid symbol's row, which is also what a symbol the walk did not
//! name reads — including the symbols of every other declaration kind, which
//! share the one symbol arena — so a missing row denies every answer instead
//! of inventing one.

use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;

/// One state's reference-result custody. Every field is ZII, so the derived
/// default *is* the absence row: the invalid expression handle means this
/// tail produces no returned value, and a false flag is the absence of that
/// ingress shape rather than a denial of some other one.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StateReferenceCustodyRow {
    /// The expression producing this state's returned value: its tail
    /// expression statement, or the exit arm of a terminal return
    /// transition. Invalid when the tail statement is neither.
    pub tail_return_expression: ExpressionHandle,
    /// Whether every normal completion of this state contributes an exact
    /// caller-relative reference origin, so the returned-leaf roster is
    /// complete. False both for an incomplete roster and for a state that
    /// returns no reference leaf at all; the roster is never empty and
    /// complete at once.
    pub returns_complete_reference_origins: bool,
    /// Whether the authored tail names one ingress parameter directly, so a
    /// bare reference result replays that formal's existing loan.
    pub tail_names_source_parameter: bool,
    /// Whether the authored tail instead selects one declared leaf of an
    /// owned record formal, which transports that leaf's borrowed referent
    /// without minting a fresh loan.
    pub tail_selects_source_leaf: bool,
}

/// Every declared machine state's custody row, dense by state symbol arena
/// index.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateReferenceCustodyFacts {
    /// Indexed by the state symbol's `Handle::arena_index`. Index 0 is the
    /// invalid symbol, and every index the walk did not name carries that
    /// same row.
    rows: Vec<StateReferenceCustodyRow>,
}

impl StateReferenceCustodyFacts {
    /// Adopt a classification built over every declared machine state.
    /// `rows` is positional: `rows[index]` classifies the state whose symbol
    /// has arena index `index`, and `rows[0]` is the row every unnamed
    /// symbol reads.
    pub fn with_rows(rows: Vec<StateReferenceCustodyRow>) -> Self {
        Self { rows }
    }

    /// `state`'s custody. A symbol the walk did not name reads the invalid
    /// symbol's row, which denies every answer.
    pub fn custody(&self, state: SymbolHandle) -> StateReferenceCustodyRow {
        self.rows
            .get(state.arena_index() as usize)
            .or_else(|| self.rows.first())
            .copied()
            .unwrap_or_default()
    }

    /// How many symbol arena indices the classification spans. Zero before
    /// checking records it.
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

//! `RangeFacts`, the fact store the range check carries through one state's
//! statements: collection lengths of fields and locals, integer values,
//! Boolean guard locals, and the proven index, upper bound, non-negative,
//! ordering, range bound, minimum length, exact length and subslice window
//! relations, keyed by expression labels.
//!
//! `ranges::check_indexed_accesses` and `state_arguments` create one per
//! state. Its methods are split by concern: `values` defines, assigns and
//! reads locals and field integers; `proofs` records and queries the proven
//! relations; `dependencies` records the typed reads behind each expression
//! label, so a write can tell which labels it preserves or affects;
//! scalar binding rows retain live descriptor measurements separately from
//! the captured scalar's own storage, so guards never replay initializers;
//! `invalidation` retires facts after an assignment or a call's writes; and
//! `call_writes` provides `RangeCallContext`, the per-state view of checked
//! call and borrow facts through which a call's writes are found.

/// Membership sets take the integer hasher; none is iterated for output order.
type HashSet<T> = std::collections::HashSet<T, symbols::BuildSymbolHasher>;
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;

mod call_writes;
mod closed_arrivals;
pub(super) use call_writes::RangeCallContext;
mod dependencies;
mod invalidation;
mod proofs;
mod values;
pub(super) use dependencies::ReceiverLength;

#[derive(Clone)]
pub(super) struct RangeFacts<'field> {
    pub(super) collection_bounds: Option<&'field validation::CollectionBoundsObservations<'field>>,
    pub(super) checked_operators: Option<&'field checked_trees::CheckedOperatorFacts>,
    /// Integer expression intervals from validation's arithmetic walk.
    pub(super) expression_intervals: &'field [validation::ExpressionInterval],
    pub(super) checked_calls: Option<&'field RangeCallContext<'field>>,
    pub(super) mutation_summaries: std::borrow::Cow<'field, crate::flow::StateMutationSummaryCache>,
    pub(super) statement_index: usize,
    expression_dependencies: Vec<dependencies::ExpressionDependencies>,
    scalar_binding_dependencies: Vec<dependencies::ScalarBindingDependency>,
    measured_results: Vec<(ExpressionHandle, dependencies::CapturedLength)>,
    recorded_expressions: HashSet<(ExpressionHandle, SymbolHandle, SymbolHandle)>,
    fields: &'field [(SymbolHandle, String, usize)],
    integer_fields: Vec<(SymbolHandle, String, i64)>,
    locals: Vec<(SymbolHandle, String, usize)>,
    integer_locals: Vec<(SymbolHandle, String, i64)>,
    proven_indexes: Vec<(String, String)>,
    proven_index_upper_bounds: Vec<(String, i64)>,
    /// Invocation-fixed const binder floors established by this branch.
    /// Symbol identity prevents same-spelled locals or unrelated binders from
    /// refining an array extent; immutable binders require no write retirement.
    const_binder_floors: Vec<(SymbolHandle, i64)>,
    /// Selected closed integer comparisons retain their exact source operands
    /// and asserted/refuted ordering. They contain no mutable observation.
    closed_integer_arrivals: Vec<(
        ExpressionHandle,
        ExpressionHandle,
        typed_trees::expression::BinaryOperator,
    )>,
    /// Names provably `>= 0`. A SIGNED index access needs `0 <= i < len`; the
    /// upper-bound facts only give `i < len`, so a signed (negative-capable)
    /// index also requires its name here (seeded from `i >= 0`-style guards).
    /// Unsigned indices are non-negative by type and never consult this.
    proven_non_negatives: Vec<String>,
    /// `(lower, upper, strict)`: `lower <= upper`, or `lower < upper` when
    /// strict. Strict entries are the stronger fact — a `<` guard mints one
    /// and a later `<=` on the same pair must not weaken it.
    proven_orderings: Vec<(String, String, bool)>,
    proven_range_bounds: Vec<(String, String)>,
    minimum_lengths: Vec<(String, i64)>,
    /// Exact-length facts: a collection whose length is provably an exact value.
    /// A subslice `a..b` over constant bounds has exact length `b - a`; this is
    /// strictly stronger than a `minimum_length` floor because it also bounds
    /// the length from above, letting a literal index `i < len` be proven and a
    /// range bound `b <= len` be discharged against the precise length.
    exact_lengths: Vec<(String, i64)>,
    /// Window-shrinking facts: `(child, parent, bounds)` records that `child` is
    /// a subslice of `parent` and is therefore no longer than `parent`. `bounds`
    /// holds the child's constant-folded `[start, end)` offsets into the parent
    /// when both are known, enabling provable-disjoint overlap reasoning between
    /// two windows of the same base. A `None` bounds is the conservative case
    /// (offsets unknown — assume overlap with any sibling).
    window_parents: Vec<(String, String, Option<(i64, i64)>)>,
    boolean_locals: Vec<(
        SymbolHandle,
        String,
        typed_trees::expression::ExpressionHandle,
    )>,
    /// Branches borrow one worker-owned lookup over the immutable program.
    /// The lookup itself lazily obtains the invocation's shared bound maps;
    /// fact snapshots never acquire mutable access to query storage.
    bound_lookup: &'field validation::ImmutableBoundLookup<'field>,
}

impl<'field> RangeFacts<'field> {
    pub(super) fn new(
        fields: &'field [(SymbolHandle, String, usize)],
        bound_lookup: &'field validation::ImmutableBoundLookup<'field>,
    ) -> Self {
        Self {
            collection_bounds: None,
            checked_operators: None,
            expression_intervals: &[],
            checked_calls: None,
            mutation_summaries: std::borrow::Cow::Owned(
                crate::flow::StateMutationSummaryCache::default(),
            ),
            statement_index: 0,
            expression_dependencies: Vec::new(),
            scalar_binding_dependencies: Vec::new(),
            measured_results: Vec::new(),
            recorded_expressions: HashSet::default(),
            fields,
            integer_fields: Vec::new(),
            locals: Vec::new(),
            integer_locals: Vec::new(),
            proven_indexes: Vec::new(),
            proven_index_upper_bounds: Vec::new(),
            const_binder_floors: Vec::new(),
            closed_integer_arrivals: Vec::new(),
            proven_non_negatives: Vec::new(),
            proven_orderings: Vec::new(),
            proven_range_bounds: Vec::new(),
            minimum_lengths: Vec::new(),
            exact_lengths: Vec::new(),
            window_parents: Vec::new(),
            boolean_locals: Vec::new(),
            bound_lookup,
        }
    }

    pub(super) fn bound_lookup(&self) -> &'field validation::ImmutableBoundLookup<'field> {
        self.bound_lookup
    }
}

// Count derived snapshots without changing production fact storage or counting
// unrelated allocations performed by guard and expression checking.

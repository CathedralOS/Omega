use symbols::SymbolHandle;

mod dependencies;
mod invalidation;
mod proofs;
mod values;

#[derive(Clone)]
pub(super) struct RangeFacts<'field> {
    pub(super) checked_operators: Option<&'field checked_trees::CheckedOperatorFacts>,
    pub(super) statement_index: usize,
    expression_dependencies: Vec<dependencies::ExpressionDependencies>,
    fields: &'field [(SymbolHandle, String, usize)],
    integer_fields: Vec<(SymbolHandle, String, i64)>,
    locals: Vec<(SymbolHandle, String, usize)>,
    integer_locals: Vec<(SymbolHandle, String, i64)>,
    proven_indexes: Vec<(String, String)>,
    proven_index_upper_bounds: Vec<(String, i64)>,
    /// Names provably `>= 0`. A SIGNED index access needs `0 <= i < len`; the
    /// upper-bound facts only give `i < len`, so a signed (negative-capable)
    /// index also requires its name here (seeded from `i >= 0`-style guards).
    /// Unsigned indices are non-negative by type and never consult this.
    proven_non_negatives: Vec<String>,
    proven_orderings: Vec<(String, String)>,
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
}

impl<'field> RangeFacts<'field> {
    pub(super) fn new(fields: &'field [(SymbolHandle, String, usize)]) -> Self {
        Self {
            checked_operators: None,
            statement_index: 0,
            expression_dependencies: Vec::new(),
            fields,
            integer_fields: Vec::new(),
            locals: Vec::new(),
            integer_locals: Vec::new(),
            proven_indexes: Vec::new(),
            proven_index_upper_bounds: Vec::new(),
            proven_non_negatives: Vec::new(),
            proven_orderings: Vec::new(),
            proven_range_bounds: Vec::new(),
            minimum_lengths: Vec::new(),
            exact_lengths: Vec::new(),
            window_parents: Vec::new(),
            boolean_locals: Vec::new(),
        }
    }
}

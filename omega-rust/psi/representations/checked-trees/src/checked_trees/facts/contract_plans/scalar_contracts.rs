//! Closed float range requirements and closed scalar value contracts.

use numerics::literals::IntegerLiteral;

/// One authored floating range constraint retained on an entry scalar
/// parameter. The endpoints keep IEEE bit identity at the declared carrier
/// and the authored boundary kind verbatim: `maximum_inclusive == false`
/// means the admitted window is `minimum <= x < maximum` under IEEE order,
/// never an integer predecessor of the endpoint. NaN satisfies neither
/// comparison. `position` names the dense entry scalar parameter position,
/// the same namespace `ClosedScalarContractValue` positions use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClosedFloatRangeRequirement {
    pub position: usize,
    pub primitive_type: typed_trees::types::PrimitiveType,
    pub minimum: semantic_vocabulary::IeeeFloatValue,
    pub maximum: semantic_vocabulary::IeeeFloatValue,
    pub maximum_inclusive: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClosedScalarContractValue {
    Boolean(bool),
    Integer(IntegerLiteral),
    /// Integer contract predicate. Positions name entry scalar parameters in
    /// source order; only ensures may additionally name the result at the
    /// position immediately after the last parameter. Source locals and
    /// mutable post-state values do not inhabit this namespace.
    Predicate(crate::CheckedBooleanExpression),
    /// One authored floating entry range in that same entry-parameter
    /// namespace. The clause keeps IEEE bit-exact endpoints and the authored
    /// boundary kind verbatim — an exclusive maximum stays authored, never an
    /// integer predecessor. It discharges through the retained floating range
    /// roster and its terminal catalog rows, not through `Predicate`
    /// propositions, so it may only appear in the requires tail.
    FloatRange(ClosedFloatRangeRequirement),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClosedScalarValueContractPlan {
    requires: Vec<Option<ClosedScalarContractValue>>,
    /// Length of the authored `requires` prefix inside `requires`; the
    /// remainder is the derived parameter-range tail the plan appends.
    authored_requires_len: usize,
    ensures: Vec<Option<ClosedScalarContractValue>>,
    has_crash_clauses: bool,
    has_outcome_specific_clauses: bool,
    /// Floating entry range evidence: one row per authored `f32[..]`/`f64[..]`
    /// range constraint in dense scalar-parameter order. `None` records an
    /// incomplete roster (an authored floating range whose endpoints could
    /// not be retained exactly); consumers must fail closed on `None` and
    /// never read an empty roster as "no ranges". Each retained row also
    /// occupies its requires-tail position as a `FloatRange` clause, so the
    /// closed scalar vocabulary itself carries the authored IEEE window and
    /// no requires row is left unsupported by a range.
    float_entry_ranges: Option<Vec<ClosedFloatRangeRequirement>>,
}

impl ClosedScalarValueContractPlan {
    pub fn new(
        requires: Vec<Option<ClosedScalarContractValue>>,
        ensures: Vec<Option<ClosedScalarContractValue>>,
        has_crash_clauses: bool,
        has_outcome_specific_clauses: bool,
    ) -> Self {
        // A caller that cannot name the authored/range split built this plan
        // from authored rows only, so the whole requires roster is authored.
        let authored_requires_len = requires.len();
        Self {
            requires,
            authored_requires_len,
            ensures,
            has_crash_clauses,
            has_outcome_specific_clauses,
            // Rebuilding callers cannot reconstruct the retained roster; it
            // must ride back on through `with_float_entry_ranges`.
            float_entry_ranges: None,
        }
    }

    pub fn with_authored_requires_len(mut self, authored_requires_len: usize) -> Self {
        self.authored_requires_len = authored_requires_len;
        self
    }

    pub fn with_float_entry_ranges(
        mut self,
        float_entry_ranges: Option<Vec<ClosedFloatRangeRequirement>>,
    ) -> Self {
        self.float_entry_ranges = float_entry_ranges;
        self
    }

    /// The retained floating entry roster, or `None` when it is incomplete.
    pub fn float_entry_ranges(&self) -> Option<&[ClosedFloatRangeRequirement]> {
        self.float_entry_ranges.as_deref()
    }

    pub fn requires(&self) -> &[Option<ClosedScalarContractValue>] {
        &self.requires
    }

    /// Requires rows lowered from authored `requires` contract clauses,
    /// ahead of the derived parameter-range tail.
    pub fn authored_requires(&self) -> &[Option<ClosedScalarContractValue>] {
        &self.requires[..self.authored_requires_len]
    }

    pub fn ensures(&self) -> &[Option<ClosedScalarContractValue>] {
        &self.ensures
    }

    pub const fn has_crash_clauses(&self) -> bool {
        self.has_crash_clauses
    }

    pub const fn has_outcome_specific_clauses(&self) -> bool {
        self.has_outcome_specific_clauses
    }
}

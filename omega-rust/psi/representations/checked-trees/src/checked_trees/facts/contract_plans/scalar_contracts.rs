//! Closed float/integer range requirements and closed scalar value contracts.

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

/// One authored integer range constraint retained on an entry scalar
/// parameter. `position` names the dense entry scalar parameter position,
/// the same namespace `ClosedScalarContractValue` positions use, and
/// `primitive_type` is the parameter's declared fixed-width integer carrier
/// (an address carrier is not an entry-range carrier). The endpoints are the
/// normalized inclusive bounds landed in that carrier — an authored
/// exclusive maximum already became its predecessor — so the row always
/// reads `minimum <= x <= maximum`. Each endpoint literal keeps its exact
/// landing, the same identity the requires-tail `Predicate` conjunction
/// carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosedIntegerRangeRequirement {
    pub position: usize,
    pub primitive_type: typed_trees::types::PrimitiveType,
    pub minimum: IntegerLiteral,
    pub maximum: IntegerLiteral,
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
    /// One authored `FloatMeaning` equality clause over float-semantics
    /// catalog results. `expression` keeps the authored `==` contract node
    /// verbatim; `equality` starts empty and the lowering preparation rejoins
    /// it to the checked float-meaning equality roster — recording the dense
    /// row coordinate the clause discharges through — before the contract
    /// lowers to a terminal proposition. It may only appear in the ensures
    /// tail.
    FloatMeaningEquality {
        expression: typed_trees::expression::ExpressionHandle,
        equality: Option<crate::CheckedProofPropositionId>,
    },
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
    /// Integer entry range evidence: one row per retained authored integer
    /// range constraint in dense scalar-parameter order. `None` records an
    /// incomplete roster (a present integer range whose normalized endpoints
    /// could not be retained exactly); consumers must fail closed on `None`
    /// and never read an empty roster as "no ranges". Unlike the floating
    /// roster these rows duplicate no clause position: the requires tail
    /// already carries each integer range as a `Predicate` conjunction, and
    /// this roster preserves the same landed endpoints as exact evidence for
    /// consumers that cannot read predicate structure.
    integer_entry_ranges: Option<Vec<ClosedIntegerRangeRequirement>>,
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
            // Rebuilding callers cannot reconstruct the retained rosters;
            // they must ride back on through `with_float_entry_ranges` and
            // `with_integer_entry_ranges`.
            float_entry_ranges: None,
            integer_entry_ranges: None,
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

    pub fn with_integer_entry_ranges(
        mut self,
        integer_entry_ranges: Option<Vec<ClosedIntegerRangeRequirement>>,
    ) -> Self {
        self.integer_entry_ranges = integer_entry_ranges;
        self
    }

    /// The retained integer entry roster, or `None` when it is incomplete.
    pub fn integer_entry_ranges(&self) -> Option<&[ClosedIntegerRangeRequirement]> {
        self.integer_entry_ranges.as_deref()
    }

    /// Fold the authored `requires` conjuncts on one entry scalar parameter
    /// into the closed inclusive interval they prove — the contract-fact
    /// evidence a retained integer entry-range row would carry for a bound
    /// the revoked range suffix never declared. `position` is the dense
    /// entry scalar parameter position the `Predicate` clauses and
    /// `CheckedScalarExpression::Parameter` share. An unsigned carrier
    /// supplies its own `0 <=` half; a signed carrier owes an explicit lower
    /// conjunct. Conjuncts this fold cannot read — other parameters,
    /// composed terms, non-literal endpoints, disjunctions, negations —
    /// stay outside the interval rather than declining it; a missing half
    /// or an interval that cannot name a nonnegative element declines.
    pub fn requires_bound_interval(
        &self,
        position: usize,
        primitive_type: typed_trees::types::PrimitiveType,
    ) -> Option<(i128, i128)> {
        let mut minimum: Option<i128> = (!primitive_type.is_signed_integer()).then_some(0);
        let mut maximum = None;
        for clause in self.authored_requires() {
            let Some(ClosedScalarContractValue::Predicate(predicate)) = clause else {
                continue;
            };
            fold_requires_bound_conjunct(
                predicate,
                position,
                primitive_type,
                &mut minimum,
                &mut maximum,
            );
        }
        let (minimum, maximum) = minimum.zip(maximum)?;
        (0 <= minimum && minimum <= maximum).then_some((minimum, maximum))
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

    /// Rejoin each retained float-meaning equality clause to its checked
    /// equality row. `resolve` maps the authored `==` contract expression to
    /// that row's dense coordinate; an unresolved clause reports the authored
    /// expression so the caller can reject it without silent erasure.
    pub fn resolve_float_meaning_equalities(
        &mut self,
        mut resolve: impl FnMut(
            typed_trees::expression::ExpressionHandle,
        ) -> Option<crate::CheckedProofPropositionId>,
    ) -> Result<(), typed_trees::expression::ExpressionHandle> {
        for clause in self.ensures.iter_mut().flatten() {
            let ClosedScalarContractValue::FloatMeaningEquality {
                expression,
                equality,
            } = clause
            else {
                continue;
            };
            if equality.is_none() {
                *equality = Some(resolve(*expression).ok_or(*expression)?);
            }
        }
        Ok(())
    }

    pub const fn has_crash_clauses(&self) -> bool {
        self.has_crash_clauses
    }

    pub const fn has_outcome_specific_clauses(&self) -> bool {
        self.has_outcome_specific_clauses
    }
}

/// Meet one `requires` conjunct's literal bound on the subject into the
/// running interval. Canonical lowering leaves only `Equal`, `LessThan` and
/// `LessOrEqual` integer comparisons: `p <= k` on the left is the upper half
/// and `k <= p` the lower. Conjuncts over other parameters, composed terms,
/// or non-literal endpoints are proof facts this fold does not read, not a
/// reason to decline.
fn fold_requires_bound_conjunct(
    predicate: &crate::CheckedBooleanExpression,
    position: usize,
    primitive_type: typed_trees::types::PrimitiveType,
    minimum: &mut Option<i128>,
    maximum: &mut Option<i128>,
) {
    match predicate {
        crate::CheckedBooleanExpression::And { left, right } => {
            fold_requires_bound_conjunct(left, position, primitive_type, minimum, maximum);
            fold_requires_bound_conjunct(right, position, primitive_type, minimum, maximum);
        }
        crate::CheckedBooleanExpression::IntegerComparison { kind, left, right } => {
            let (endpoint, subject_is_left) = if conjunct_subject(left, position, primitive_type) {
                (conjunct_literal(right), true)
            } else if conjunct_subject(right, position, primitive_type) {
                (conjunct_literal(left), false)
            } else {
                return;
            };
            let Some(endpoint) = endpoint else {
                return;
            };
            let (lower, upper) = match (kind, subject_is_left) {
                (crate::CheckedIntegerComparisonKind::Equal, _) => (Some(endpoint), Some(endpoint)),
                (crate::CheckedIntegerComparisonKind::LessOrEqual, true) => (None, Some(endpoint)),
                (crate::CheckedIntegerComparisonKind::LessThan, true) => {
                    (None, endpoint.checked_sub(1))
                }
                (crate::CheckedIntegerComparisonKind::LessOrEqual, false) => (Some(endpoint), None),
                (crate::CheckedIntegerComparisonKind::LessThan, false) => {
                    (endpoint.checked_add(1), None)
                }
            };
            if let Some(lower) = lower {
                *minimum = Some(minimum.map_or(lower, |bound| bound.max(lower)));
            }
            if let Some(upper) = upper {
                *maximum = Some(maximum.map_or(upper, |bound| bound.min(upper)));
            }
        }
        _ => {}
    }
}

/// The conjunct subject is exactly this entry scalar parameter, in the same
/// dense scalar-parameter namespace the retained `Predicate` positions use.
fn conjunct_subject(
    expression: &crate::CheckedScalarExpression,
    position: usize,
    primitive_type: typed_trees::types::PrimitiveType,
) -> bool {
    matches!(
        expression,
        crate::CheckedScalarExpression::Parameter {
            position: subject,
            primitive_type: carrier,
        } if *subject == position && *carrier == primitive_type
    )
}

/// A conjunct endpoint lands as a literal only when it carries an exact
/// integer value; anything wider than both machine carriers stays unread.
fn conjunct_literal(expression: &crate::CheckedScalarExpression) -> Option<i128> {
    let crate::CheckedScalarExpression::IntegerLiteral { literal } = expression else {
        return None;
    };
    let value = literal.value_bignum()?;
    value
        .to_i64()
        .map(i128::from)
        .or_else(|| value.to_u64().map(i128::from))
}

#[cfg(test)]
mod tests {
    use super::{ClosedScalarContractValue, ClosedScalarValueContractPlan};
    use crate::{CheckedBooleanExpression, CheckedIntegerComparisonKind, CheckedScalarExpression};
    use numerics::literals::IntegerLiteral;
    use typed_trees::types::PrimitiveType;

    fn parameter(position: usize, primitive_type: PrimitiveType) -> Box<CheckedScalarExpression> {
        Box::new(CheckedScalarExpression::Parameter {
            position,
            primitive_type,
        })
    }

    fn literal(value: i64) -> Box<CheckedScalarExpression> {
        Box::new(CheckedScalarExpression::IntegerLiteral {
            literal: IntegerLiteral::from_value(value),
        })
    }

    fn comparison(
        kind: CheckedIntegerComparisonKind,
        left: Box<CheckedScalarExpression>,
        right: Box<CheckedScalarExpression>,
    ) -> CheckedBooleanExpression {
        CheckedBooleanExpression::IntegerComparison { kind, left, right }
    }

    fn plan(predicates: Vec<CheckedBooleanExpression>) -> ClosedScalarValueContractPlan {
        ClosedScalarValueContractPlan::new(
            predicates
                .into_iter()
                .map(|predicate| Some(ClosedScalarContractValue::Predicate(predicate)))
                .collect(),
            Vec::new(),
            false,
            false,
        )
    }

    #[test]
    fn unsigned_upper_only_supplies_the_zero_half() {
        for (kind, expected) in [
            (CheckedIntegerComparisonKind::LessOrEqual, (0, 1)),
            (CheckedIntegerComparisonKind::LessThan, (0, 0)),
            (CheckedIntegerComparisonKind::Equal, (1, 1)),
        ] {
            let contract = plan(vec![comparison(
                kind,
                parameter(0, PrimitiveType::U64),
                literal(1),
            )]);
            assert_eq!(
                contract.requires_bound_interval(0, PrimitiveType::U64),
                Some(expected)
            );
        }
    }

    #[test]
    fn literal_left_conjuncts_supply_the_lower_half() {
        let contract = plan(vec![
            comparison(
                CheckedIntegerComparisonKind::LessOrEqual,
                literal(2),
                parameter(0, PrimitiveType::U64),
            ),
            comparison(
                CheckedIntegerComparisonKind::LessThan,
                literal(0),
                parameter(0, PrimitiveType::U64),
            ),
            comparison(
                CheckedIntegerComparisonKind::LessOrEqual,
                parameter(0, PrimitiveType::U64),
                literal(5),
            ),
        ]);
        // 2 <= i and 0 < i meet at the tighter lower bound.
        assert_eq!(
            contract.requires_bound_interval(0, PrimitiveType::U64),
            Some((2, 5))
        );
    }

    #[test]
    fn nested_conjunctions_meet_every_literal_bound() {
        let contract = plan(vec![CheckedBooleanExpression::And {
            left: Box::new(comparison(
                CheckedIntegerComparisonKind::LessOrEqual,
                literal(1),
                parameter(0, PrimitiveType::U64),
            )),
            right: Box::new(comparison(
                CheckedIntegerComparisonKind::LessOrEqual,
                parameter(0, PrimitiveType::U64),
                literal(3),
            )),
        }]);
        assert_eq!(
            contract.requires_bound_interval(0, PrimitiveType::U64),
            Some((1, 3))
        );
    }

    #[test]
    fn signed_carriers_owe_an_explicit_lower_half() {
        let contract = plan(vec![comparison(
            CheckedIntegerComparisonKind::LessOrEqual,
            parameter(0, PrimitiveType::I64),
            literal(4),
        )]);
        assert_eq!(
            contract.requires_bound_interval(0, PrimitiveType::I64),
            None
        );

        let contract = plan(vec![
            comparison(
                CheckedIntegerComparisonKind::LessOrEqual,
                literal(0),
                parameter(0, PrimitiveType::I64),
            ),
            comparison(
                CheckedIntegerComparisonKind::LessOrEqual,
                parameter(0, PrimitiveType::I64),
                literal(4),
            ),
        ]);
        assert_eq!(
            contract.requires_bound_interval(0, PrimitiveType::I64),
            Some((0, 4))
        );
    }

    #[test]
    fn conjuncts_on_other_parameters_contribute_nothing() {
        let contract = plan(vec![
            comparison(
                CheckedIntegerComparisonKind::LessOrEqual,
                parameter(0, PrimitiveType::U64),
                parameter(1, PrimitiveType::U64),
            ),
            comparison(
                CheckedIntegerComparisonKind::LessOrEqual,
                parameter(1, PrimitiveType::U64),
                literal(9),
            ),
            comparison(
                CheckedIntegerComparisonKind::LessOrEqual,
                parameter(0, PrimitiveType::U64),
                literal(7),
            ),
        ]);
        // i <= j reads no literal endpoint and j's bound never transfers.
        assert_eq!(
            contract.requires_bound_interval(0, PrimitiveType::U64),
            Some((0, 7))
        );
        assert_eq!(
            contract.requires_bound_interval(1, PrimitiveType::U64),
            Some((0, 9))
        );
    }

    #[test]
    fn missing_upper_half_or_empty_interval_declines() {
        let contract = plan(vec![comparison(
            CheckedIntegerComparisonKind::LessOrEqual,
            literal(0),
            parameter(0, PrimitiveType::U64),
        )]);
        assert_eq!(
            contract.requires_bound_interval(0, PrimitiveType::U64),
            None
        );

        let contract = plan(vec![
            comparison(
                CheckedIntegerComparisonKind::LessOrEqual,
                parameter(0, PrimitiveType::U64),
                literal(0),
            ),
            comparison(
                CheckedIntegerComparisonKind::LessOrEqual,
                literal(5),
                parameter(0, PrimitiveType::U64),
            ),
        ]);
        // 5 <= i <= 0 names no element, so no bound evidence survives.
        assert_eq!(
            contract.requires_bound_interval(0, PrimitiveType::U64),
            None
        );
    }

    #[test]
    fn disjunctions_and_negations_stay_unread() {
        let contract = plan(vec![CheckedBooleanExpression::Or {
            left: Box::new(comparison(
                CheckedIntegerComparisonKind::LessOrEqual,
                parameter(0, PrimitiveType::U64),
                literal(1),
            )),
            right: Box::new(comparison(
                CheckedIntegerComparisonKind::LessOrEqual,
                parameter(0, PrimitiveType::U64),
                literal(5),
            )),
        }]);
        assert_eq!(
            contract.requires_bound_interval(0, PrimitiveType::U64),
            None
        );

        let contract = plan(vec![CheckedBooleanExpression::Not(Box::new(comparison(
            CheckedIntegerComparisonKind::LessOrEqual,
            parameter(0, PrimitiveType::U64),
            literal(1),
        )))]);
        assert_eq!(
            contract.requires_bound_interval(0, PrimitiveType::U64),
            None
        );
    }
}

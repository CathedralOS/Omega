//! The closed integer interval a declared domain's predicates guarantee.
//!
//! One derivation shared by the two readers that need it: the index prover's
//! `enforced_range_of_type_reference` and this crate's
//! `range_constraint_interval`. A bound stated as a domain must reach both, or
//! moving it off the revoked bracketed suffix silently loses the fact.

/// The closed integer interval a DECLARED domain's predicates guarantee for any
/// value in it, or `None` when none is recognizable.
///
/// Membership requires every predicate the domain declares -- routing adds a
/// provenance obligation, it does not remove the predicates -- so each
/// predicate is true of every member and an unrecognized one costs only
/// precision, never soundness. A predicate naming anything but `self` and a
/// closed integer literal is simply not read.
///
/// An ALIAS expands to constituent requirements this reader does not walk, and
/// an INDEXED family's bound carries a retained index identity a bare interval
/// cannot represent; both decline.
pub fn declared_domain_predicate_bounds(
    program: &typed_trees::TypedTrees,
    domain: &typed_trees::types::DomainConstraint,
) -> Option<(numerics::bignum::BigInt, numerics::bignum::BigInt)> {
    use typed_trees::types::DomainConstraintSubject;
    if domain.subject != DomainConstraintSubject::Declared || !domain.arguments.is_empty() {
        return None;
    }
    let definition = program
        .domain_definitions()
        .iter()
        .find(|definition| definition.symbol == domain.symbol)?;
    if definition.alias.is_some()
        || !definition.index_arguments.is_empty()
        || !typed_trees::domain::index_parameters(program, definition).is_empty()
    {
        return None;
    }
    let mut bounds: Option<(numerics::bignum::BigInt, numerics::bignum::BigInt)> = None;
    for fact in program.proof_facts(definition) {
        let typed_trees::domain::ProofFact::Expression(expression) = fact else {
            continue;
        };
        for (minimum, maximum) in self_predicate_bounds(program, *expression) {
            bounds = Some(match bounds {
                Some((prior_minimum, prior_maximum)) => {
                    (minimum.max(prior_minimum), maximum.min(prior_maximum))
                }
                None => (minimum, maximum),
            });
        }
    }
    bounds
}

/// Each closed bound a predicate over `self` states, as a full interval with
/// the unconstrained side left at the carrier-independent extreme. `&&`
/// contributes both sides; any other shape contributes nothing.
fn self_predicate_bounds(
    program: &typed_trees::TypedTrees,
    expression: typed_trees::expression::ExpressionHandle,
) -> Vec<(numerics::bignum::BigInt, numerics::bignum::BigInt)> {
    use numerics::bignum::BigInt;
    use typed_trees::expression::BinaryOperator;
    let Some(binary) = (match program.expression_table.expression(expression) {
        typed_trees::expression::ExpressionNode::Binary(binary) => Some(binary),
        _ => None,
    }) else {
        return Vec::new();
    };
    if binary.operator == BinaryOperator::And {
        let mut bounds = self_predicate_bounds(program, binary.left);
        bounds.extend(self_predicate_bounds(program, binary.right));
        return bounds;
    }
    let left_is_self = expression_is_bare_self(program, binary.left);
    let right_is_self = expression_is_bare_self(program, binary.right);
    // Exactly one side is the subject; `self <= self` states nothing this
    // reader can turn into a literal interval.
    let (literal, subject_on_left) = match (left_is_self, right_is_self) {
        (true, false) => (binary.right, true),
        (false, true) => (binary.left, false),
        _ => return Vec::new(),
    };
    let Some(value) = crate::closed_integer_range_bound(program, literal) else {
        return Vec::new();
    };
    let one = BigInt::from_i64(1);
    // `self OP value` when the subject is on the left; otherwise the mirrored
    // reading of `value OP self`.
    let (low, high) = match (binary.operator, subject_on_left) {
        (BinaryOperator::LessOrEqual, true) | (BinaryOperator::GreaterOrEqual, false) => {
            (None, Some(value))
        }
        (BinaryOperator::Less, true) | (BinaryOperator::Greater, false) => {
            (None, Some(value.sub(&one)))
        }
        (BinaryOperator::GreaterOrEqual, true) | (BinaryOperator::LessOrEqual, false) => {
            (Some(value), None)
        }
        (BinaryOperator::Greater, true) | (BinaryOperator::Less, false) => {
            (Some(value.add(&one)), None)
        }
        (BinaryOperator::Equal, _) => (Some(value.clone()), Some(value)),
        _ => return Vec::new(),
    };
    let minimum = low.unwrap_or_else(|| BigInt::from_i64(i64::MIN));
    let maximum = high.unwrap_or_else(|| BigInt::from_i64(i64::MAX));
    vec![(minimum, maximum)]
}

/// The bare `self` subject of a domain predicate: a name with no receiver and
/// no field path. `self.field` is a different value and states nothing about
/// the place the domain qualifies.
fn expression_is_bare_self(
    program: &typed_trees::TypedTrees,
    expression: typed_trees::expression::ExpressionHandle,
) -> bool {
    matches!(
        program.expression_table.expression(expression),
        typed_trees::expression::ExpressionNode::Name(path)
            if program
                .expression_table
                .name_path_members(path.members)
                .last()
                .is_some_and(|name| name.as_str() == "self")
    )
}

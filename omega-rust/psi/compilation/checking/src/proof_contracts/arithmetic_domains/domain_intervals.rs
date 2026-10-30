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
/// closed integer literal or selected closed integer index is simply not read.
///
/// An alias expands to constituent requirements this reader does not walk.
/// An indexed family contributes bounds only after its exact normalized
/// application supplies every closed integer index. The summary never changes
/// that application identity: it describes values of this selected instance,
/// not compatibility between different indices or a way to infer an index.
pub fn declared_domain_predicate_bounds(
    program: &dyn crate::CheckingInput,
    domain: &typed_trees::types::DomainConstraint,
) -> Option<(numerics::bignum::BigInt, numerics::bignum::BigInt)> {
    let interval = predicate_interval(program, domain)?;
    if interval.minimum.is_none() && interval.maximum.is_none() {
        return None;
    }
    let declaration = program.domain_definition(domain.symbol)?;
    let primitive =
        crate::value_custody::recasts::exact_primitive_type(program, declaration.target_type)?;
    let (minimum, maximum) = crate::proof_contracts::proof_embeddings::primitive_range(primitive)?;
    Some((
        interval.minimum.unwrap_or(minimum),
        interval.maximum.unwrap_or(maximum),
    ))
}

/// The interval a declared domain MEANS, when its membership is exactly that
/// interval: a route-free domain every one of whose predicates is a closed
/// bound on `self`. A carrier that checks the interval at every write and
/// trusts it at every read then represents membership completely. An
/// unstated side is `None`, the carrier's own extreme; a routed domain, an
/// unrecognized predicate, or no stated side at all declines.
pub fn exact_declared_domain_interval(
    program: &dyn crate::CheckingInput,
    domain: &typed_trees::types::DomainConstraint,
) -> Option<(
    Option<numerics::bignum::BigInt>,
    Option<numerics::bignum::BigInt>,
)> {
    let interval = predicate_interval(program, domain)?;
    let definition = program.domain_definition(domain.symbol)?;
    (interval.exact
        && definition.establishment_routes.is_empty()
        && (interval.minimum.is_some() || interval.maximum.is_some()))
    .then_some((interval.minimum, interval.maximum))
}

/// [`exact_declared_domain_interval`] closed over its integer carrier: an
/// unstated side is the carrier's extreme, and a stated side never widens
/// past it.
pub fn exact_declared_domain_carrier_interval(
    program: &dyn crate::CheckingInput,
    carrier: typed_trees::types::PrimitiveType,
    domain: &typed_trees::types::DomainConstraint,
) -> Option<(numerics::bignum::BigInt, numerics::bignum::BigInt)> {
    let (minimum, maximum) = exact_declared_domain_interval(program, domain)?;
    let definition = typed_trees::domain::domain_by_symbol(program, domain.symbol)?;
    if crate::value_custody::recasts::exact_primitive_type(program, definition.target_type)?
        != carrier
    {
        return None;
    }
    let (carrier_minimum, carrier_maximum) =
        crate::proof_contracts::proof_embeddings::primitive_range(carrier)?;
    Some((
        minimum.map_or(carrier_minimum.clone(), |minimum| {
            minimum.max(carrier_minimum)
        }),
        maximum.map_or(carrier_maximum.clone(), |maximum| {
            maximum.min(carrier_maximum)
        }),
    ))
}

/// The recognized sides of a declared domain's predicates, and whether every
/// predicate was recognized.
struct PredicateInterval {
    minimum: Option<numerics::bignum::BigInt>,
    maximum: Option<numerics::bignum::BigInt>,
    exact: bool,
}

fn predicate_interval(
    program: &dyn crate::CheckingInput,
    domain: &typed_trees::types::DomainConstraint,
) -> Option<PredicateInterval> {
    use typed_trees::types::DomainConstraintSubject;
    if domain.subject != DomainConstraintSubject::Declared {
        return None;
    }
    application_predicate_interval(
        program,
        domain.symbol,
        &domain.arguments,
        domain.semantic_id,
    )
}

/// Numeric consequences of a membership the caller already assumes. Open
/// endpoints remain open; a routed identity is never inferred from this range.
pub(crate) fn membership_predicate_bounds(
    program: &dyn crate::CheckingInput,
    membership: &typed_trees::domain::ProofMembershipFact,
) -> Option<(
    Option<numerics::bignum::BigInt>,
    Option<numerics::bignum::BigInt>,
)> {
    let arguments = program
        .type_reference_table
        .type_reference_handles(membership.domain_arguments);
    if arguments.len() != membership.domain_arguments.len() {
        return None;
    }
    let interval = application_predicate_interval(
        program,
        membership.domain_symbol,
        arguments,
        membership.semantic_domain,
    )?;
    Some((interval.minimum, interval.maximum))
}

fn application_predicate_interval(
    program: &dyn crate::CheckingInput,
    symbol: symbols::SymbolHandle,
    arguments: &[typed_trees::types::TypeReferenceHandle],
    semantic_domain: language_semantics::SemanticDomainId,
) -> Option<PredicateInterval> {
    let definition = program.domain_definition(symbol)?;
    if definition.alias.is_some() {
        return None;
    }
    let indices = crate::proof_contracts::domains::closed_integer_index_bindings(
        program,
        definition,
        arguments,
        semantic_domain,
    )?;
    let mut interval = PredicateInterval {
        minimum: None,
        maximum: None,
        exact: true,
    };
    for fact in program.proof_facts(definition) {
        let Some(expression) = fact.as_expression() else {
            interval.exact = false;
            continue;
        };
        interval.exact &= self_predicate_bounds(
            program,
            definition,
            *expression,
            &indices,
            &mut |minimum, maximum| {
                if let Some(minimum) = minimum {
                    interval.minimum = Some(match interval.minimum.take() {
                        Some(prior) => minimum.max(prior),
                        None => minimum,
                    });
                }
                if let Some(maximum) = maximum {
                    interval.maximum = Some(match interval.maximum.take() {
                        Some(prior) => maximum.min(prior),
                        None => maximum,
                    });
                }
            },
        );
    }
    Some(interval)
}

/// Report each closed bound a predicate over `self` states, one side at a
/// time, and return whether the whole predicate was read. `&&` contributes
/// both sides; any other shape contributes nothing and is not read.
fn self_predicate_bounds(
    program: &dyn crate::CheckingInput,
    definition: &typed_trees::domain::DomainDefinition,
    expression: typed_trees::expression::ExpressionHandle,
    indices: &[(symbols::SymbolHandle, numerics::bignum::BigInt)],
    bound: &mut impl FnMut(Option<numerics::bignum::BigInt>, Option<numerics::bignum::BigInt>),
) -> bool {
    use typed_trees::expression::BinaryOperator;
    if !crate::has_builtin_domain_decomposed_guard_meaning(program, definition, expression) {
        return false;
    }
    let typed_trees::expression::ExpressionNode::Binary(binary) =
        program.expression_table.expression(expression)
    else {
        return false;
    };
    if binary.operator == BinaryOperator::And {
        let left = self_predicate_bounds(program, definition, binary.left, indices, bound);
        let right = self_predicate_bounds(program, definition, binary.right, indices, bound);
        return left && right;
    }
    let left_is_self = crate::exact_domain_subject_type(program, definition, binary.left).is_some();
    let right_is_self =
        crate::exact_domain_subject_type(program, definition, binary.right).is_some();
    // Exactly one side is the subject; `self <= self` states nothing this
    // reader can turn into a literal interval.
    let (literal, subject_on_left) = match (left_is_self, right_is_self) {
        (true, false) => (binary.right, true),
        (false, true) => (binary.left, false),
        _ => return false,
    };
    let Some(value) = crate::closed_integer_range_bound(program, literal).or_else(|| {
        let typed_trees::expression::ExpressionNode::Name(path) =
            program.expression_table.expression(literal)
        else {
            return None;
        };
        (program
            .expression_table
            .name_path_members(path.members)
            .len()
            == 1)
            .then(|| indices.iter().find(|(symbol, _)| *symbol == path.symbol))
            .flatten()
            .map(|(_, value)| value.clone())
    }) else {
        return false;
    };
    let Some((low, high)) =
        closed_interval_from_comparison(binary.operator, subject_on_left, value)
    else {
        return false;
    };
    bound(low, high);
    true
}

/// The closed bounds one comparison states: `subject OP value` when the
/// subject is on the left, the mirrored reading otherwise. `!=` and the
/// non-comparison operators bound nothing closed.
pub fn closed_interval_from_comparison(
    operator: typed_trees::expression::BinaryOperator,
    subject_on_left: bool,
    value: numerics::bignum::BigInt,
) -> Option<(
    Option<numerics::bignum::BigInt>,
    Option<numerics::bignum::BigInt>,
)> {
    use numerics::bignum::BigInt;
    use typed_trees::expression::BinaryOperator as Op;
    let one = BigInt::from_i64(1);
    let (low, high) = match (operator, subject_on_left) {
        (Op::LessOrEqual, true) | (Op::GreaterOrEqual, false) => (None, Some(value)),
        (Op::Less, true) | (Op::Greater, false) => (None, Some(value.sub(&one))),
        (Op::GreaterOrEqual, true) | (Op::LessOrEqual, false) => (Some(value), None),
        (Op::Greater, true) | (Op::Less, false) => (Some(value.add(&one)), None),
        (Op::Equal, _) => (Some(value.clone()), Some(value)),
        _ => return None,
    };
    Some((low, high))
}

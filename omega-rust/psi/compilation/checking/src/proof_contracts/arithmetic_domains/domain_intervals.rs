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
    program: &typed_trees::TypedTrees,
    domain: &typed_trees::types::DomainConstraint,
) -> Option<(numerics::bignum::BigInt, numerics::bignum::BigInt)> {
    use numerics::bignum::BigInt;
    let interval = predicate_interval(program, domain)?;
    if interval.minimum.is_none() && interval.maximum.is_none() {
        return None;
    }
    Some((
        interval
            .minimum
            .unwrap_or_else(|| BigInt::from_i64(i64::MIN)),
        interval
            .maximum
            .unwrap_or_else(|| BigInt::from_i64(i64::MAX)),
    ))
}

/// The interval a declared domain MEANS, when its membership is exactly that
/// interval: a route-free domain every one of whose predicates is a closed
/// bound on `self`. A carrier that checks the interval at every write and
/// trusts it at every read then represents membership completely. An
/// unstated side is `None`, the carrier's own extreme; a routed domain, an
/// unrecognized predicate, or no stated side at all declines.
pub fn exact_declared_domain_interval(
    program: &typed_trees::TypedTrees,
    domain: &typed_trees::types::DomainConstraint,
) -> Option<(
    Option<numerics::bignum::BigInt>,
    Option<numerics::bignum::BigInt>,
)> {
    let interval = predicate_interval(program, domain)?;
    let definition = program
        .domain_definitions()
        .iter()
        .find(|definition| definition.symbol == domain.symbol)?;
    (interval.exact
        && definition.establishment_routes.is_empty()
        && (interval.minimum.is_some() || interval.maximum.is_some()))
    .then_some((interval.minimum, interval.maximum))
}

/// [`exact_declared_domain_interval`] closed over its integer carrier: an
/// unstated side is the carrier's extreme, and a stated side never widens
/// past it.
pub fn exact_declared_domain_carrier_interval(
    program: &typed_trees::TypedTrees,
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
    program: &typed_trees::TypedTrees,
    domain: &typed_trees::types::DomainConstraint,
) -> Option<PredicateInterval> {
    use typed_trees::types::DomainConstraintSubject;
    if domain.subject != DomainConstraintSubject::Declared {
        return None;
    }
    let definition = program
        .domain_definitions()
        .iter()
        .find(|definition| definition.symbol == domain.symbol)?;
    if definition.alias.is_some() {
        return None;
    }
    let indices = closed_index_bindings(program, definition, domain)?;
    let mut interval = PredicateInterval {
        minimum: None,
        maximum: None,
        exact: true,
    };
    for fact in program.proof_facts(definition) {
        let typed_trees::domain::ProofFact::Expression(expression) = fact else {
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
    program: &typed_trees::TypedTrees,
    definition: &typed_trees::domain::DomainDefinition,
    expression: typed_trees::expression::ExpressionHandle,
    indices: &[(symbols::SymbolHandle, numerics::bignum::BigInt)],
    bound: &mut impl FnMut(Option<numerics::bignum::BigInt>, Option<numerics::bignum::BigInt>),
) -> bool {
    use numerics::bignum::BigInt;
    use typed_trees::expression::BinaryOperator;
    if !crate::has_builtin_domain_decomposed_guard_meaning(program, definition, expression) {
        return false;
    }
    let Some(binary) = (match program.expression_table.expression(expression) {
        typed_trees::expression::ExpressionNode::Binary(binary) => Some(binary),
        _ => None,
    }) else {
        return false;
    };
    if binary.operator == BinaryOperator::And {
        let left = self_predicate_bounds(program, definition, binary.left, indices, bound);
        let right = self_predicate_bounds(program, definition, binary.right, indices, bound);
        return left && right;
    }
    let left_is_self = crate::exact_domain_self_type(program, definition, binary.left).is_some();
    let right_is_self = crate::exact_domain_self_type(program, definition, binary.right).is_some();
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
        _ => return false,
    };
    bound(low, high);
    true
}

/// Bind the declaration's static telescope without consulting runtime values
/// or flow bounds. Exact normalized identity and carrier checks keep a summary
/// of `AtMost<256>` from authorizing `AtMost<512>` or an open application.
fn closed_index_bindings(
    program: &typed_trees::TypedTrees,
    definition: &typed_trees::domain::DomainDefinition,
    domain: &typed_trees::types::DomainConstraint,
) -> Option<Vec<(symbols::SymbolHandle, numerics::bignum::BigInt)>> {
    use language_semantics::const_value::{CanonicalConstValue, DecodedCanonicalConstValue};
    use numerics::bignum::BigInt;
    use typed_trees::data::TypeParameterKind;
    use typed_trees::types::{PrimitiveType, TypeReferenceNode};

    let parameters = typed_trees::domain::index_parameters(program, definition);
    if parameters.len() != domain.arguments.len()
        || definition.index_arguments.len() != parameters.len()
        || typed_trees::domain::has_generic_carrier(program, definition)
    {
        return None;
    }
    // The declaration's head must name its index binders in telescope order.
    // A computed or reordered head is not permission to solve an equation for
    // a binder or substitute one index's value into another index's predicate.
    if !parameters
        .iter()
        .zip(&definition.index_arguments)
        .all(|(parameter, argument)| {
            matches!(program.type_reference_table.type_reference(*argument),
            TypeReferenceNode::Named { symbol, .. } if *symbol == parameter.symbol)
        })
    {
        return None;
    }
    let identity = typed_trees::domain::indexed_domain_instance_name(
        program,
        definition,
        parameters,
        &domain.arguments,
    )
    .ok()?;
    if !domain.semantic_id.is_valid()
        || program.semantic_domains.name(domain.semantic_id) != Some(identity.as_str())
    {
        return None;
    }
    parameters
        .iter()
        .zip(&domain.arguments)
        .map(|(parameter, argument)| {
            let TypeParameterKind::Const { type_reference } = parameter.kind else {
                return None;
            };
            if !parameter.symbol.is_valid() {
                return None;
            }
            let carrier = program.primitive_type_reference(type_reference)?;
            let (minimum, maximum) =
                crate::proof_contracts::proof_embeddings::primitive_range(carrier)?;
            let value = match program.type_reference_table.type_reference(*argument) {
                TypeReferenceNode::Named { name, .. } => {
                    if let Some(canonical) = CanonicalConstValue::from_atom(name.as_str()) {
                        if PrimitiveType::from_name(&canonical.type_name) != Some(carrier) {
                            return None;
                        }
                        let DecodedCanonicalConstValue::Integer { value, .. } =
                            canonical.decode_encoding()?
                        else {
                            return None;
                        };
                        BigInt::from_i128(value)
                    } else {
                        BigInt::from_decimal_str(name.as_str())?
                    }
                }
                TypeReferenceNode::ConstExpression(expression) => {
                    crate::closed_integer_range_bound(program, *expression)?
                }
                _ => return None,
            };
            (minimum <= value && value <= maximum).then_some((parameter.symbol, value))
        })
        .collect()
}

#[cfg(test)]
mod tests;

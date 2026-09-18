//! Summary crash predicates, guards, buckets and their normalization.

use crate::facts::crash_calls::route_substitution::substitute_checked_boolean_expression;
use checked_trees::CrashPredicateExpression;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum SummaryCrashRouteGuard {
    Truth,
    Predicate(SummaryCrashPredicate),
}

#[derive(Debug, Clone)]
pub(crate) struct SummaryCrashPredicate {
    pub(crate) identity: CrashPredicateExpression,
    pub(crate) builtin_meaning: bool,
    pub(crate) scalar: Option<checked_trees::CheckedBooleanExpression>,
}

impl PartialEq for SummaryCrashPredicate {
    fn eq(&self, other: &Self) -> bool {
        self.identity == other.identity && self.builtin_meaning == other.builtin_meaning
    }
}

impl Eq for SummaryCrashPredicate {}

impl PartialOrd for SummaryCrashPredicate {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SummaryCrashPredicate {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.identity
            .cmp(&other.identity)
            .then_with(|| self.builtin_meaning.cmp(&other.builtin_meaning))
    }
}

#[derive(Debug, Clone)]
pub(crate) struct CallArgumentSubstitution {
    pub(crate) identity: Vec<Option<CrashPredicateExpression>>,
    pub(crate) scalar: Vec<Option<checked_trees::CheckedScalarExpression>>,
    /// One caller structural root per target parameter, in the callee's
    /// telescope order: `self` rows and actuals that do not resolve to a
    /// frozen parameter-rooted caller place keep `None`. A
    /// `StructuralParameterField` leaf names the authored telescope, not the
    /// dense scalar namespace, so it re-roots through this channel — the
    /// surviving leaf keeps the caller position and prepends the actual's own
    /// member path.
    pub(crate) fields: Vec<Option<checked_trees::CheckedStructuralParameterField>>,
    /// One literal operand per target parameter: the exact value this call's
    /// own entry contexts prove for its actual, `None` where the flow proves
    /// no single value. These decide a retained guard under its own fold
    /// rules; they never become caller entry identity and never re-read a
    /// mutable initializer.
    pub(crate) values: Vec<Option<CrashPredicateExpression>>,
}

/// Reduce only closed proof-literal comparisons after entry substitution.
/// The predicate identity is domain-free: arithmetic operands stay opaque
/// here so no caller-selected meaning is silently replaced by builtin laws.
pub(crate) fn summary_boolean_value(expression: &CrashPredicateExpression) -> Option<bool> {
    use typed_trees::expression::{BinaryOperator, UnaryOperator};

    match expression {
        CrashPredicateExpression::Boolean(value) => Some(*value),
        CrashPredicateExpression::Unary { operator, operand }
            if *operator == UnaryOperator::LogicalNot as u8 =>
        {
            Some(!summary_boolean_value(operand)?)
        }
        CrashPredicateExpression::Binary {
            operator,
            left,
            right,
        } => {
            if *operator == BinaryOperator::And as u8 {
                return Some(summary_boolean_value(left)? && summary_boolean_value(right)?);
            }
            if *operator == BinaryOperator::Or as u8 {
                return Some(summary_boolean_value(left)? || summary_boolean_value(right)?);
            }
            let left = summary_integer_literal(left)?;
            let right = summary_integer_literal(right)?;
            match *operator {
                operator if operator == BinaryOperator::Equal as u8 => Some(left == right),
                operator if operator == BinaryOperator::NotEqual as u8 => Some(left != right),
                operator if operator == BinaryOperator::Less as u8 => Some(left < right),
                operator if operator == BinaryOperator::LessOrEqual as u8 => Some(left <= right),
                operator if operator == BinaryOperator::Greater as u8 => Some(left > right),
                operator if operator == BinaryOperator::GreaterOrEqual as u8 => Some(left >= right),
                _ => None,
            }
        }
        _ => None,
    }
}

fn summary_integer_literal(
    expression: &CrashPredicateExpression,
) -> Option<numerics::bignum::BigInt> {
    let CrashPredicateExpression::Integer(text) = expression else {
        return None;
    };
    let (negative, unsigned) = text
        .strip_prefix('-')
        .map_or((false, text.as_str()), |digits| (true, digits));
    let (base, digits) = if let Some(digits) = unsigned.strip_prefix("0x") {
        (16, digits)
    } else if let Some(digits) = unsigned.strip_prefix("0o") {
        (8, digits)
    } else if let Some(digits) = unsigned.strip_prefix("0b") {
        (2, digits)
    } else {
        (10, unsigned)
    };
    // Predicate identities carry canonical literal text, not source suffixes
    // or arbitrary numeric expressions. Reject malformed identities explicitly.
    if digits.is_empty() || !digits.chars().all(|character| character.is_digit(base)) {
        return None;
    }
    let magnitude = numerics::bignum::BigInt::from_str_radix(digits, base)?;
    Some(if negative {
        magnitude.negate()
    } else {
        magnitude
    })
}

/// The checked scalar evidence for an arithmetic guard is a (possibly negated)
/// integer comparison. It is the only annotation form the domain-free identity
/// cannot fold itself, so it is the only form consulted for discharge.
pub(crate) fn scalar_guard_is_integer_comparison(
    expression: &checked_trees::CheckedBooleanExpression,
) -> bool {
    use checked_trees::CheckedBooleanExpression;
    match expression {
        CheckedBooleanExpression::IntegerComparison { .. } => true,
        CheckedBooleanExpression::Not(operand) => {
            matches!(
                &**operand,
                CheckedBooleanExpression::IntegerComparison { .. }
            )
        }
        _ => false,
    }
}

/// Evaluate a fully substituted checked scalar guard. `Some` is decided only
/// from closed concrete operands under the selected domains; `None` leaves
/// the route conservative and never establishes or erases a cause by itself.
pub(crate) fn concrete_guard_scalar_value(
    expression: &checked_trees::CheckedBooleanExpression,
) -> Option<bool> {
    match crate::values::evaluate_checked_scalar(
        &checked_trees::CheckedScalarExpression::Boolean(Box::new(expression.clone())),
        &mut |_| None,
    )? {
        facts::ScalarValue::Boolean(value) => Some(value),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct SummaryCrashBucket {
    pub(crate) cause: checked_trees::CrashCause,
    pub(crate) alternative_guards: Vec<SummaryCrashRouteGuard>,
}

impl SummaryCrashBucket {
    pub(crate) fn unconditional(cause: checked_trees::CrashCause) -> Self {
        Self {
            cause,
            alternative_guards: vec![SummaryCrashRouteGuard::Truth],
        }
    }

    pub(crate) fn substitute(&self, arguments: &CallArgumentSubstitution) -> Self {
        self.substitute_with_entry_resolver(arguments, None)
    }

    /// `substitute` with an optional per-projection operand resolver. When
    /// `resolve` is present it answers each referenced formal's entry operand
    /// at the member projection the guard reads — a caller written only in a
    /// sibling field keeps the read field's caller name instead of widening
    /// to `Truth`. Callers without live invocation context (the private
    /// fixed point, hand-built test substitutions) pass `None` and keep the
    /// whole-operand `arguments.identity` boundary.
    pub(crate) fn substitute_with_entry_resolver(
        &self,
        arguments: &CallArgumentSubstitution,
        mut resolve: Option<&mut dyn FnMut(u32, &[String]) -> Option<CrashPredicateExpression>>,
    ) -> Self {
        let mut guards = self
            .alternative_guards
            .iter()
            .filter_map(|guard| match guard {
                SummaryCrashRouteGuard::Truth => Some(SummaryCrashRouteGuard::Truth),
                SummaryCrashRouteGuard::Predicate(predicate) => {
                    // Live storage evidence decides a guard entry custody
                    // cannot carry: each referenced formal is replaced by the
                    // one literal the call's own entry contexts prove for its
                    // actual. A missing value leaves the substitution
                    // missing rather than inventing an origin, and the fold
                    // still honors this route's own selected meaning.
                    if let Some(value) = crate::facts::crash_entry_values::substitute_entry(
                        &predicate.identity,
                        &arguments.values,
                    )
                    .and_then(|substituted| {
                        if predicate.builtin_meaning {
                            summary_boolean_value(&substituted)
                        } else {
                            substituted.boolean_value()
                        }
                    }) {
                        return value.then_some(SummaryCrashRouteGuard::Truth);
                    }
                    let identity = match resolve.as_deref_mut() {
                        Some(resolve) => {
                            crate::facts::crash_entry_values::substitute_entry_projected(
                                &predicate.identity,
                                resolve,
                            )
                        }
                        None => crate::facts::crash_entry_values::substitute_entry(
                            &predicate.identity,
                            &arguments.identity,
                        ),
                    };
                    let Some(identity) = identity else {
                        // A current storage read with no entry-value custody
                        // cannot become a caller Parameter. Retain its cause,
                        // without inventing a guard in the caller namespace.
                        return Some(SummaryCrashRouteGuard::Truth);
                    };
                    let scalar = predicate.scalar.as_ref().and_then(|scalar| {
                        substitute_checked_boolean_expression(
                            scalar,
                            &arguments.scalar,
                            &arguments.fields,
                        )
                    });
                    let folded = if predicate.builtin_meaning {
                        summary_boolean_value(&identity)
                    } else {
                        identity.boolean_value()
                    };
                    // Domain-free folding stands down on arithmetic operands:
                    // a checked integer-comparison annotation remains the only
                    // authority that can decide them under their selected
                    // domains. An annotation never substitutes for the
                    // retained identity and never erases a guard it cannot
                    // describe.
                    let value = folded.or_else(|| {
                        (predicate.builtin_meaning
                            && scalar
                                .as_ref()
                                .is_some_and(scalar_guard_is_integer_comparison))
                        .then(|| scalar.as_ref().and_then(concrete_guard_scalar_value))
                        .flatten()
                    });
                    match value {
                        Some(false) => None,
                        Some(true) => Some(SummaryCrashRouteGuard::Truth),
                        None => Some(SummaryCrashRouteGuard::Predicate(SummaryCrashPredicate {
                            identity,
                            builtin_meaning: predicate.builtin_meaning,
                            scalar,
                        })),
                    }
                }
            })
            .collect::<Vec<_>>();
        normalize_summary_guards(&mut guards);
        Self {
            cause: self.cause,
            alternative_guards: guards,
        }
    }

    pub(crate) fn into_checked(self) -> Option<checked_trees::CrashRouteBucket> {
        checked_trees::CrashRouteBucket::new(
            self.cause,
            self.alternative_guards
                .into_iter()
                .map(|guard| match guard {
                    SummaryCrashRouteGuard::Truth => checked_trees::CrashRouteGuard::Truth,
                    SummaryCrashRouteGuard::Predicate(predicate) => {
                        let identity = if let Some(scalar) = predicate.scalar {
                            checked_trees::CrashPredicateIdentity::from_expression_and_scalar(
                                predicate.identity,
                                scalar,
                            )
                        } else {
                            checked_trees::CrashPredicateIdentity::from_expression(
                                predicate.identity,
                            )
                        };
                        checked_trees::CrashRouteGuard::Predicate(identity)
                    }
                })
                .collect(),
        )
    }
}

pub(crate) fn normalize_summary_guards(guards: &mut Vec<SummaryCrashRouteGuard>) {
    if guards.contains(&SummaryCrashRouteGuard::Truth) {
        guards.clear();
        guards.push(SummaryCrashRouteGuard::Truth);
        return;
    }
    guards.sort();
    let mut normalized = Vec::<SummaryCrashRouteGuard>::with_capacity(guards.len());
    for guard in guards.drain(..) {
        if let (
            Some(SummaryCrashRouteGuard::Predicate(existing)),
            SummaryCrashRouteGuard::Predicate(candidate),
        ) = (normalized.last_mut(), &guard)
            && existing == candidate
        {
            if existing.scalar.is_none() {
                existing.scalar.clone_from(&candidate.scalar);
            }
            continue;
        }
        normalized.push(guard);
    }
    *guards = normalized;
}

pub(crate) fn normalize_summary_buckets(
    buckets: Vec<SummaryCrashBucket>,
) -> Vec<SummaryCrashBucket> {
    let mut grouped =
        std::collections::BTreeMap::<checked_trees::CrashCause, Vec<SummaryCrashRouteGuard>>::new();
    for bucket in buckets {
        grouped
            .entry(bucket.cause)
            .or_default()
            .extend(bucket.alternative_guards);
    }
    grouped
        .into_iter()
        .filter_map(|(cause, mut alternative_guards)| {
            normalize_summary_guards(&mut alternative_guards);
            (!alternative_guards.is_empty()).then_some(SummaryCrashBucket {
                cause,
                alternative_guards,
            })
        })
        .collect()
}

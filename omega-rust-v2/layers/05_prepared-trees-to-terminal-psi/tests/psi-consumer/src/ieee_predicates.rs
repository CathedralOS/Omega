use omega_psi::{ContractClause, DeclarationId, IeeeComparisonKind, IeeeFloatBits, IeeeFloatFormat, PredicateOperator, PredicateStep};

/// A value this reader can substitute for a clause subject.
#[derive(Clone, Debug, PartialEq)]
pub enum PredicateValue {
    Bits(IeeeFloatBits),
    Bool(bool),
    /// A record subject: its declared fields and their values.
    Record(Vec<(DeclarationId, PredicateValue)>),
}

/// Decide one published clause using only Psi, substituting `formals`,
/// `result` and `self` for their terms. IEEE comparisons follow IEEE 754:
/// an ordered kind or `Equal` is false when either operand is NaN, `NotEqual`
/// is true then, and signed zeros compare equal. Each operand must have the
/// step's format. `None` means the clause uses something else or is ill-formed.
pub fn evaluate_ieee_clause(
    clause: &ContractClause,
    formals: &[PredicateValue],
    result: Option<&PredicateValue>,
    self_value: Option<&PredicateValue>,
) -> Option<bool> {
    let mut stack: Vec<PredicateValue> = Vec::new();
    for step in &clause.predicate {
        let value = match step {
            PredicateStep::Formal(index) => formals.get(*index)?.clone(),
            PredicateStep::Result => result?.clone(),
            PredicateStep::SelfValue => self_value?.clone(),
            PredicateStep::Field(field) => match stack.pop()? {
                PredicateValue::Record(fields) => fields.into_iter().find(|(declared, _)| declared == field)?.1,
                _ => return None,
            },
            PredicateStep::IeeeFloat(bits) => PredicateValue::Bits(*bits),
            PredicateStep::Boolean(value) => PredicateValue::Bool(*value),
            PredicateStep::IeeeComparison { kind, format } => {
                let right = stack.pop()?;
                let left = stack.pop()?;
                PredicateValue::Bool(compare(*kind, *format, &left, &right)?)
            }
            PredicateStep::Apply(PredicateOperator::Not) => match stack.pop()? {
                PredicateValue::Bool(value) => PredicateValue::Bool(!value),
                _ => return None,
            },
            PredicateStep::Apply(operator @ (PredicateOperator::And | PredicateOperator::Or)) => {
                let (PredicateValue::Bool(right), PredicateValue::Bool(left)) = (stack.pop()?, stack.pop()?) else {
                    return None;
                };
                PredicateValue::Bool(if *operator == PredicateOperator::And { left && right } else { left || right })
            }
            _ => return None,
        };
        stack.push(value);
    }
    match stack.as_slice() {
        [PredicateValue::Bool(value)] => Some(*value),
        _ => None,
    }
}

// Host `f32` and `f64` comparisons are IEEE 754 comparisons.
fn compare(
    kind: IeeeComparisonKind,
    format: IeeeFloatFormat,
    left: &PredicateValue,
    right: &PredicateValue,
) -> Option<bool> {
    use IeeeComparisonKind::*;
    let (left, right) = match (format, left, right) {
        (IeeeFloatFormat::Binary32, PredicateValue::Bits(IeeeFloatBits::Binary32(a)), PredicateValue::Bits(IeeeFloatBits::Binary32(b))) => {
            (f64::from(f32::from_bits(*a)), f64::from(f32::from_bits(*b)))
        }
        (IeeeFloatFormat::Binary64, PredicateValue::Bits(IeeeFloatBits::Binary64(a)), PredicateValue::Bits(IeeeFloatBits::Binary64(b))) => {
            (f64::from_bits(*a), f64::from_bits(*b))
        }
        _ => return None,
    };
    // Widening binary32 to binary64 is exact and keeps NaN and signed zero,
    // so the comparison is unchanged.
    Some(match kind {
        Equal => left == right,
        NotEqual => left != right,
        LessThan => left < right,
        LessOrEqual => left <= right,
    })
}

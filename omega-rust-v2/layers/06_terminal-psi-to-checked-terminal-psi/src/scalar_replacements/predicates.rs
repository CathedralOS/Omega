use super::Reason;
use omega_psi::*;

#[derive(Clone, Default)]
pub(super) struct Facts {
    pub literal: Option<ScalarLiteral>,
    pub origin: Option<ValueId>,
    pub guard: Option<super::guards::Guard>,
    predicates: Vec<Vec<PredicateStep>>,
}

impl Facts {
    pub fn at(id: ValueId) -> Self {
        Self {
            origin: Some(id),
            ..Default::default()
        }
    }

    pub fn intersect(
        &mut self,
        other: &Self,
    ) {
        if self.origin != other.origin {
            self.origin = None;
        }
        if self.guard != other.guard {
            self.guard = None;
        }
        if self.literal != other.literal {
            self.literal = None;
        }
        self.predicates.retain(|p| other.predicates.contains(p));
    }

    pub fn assume(
        &mut self,
        p: Vec<PredicateStep>,
    ) {
        if !self.predicates.contains(&p) {
            self.predicates.push(p);
        }
    }

    pub fn proves(
        &self,
        predicate: &[PredicateStep],
    ) -> bool {
        self.predicates.iter().any(|p| p == predicate) || evaluate(predicate, self.literal.as_ref()) == Some(ScalarLiteral::Boolean(true))
    }
}

pub(super) fn fits(
    literal: &ScalarLiteral,
    carrier: ScalarType,
) -> bool {
    match (literal, carrier) {
        (ScalarLiteral::Integer(n), ScalarType::Integer { signed, width }) if width > 0 => {
            if n < &Default::default() {
                signed && (!n).bits() < width
            } else if signed {
                n.bits() < width
            } else {
                n.bits() <= width
            }
        }
        (ScalarLiteral::Boolean(_), ScalarType::Boolean) => true,
        (ScalarLiteral::IeeeFloat(value), ScalarType::IeeeFloat(format)) => value.format() == format,
        _ => false,
    }
}

pub(super) fn formal_fact(
    predicate: &[PredicateStep],
    formal: usize,
) -> Option<Vec<PredicateStep>> {
    predicate
        .iter()
        .map(|s| match s {
            PredicateStep::Formal(i) if *i == formal => Some(PredicateStep::SelfValue),
            PredicateStep::Integer(_) | PredicateStep::Boolean(_) | PredicateStep::Apply(_) => Some(s.clone()),
            _ => None,
        })
        .collect()
}

pub(super) fn instantiate(
    predicate: &[PredicateStep],
    domain: &LoweredDomain,
    atom: &DomainAtom,
) -> Reason<Vec<PredicateStep>> {
    predicate
        .iter()
        .map(|step| match step {
            PredicateStep::ValueIndex(index) => {
                let ordinal = domain
                    .index_binders
                    .iter()
                    .position(|i| i == index)
                    .ok_or("unknown predicate index binder")?;
                match atom.arguments.get(ordinal) {
                    Some(DeclarationTerm::StaticInteger { value, .. }) => Ok(PredicateStep::Integer(value.clone())),
                    Some(DeclarationTerm::StaticBoolean(v)) => Ok(PredicateStep::Boolean(*v)),
                    _ => Err("unsupported predicate index argument"),
                }
            }
            PredicateStep::SelfValue | PredicateStep::Integer(_) | PredicateStep::Boolean(_) | PredicateStep::Apply(_) => Ok(step.clone()),
            _ => Err("domain predicate requires a proof outside the scalar replacement judgment"),
        })
        .collect()
}

fn evaluate(
    predicate: &[PredicateStep],
    value: Option<&ScalarLiteral>,
) -> Option<ScalarLiteral> {
    use PredicateOperator as Op;
    use ScalarLiteral::{Boolean, Integer};
    let mut stack = Vec::new();
    for step in predicate {
        let result = match step {
            PredicateStep::SelfValue => value?.clone(),
            PredicateStep::Integer(n) => Integer(n.clone()),
            PredicateStep::Boolean(v) => Boolean(*v),
            PredicateStep::Apply(Op::Not) => {
                let Boolean(v) = stack.pop()? else { return None };
                Boolean(!v)
            }
            PredicateStep::Apply(op) => {
                let right = stack.pop()?;
                let left = stack.pop()?;
                Boolean(match (op, left, right) {
                    (Op::And, Boolean(a), Boolean(b)) => a && b,
                    (Op::Or, Boolean(a), Boolean(b)) => a || b,
                    (Op::Equal, Boolean(a), Boolean(b)) => a == b,
                    (Op::NotEqual, Boolean(a), Boolean(b)) => a != b,
                    (Op::Equal, Integer(a), Integer(b)) => a == b,
                    (Op::NotEqual, Integer(a), Integer(b)) => a != b,
                    (Op::GreaterThan, Integer(a), Integer(b)) => a > b,
                    (Op::LessThan, Integer(a), Integer(b)) => a < b,
                    (Op::GreaterOrEqual, Integer(a), Integer(b)) => a >= b,
                    (Op::LessOrEqual, Integer(a), Integer(b)) => a <= b,
                    _ => return None,
                })
            }
            _ => return None,
        };
        stack.push(result);
    }
    if stack.len() == 1 { stack.pop() } else { None }
}

// This integer/Boolean predicate fragment has no partial operations. Evaluating
// with a representative of the carrier checks the stack's types independently
// of whether a real argument satisfies the predicate.
pub(super) fn well_typed(
    predicate: &[PredicateStep],
    carrier: ScalarType,
) -> bool {
    let representative = match carrier {
        ScalarType::Integer { width: 0, .. } => return false,
        ScalarType::Integer { .. } => Some(ScalarLiteral::Integer(Default::default())),
        ScalarType::Boolean => Some(ScalarLiteral::Boolean(false)),
        ScalarType::IeeeFloat(_) => None,
    };
    matches!(evaluate(predicate, representative.as_ref()), Some(ScalarLiteral::Boolean(_)))
}

//! Construct closed witnesses on arena handles, preserving search order.
//!
//! A closed-decidable goal is still the original kernel question: absence of
//! an inhabitant never licenses an axiom or a dependent elimination. Pending
//! product members, alternative branches and lambda bodies live here rather
//! than on the Rust stack. Leaf decisions and allocation order are unchanged.

use super::*;

enum Step {
    Ready(Option<TermHandle>),
    Product {
        domain: TermHandle,
        codomain: TermHandle,
    },
    Sum {
        zero: TermHandle,
        one: TermHandle,
    },
    Function {
        domain: TermHandle,
        codomain: TermHandle,
    },
}

enum Pending {
    First { second: TermHandle },
    Second { first: TermHandle },
    Zero { one: TermHandle },
    One,
    Body { domain: TermHandle },
}

pub(super) fn construct(
    denotation: &mut Denotation,
    goal: TermHandle,
) -> Result<Option<TermHandle>, BoundedDenotationError> {
    let mut pending = Vec::new();
    let mut current = goal;
    'descend: loop {
        let mut value = match step(denotation, current)? {
            Step::Ready(value) => value,
            Step::Product { domain, codomain } => {
                pending.push(Pending::First { second: codomain });
                current = domain;
                continue 'descend;
            }
            Step::Sum { zero, one } => {
                if !denotation.occurs_free(zero, 0) {
                    pending.push(Pending::Zero { one });
                    current = zero;
                    continue 'descend;
                }
                if !denotation.occurs_free(one, 0) {
                    pending.push(Pending::One);
                    current = one;
                    continue 'descend;
                }
                None
            }
            Step::Function { domain, codomain } => {
                pending.push(Pending::Body { domain });
                current = codomain;
                continue 'descend;
            }
        };
        loop {
            match pending.pop() {
                Some(Pending::First { second }) => {
                    if let Some(first) = value {
                        pending.push(Pending::Second { first });
                        current = second;
                        continue 'descend;
                    }
                }
                Some(Pending::Second { first }) => {
                    value =
                        value.map(|second| denotation.arena.insert(Term::Pair { first, second }));
                }
                Some(Pending::Zero { one }) => {
                    if let Some(payload) = value {
                        value = Some(denotation.arena.insert(Term::Pair {
                            first: denotation.two_zero,
                            second: payload,
                        }));
                    } else if !denotation.occurs_free(one, 0) {
                        pending.push(Pending::One);
                        current = one;
                        continue 'descend;
                    }
                }
                Some(Pending::One) => {
                    value = value.map(|payload| {
                        denotation.arena.insert(Term::Pair {
                            first: denotation.two_one,
                            second: payload,
                        })
                    });
                }
                Some(Pending::Body { domain }) => {
                    value =
                        value.map(|body| denotation.arena.insert(Term::Lambda { domain, body }));
                }
                None => return Ok(value),
            }
        }
    }
}

fn step(denotation: &mut Denotation, goal: TermHandle) -> Result<Step, BoundedDenotationError> {
    if let Some((ty, left, right)) = denotation.identity_parts(goal) {
        // Non-reflexive identity remains uninhabited here. Refutation belongs
        // to the premise side, not a new decision on this goal.
        return Ok(Step::Ready(
            if denotation.arena.structurally_equal(left, right) {
                Some(denotation.arena.insert(Term::Refl { ty, value: left }))
            } else {
                None
            },
        ));
    }
    match denotation.integer_relation(goal) {
        Some(IntegerRelation::LessThan { left, right }) => {
            let (Some(lower), Some(upper)) = (
                denotation.closed_math_value(left),
                denotation.closed_math_value(right),
            ) else {
                return Ok(Step::Ready(None));
            };
            if lower >= upper {
                return Ok(Step::Ready(None));
            }
            let (Some(lower), Some(upper)) = (integer_value_of(&lower), integer_value_of(&upper))
            else {
                return Ok(Step::Ready(None));
            };
            return denotation
                .literal_order(lower, upper)
                .map(|term| Step::Ready(Some(term)));
        }
        Some(IntegerRelation::LessOrEqual { left, right }) => {
            let (Some(lower), Some(upper)) = (
                denotation.closed_math_value(left),
                denotation.closed_math_value(right),
            ) else {
                return Ok(Step::Ready(None));
            };
            if lower > upper {
                return Ok(Step::Ready(None));
            }
            let evidence = if lower == upper {
                // Equal endpoints need no literal-range derivation, including
                // an opaque closed constant outside that range.
                let integer = denotation.integer_constant()?;
                let reflexive = denotation.arena.insert(Term::Refl {
                    ty: integer,
                    value: left,
                });
                denotation.integer_law_application(
                    IntegerLaw::EqualityToLessOrEqual,
                    &[left, right, reflexive],
                )?
            } else {
                let (Some(lower), Some(upper)) =
                    (integer_value_of(&lower), integer_value_of(&upper))
                else {
                    return Ok(Step::Ready(None));
                };
                let strict = denotation.literal_order(lower, upper)?;
                denotation.integer_law_application(
                    IntegerLaw::LessThanToLessOrEqual,
                    &[left, right, strict],
                )?
            };
            return Ok(Step::Ready(Some(evidence)));
        }
        _ => {}
    }
    Ok(match denotation.arena.get(goal) {
        Term::Sigma { domain, codomain } => {
            if !denotation.occurs_free(codomain, 0) {
                Step::Product { domain, codomain }
            } else if denotation.arena.structurally_equal(domain, denotation.two) {
                // Only the original tagged, nondependent branch family is
                // eligible. Branch independence is checked just before search.
                match denotation.arena.get(codomain) {
                    Term::CaseTwo {
                        zero_branch,
                        one_branch,
                        scrutinee,
                        ..
                    } if matches!(denotation.arena.get(scrutinee), Term::Variable(0)) => {
                        Step::Sum {
                            zero: zero_branch,
                            one: one_branch,
                        }
                    }
                    _ => Step::Ready(None),
                }
            } else {
                Step::Ready(None)
            }
        }
        Term::Pi { domain, codomain } if !denotation.occurs_free(codomain, 0) => {
            Step::Function { domain, codomain }
        }
        _ => Step::Ready(None),
    })
}

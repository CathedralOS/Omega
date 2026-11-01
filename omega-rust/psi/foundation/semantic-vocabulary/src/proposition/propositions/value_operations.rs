//! Exact owned-value operations for the logical proposition tree.
//!
//! Premise rosters and material receipts copy these values before checking.
//! Their depth must not require a wider thread stack. These worklists preserve
//! the derived operations' constructor, field and member identities; neither
//! operation validates, normalizes, reorders or decides a proposition. Atomic
//! payload operations remain with their existing types. Wire representation,
//! ordering, hashing and logical/IEEE comparison rules are unchanged.

use super::Proposition;

enum CloneWork<'source> {
    Enter(&'source Proposition),
    Conjunction(usize),
    Disjunction(usize),
    Implication,
}

fn clone_atomic(proposition: &Proposition) -> Proposition {
    use Proposition::*;
    match proposition {
        Truth => Truth,
        Falsehood => Falsehood,
        Atom(identity) => Atom(*identity),
        Equal(left, right) => Equal(left.clone(), right.clone()),
        LessThan(left, right) => LessThan(left.clone(), right.clone()),
        LessOrEqual(left, right) => LessOrEqual(left.clone(), right.clone()),
        IntegerMathEqual(left, right) => IntegerMathEqual(left.clone(), right.clone()),
        IntegerMathLessThan(left, right) => IntegerMathLessThan(left.clone(), right.clone()),
        IntegerMathLessOrEqual(left, right) => IntegerMathLessOrEqual(left.clone(), right.clone()),
        IeeeFloatComparison {
            kind,
            format,
            left,
            right,
        } => IeeeFloatComparison {
            kind: *kind,
            format: *format,
            left: left.clone(),
            right: right.clone(),
        },
        ScalarIeeeFloatComparison {
            kind,
            format,
            left,
            right,
        } => ScalarIeeeFloatComparison {
            kind: *kind,
            format: *format,
            left: left.clone(),
            right: right.clone(),
        },
        ByteSequenceEqual { left, right } => ByteSequenceEqual {
            left: left.clone(),
            right: right.clone(),
        },
        StructuralIdentity { left, right } => StructuralIdentity {
            left: left.clone(),
            right: right.clone(),
        },
        StructuralCaseMembership { subject, case } => StructuralCaseMembership {
            subject: subject.clone(),
            case: *case,
        },
        ContentConservation(conservation) => ContentConservation(conservation.clone()),
        Conjunction(_) | Disjunction(_) | Implication { .. } => {
            unreachable!("the logical clone continuation owns these children")
        }
    }
}

impl Clone for Proposition {
    fn clone(&self) -> Self {
        use Proposition::*;
        if !matches!(self, Conjunction(_) | Disjunction(_) | Implication { .. }) {
            return clone_atomic(self);
        }
        let mut pending = vec![CloneWork::Enter(self)];
        let mut values = Vec::new();
        while let Some(work) = pending.pop() {
            let value = match work {
                CloneWork::Enter(proposition) => match proposition {
                    Conjunction(children) => {
                        pending.push(CloneWork::Conjunction(children.len()));
                        pending.extend(children.iter().rev().map(CloneWork::Enter));
                        continue;
                    }
                    Disjunction(children) => {
                        pending.push(CloneWork::Disjunction(children.len()));
                        pending.extend(children.iter().rev().map(CloneWork::Enter));
                        continue;
                    }
                    Implication {
                        premise,
                        conclusion,
                    } => {
                        pending.push(CloneWork::Implication);
                        pending.push(CloneWork::Enter(conclusion));
                        pending.push(CloneWork::Enter(premise));
                        continue;
                    }
                    _ => clone_atomic(proposition),
                },
                CloneWork::Conjunction(count) => {
                    let children = values.split_off(values.len() - count);
                    Conjunction(children)
                }
                CloneWork::Disjunction(count) => {
                    let children = values.split_off(values.len() - count);
                    Disjunction(children)
                }
                CloneWork::Implication => {
                    let conclusion = values.pop().expect("one cloned conclusion");
                    let premise = values.pop().expect("one cloned premise");
                    Implication {
                        premise: Box::new(premise),
                        conclusion: Box::new(conclusion),
                    }
                }
            };
            values.push(value);
        }
        assert_eq!(values.len(), 1, "one complete owned proposition");
        values.pop().expect("one complete owned proposition")
    }
}

impl PartialEq for Proposition {
    fn eq(&self, other: &Self) -> bool {
        use Proposition::*;
        // Atomic comparisons keep their direct, allocation-free route.
        let mut pending = Vec::new();
        let mut current = (self, other);
        loop {
            let (left, right) = current;
            // Exhaustive on the left/source variant: adding a constructor must
            // decide its complete exact-value comparison, not inherit false.
            let equal = match left {
                Truth => matches!(right, Truth),
                Falsehood => matches!(right, Falsehood),
                Atom(a) => matches!(right, Atom(b) if a == b),
                Equal(a, b) => matches!(right, Equal(c, d) if a == c && b == d),
                LessThan(a, b) => matches!(right, LessThan(c, d) if a == c && b == d),
                LessOrEqual(a, b) => matches!(right, LessOrEqual(c, d) if a == c && b == d),
                IntegerMathEqual(a, b) => {
                    matches!(right, IntegerMathEqual(c, d) if a == c && b == d)
                }
                IntegerMathLessThan(a, b) => {
                    matches!(right, IntegerMathLessThan(c, d) if a == c && b == d)
                }
                IntegerMathLessOrEqual(a, b) => {
                    matches!(right, IntegerMathLessOrEqual(c, d) if a == c && b == d)
                }
                IeeeFloatComparison {
                    kind,
                    format,
                    left: a,
                    right: b,
                } => matches!(right,
                    IeeeFloatComparison { kind: other_kind, format: other_format, left: c, right: d }
                        if kind == other_kind && format == other_format && a == c && b == d),
                ScalarIeeeFloatComparison {
                    kind,
                    format,
                    left: a,
                    right: b,
                } => matches!(right,
                    ScalarIeeeFloatComparison { kind: other_kind, format: other_format, left: c, right: d }
                        if kind == other_kind && format == other_format && a == c && b == d),
                ByteSequenceEqual { left: a, right: b } => matches!(right,
                    ByteSequenceEqual { left: c, right: d } if a == c && b == d),
                StructuralIdentity { left: a, right: b } => matches!(right,
                    StructuralIdentity { left: c, right: d } if a == c && b == d),
                StructuralCaseMembership { subject, case } => matches!(right,
                    StructuralCaseMembership { subject: other_subject, case: other_case }
                        if subject == other_subject && case == other_case),
                Conjunction(a) => {
                    let Conjunction(b) = right else {
                        return false;
                    };
                    if a.len() != b.len() {
                        return false;
                    }
                    pending.extend(a.iter().zip(b).rev());
                    true
                }
                Disjunction(a) => {
                    let Disjunction(b) = right else {
                        return false;
                    };
                    if a.len() != b.len() {
                        return false;
                    }
                    pending.extend(a.iter().zip(b).rev());
                    true
                }
                Implication {
                    premise: a,
                    conclusion: b,
                } => {
                    let Implication {
                        premise: c,
                        conclusion: d,
                    } = right
                    else {
                        return false;
                    };
                    pending.push((b, d));
                    pending.push((a, c));
                    true
                }
                ContentConservation(a) => matches!(right, ContentConservation(b) if a == b),
            };
            if !equal {
                return false;
            }
            match pending.pop() {
                Some(pair) => current = pair,
                None => return true,
            }
        }
    }
}

impl Eq for Proposition {}

impl std::hash::Hash for Proposition {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        use Proposition::*;
        use std::hash::Hash;
        let mut pending = Vec::new();
        let mut current = self;
        loop {
            // This is the same enum tag the derived Hash emitted, followed by
            // fields in declaration order. Hash and exact equality therefore
            // retain the same complete value, including invalid logical arity.
            std::mem::discriminant(current).hash(state);
            match current {
                Truth | Falsehood => {}
                Atom(identity) => identity.hash(state),
                Equal(left, right) | LessThan(left, right) | LessOrEqual(left, right) => {
                    left.hash(state);
                    right.hash(state);
                }
                IntegerMathEqual(left, right)
                | IntegerMathLessThan(left, right)
                | IntegerMathLessOrEqual(left, right) => {
                    left.hash(state);
                    right.hash(state);
                }
                IeeeFloatComparison {
                    kind,
                    format,
                    left,
                    right,
                } => {
                    kind.hash(state);
                    format.hash(state);
                    left.hash(state);
                    right.hash(state);
                }
                ScalarIeeeFloatComparison {
                    kind,
                    format,
                    left,
                    right,
                } => {
                    kind.hash(state);
                    format.hash(state);
                    left.hash(state);
                    right.hash(state);
                }
                ByteSequenceEqual { left, right } => {
                    left.hash(state);
                    right.hash(state);
                }
                StructuralIdentity { left, right } => {
                    left.hash(state);
                    right.hash(state);
                }
                StructuralCaseMembership { subject, case } => {
                    subject.hash(state);
                    case.hash(state);
                }
                Conjunction(children) | Disjunction(children) => {
                    hash_member_count(children.len(), state);
                    pending.extend(children.iter().rev());
                }
                Implication {
                    premise,
                    conclusion,
                } => {
                    pending.push(conclusion.as_ref());
                    pending.push(premise.as_ref());
                }
                ContentConservation(conservation) => conservation.hash(state),
            }
            match pending.pop() {
                Some(next) => current = next,
                None => return,
            }
        }
    }
}

fn hash_member_count<H: std::hash::Hasher>(count: usize, state: &mut H) {
    use std::hash::Hash;
    // Collection Hash uses the Hasher's length-prefix hook, which a custom
    // hasher may override. Its direct method is still unstable. A zero-sized
    // unit vector invokes that exact stable collection protocol: no payload
    // storage is allocated, and unit elements emit no hash operations. Replace
    // this adapter with write_length_prefix when that API becomes stable.
    vec![(); count].hash(state);
}

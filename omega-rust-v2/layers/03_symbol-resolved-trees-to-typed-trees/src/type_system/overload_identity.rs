use crate::{DomainApplication, IntegerIndex, Site, TypedSignature, ValueType};
use omega_resolved::DeclarationId;
use omega_tokens::SourceSpan;

type Operands<'a> = &'a [(SourceSpan, ValueType)];

pub(crate) fn same_operands(
    left: &TypedSignature,
    right: &TypedSignature,
) -> bool {
    compare(
        SignatureSide {
            operands: &left.parameters,
            owner: &left.machine,
            site: &left.site,
        },
        SignatureSide {
            operands: &right.parameters,
            owner: &right.machine,
            site: &right.site,
        },
        &[],
        &[],
    )
}

pub(crate) fn same_signature(
    parameters: Operands<'_>,
    owner: &DeclarationId,
    site: &Site,
    result: &ValueType,
    previous: &TypedSignature,
) -> bool {
    compare(
        SignatureSide {
            operands: parameters,
            owner,
            site,
        },
        SignatureSide {
            operands: &previous.parameters,
            owner: &previous.machine,
            site: &previous.site,
        },
        result.domains(),
        previous.result.domains(),
    )
}

struct SignatureSide<'a> {
    operands: Operands<'a>,
    owner: &'a DeclarationId,
    site: &'a Site,
}

impl SignatureSide<'_> {
    fn operand_subject(
        &self,
        subject: &Site,
    ) -> Option<usize> {
        if (&self.site.package, &self.site.file) != (&subject.package, &subject.file) {
            return None;
        }
        self.operands.iter().position(|(span, _)| *span == subject.span)
    }
}

/// One correspondence covers operands and result domains. A domain conjunction
/// can admit more than one pairing, so keep its alternatives until the rest of
/// the signature confirms which pairings preserve the binder relationships.
#[derive(Clone, Default)]
struct Match<'a> {
    binders: Vec<(usize, usize)>,
    remaining: Vec<Compare<'a>>,
}

#[derive(Clone)]
enum Compare<'a> {
    Type(&'a ValueType, &'a ValueType),
    Domains(&'a [DomainApplication], Vec<&'a DomainApplication>),
    Index(&'a IntegerIndex, &'a IntegerIndex),
    Commutative(&'a [IntegerIndex], Vec<&'a IntegerIndex>),
}

impl<'a> Match<'a> {
    fn binder(
        &mut self,
        left: usize,
        right: usize,
    ) -> bool {
        if let Some(&(previous_left, previous_right)) = self.binders.iter().find(|&&(a, b)| a == left || b == right) {
            return (left, right) == (previous_left, previous_right);
        }
        self.binders.push((left, right));
        true
    }

    fn types(
        &mut self,
        left: &'a [ValueType],
        right: &'a [ValueType],
    ) {
        self.remaining
            .extend(left.iter().zip(right).rev().map(|(a, b)| Compare::Type(a, b)));
    }

    fn domains(
        &mut self,
        left: &'a [DomainApplication],
        right: &'a [DomainApplication],
    ) {
        self.remaining
            .push(Compare::Domains(left, right.iter().collect()));
    }
}

fn compare<'a>(
    left: SignatureSide<'a>,
    right: SignatureSide<'a>,
    left_domains: &'a [DomainApplication],
    right_domains: &'a [DomainApplication],
) -> bool {
    if left.operands.len() != right.operands.len() {
        return false;
    }
    let mut initial = Match::default();
    initial.domains(left_domains, right_domains);
    initial.remaining.extend(
        left.operands
            .iter()
            .zip(right.operands)
            .rev()
            .map(|((_, a), (_, b))| Compare::Type(a, b)),
    );
    let mut choices = vec![initial];
    'choice: while let Some(mut candidate) = choices.pop() {
        while let Some(work) = candidate.remaining.pop() {
            let compatible = match work {
                Compare::Type(a, b) => compare_type(a, b, &left, &right, &mut candidate),
                Compare::Index(a, b) => compare_index(a, b, &left, &right, &mut candidate),
                Compare::Domains(a, b) => choose_pairing(
                    a,
                    b,
                    &mut candidate,
                    &mut choices,
                    |a, b| a.declaration == b.declaration && a.arguments.len() == b.arguments.len(),
                    |a, b, tail, rest, candidate| {
                        candidate.remaining.push(Compare::Domains(tail, rest));
                        candidate.types(&a.arguments, &b.arguments);
                    },
                ),
                Compare::Commutative(a, b) => choose_pairing(
                    a,
                    b,
                    &mut candidate,
                    &mut choices,
                    |a, b| match (a, b) {
                        (IntegerIndex::Value(_), IntegerIndex::Value(_)) => a == b,
                        _ => std::mem::discriminant(a) == std::mem::discriminant(b),
                    },
                    |a, b, tail, rest, candidate| {
                        candidate
                            .remaining
                            .extend([Compare::Commutative(tail, rest), Compare::Index(a, b)]);
                    },
                ),
            };
            if !compatible {
                continue 'choice;
            }
        }
        return true;
    }
    false
}

fn choose_pairing<'a, T>(
    left: &'a [T],
    right: Vec<&'a T>,
    candidate: &mut Match<'a>,
    choices: &mut Vec<Match<'a>>,
    compatible: impl Fn(&T, &T) -> bool,
    mut pair: impl FnMut(&'a T, &'a T, &'a [T], Vec<&'a T>, &mut Match<'a>),
) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let Some((first, tail)) = left.split_first() else { return true };
    let mut possible = right
        .iter()
        .enumerate()
        .filter(|(_, other)| compatible(first, other));
    let Some((position, other)) = possible.next() else { return false };
    let other = *other;
    for (position, &other) in possible {
        let mut alternative = candidate.clone();
        let mut rest = right.clone();
        rest.remove(position);
        pair(first, other, tail, rest, &mut alternative);
        choices.push(alternative);
    }
    let mut rest = right;
    rest.remove(position);
    pair(first, other, tail, rest, candidate);
    true
}

fn compare_type<'a>(
    left: &'a ValueType,
    right: &'a ValueType,
    left_signature: &SignatureSide<'a>,
    right_signature: &SignatureSide<'a>,
    candidate: &mut Match<'a>,
) -> bool {
    use ValueType::*;
    let work = &mut candidate.remaining;
    match (left, right) {
        (Parameter { owner: a, index: i }, Parameter { owner: b, index: j }) if a.as_ref() == left_signature.owner && b.as_ref() == right_signature.owner => {
            return candidate.binder(*i, *j);
        }
        (Subject { declaration: a, version: None }, Subject { declaration: b, version: None }) => {
            return match (left_signature.operand_subject(a), right_signature.operand_subject(b)) {
                (Some(i), Some(j)) => i == j,
                (None, None) => a == b,
                _ => false,
            };
        }
        (AtLifetime(a, l), AtLifetime(b, r)) | (IndexedArray(a, l), IndexedArray(b, r)) | (Function(a, l), Function(b, r)) => {
            work.extend([Compare::Type(l, r), Compare::Type(a, b)]);
        }
        (Qualified(a, d), Qualified(b, e)) => {
            candidate.domains(d, e);
            candidate.remaining.push(Compare::Type(a, b));
        }
        (StaticFormula { term: a, carrier: x }, StaticFormula { term: b, carrier: y }) => {
            work.extend([Compare::Index(a, b), Compare::Type(x, y)]);
        }
        (StaticInteger { carrier: a, value: x }, StaticInteger { carrier: b, value: y }) if x == y => work.push(Compare::Type(a, b)),
        (Tuple(a), Tuple(b)) if a.len() == b.len() => candidate.types(a, b),
        (IndexedInteger { signed: a, width: x }, IndexedInteger { signed: b, width: y }) if a == b => work.push(Compare::Type(x, y)),
        (Universe { strict: a, level: x }, Universe { strict: b, level: y }) if a == b => work.push(Compare::Type(x, y)),
        (Applied(a, x), Applied(b, y)) if a == b && x.len() == y.len() => candidate.types(x, y),
        (MachineSelection(a), MachineSelection(b))
            if a.machine == b.machine && a.arguments.len() == b.arguments.len() && a.enclosing_arguments.len() == b.enclosing_arguments.len() =>
        {
            candidate.types(&a.arguments, &b.arguments);
            for ((left_owner, left_args), (right_owner, right_args)) in a.enclosing_arguments.iter().zip(&b.enclosing_arguments) {
                if left_owner != right_owner || left_args.len() != right_args.len() {
                    return false;
                }
                candidate.types(left_args, right_args);
            }
        }
        (MathematicalValue { definition: a, subjects: x }, MathematicalValue { definition: b, subjects: y }) if a == b && x.len() == y.len() => {
            candidate.types(x, y)
        }
        (FamilyApplication { family: a, subjects: x }, FamilyApplication { family: b, subjects: y }) if x.len() == y.len() => {
            candidate.types(x, y);
            candidate.remaining.push(Compare::Type(a, b));
        }
        (Reference(a, x), Reference(b, y)) if a == b => work.push(Compare::Type(x, y)),
        (Array(a, x), Array(b, y)) if x == y => work.push(Compare::Type(a, b)),
        (Slice(a), Slice(b)) | (Squash(a), Squash(b)) | (LevelOf(a), LevelOf(b)) => work.push(Compare::Type(a, b)),
        _ => return left == right,
    }
    true
}

fn compare_index<'a>(
    left: &'a IntegerIndex,
    right: &'a IntegerIndex,
    left_signature: &SignatureSide<'a>,
    right_signature: &SignatureSide<'a>,
    candidate: &mut Match<'a>,
) -> bool {
    use IntegerIndex::*;
    match (left, right) {
        (Binder { owner: a, index: i }, Binder { owner: b, index: j }) if a.as_ref() == left_signature.owner && b.as_ref() == right_signature.owner => {
            return candidate.binder(*i, *j);
        }
        (Add(a), Add(b)) | (Multiply(a), Multiply(b)) => candidate
            .remaining
            .push(Compare::Commutative(a, b.iter().collect())),
        (
            Primitive {
                token: a,
                carrier: x,
                operands: left,
            },
            Primitive {
                token: b,
                carrier: y,
                operands: right,
            },
        ) if a == b && left.len() == right.len() => {
            if matches!(a.as_str(), "+%" | "*%" | "+|") {
                candidate
                    .remaining
                    .push(Compare::Commutative(left, right.iter().collect()));
            } else {
                candidate
                    .remaining
                    .extend(left.iter().zip(right).rev().map(|(a, b)| Compare::Index(a, b)));
            }
            candidate.remaining.push(Compare::Type(x, y));
        }
        _ => return left == right,
    }
    true
}

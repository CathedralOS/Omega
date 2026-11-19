use crate::ValueType;
use omega_lexer::SourceSpan;
use omega_resolver::DeclarationId;
use std::collections::{HashMap, HashSet};

pub(crate) fn same_operands(
    left: &[(SourceSpan, ValueType)],
    left_owner: &DeclarationId,
    right: &[(SourceSpan, ValueType)],
    right_owner: &DeclarationId,
) -> bool {
    let mut mapping = HashMap::new();
    let mut used = HashSet::new();
    if left.len() != right.len() {
        return false;
    }
    let mut work = left
        .iter()
        .zip(right)
        .map(|((_, l), (_, r))| (l, r))
        .collect::<Vec<_>>();
    while let Some((l, r)) = work.pop() {
        use ValueType::*;
        match (l, r) {
            (Parameter { owner: a, index: i }, Parameter { owner: b, index: j }) if a.as_ref() == left_owner && b.as_ref() == right_owner => {
                if let Some(previous) = mapping.get(i) {
                    if previous != j {
                        return false;
                    }
                } else {
                    if !used.insert(j) {
                        return false;
                    }
                    mapping.insert(*i, *j);
                }
            }
            (AtLifetime(a, l), AtLifetime(b, r)) => {
                work.push((a, b));
                work.push((l, r));
            }
            (Qualified(a, d), Qualified(b, e)) if d.len() == e.len() => {
                work.push((a, b));
                for (l, r) in d.iter().zip(e) {
                    if l.declaration != r.declaration || l.arguments.len() != r.arguments.len() {
                        return false;
                    }
                    work.extend(l.arguments.iter().zip(&r.arguments));
                }
            }
            (StaticFormula { term: a, carrier: x }, StaticFormula { term: b, carrier: y }) if x == y && a.renamed(left_owner, right_owner) == *b => {}
            (Tuple(a), Tuple(b)) if a.len() == b.len() => work.extend(a.iter().zip(b)),
            (IndexedInteger { signed: a, width: x }, IndexedInteger { signed: b, width: y }) if a == b => work.push((x, y)),
            (IndexedArray(a, x), IndexedArray(b, y)) | (Function(a, x), Function(b, y)) => {
                work.push((a, b));
                work.push((x, y));
            }
            (Applied(a, x), Applied(b, y)) if a == b && x.len() == y.len() => work.extend(x.iter().zip(y)),
            (Reference(a, x), Reference(b, y)) if a == b => work.push((x, y)),
            (Array(a, x), Array(b, y)) if x == y => work.push((a, b)),
            (Slice(a), Slice(b)) => work.push((a, b)),
            _ if l == r => {}
            _ => return false,
        }
    }
    true
}

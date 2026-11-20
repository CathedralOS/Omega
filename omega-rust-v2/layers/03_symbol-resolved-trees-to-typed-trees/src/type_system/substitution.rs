use crate::{DomainApplication, ValueType};
use crate::type_relations::TypeRelations;
use omega_resolved::DeclarationId;

pub(crate) trait TypeInference: Sized {
    fn infer(
        &self,
        actual: &ValueType,
        owner: &DeclarationId,
        args: &mut [Option<ValueType>],
    ) -> bool;
    fn infer_with(
        &self,
        actual: &ValueType,
        owner: &DeclarationId,
        args: &mut [Option<ValueType>],
        compare_pending: &mut impl FnMut(&ValueType, &ValueType) -> bool,
    ) -> bool;
    fn validate_closed_indices(&self) -> Result<(), String>;
    fn validate_index(&self) -> Result<(), String>;
}

impl TypeInference for ValueType {
    fn infer(
        &self,
        actual: &ValueType,
        owner: &DeclarationId,
        args: &mut [Option<ValueType>],
    ) -> bool {
        self.infer_with(actual, owner, args, &mut |_, _| false)
    }
    fn infer_with(
        &self,
        actual: &ValueType,
        owner: &DeclarationId,
        args: &mut [Option<ValueType>],
        compare_pending: &mut impl FnMut(&ValueType, &ValueType) -> bool,
    ) -> bool {
        use ValueType::*;
        match (self, actual) {
            (Parameter { owner: b, index }, actual) if b.as_ref() == owner => match &args[*index] {
                Some(bound) => bound.admits(actual) || compare_pending(bound, actual),
                None if matches!(actual, Exact { .. }) => true,
                None => {
                    args[*index] = Some(actual.clone());
                    true
                }
            },
            (Universe { strict: a, level: x }, Universe { strict: b, level: y }) => a == b && x.infer_with(y, owner, args, compare_pending),
            (Tuple(left), Tuple(right)) => {
                left.len() == right.len()
                    && left
                        .iter()
                        .zip(right)
                        .all(|(a, b)| a.infer_with(b, owner, args, compare_pending))
            }
            (IndexedInteger { signed: a, width }, Integer { signed: b, width: Some(n) }) if a == b => {
                width.infer_with(&Self::static_size(*n), owner, args, compare_pending)
            }
            (IndexedInteger { signed: a, width: x }, IndexedInteger { signed: b, width: y }) if a == b => x.infer_with(y, owner, args, compare_pending),
            (IndexedArray(t, index), Array(v, n)) => {
                t.infer_with(v, owner, args, compare_pending) && index.infer_with(&Self::static_size(*n), owner, args, compare_pending)
            }
            (IndexedArray(t, index), IndexedArray(v, n)) => t.infer_with(v, owner, args, compare_pending) && index.infer_with(n, owner, args, compare_pending),
            (Function(a, b), Function(c, d)) => a.infer_with(c, owner, args, compare_pending) && b.infer_with(d, owner, args, compare_pending),
            (AtLifetime(t, l), AtLifetime(v, r)) => t.infer_with(v, owner, args, compare_pending) && l.infer_with(r, owner, args, compare_pending),
            (Qualified(t, a), Qualified(v, b)) => {
                a.len() == b.len()
                    && t.infer_with(v, owner, args, compare_pending)
                    && a.chunk_by(|x, y| x.declaration == y.declaration)
                        .zip(b.chunk_by(|x, y| x.declaration == y.declaration))
                        .all(|(left, right)| {
                            if left.len() != right.len() || left[0].declaration != right[0].declaration {
                                return false;
                            }
                            if left.len() > 1 {
                                return infer_repeated_domains(left, right, owner, args) || compare_pending(self, actual);
                            }
                            let (left, right) = (&left[0].arguments, &right[0].arguments);
                            left.len() == right.len()
                                && left
                                    .iter()
                                    .zip(right)
                                    .all(|(i, j)| i.infer_with(j, owner, args, compare_pending))
                        })
            }
            (Applied(id, a), Applied(id2, b)) => {
                id == id2
                    && a.len() == b.len()
                    && a.iter()
                        .zip(b)
                        .all(|(t, v)| t.infer_with(v, owner, args, compare_pending))
            }
            (Reference(a, t), Reference(b, v)) => a == b && t.infer_with(v, owner, args, compare_pending),
            (Array(t, n), Array(v, m)) => n == m && t.infer_with(v, owner, args, compare_pending),
            (Slice(t), Slice(v)) => t.infer_with(v, owner, args, compare_pending),
            _ => self.admits(actual) || compare_pending(self, actual),
        }
    }
    fn validate_closed_indices(&self) -> Result<(), String> {
        self.components().try_for_each(Self::validate_index)
    }
    fn validate_index(&self) -> Result<(), String> {
        use ValueType::*;
        match self {
            StaticFormula {
                term: crate::IntegerIndex::Invalid(message),
                ..
            }
            | InvalidStatic(message) => return Err(message.clone()),
            StaticInteger { carrier, value } if !carrier.contains_integer(value) => {
                return Err(format!("static index {value} exceeds {carrier}"));
            }
            IndexedInteger { width, .. } => {
                if let StaticInteger { value, .. } = width.as_ref()
                    && value.sign() != num_bigint::Sign::Plus
                {
                    return Err("integer width must be positive".into());
                }
            }
            _ => {}
        }
        Ok(())
    }
}

/// Multiple applications of one domain form an unordered set. Only a unique
/// partner can supply bindings; canonical display order cannot pair indices.
fn infer_repeated_domains(
    formals: &[DomainApplication],
    actuals: &[DomainApplication],
    owner: &DeclarationId,
    bindings: &mut [Option<ValueType>],
) -> bool {
    let mut remaining: Vec<_> = formals.iter().collect();
    let mut available: Vec<_> = actuals.iter().collect();
    while !remaining.is_empty() {
        let mut selected = None;
        for (position, formal) in remaining.iter().enumerate() {
            let mut matches = available.iter().enumerate().filter_map(|(index, actual)| {
                let mut inferred = bindings.to_vec();
                let compatible = formal.arguments.len() == actual.arguments.len()
                    && formal
                        .arguments
                        .iter()
                        .zip(&actual.arguments)
                        .all(|(pattern, ty)| pattern.infer(ty, owner, &mut inferred));
                compatible.then_some((index, inferred))
            });
            let Some((index, inferred)) = matches.next() else { return false };
            if matches.next().is_none() {
                selected = Some((position, index, inferred));
                break;
            }
        }
        let Some((position, index, inferred)) = selected else { return false };
        bindings.clone_from_slice(&inferred);
        remaining.swap_remove(position);
        available.swap_remove(index);
    }
    true
}

/// Pair each field component with the value that supplies its type.
pub(crate) fn field_components<'a>(
    ty: &'a ValueType,
    actual: &'a ValueType,
) -> impl Iterator<Item = (&'a ValueType, &'a ValueType)> {
    use ValueType::*;
    let mut pending = vec![(ty, actual)];
    std::iter::from_fn(move || {
        let (template, value) = pending.pop()?;
        match (template, value) {
            (Applied(a, x), Applied(b, y)) if a == b && x.len() == y.len() => pending.extend(x.iter().zip(y)),
            (Qualified(x, left), Qualified(y, right)) => {
                pending.push((x, y));
                for domain in left {
                    let mut matches = right
                        .iter()
                        .filter(|candidate| candidate.declaration == domain.declaration);
                    if let (Some(actual), None) = (matches.next(), matches.next()) {
                        pending.extend(domain.arguments.iter().zip(&actual.arguments));
                    }
                }
            }
            (IndexedArray(x, n), IndexedArray(y, m)) if n == m => pending.push((x, y)),
            (Function(x, a), Function(y, b)) => {
                pending.push((x, y));
                pending.push((a, b));
            }
            (Reference(a, x), Reference(b, y)) if a == b => pending.push((x, y)),
            (AtLifetime(x, _), AtLifetime(y, _)) => pending.push((x, y)),
            (AtLifetime(x, _), Reference(..)) => pending.push((x, value)),
            (Tuple(xs), Tuple(ys)) if xs.len() == ys.len() => pending.extend(xs.iter().zip(ys)),
            (Array(x, a), Array(y, b)) if a == b => pending.push((x, y)),
            (Slice(x), Slice(y)) => pending.push((x, y)),
            (Callable(x), Callable(y)) => {
                pending.extend(x.parameters.iter().zip(&y.parameters));
                pending.push((&x.result, &y.result));
            }
            _ => {}
        }
        Some((template, value))
    })
}

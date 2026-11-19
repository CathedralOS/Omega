use crate::expressions::body_typer::BodyTyper;
use crate::{Opening, Site, ValueType};
use omega_resolver::DeclarationId;
use omega_resolver::Symbol;

impl BodyTyper<'_, '_> {
    pub(super) fn opening(
        &self,
        node_index: usize,
    ) -> Opening {
        let mut subject = Site::at(self.unit.file, node_index);
        if let Some(Symbol::Binding { span, .. }) = self.unit.symbol(node_index) {
            subject.span = *span;
        }
        Opening {
            subject,
            fields: Vec::new(),
            version: self.version.clone(),
        }
    }
}
fn transform(
    ty: &ValueType,
    owner: &DeclarationId,
    resolve: &mut impl FnMut(usize) -> ValueType,
) -> ValueType {
    use ValueType::*;
    match ty {
        Witness {
            owner: o,
            index,
            opening: None,
        } if o.as_ref() == owner => resolve(*index),
        Applied(id, args) => Applied(id.clone(), args.iter().map(|t| transform(t, owner, resolve)).collect()),
        Reference(a, t) => Reference(a.clone(), Box::new(transform(t, owner, resolve))),
        AtLifetime(t, r) => AtLifetime(Box::new(transform(t, owner, resolve)), r.clone()),
        Slice(t) => Slice(Box::new(transform(t, owner, resolve))),
        Array(t, node_index) => Array(Box::new(transform(t, owner, resolve)), *node_index),
        Qualified(t, d) => Qualified(Box::new(transform(t, owner, resolve)), d.clone()),
        Callable(c) => {
            let mut c = c.clone();
            c.parameters = c
                .parameters
                .iter()
                .map(|p| transform(p, owner, resolve))
                .collect();
            c.result = transform(&c.result, owner, resolve);
            Callable(c)
        }
        _ => ty.clone(),
    }
}
pub(super) fn open(
    ty: &ValueType,
    owner: &DeclarationId,
    opening: &Opening,
) -> ValueType {
    transform(ty, owner, &mut |index| ValueType::Witness {
        owner: Box::new(owner.clone()),
        index,
        opening: Some(Box::new(opening.clone())),
    })
}
pub(super) fn select(
    ty: &ValueType,
    actual: &ValueType,
    owner: &DeclarationId,
    bindings: &mut [Option<ValueType>],
) -> Option<ValueType> {
    use ValueType::*;
    if bindings.is_empty() {
        return Some(ty.clone());
    }
    let mut pending = vec![(ty, actual)];
    while let Some((template, value)) = pending.pop() {
        match (template, value) {
            (
                Witness {
                    owner: o,
                    index,
                    opening: None,
                },
                v,
            ) if o.as_ref() == owner => {
                if matches!(v, Exact { .. } | NoResult) {
                    return None;
                }
                if bindings[*index].as_ref().is_some_and(|bound| bound != v) {
                    return None;
                }
                bindings[*index] = Some(v.clone());
            }
            (Applied(a, x), Applied(b, y)) if a == b => pending.extend(x.iter().zip(y)),
            (Reference(a, x), Reference(b, y)) if a == b => pending.push((x, y)),
            (AtLifetime(x, _), AtLifetime(y, _)) => pending.push((x, y)),
            (Array(x, a), Array(y, b)) if a == b => pending.push((x, y)),
            (Slice(x), Slice(y)) => pending.push((x, y)),
            (Callable(x), Callable(y)) => {
                pending.extend(x.parameters.iter().zip(&y.parameters));
                pending.push((&x.result, &y.result));
            }
            _ => {}
        }
    }
    Some(transform(ty, owner, &mut |index| {
        bindings[index].clone().unwrap_or_else(|| Witness {
            owner: Box::new(owner.clone()),
            index,
            opening: None,
        })
    }))
}

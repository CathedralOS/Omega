use super::{Site, TypeError, Unit, ValueType, types::Types};
use num_bigint::BigInt;
use omega_parser::NodeKind;
use omega_resolver::{DeclarationId, Symbol};

/// The primitive exact integer operations whose fixed profile licenses
/// associative, commutative and neutral-element normalization. Source
/// operations and their independent formation obligations are kept in L3.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum IntegerIndex {
    Value(BigInt),
    Binder { owner: Box<DeclarationId>, index: usize },
    Add(Vec<IntegerIndex>),
    Multiply(Vec<IntegerIndex>),
}

impl IntegerIndex {
    fn operation(
        add: bool,
        inputs: Vec<Self>,
    ) -> Self {
        let neutral = BigInt::from(u8::from(!add));
        let mut terms = Vec::new();
        for term in inputs {
            match term {
                Self::Add(children) if add => terms.extend(children),
                Self::Multiply(children) if !add => terms.extend(children),
                Self::Value(n) if n == neutral => {}
                other => terms.push(other),
            }
        }
        if terms.iter().all(|v| matches!(v, Self::Value(_))) {
            let result = terms.into_iter().fold(neutral, |acc, t| {
                let Self::Value(n) = t else { unreachable!() };
                if add { acc + n } else { acc * n }
            });
            return Self::Value(result);
        }
        if terms.len() == 1 {
            return terms.pop().unwrap();
        }
        terms.sort_by_cached_key(|t| format!("{t:?}"));
        if add { Self::Add(terms) } else { Self::Multiply(terms) }
    }
    pub(super) fn renamed(
        &self,
        from: &DeclarationId,
        to: &DeclarationId,
    ) -> Self {
        match self {
            Self::Binder { owner, index } if owner.as_ref() == from => Self::Binder {
                owner: Box::new(to.clone()),
                index: *index,
            },
            Self::Add(terms) | Self::Multiply(terms) => Self::operation(matches!(self, Self::Add(_)), terms.iter().map(|t| t.renamed(from, to)).collect()),
            t => t.clone(),
        }
    }
    pub(super) fn substitute(
        &self,
        owner: &DeclarationId,
        args: &[ValueType],
    ) -> Self {
        match self {
            Self::Binder { owner: b, index } if b.as_ref() == owner => match &args[*index] {
                ValueType::StaticInteger { value, .. } => Self::Value(value.clone()),
                ValueType::Parameter { owner, index } => Self::Binder {
                    owner: owner.clone(),
                    index: *index,
                },
                ValueType::StaticFormula { term, .. } => term.clone(),
                _ => self.clone(),
            },
            Self::Add(terms) | Self::Multiply(terms) => {
                Self::operation(matches!(self, Self::Add(_)), terms.iter().map(|t| t.substitute(owner, args)).collect())
            }
            other => other.clone(),
        }
    }
    pub(super) fn typed(
        self,
        carrier: ValueType,
    ) -> ValueType {
        match self {
            Self::Value(value) => ValueType::StaticInteger {
                value,
                carrier: Box::new(carrier),
            },
            Self::Binder { owner, index } => ValueType::Parameter { owner, index },
            term => ValueType::StaticFormula {
                term,
                carrier: Box::new(carrier),
            },
        }
    }
}
impl Unit<'_> {
    pub(super) fn open_index(
        &self,
        types: &Types<'_>,
        n: usize,
        carrier: &ValueType,
    ) -> Result<Option<ValueType>, TypeError> {
        let f = &self.file;
        if !matches!(carrier, ValueType::Integer { .. }) || !matches!(f.nodes[n].kind, NodeKind::Binary | NodeKind::Paren) {
            return Ok(None);
        }
        let mut dynamic = std::collections::HashSet::new();
        for i in (n..f.nodes[n].end).rev() {
            if matches!(
                self.symbol(i),
                Some(Symbol::Binding {
                    kind: NodeKind::ConstParam | NodeKind::KindedParam,
                    ..
                })
            ) || f.nodes[i].children.iter().any(|c| dynamic.contains(c))
            {
                dynamic.insert(i);
            }
        }
        if !dynamic.contains(&n) {
            return Ok(None);
        }
        let mut pending = vec![(n, false)];
        let mut terms = Vec::new();
        while let Some((i, exit)) = pending.pop() {
            let node = &f.nodes[i];
            if !dynamic.contains(&i) {
                let (number, suffix) = self.closed_number(types, i, carrier)?;
                if suffix.as_ref().is_some_and(|s| s != carrier) || !number.fits(carrier) {
                    return Err(TypeError::at(f, i, format!("static operand {number} does not land in {carrier}")));
                }
                let ValueType::Exact {
                    value,
                    fractional_origin: None,
                } = number
                else {
                    return Err(TypeError::at(f, i, "static integer operand requires an integral landing"));
                };
                terms.push(IntegerIndex::Value(value.to_integer()));
            } else if let Some(Symbol::Binding {
                span,
                kind: NodeKind::ConstParam | NodeKind::KindedParam,
            }) = self.symbol(i)
            {
                let Some(formal @ ValueType::Parameter { owner, index }) = self.parameters.get(span) else {
                    return Err(TypeError::unsupported(f, i));
                };
                if types.parameter_carrier(formal) != Some(carrier) {
                    return Err(TypeError::at(f, i, "open index operands require a common integer carrier"));
                }
                terms.push(IntegerIndex::Binder {
                    owner: owner.clone(),
                    index: *index,
                });
            } else if node.kind == NodeKind::Paren {
                pending.push((self.single_child(i)?, false));
            } else if node.kind == NodeKind::Binary && node.leaves.len() == 1 && matches!(node.leaves[0].text, "+" | "*") {
                if exit {
                    let right = terms.pop().unwrap();
                    let left = terms.pop().unwrap();
                    terms.push(IntegerIndex::operation(node.leaves[0].text == "+", vec![left, right]));
                } else {
                    pending.push((i, true));
                    pending.extend(node.children.iter().rev().map(|&child| (child, false)));
                }
            } else {
                return Err(TypeError::at(f, i, "open index requires an admitted static operation"));
            }
        }
        types
            .open_indices
            .borrow_mut()
            .insert(Site::at(f, n), (n, carrier.clone()));
        Ok(terms.pop().map(|term| term.typed(carrier.clone())))
    }
}
impl std::fmt::Display for IntegerIndex {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        match self {
            Self::Value(n) => write!(f, "{n}"),
            Self::Binder { owner, index } => write!(f, "{}::{}<{}>", owner.package, owner.path, index),
            Self::Add(terms) | Self::Multiply(terms) => {
                let token = if matches!(self, Self::Add(_)) { " + " } else { " * " };
                write!(
                    f,
                    "({})",
                    terms
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(token)
                )
            }
        }
    }
}

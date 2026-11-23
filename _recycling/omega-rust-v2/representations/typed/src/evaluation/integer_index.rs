use super::integer_arithmetic;
use crate::ValueType;
use num_bigint::BigInt;
use omega_resolved::DeclarationId;

/// The primitive exact integer operations whose fixed profile licenses
/// associative, commutative and neutral-element normalization. Source
/// operations and their independent formation obligations are kept in L3.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum IntegerIndex {
    Value(BigInt),
    Binder {
        owner: Box<DeclarationId>,
        index: usize,
    },
    Add(Vec<IntegerIndex>),
    Multiply(Vec<IntegerIndex>),
    Primitive {
        token: String,
        carrier: Box<ValueType>,
        operands: Vec<IntegerIndex>,
    },
    Invalid(String),
    Pending(std::sync::Arc<crate::StaticValueDemand>),
    Deferred {
        template: std::sync::Arc<crate::StaticTemplate>,
        arguments: Vec<ValueType>,
    },
}

impl IntegerIndex {
    pub fn demands(&self) -> impl Iterator<Item = &crate::StaticValueDemand> {
        let mut pending = vec![self];
        std::iter::from_fn(move || {
            while let Some(term) = pending.pop() {
                match term {
                    Self::Pending(demand) => return Some(demand.as_ref()),
                    Self::Add(terms) | Self::Multiply(terms) | Self::Primitive { operands: terms, .. } => {
                        pending.extend(terms.iter().rev());
                    }
                    _ => {}
                }
            }
            None
        })
    }

    pub fn depends_on_unsupplied_parameter(
        &self,
        declaration: &DeclarationId,
        bindings: &[Option<ValueType>],
    ) -> bool {
        let mut terms = vec![self];
        while let Some(term) = terms.pop() {
            match term {
                Self::Binder { owner, index } if owner.as_ref() == declaration && bindings.get(*index).is_none_or(Option::is_none) => return true,
                Self::Add(children) | Self::Multiply(children) => terms.extend(children),
                Self::Primitive { carrier, operands, .. } => {
                    if carrier.depends_on_unsupplied_parameter(declaration, bindings) {
                        return true;
                    }
                    terms.extend(operands);
                }
                Self::Deferred { arguments, .. }
                    if arguments
                        .iter()
                        .any(|argument| argument.depends_on_unsupplied_parameter(declaration, bindings)) =>
                {
                    return true;
                }
                _ => {}
            }
        }
        false
    }

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
    pub fn primitive(
        token: &str,
        carrier: &ValueType,
        mut operands: Vec<Self>,
    ) -> Self {
        if let Some(Self::Invalid(message)) = operands.iter().find(|term| matches!(term, Self::Invalid(_))) {
            return Self::Invalid(message.clone());
        }
        if matches!(token, "+" | "*") {
            return Self::operation(token == "+", operands);
        }
        if matches!(token, "+%" | "*%") {
            let neutral = BigInt::from(u8::from(token == "*%"));
            operands = operands
                .into_iter()
                .flat_map(|term| match term {
                    Self::Primitive {
                        token: inner,
                        carrier: ty,
                        operands,
                    } if inner == token && ty.as_ref() == carrier => operands,
                    Self::Value(value) if value == neutral => Vec::new(),
                    other => vec![other],
                })
                .collect();
            if operands.is_empty() {
                return Self::Value(neutral);
            }
            if operands.len() == 1 {
                return operands.pop().unwrap();
            }
        }
        if matches!(token, "+%" | "*%" | "+|") {
            operands.sort_by_cached_key(|t| format!("{t:?}"));
        }
        if let Some(values) = operands
            .iter()
            .map(|t| if let Self::Value(v) = t { Some(v.clone()) } else { None })
            .collect::<Option<Vec<_>>>()
        {
            return integer_arithmetic::evaluate_integer_index(token, &values, carrier).map_or_else(Self::Invalid, Self::Value);
        }
        Self::Primitive {
            token: token.into(),
            carrier: Box::new(carrier.clone()),
            operands,
        }
    }
    pub fn renamed(
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
            Self::Primitive { token, carrier, operands } => Self::primitive(token, carrier, operands.iter().map(|t| t.renamed(from, to)).collect()),
            t => t.clone(),
        }
    }
    pub fn substitute(
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
            Self::Primitive { token, carrier, operands } => Self::primitive(token, carrier, operands.iter().map(|t| t.substitute(owner, args)).collect()),
            Self::Deferred { template, arguments } => template.apply_integer(arguments.iter().map(|t| t.substitute(owner, args)).collect()),
            other => other.clone(),
        }
    }
    pub fn typed(
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
impl std::fmt::Display for IntegerIndex {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        match self {
            Self::Value(n) => write!(f, "{n}"),
            Self::Invalid(message) => write!(f, "{message}"),
            Self::Pending(demand) => {
                let site = &demand.request().expression;
                write!(f, "pending static expression at {}:{}", site.file, site.span.get_start_offset())
            }
            Self::Deferred { template, .. } => {
                let site = &template.request.expression;
                write!(f, "static expression at {}:{}", site.file, site.span.get_start_offset())
            }
            Self::Primitive { token, operands, .. } => write!(
                f,
                "({})",
                operands
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(&format!(" {token} "))
            ),
            Self::Binder { owner, index } => write!(f, "{}\\{}<{}>", owner.package, owner.path, index),
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

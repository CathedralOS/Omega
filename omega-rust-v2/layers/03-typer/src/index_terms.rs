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
    Deferred {
        template: std::sync::Arc<crate::StaticTemplate>,
        arguments: Vec<ValueType>,
    },
    Application {
        routine: std::sync::Arc<crate::IntegerRoutine>,
        arguments: Vec<IntegerIndex>,
    },
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
    pub(super) fn primitive(
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
            return crate::integers::compute(token, &values, carrier).map_or_else(Self::Invalid, Self::Value);
        }
        Self::Primitive {
            token: token.into(),
            carrier: Box::new(carrier.clone()),
            operands,
        }
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
            Self::Primitive { token, carrier, operands } => Self::primitive(token, carrier, operands.iter().map(|t| t.renamed(from, to)).collect()),
            Self::Application { routine, arguments } => routine.apply(arguments.iter().map(|a| a.renamed(from, to)).collect()),
            t => t.clone(),
        }
    }
    pub(super) fn substitute(
        &self,
        owner: &DeclarationId,
        args: &[ValueType],
    ) -> Self {
        self.evaluate(owner, args, &mut Default::default())
    }
    pub(super) fn evaluate(
        &self,
        owner: &DeclarationId,
        args: &[ValueType],
        cache: &mut crate::static_program::EvaluationCache,
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
                Self::operation(matches!(self, Self::Add(_)), terms.iter().map(|t| t.evaluate(owner, args, cache)).collect())
            }
            Self::Primitive { token, carrier, operands } => Self::primitive(
                token,
                carrier,
                operands
                    .iter()
                    .map(|t| t.evaluate(owner, args, cache))
                    .collect(),
            ),
            Self::Deferred { template, arguments } => template.apply(arguments.iter().map(|t| t.substitute(owner, args)).collect()),
            Self::Application { routine, arguments } => routine.evaluate(
                arguments
                    .iter()
                    .map(|a| a.evaluate(owner, args, cache))
                    .collect(),
                cache,
            ),
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
        if !matches!(carrier, ValueType::Integer { .. })
            || !matches!(
                f.nodes[n].kind,
                NodeKind::Binary | NodeKind::Paren | NodeKind::Call | NodeKind::Cast | NodeKind::Unary | NodeKind::FieldAccess | NodeKind::Match
            )
        {
            return Ok(None);
        }
        let Some(owner) = f
            .enclosing_contexts(n)
            .find_map(|p| self.definitions.get(&f.span(p)))
        else {
            return Ok(None);
        };
        let has_static = (n..f.nodes[n].end).any(|i| {
            matches!(
                self.symbol(i),
                Some(Symbol::Binding {
                    kind: NodeKind::ConstParam | NodeKind::KindedParam | NodeKind::TypeParam,
                    ..
                })
            )
        });
        let args = types.formals(owner);
        let may_select_evidence = args
            .iter()
            .any(|arg| matches!(types.parameter_carrier(arg), Some(ValueType::Evidence(_))))
            && (n..f.nodes[n].end).any(|i| {
                f.nodes[i]
                    .leaves
                    .iter()
                    .any(|t| types.operator_tokens.contains(t.text))
            });
        if !has_static && !may_select_evidence {
            return Ok(None);
        }
        if (n..f.nodes[n].end).any(|i| {
            matches!(
                self.symbol(i),
                Some(Symbol::Binding {
                    kind: NodeKind::Param | NodeKind::BindingPattern,
                    ..
                })
            )
        }) {
            return Err(TypeError::at(f, n, "const index needs a static argument"));
        }
        let prepared = if may_select_evidence {
            None
        } else {
            match types.integer_program.expression(self, n, types, carrier) {
                Ok(term) => Some(term),
                Err(e) if e.message == "static expression requires an admitted evaluator for this declaration" => None,
                Err(e) => return Err(e),
            }
        };
        let term = if let Some(term) = prepared {
            term
        } else {
            let site = Site::at(f, n);
            types.deferred_sites.borrow_mut().insert(site.clone());
            let term = types.specializations.prepare(self, n, owner, carrier, args);
            types
                .formations
                .borrow_mut()
                .entry((**owner).clone())
                .or_default()
                .insert(site, term.clone().typed(carrier.clone()));
            term
        };
        types
            .open_indices
            .borrow_mut()
            .insert(Site::at(f, n), (n, carrier.clone()));
        Ok(Some(term.typed(carrier.clone())))
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
            Self::Deferred { template, .. } => write!(f, "static expression at {}:{}", template.site.file, template.site.span.get_start_offset()),
            Self::Application { routine, arguments } => write!(
                f,
                "{}::{}({})",
                routine.declaration.package,
                routine.declaration.path,
                arguments
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::Primitive { token, operands, .. } => write!(
                f,
                "({})",
                operands
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(&format!(" {token} "))
            ),
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

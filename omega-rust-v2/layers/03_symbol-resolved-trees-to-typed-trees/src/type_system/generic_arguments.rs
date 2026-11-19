use crate::declarations::resolved_source_file::ResolvedSourceFile;
use crate::evaluation::numeric_literals;
use crate::type_system::type_catalog::TypeCatalog;
use crate::{TypeError, ValueType};
use omega_parser::NodeKind;
use omega_resolver::{DeclarationId, Symbol};

impl<'a> ResolvedSourceFile<'a> {
    pub fn argument(
        &self,
        node_index: usize,
        types: &TypeCatalog<'_>,
        owner: &DeclarationId,
        index: usize,
    ) -> Result<ValueType, TypeError> {
        let arg = self.raw_argument(node_index, types, owner, index, Some(owner))?;
        types
            .normalize_at(arg, self.file, node_index)
            .map_err(|m| TypeError::at(self.file, node_index, m))
    }
    pub(crate) fn raw_argument(
        &self,
        node_index: usize,
        types: &TypeCatalog<'_>,
        owner: &DeclarationId,
        index: usize,
        candidate: Option<&DeclarationId>,
    ) -> Result<ValueType, TypeError> {
        self.raw_bound_argument(node_index, types, owner, index, candidate, types.bound(owner, index))
    }
    pub(crate) fn raw_bound_argument(
        &self,
        node_index: usize,
        types: &TypeCatalog<'_>,
        owner: &DeclarationId,
        index: usize,
        candidate: Option<&DeclarationId>,
        bound: Option<&ValueType>,
    ) -> Result<ValueType, TypeError> {
        if types.lifetimes.contains(&(owner, index)) {
            if self.file.nodes[node_index].kind != NodeKind::Lifetime {
                return Err(TypeError::at(self.file, node_index, "expected a lifetime argument"));
            }
            return self.raw_type(node_index, candidate, types);
        }
        if self.file.nodes[node_index].kind == NodeKind::Lifetime {
            return Err(TypeError::at(self.file, node_index, "unexpected lifetime argument"));
        }
        let Some(carrier) = bound else {
            return self.raw_type(node_index, candidate, types);
        };
        let symbol = match self.symbol(node_index) {
            Some(Symbol::ByOverload(options)) => options
                .iter()
                .find(|o| Some(&o.candidate) == candidate)
                .and_then(|o| o.target.as_deref()),
            other => other,
        };
        if carrier == &ValueType::LevelKind {
            return self.universe_level(node_index, types);
        }
        if matches!(carrier, ValueType::Universe { .. }) {
            return self.raw_type(node_index, candidate, types);
        }
        if matches!(carrier, ValueType::LogicalRequirement(_)) {
            if matches!(self.file.nodes[node_index].kind, NodeKind::FieldAccess | NodeKind::Call)
                || matches!(
                    symbol,
                    Some(Symbol::Binding {
                        kind: NodeKind::Param | NodeKind::BindingPattern,
                        ..
                    })
                )
            {
                return Ok(ValueType::ProofTerm(crate::Site::at(self.file, node_index)));
            }
            return match symbol {
                Some(Symbol::Declaration(id)) if id.kind == NodeKind::Machine => Ok(ValueType::ProofCitation(Box::new(id.clone()))),
                Some(Symbol::Binding {
                    span,
                    kind: NodeKind::MachineParam | NodeKind::Hypothesis,
                }) => self
                    .hypotheses
                    .get(span)
                    .cloned()
                    .map(ValueType::LogicalRequirement)
                    .ok_or_else(|| TypeError::at(self.file, node_index, "unbound logical hypothesis")),
                _ => Err(TypeError::at(self.file, node_index, "expected explicitly supplied logical evidence")),
            };
        }
        if let Some(Symbol::Binding { span, kind: NodeKind::Param }) = symbol
            && self.file.nodes[node_index].kind != NodeKind::Call
        {
            if !types.runtime_indices.contains(&(owner, index)) && !matches!(carrier, ValueType::Function(..)) {
                return Err(TypeError::at(self.file, node_index, "const index needs a static argument"));
            }
            let annotation = self
                .binding_annotations
                .get(span)
                .ok_or_else(|| TypeError::unsupported(self.file, node_index))?;
            if &self.raw_type(*annotation, None, types)? != carrier {
                return Err(TypeError::at(self.file, node_index, "index subject has the wrong carrier"));
            }
            return Ok(self.term_subject(node_index, carrier, None));
        }
        if matches!(carrier, ValueType::Evidence(_))
            && let Some(Symbol::Declaration(id)) = symbol
            && id.kind == NodeKind::Conformance
        {
            return Ok(ValueType::Implementation(Box::new(id.clone())));
        }
        if matches!(carrier, ValueType::Callable(_))
            && let Some(Symbol::Declaration(id)) = symbol
            && id.kind == NodeKind::Machine
        {
            return Ok(ValueType::MachineSelection(Box::new(id.clone())));
        }
        if let Some(Symbol::Binding {
            span,
            kind: NodeKind::ConstParam | NodeKind::KindedParam,
        }) = symbol
        {
            let value = self
                .parameters
                .get(span)
                .ok_or_else(|| TypeError::unsupported(self.file, node_index))?;
            if types.parameter_carrier(value) != Some(carrier)
                && !matches!(
                    (types.parameter_carrier(value), carrier),
                    (Some(ValueType::Callable(_)), ValueType::Callable(_)) | (Some(ValueType::Evidence(_)), ValueType::Evidence(_))
                )
            {
                return Err(TypeError::at(self.file, node_index, "const parameter carrier mismatch"));
            }
            return Ok(value.clone());
        }
        if let Some(term) = self.open_index(types, node_index, carrier)? {
            return Ok(term);
        }
        let (value, suffix) = self.closed_number(types, node_index, carrier)?;
        if suffix.as_ref().is_some_and(|t| t != carrier)
            || !value.fits(carrier)
            || matches!(
                value,
                ValueType::Exact {
                    fractional_origin: Some(_),
                    ..
                }
            )
        {
            return Err(TypeError::at(
                self.file,
                node_index,
                format!("const argument {value} does not land in {carrier}"),
            ));
        }
        let ValueType::Exact { value, .. } = value else {
            return Err(TypeError::unsupported(self.file, node_index));
        };
        Ok(ValueType::StaticInteger {
            carrier: Box::new(carrier.clone()),
            value: value.to_integer(),
        })
    }
    pub(crate) fn closed_number(
        &self,
        types: &TypeCatalog<'_>,
        node_index: usize,
        carrier: &ValueType,
    ) -> Result<(ValueType, Option<ValueType>), TypeError> {
        let source = self.file;
        let needs_evaluation = (node_index..source.nodes[node_index].end).any(|i| {
            matches!(
                source.nodes[i].kind,
                NodeKind::Call | NodeKind::PathExpression | NodeKind::PathType | NodeKind::Cast
            )
        });
        if !needs_evaluation {
            let value = numeric_literals::static_number(source, node_index)?;
            if source.nodes[node_index].kind != NodeKind::Literal && value.1.is_some() {
                types
                    .open_indices
                    .borrow_mut()
                    .insert(crate::Site::at(source, node_index), (node_index, carrier.clone()));
            }
            return Ok(value);
        }
        if (node_index..source.nodes[node_index].end).any(|i| matches!(self.symbol(i), Some(Symbol::Binding { .. }))) {
            return Err(TypeError::at(source, node_index, "static expression needs specialization before evaluation"));
        }
        let request = crate::StaticIntegerRequest {
            expression: crate::Site::at(source, node_index),
            carrier: carrier.clone(),
        };
        let cached = types.evaluations.borrow().get(&request).cloned();
        let value = if let Some((_, value)) = cached {
            value
        } else {
            let value = if let Some(evaluator) = types.evaluator {
                evaluator
                    .evaluate(&request)
                    .map_err(|m| TypeError::at(source, node_index, m))?
            } else {
                match types
                    .integer_program
                    .expression(self, node_index, types, carrier)?
                {
                    crate::IntegerIndex::Value(value) => value,
                    crate::IntegerIndex::Invalid(message) => return Err(TypeError::at(source, node_index, message)),
                    _ => return Err(TypeError::at(source, node_index, "static expression has unresolved subjects")),
                }
            };
            types
                .evaluations
                .borrow_mut()
                .insert(request, (node_index, value.clone()));
            value
        };
        Ok((
            ValueType::Exact {
                value: num_rational::BigRational::from_integer(value),
                fractional_origin: None,
            },
            Some(carrier.clone()),
        ))
    }
    pub(crate) fn static_size(
        &self,
        types: &TypeCatalog<'_>,
        node_index: usize,
    ) -> Result<u64, TypeError> {
        let (value, suffix) = self.closed_number(types, node_index, &ValueType::integer("u64").unwrap())?;
        numeric_literals::array_length(self.file, node_index, value, suffix)
    }
}

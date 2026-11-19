use super::{numeric_literals, specialized_indices::IndexSpecializations};
use crate::declarations::declaration_catalog::DeclarationCatalog;
use crate::declarations::resolved_source_file::ResolvedSourceFile;
use crate::expressions::declaration_bodies;
use crate::type_system::type_catalog::TypeCatalog;
use crate::{IntegerIndex, StaticIntegerDemand, StaticIntegerRequest, TypeError, TypedStage, TypingEnvironment, ValueType};
use num_bigint::BigInt;
use omega_parser::NodeKind;
use omega_resolver::Symbol;
use std::{cell::RefCell, collections::HashMap};

#[derive(Clone)]
pub(crate) enum IndexEvaluation {
    Primitive,
    Specialized(usize),
    Closed(num_bigint::BigInt),
    Pending,
}
/// Evaluation supplies values across the stage boundary. Internally each request retains its resolved node.
#[derive(Default)]
pub(crate) struct IndexEvaluations<'a> {
    allow_pending: bool,
    demands: RefCell<Vec<StaticIntegerDemand>>,
    pub(super) specializations: IndexSpecializations,
    supplied: HashMap<&'a StaticIntegerRequest, &'a BigInt>,
    pub(super) uses: RefCell<HashMap<StaticIntegerRequest, IndexEvaluation>>,
}
impl<'a> IndexEvaluations<'a> {
    pub(crate) fn admit(
        units: &[ResolvedSourceFile<'_>],
        environment: &TypingEnvironment<'a>,
        allow_pending: bool,
    ) -> Result<Self, TypeError> {
        let specializations = IndexSpecializations::build(units, environment.specialized_indices, allow_pending)?;
        let mut supplied = HashMap::new();
        for (request, value) in environment.evaluated_indices {
            if supplied.insert(request, value).is_some_and(|old| old != value) {
                return Err(TypeError::for_site(
                    &request.expression,
                    "conflicting evaluation results for a closed expression",
                ));
            }
        }
        Ok(Self {
            specializations,
            supplied,
            allow_pending,
            demands: RefCell::default(),
            uses: RefCell::default(),
        })
    }
    pub(crate) fn take_demands(&self) -> Vec<StaticIntegerDemand> {
        self.demands.take()
    }

    pub(crate) fn retain_demands(
        &self,
        value: &ValueType,
    ) {
        if !self.allow_pending {
            return;
        }
        let mut demands = self.demands.borrow_mut();
        for part in value.components() {
            if let ValueType::StaticFormula { term, .. } = part {
                for demand in term.demands() {
                    if !demands.contains(demand) {
                        demands.push(demand.clone());
                    }
                }
            }
        }
    }

    /// A nominal landing can wait for its evaluated arguments only when the
    /// declaration, arity, and every known argument already agree.
    pub(crate) fn defer_indexed_landing(
        &self,
        actual: &ValueType,
        expected: &ValueType,
    ) -> bool {
        let (ValueType::Applied(a, actual), ValueType::Applied(b, expected)) = (actual, expected) else {
            return false;
        };
        if !self.allow_pending || a != b || actual.len() != expected.len() {
            return false;
        }
        let pending_index = |value: &ValueType| match value {
            ValueType::StaticFormula { term, .. } => term.demands().next().is_some(),
            _ => false,
        };
        let carrier = |value: &ValueType| match value {
            ValueType::StaticFormula { carrier, .. } | ValueType::StaticInteger { carrier, .. } => Some(carrier.clone()),
            _ => None,
        };
        if !actual
            .iter()
            .zip(expected)
            .all(|(a, b)| a == b || { (pending_index(a) || pending_index(b)) && carrier(a).is_some_and(|carrier_a| Some(carrier_a) == carrier(b)) })
        {
            return false;
        }
        actual
            .iter()
            .chain(expected)
            .for_each(|i| self.retain_demands(i));
        true
    }

    pub(crate) fn has_supplied_value(
        &self,
        request: &StaticIntegerRequest,
    ) -> bool {
        self.supplied.contains_key(request)
    }
}

pub(crate) fn check_static_indices(
    units: &[ResolvedSourceFile<'_>],
    catalog: &DeclarationCatalog<'_>,
    environment: &TypingEnvironment<'_>,
    output: &mut TypedStage,
) -> Result<(), TypeError> {
    // Checking a call can reveal a new static expression in its generic arguments.
    let mut checked = std::collections::HashSet::new();
    while checked.len() < catalog.types.indices.uses.borrow().len() {
        let mut uses: Vec<_> = catalog
            .types
            .indices
            .uses
            .borrow()
            .iter()
            .filter(|(request, _)| checked.insert((*request).clone()))
            .map(|(request, kind)| (request.clone(), kind.clone()))
            .collect();
        uses.sort_by_cached_key(|(request, _)| (request.expression.clone(), request.node, request.carrier.to_string()));
        for (request, evaluation) in uses {
            let node = request.node;
            declaration_bodies::check_static_argument(&units[node.file], catalog, node.node, &request, &evaluation, output)?;
            if let IndexEvaluation::Closed(value) = evaluation {
                output.evaluated_indices.push((request, value));
            }
        }
    }
    let uses = catalog.types.indices.uses.borrow();
    for (request, _) in environment.evaluated_indices {
        if !matches!(uses.get(request), Some(IndexEvaluation::Closed(_))) {
            return Err(TypeError::for_site(
                &request.expression,
                "evaluation result has no matching static expression and carrier",
            ));
        }
    }
    for result in environment.specialized_indices {
        let request = &result.request;
        if !matches!(uses.get(request), Some(IndexEvaluation::Specialized(arity)) if *arity == result.arguments.len()) {
            return Err(TypeError::for_site(
                &request.expression,
                "evaluation result has no matching specialized expression, carrier and owner arity",
            ));
        }
    }
    Ok(())
}

impl ResolvedSourceFile<'_> {
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
            let value = numeric_literals::static_number(self, node_index)?;
            if source.nodes[node_index].kind != NodeKind::Literal && value.1.is_some() {
                types
                    .indices
                    .uses
                    .borrow_mut()
                    .insert(crate::StaticIntegerRequest::at(source, node_index, carrier), IndexEvaluation::Primitive);
            }
            return Ok(value);
        }
        if (node_index..source.nodes[node_index].end).any(|i| matches!(self.name_in_role(i, false), Some(Symbol::Binding { .. }))) {
            return Err(TypeError::at(source, node_index, "static expression needs specialization before evaluation"));
        }
        let request = crate::StaticIntegerRequest::at(source, node_index, carrier);
        let cached = types.indices.uses.borrow().get(&request).cloned();
        let value = if let Some(IndexEvaluation::Closed(value)) = cached {
            value
        } else {
            let supplied = types.indices.supplied.get(&request).copied();
            let value = match self.primitive_index(node_index, types, carrier)? {
                Some(crate::IntegerIndex::Value(value)) => {
                    if supplied.is_some_and(|v| v != &value) {
                        return Err(TypeError::at(source, node_index, "evaluation result disagrees with the primitive index"));
                    }
                    value
                }
                Some(crate::IntegerIndex::Invalid(message)) => return Err(TypeError::at(source, node_index, message)),
                Some(_) => return Err(TypeError::at(source, node_index, "static expression has unresolved subjects")),
                None if types.indices.allow_pending && supplied.is_none() => {
                    let term = IntegerIndex::Pending(std::sync::Arc::new(StaticIntegerDemand::Closed(request.clone()))).typed(carrier.clone());
                    types.indices.retain_demands(&term);
                    types
                        .indices
                        .uses
                        .borrow_mut()
                        .insert(request, IndexEvaluation::Pending);
                    return Ok((term, Some(carrier.clone())));
                }
                None => supplied
                    .cloned()
                    .ok_or_else(|| TypeError::at(source, node_index, "static expression requires an admitted evaluation result"))?,
            };
            types
                .indices
                .uses
                .borrow_mut()
                .insert(request, IndexEvaluation::Closed(value.clone()));
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
}

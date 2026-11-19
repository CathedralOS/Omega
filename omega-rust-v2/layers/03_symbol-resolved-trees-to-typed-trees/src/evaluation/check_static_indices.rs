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

    /// Preparation may postpone equality only when the known structure agrees
    /// and a requested integer value can settle the remaining difference.
    pub(crate) fn defer_indexed_landing(
        &self,
        actual: &ValueType,
        expected: &ValueType,
    ) -> bool {
        if !self.allow_pending || actual == expected || !compatible_pending_indices(actual, expected) {
            return false;
        }
        self.retain_demands(actual);
        self.retain_demands(expected);
        true
    }

    pub(crate) fn has_supplied_value(
        &self,
        request: &StaticIntegerRequest,
    ) -> bool {
        self.supplied.contains_key(request)
    }
}

fn compatible_pending_indices(
    actual: &ValueType,
    expected: &ValueType,
) -> bool {
    use ValueType::{Applied, Array, IndexedArray, StaticFormula, StaticInteger};
    if actual == expected {
        return true;
    }
    match (actual, expected) {
        (Applied(a, left), Applied(b, right)) => {
            a == b
                && left.len() == right.len()
                && left
                    .iter()
                    .zip(right)
                    .all(|(a, b)| compatible_pending_indices(a, b))
        }
        (Array(a, left), Array(b, right)) => left == right && compatible_pending_indices(a, b),
        (IndexedArray(a, left), IndexedArray(b, right)) => compatible_pending_indices(a, b) && compatible_pending_indices(left, right),
        (Array(a, length), IndexedArray(b, index)) | (IndexedArray(b, index), Array(a, length)) => {
            compatible_pending_indices(a, b) && compatible_pending_indices(index, &ValueType::static_size(*length))
        }
        (StaticFormula { carrier: a, term }, StaticInteger { carrier: b, .. }) | (StaticInteger { carrier: b, .. }, StaticFormula { carrier: a, term }) => {
            a == b && term.demands().next().is_some()
        }
        (StaticFormula { carrier: a, term: left }, StaticFormula { carrier: b, term: right }) => {
            a == b && (left.demands().next().is_some() || right.demands().next().is_some())
        }
        _ => false,
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

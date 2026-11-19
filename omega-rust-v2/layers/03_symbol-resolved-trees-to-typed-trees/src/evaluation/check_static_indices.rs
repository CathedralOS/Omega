use super::{numeric_literals, specialized_indices::IndexSpecializations};
use crate::declarations::declaration_catalog::DeclarationCatalog;
use crate::declarations::resolved_source_file::ResolvedSourceFile;
use crate::expressions::declaration_bodies;
use crate::type_system::type_catalog::TypeCatalog;
use crate::{IntegerIndex, StaticValue, StaticValueDemand, StaticValueRequest, TypeError, TypedStage, TypingEnvironment, ValueType};
use omega_parser::NodeKind;
use omega_resolver::{DeclarationId, IndexedSource, Symbol, TreeNodeId};
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
};

#[derive(Clone)]
pub(crate) enum IndexEvaluation {
    Primitive,
    Specialized(usize),
    Closed(StaticValue),
    Pending,
}
/// Evaluation supplies values across the stage boundary. Internally each request retains its resolved node.
#[derive(Default)]
pub(crate) struct IndexEvaluations<'a> {
    allow_pending: bool,
    demands: RefCell<Vec<StaticValueDemand>>,
    blocked: RefCell<Vec<TreeNodeId>>,
    pending_constants: RefCell<HashSet<DeclarationId>>,
    pub(super) specializations: IndexSpecializations,
    supplied: HashMap<&'a StaticValueRequest, &'a StaticValue>,
    pub(super) uses: RefCell<HashMap<StaticValueRequest, IndexEvaluation>>,
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
            blocked: RefCell::default(),
            pending_constants: RefCell::default(),
            uses: RefCell::default(),
        })
    }
    pub(crate) fn take_demands(&self) -> Vec<StaticValueDemand> {
        self.demands.take()
    }

    pub(crate) fn defer_constant(
        &self,
        declaration: &DeclarationId,
    ) {
        self.pending_constants.borrow_mut().insert(declaration.clone());
    }

    pub(crate) fn constant_is_pending(
        &self,
        declaration: &DeclarationId,
    ) -> bool {
        self.pending_constants.borrow().contains(declaration)
    }

    pub(crate) fn take_blocked_nodes(&self) -> Vec<TreeNodeId> {
        self.blocked.take()
    }

    pub(crate) fn wait(
        &self,
        file: &IndexedSource<'_>,
        node: usize,
    ) -> TypeError {
        self.blocked.borrow_mut().push(file.node_id(node));
        TypeError::waiting(file, node)
    }

    pub(crate) fn retain_demands_at(
        &self,
        node: TreeNodeId,
        value: &ValueType,
    ) {
        if self.retain_demands(value) && static_demands(value).any(|demand| demand.request().node != node) {
            self.blocked.borrow_mut().push(node);
        }
    }

    pub(crate) fn retain_demands(
        &self,
        value: &ValueType,
    ) -> bool {
        if !self.allow_pending {
            return false;
        }
        let mut needs_value = false;
        let mut demands = self.demands.borrow_mut();
        for demand in static_demands(value) {
            needs_value = true;
            if !demands.contains(demand) {
                demands.push(demand.clone());
            }
        }
        needs_value
    }

    /// The known parts must agree, and a missing value must account for
    /// the difference. The caller retains demands only for viable candidates.
    pub(crate) fn can_wait_for_equality(
        &self,
        actual: &ValueType,
        expected: &ValueType,
    ) -> bool {
        self.allow_pending
            && actual != expected
            && compatible_pending_indices(actual, expected)
            && static_demands(actual)
                .chain(static_demands(expected))
                .next()
                .is_some()
    }

    pub(crate) fn can_wait_for_result_domains(
        &self,
        actual: &ValueType,
        expected: &ValueType,
    ) -> bool {
        self.allow_pending
            && compatible_pending_domains(actual.domains(), expected.domains())
            && actual
                .domains()
                .iter()
                .chain(expected.domains())
                .flat_map(|domain| &domain.arguments)
                .flat_map(static_demands)
                .next()
                .is_some()
    }

    pub(crate) fn has_supplied_value(
        &self,
        request: &StaticValueRequest,
    ) -> bool {
        self.supplied.contains_key(request)
    }
}

fn static_demands(value: &ValueType) -> impl Iterator<Item = &StaticValueDemand> {
    value.components().flat_map(|part| {
        let term = match part {
            ValueType::StaticFormula { term, .. } => Some(term),
            _ => None,
        };
        let demand = match part {
            ValueType::PendingStatic(demand) => Some(demand.as_ref()),
            _ => None,
        };
        term.into_iter().flat_map(IntegerIndex::demands).chain(demand)
    })
}

fn compatible_pending_indices(
    actual: &ValueType,
    expected: &ValueType,
) -> bool {
    use ValueType::{
        Applied, Array, AtLifetime, Function, IndexedArray, IndexedInteger, Integer, PendingStatic, Qualified, Reference, Slice, StaticFormula, StaticInteger,
        StaticValue, Tuple,
    };
    let parts = |left: &[ValueType], right: &[ValueType]| {
        left.len() == right.len()
            && left
                .iter()
                .zip(right)
                .all(|(a, b)| compatible_pending_indices(a, b))
    };
    if actual == expected {
        return true;
    }
    match (actual, expected) {
        (Applied(a, left), Applied(b, right)) => a == b && parts(left, right),
        (Tuple(left), Tuple(right)) => parts(left, right),
        (Reference(a, left), Reference(b, right)) => a == b && compatible_pending_indices(left, right),
        (Slice(left), Slice(right)) => compatible_pending_indices(left, right),
        (Function(a, left), Function(b, right)) | (AtLifetime(a, left), AtLifetime(b, right)) => {
            compatible_pending_indices(a, b) && compatible_pending_indices(left, right)
        }
        (Qualified(a, left), Qualified(b, right)) => compatible_pending_indices(a, b) && compatible_pending_domains(left, right),
        (IndexedInteger { signed: a, width: left }, IndexedInteger { signed: b, width: right }) => a == b && compatible_pending_indices(left, right),
        (Integer { signed: a, width: Some(width) }, IndexedInteger { signed: b, width: index })
        | (IndexedInteger { signed: b, width: index }, Integer { signed: a, width: Some(width) }) => {
            a == b && compatible_pending_indices(index, &ValueType::static_size(*width))
        }
        (Array(a, left), Array(b, right)) => left == right && compatible_pending_indices(a, b),
        (IndexedArray(a, left), IndexedArray(b, right)) => compatible_pending_indices(a, b) && compatible_pending_indices(left, right),
        (Array(a, length), IndexedArray(b, index)) | (IndexedArray(b, index), Array(a, length)) => {
            compatible_pending_indices(a, b) && compatible_pending_indices(index, &ValueType::static_size(*length))
        }
        (StaticFormula { carrier: a, term }, StaticInteger { carrier: b, .. }) | (StaticInteger { carrier: b, .. }, StaticFormula { carrier: a, term }) => {
            a == b && term.demands().next().is_some()
        }
        (PendingStatic(demand), StaticValue { carrier, .. }) | (StaticValue { carrier, .. }, PendingStatic(demand)) => demand.request().carrier == **carrier,
        (PendingStatic(left), PendingStatic(right)) => left.request().carrier == right.request().carrier,
        (StaticFormula { carrier: a, term: left }, StaticFormula { carrier: b, term: right }) => {
            a == b && (left.demands().next().is_some() || right.demands().next().is_some())
        }
        _ => false,
    }
}

fn compatible_pending_domains(
    left: &[crate::DomainApplication],
    right: &[crate::DomainApplication],
) -> bool {
    // Evaluation can reorder applications of one domain or merge equal ones.
    // Each application must have a compatible application on the other side.
    let covered = |source: &[crate::DomainApplication], destination: &[crate::DomainApplication]| {
        source.iter().all(|domain| {
            destination.iter().any(|other| {
                domain.declaration == other.declaration
                    && domain.arguments.len() == other.arguments.len()
                    && domain
                        .arguments
                        .iter()
                        .zip(&other.arguments)
                        .all(|(a, b)| compatible_pending_indices(a, b))
            })
        })
    };
    covered(left, right) && covered(right, left)
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
            if let Err(error) = declaration_bodies::check_static_argument(&units[node.file], catalog, node.node, &request, &evaluation, output) {
                if error.waiting_for_static_value {
                    continue;
                }
                return Err(error);
            }
            if let IndexEvaluation::Closed(value) = evaluation {
                if !value.matches_carrier(&request.carrier, catalog) {
                    return Err(TypeError::for_site(&request.expression, "evaluation result does not match its carrier"));
                }
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
    pub(crate) fn closed_static_value(
        &self,
        types: &TypeCatalog<'_>,
        node: usize,
        carrier: &ValueType,
    ) -> Result<ValueType, TypeError> {
        let source = self.file;
        let expression = source.ungroup(node);
        if carrier == &ValueType::Bool
            && let Some(Symbol::Builtin(name)) = self.name_in_role(expression, false)
            && let value @ ("true" | "false") = name.as_str()
        {
            return Ok(ValueType::StaticValue {
                carrier: Box::new(carrier.clone()),
                value: StaticValue::Bool(value == "true"),
            });
        }
        let expression_nodes = node..source.nodes[node].end;
        let request = StaticValueRequest::at(source, node, carrier);
        let mut bound = false;
        for part in expression_nodes.clone() {
            let Some(Symbol::Binding { binder_node, kind, .. }) = self.name_in_role(part, false) else {
                continue;
            };
            if expression_nodes.contains(binder_node) {
                continue;
            }
            if !matches!(kind, NodeKind::ConstParam | NodeKind::KindedParam | NodeKind::TypeParam) {
                return Err(TypeError::at(source, part, "const index needs a static argument"));
            }
            bound = true;
        }
        if bound {
            let owner = self
                .specialization_owner(node)
                .filter(|_| carrier == &ValueType::Bool)
                .ok_or_else(|| TypeError::at(source, node, "static expression needs specialization before evaluation"))?;
            let arguments = types.specialization_formals(owner);
            let arity = arguments.len();
            let value = types
                .indices
                .specializations
                .template(self, node, owner, carrier)
                .apply_value(arguments);
            types.formations.index(self, node, value.clone());
            types
                .indices
                .uses
                .borrow_mut()
                .insert(request, IndexEvaluation::Specialized(arity));
            return Ok(value);
        }
        let (value, evaluation) = match types.indices.supplied.get(&request).copied() {
            Some(value) => (
                ValueType::StaticValue {
                    carrier: Box::new(carrier.clone()),
                    value: value.clone(),
                },
                IndexEvaluation::Closed(value.clone()),
            ),
            None if types.indices.allow_pending => {
                let value = ValueType::PendingStatic(std::sync::Arc::new(StaticValueDemand::Closed(request.clone())));
                types.indices.retain_demands(&value);
                (value, IndexEvaluation::Pending)
            }
            None => return Err(TypeError::at(source, node, "static expression requires an admitted evaluation result")),
        };
        types.indices.uses.borrow_mut().insert(request, evaluation);
        Ok(value)
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
            let value = numeric_literals::static_number(self, node_index)?;
            if source.nodes[node_index].kind != NodeKind::Literal && value.1.is_some() {
                types
                    .indices
                    .uses
                    .borrow_mut()
                    .insert(crate::StaticValueRequest::at(source, node_index, carrier), IndexEvaluation::Primitive);
            }
            return Ok(value);
        }
        if (node_index..source.nodes[node_index].end).any(|i| matches!(self.name_in_role(i, false), Some(Symbol::Binding { .. }))) {
            return Err(TypeError::at(source, node_index, "static expression needs specialization before evaluation"));
        }
        let request = crate::StaticValueRequest::at(source, node_index, carrier);
        let cached = types.indices.uses.borrow().get(&request).cloned();
        let value = if let Some(IndexEvaluation::Closed(StaticValue::Integer(value))) = cached {
            value
        } else {
            let supplied = match types.indices.supplied.get(&request).copied() {
                Some(StaticValue::Integer(value)) => Some(value),
                None => None,
                Some(_) => return Err(TypeError::at(source, node_index, "evaluation result does not match its integer carrier")),
            };
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
                    let term = IntegerIndex::Pending(std::sync::Arc::new(StaticValueDemand::Closed(request.clone()))).typed(carrier.clone());
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
                .insert(request, IndexEvaluation::Closed(StaticValue::Integer(value.clone())));
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

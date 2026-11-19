//! The evaluator executes immutable reads, owned constructions, exact landings
//! and admitted scalar operations. A local invocation checks every premise
//! before its value enters the prepared product.
pub(super) mod admission;
mod collection_reads;
mod floating_point;
mod statements;
pub(crate) mod static_callbacks;
pub(super) mod termination;
mod values;

use crate::constant_evaluation::ConstantEvaluation;
use crate::integer_arithmetic::IntegerRange;
use crate::{FloatFormat, FloatOperation, PreparedValue};
use num_bigint::BigInt;
use omega_parser::NodeKind;
use omega_resolver::{DeclarationId, Symbol, TreeNodeId};
use omega_typer::{AppliedOperand, CallUse, NodeSemantics, ObligationKind, StaticValueRequest, TypedCall, TypedSignature, TypedTree, ValueType};
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

#[derive(Debug)]
pub(super) enum EvaluationError {
    Refused(&'static str),
    NeedsConstant(DeclarationId),
    AtCall(omega_typer::TypeError),
    OpenTarget(omega_typer::TypeError),
}

impl From<&'static str> for EvaluationError {
    fn from(reason: &'static str) -> Self {
        Self::Refused(reason)
    }
}

pub(super) const UNSUPPORTED_BODY: &str = "the selected body is outside the supported static evaluation fragment";
const UNSUPPORTED_ARGUMENT: &str = "the static call has an argument without a supported closed value";
const UNSUPPORTED_CONTRACT: &str = "the selected contract needs proof outside the supported static evaluation fragment";

struct BindingValue {
    binder: usize,
    carrier: ValueType,
    value: PreparedValue,
    writable: bool,
}

pub(super) struct CallBindings(RefCell<Vec<BindingValue>>, Option<Rc<TypedCall>>);

#[derive(Clone)]
pub(super) enum Scope<'a> {
    Initializer {
        owner_arguments: Option<&'a [ValueType]>,
        in_argument: bool,
    },
    Body(Rc<CallBindings>),
    Fact(Rc<CallBindings>),
}

impl Scope<'_> {
    fn application(&self) -> Option<&TypedCall> {
        match self {
            Self::Body(bindings) | Self::Fact(bindings) => bindings.1.as_deref(),
            Self::Initializer { .. } => None,
        }
    }

    fn instantiate(
        &self,
        carrier: &ValueType,
    ) -> ValueType {
        self.application()
            .map_or_else(|| carrier.clone(), |call| call.instantiate(carrier))
    }

    fn call_use(&self) -> CallUse {
        match self {
            Self::Initializer { .. } => CallUse::StaticArgument,
            Self::Body(_) => CallUse::Execution,
            Self::Fact(_) => CallUse::Fact,
        }
    }

    fn binding(
        &self,
        tree: &TypedTree<'_>,
        node: usize,
        carrier: &ValueType,
    ) -> Option<PreparedValue> {
        let (Self::Body(arguments) | Self::Fact(arguments)) = self else {
            return None;
        };
        let Symbol::Binding {
            binder_node,
            kind: NodeKind::Param | NodeKind::BindingPattern | NodeKind::FieldPattern,
            ..
        } = tree.resolved().symbol(node)?
        else {
            return None;
        };
        arguments
            .0
            .borrow()
            .iter()
            .rev()
            .find(|binding| binding.binder == *binder_node && &binding.carrier == carrier)
            .map(|binding| binding.value.clone())
    }
}

// Expressions, arguments and machine contracts all suspend on the same heap
// stack, so any mix of these calls shares cycle detection and provenance.
enum Step<'a> {
    Evaluate(TreeNodeId, ValueType, Scope<'a>),
    Invoke(TreeNodeId, ValueType, Scope<'a>),
    Enter {
        call: TreeNodeId,
        signature: &'a TypedSignature,
        requirement: Option<(&'a TypedSignature, Option<Rc<TypedCall>>)>,
        application: Option<Rc<TypedCall>>,
    },
    EnterCheckedCallback {
        request: static_callbacks::CallableUse,
        signature: &'a TypedSignature,
        application: Option<Rc<TypedCall>>,
    },
    CheckCallableGuarantee {
        request: static_callbacks::CallableUse,
        operands: Vec<PreparedValue>,
    },
    Block {
        site: TreeNodeId,
        statement: usize,
        carrier: ValueType,
        bindings: Rc<CallBindings>,
    },
    Bind {
        binder: usize,
        carrier: ValueType,
        bindings: Rc<CallBindings>,
        writable: bool,
    },
    Store {
        local: usize,
        bindings: Rc<CallBindings>,
    },
    Select {
        site: TreeNodeId,
        carrier: ValueType,
        subject_type: ValueType,
        scope: Scope<'a>,
    },
    RestoreBindings {
        count: usize,
        bindings: Rc<CallBindings>,
    },
    Reject(EvaluationError),
    Push(PreparedValue),
    FinishArray(usize),
    FinishTuple(usize),
    FinishZeroArray(usize),
    FinishConstruction {
        proof_site: Option<TreeNodeId>,
        owner: DeclarationId,
        case: Option<DeclarationId>,
        positions: Vec<usize>,
    },
    Leave {
        declaration: TreeNodeId,
        first_activation: bool,
    },
    Require(&'static str),
    CrashGuard(&'static str),
    CheckInvocationProof(admission::ProofRequest<'a>, Rc<CallBindings>),
    ApplyInteger(IntegerRange, String),
    ApplyFloat {
        site: TreeNodeId,
        format: FloatFormat,
        operation: FloatOperation,
    },
    CoerceFloat {
        site: TreeNodeId,
        carrier: ValueType,
    },
    ProjectFields(Vec<DeclarationId>),
    ReadArray,
    ArrayLength,
    ProduceZero {
        site: TreeNodeId,
        carrier: ValueType,
    },
    Compare(String),
    ShortCircuit {
        right: TreeNodeId,
        scope: Scope<'a>,
        stop_at: bool,
    },
    Equal(bool),
    Negate,
}

#[derive(Default)]
struct Execution<'a> {
    steps: Vec<Step<'a>>,
    values: Vec<PreparedValue>,
    active: HashSet<TreeNodeId>,
    checked_aggregates: HashSet<ValueType>,
    chain: Vec<(TreeNodeId, bool)>,
}

impl Execution<'_> {
    fn pop_integer(&mut self) -> BigInt {
        let Some(PreparedValue::Integer(value)) = self.values.pop() else {
            unreachable!("visited integer operand")
        };
        value
    }

    fn pop_bool(&mut self) -> bool {
        let Some(PreparedValue::Bool(value)) = self.values.pop() else {
            unreachable!("visited Boolean operand")
        };
        value
    }
}

impl ConstantEvaluation<'_, '_> {
    /// Typing records a possible loan in an open result. Discharge that
    /// obligation only for the current exact machine application whose
    /// complete result is independently known to be an owned copy value.
    fn admits_owned_return(
        &self,
        scope: &Scope<'_>,
        obligation: &ObligationKind,
        carrier: &ValueType,
    ) -> bool {
        let (Scope::Body(_), ObligationKind::ReturnBorrow { machine }) = (scope, obligation) else {
            return false;
        };
        let Some(application) = scope.application() else { return false };
        if application.machine != *machine || &application.result != carrier {
            return false;
        }
        let Some((_, signature)) = self.signatures().get(machine) else { return false };
        if application.instantiate(&signature.result) != *carrier {
            return false;
        }
        if is_aggregate(carrier) {
            return self
                .check_owned_type(carrier, matches!(carrier, ValueType::Array(..)))
                .is_ok();
        }
        carrier == &ValueType::Bool || IntegerRange::for_type(carrier).is_some() || FloatFormat::for_type(carrier).is_some()
    }

    pub(super) fn evaluate_value_request(
        &self,
        request: &StaticValueRequest,
        owner_arguments: Option<&[ValueType]>,
    ) -> Result<PreparedValue, EvaluationError> {
        self.execute(
            request.node,
            &request.carrier,
            &Scope::Initializer {
                owner_arguments,
                in_argument: false,
            },
        )
    }

    fn execute(
        &self,
        root: TreeNodeId,
        carrier: &ValueType,
        scope: &Scope<'_>,
    ) -> Result<PreparedValue, EvaluationError> {
        let mut execution = Execution::default();
        execution
            .steps
            .push(Step::Evaluate(root, carrier.clone(), scope.clone()));
        while let Some(step) = execution.steps.pop() {
            if let Err(error) = self.evaluate_step(step, &mut execution) {
                return Err(match error {
                    EvaluationError::Refused(reason)
                        if execution
                            .chain
                            .last()
                            .is_some_and(|(_, use_call_site)| *use_call_site)
                            || execution.chain.len() > 1 =>
                    {
                        self.call_refusal(reason, &execution.chain, &[])
                    }
                    other => other,
                });
            }
        }
        Ok(execution.values.pop().expect("visited root expression"))
    }

    fn evaluate_step<'a>(
        &'a self,
        step: Step<'a>,
        execution: &mut Execution<'a>,
    ) -> Result<(), EvaluationError> {
        match step {
            Step::Evaluate(site, carrier, scope) => {
                let tree = self.tree(site)?;
                let file = tree.resolved().file;
                let node = file.ungroup(site.node);
                let result = if file.nodes[node].kind == NodeKind::Match {
                    if checked_carrier_in_scope(&tree, node, scope.application())? != carrier {
                        return Err(UNSUPPORTED_BODY.into());
                    }
                    self.schedule_dispatch(&tree, node, &carrier, scope.clone(), execution)
                } else if match file.nodes[node].kind {
                    NodeKind::Index => self.schedule_array_read(&tree, node, &carrier, &scope, execution)?,
                    NodeKind::FieldAccess => {
                        self.schedule_array_length(&tree, node, &carrier, &scope, execution)?
                            || self.schedule_owned_fields(&tree, node, &carrier, &scope, execution)?
                    }
                    _ => false,
                } {
                    Ok(())
                } else if carrier == ValueType::Bool {
                    self.evaluate_boolean(&tree, site.node, &scope, execution)
                } else if is_aggregate(&carrier) {
                    self.schedule_aggregate(&tree, site.node, &carrier, &scope, execution)
                } else if FloatFormat::for_type(&carrier).is_some() {
                    self.evaluate_float(&tree, site.node, &carrier, &scope, execution)
                } else {
                    self.evaluate_integer(&tree, site.node, &carrier, &scope, execution)
                };
                result.map_err(|error| match error {
                    EvaluationError::Refused(UNSUPPORTED_BODY) if matches!(scope, Scope::Initializer { in_argument: true, .. }) => UNSUPPORTED_ARGUMENT.into(),
                    other => other,
                })?;
            }
            Step::Invoke(site, carrier, scope) => {
                let tree = self.tree(site)?;
                execution
                    .chain
                    .push((site, IntegerRange::for_type(&carrier).is_none()));
                let Some(call) = selected_call(&tree, site.node, scope.call_use())? else {
                    execution.chain.last_mut().expect("visited callback").1 = true;
                    return self.schedule_static_callback(&tree, site, &scope, &carrier, execution);
                };
                if let Some(query) = &call.layout_query {
                    execution.chain.last_mut().expect("visited layout query").1 = true;
                    if call.result != carrier
                        || !call.operands.is_empty()
                        || !call.enclosing_arguments.is_empty()
                        || call.evidence.is_some()
                        || call.acknowledgements.suspend
                        || call.acknowledgements.block
                    {
                        return Err("the request does not select one core layout query".into());
                    }
                    let arguments = match scope {
                        Scope::Initializer { owner_arguments, .. } => owner_arguments,
                        Scope::Body(_) | Scope::Fact(_) => None,
                    };
                    let [subject] = call.type_arguments.as_slice() else {
                        return Err("the request does not select one core layout query".into());
                    };
                    let subject = query
                        .close_subject(scope.instantiate(subject), arguments)
                        .ok_or("the layout query needs a closed type application")?;
                    if self.boundary_slots.iter().any(|slot| {
                        slot.requirement.declaration == call.machine
                            && slot.requirement.trait_arguments.is_empty()
                            && (slot.arguments.is_empty() || slot.arguments.as_slice() == std::slice::from_ref(&subject))
                    }) {
                        return Err("the selected runtime boundary slot cannot execute during preparation".into());
                    }
                    let target = self.target.ok_or_else(|| {
                        EvaluationError::OpenTarget(self.call_diagnostic("the layout query requires a selected target", &execution.chain, &[]))
                    })?;
                    let (value, observation) = target.observe_layout(query.projection, &subject)?;
                    self.record_observation(observation);
                    execution.values.push(PreparedValue::Integer(value.into()));
                    execution.chain.pop();
                } else {
                    self.schedule_call(&tree, call, &scope, &carrier, execution)?;
                }
            }
            Step::Enter {
                call,
                signature,
                requirement,
                application,
            } => self.enter_call(call, signature, requirement, application, execution)?,
            Step::EnterCheckedCallback {
                request,
                signature,
                application,
            } => self.enter_checked_callback(request, signature, application, execution)?,
            Step::CheckCallableGuarantee { request, operands } => {
                let result = execution.values.last().ok_or(UNSUPPORTED_BODY)?;
                self.admit_callable_use(&request, &operands, Some(result))?;
            }
            Step::CheckInvocationProof(request, arguments) => self.admit_proof(request, &arguments, &execution.values)?,
            Step::Block {
                site,
                statement,
                carrier,
                bindings,
            } => self.schedule_statement(site, statement, carrier, bindings, execution)?,
            Step::Bind {
                binder,
                carrier,
                bindings,
                writable,
            } => {
                let value = execution.values.pop().expect("visited binding initializer");
                bindings.0.borrow_mut().push(BindingValue {
                    binder,
                    carrier,
                    value,
                    writable,
                });
            }
            Step::Store { local, bindings } => {
                let value = execution.values.pop().expect("visited assignment value");
                bindings.0.borrow_mut()[local].value = value;
            }
            Step::Select {
                site,
                carrier,
                subject_type,
                scope,
            } => self.select_arm(site, carrier, &subject_type, scope, execution)?,
            Step::RestoreBindings { count, bindings } => bindings.0.borrow_mut().truncate(count),
            Step::Reject(error) => return Err(error),
            Step::Push(value) => execution.values.push(value),
            Step::FinishZeroArray(count) => {
                let element = execution.values.pop().expect("visited zero array element");
                let mut elements = Vec::new();
                elements
                    .try_reserve_exact(count)
                    .map_err(|_| "zero-filling an array requires checked preparation storage")?;
                elements.resize(count, element);
                execution.values.push(PreparedValue::Array(elements.into()));
            }
            Step::FinishTuple(count) => {
                let fields = execution.values.split_off(execution.values.len() - count);
                execution.values.push(PreparedValue::Tuple(fields.into()));
            }
            Step::FinishArray(count) => {
                let elements = execution.values.split_off(execution.values.len() - count);
                execution.values.push(PreparedValue::Array(elements.into()));
            }
            Step::FinishConstruction {
                proof_site,
                owner,
                case,
                positions,
            } => {
                let count = positions.len();
                let mut ordered = vec![None; count];
                for (position, value) in positions
                    .into_iter()
                    .zip(execution.values.drain(execution.values.len() - count..))
                {
                    if ordered[position].replace(value).is_some() {
                        return Err(UNSUPPORTED_BODY.into());
                    }
                }
                let fields: std::sync::Arc<[PreparedValue]> = ordered
                    .into_iter()
                    .collect::<Option<_>>()
                    .ok_or(UNSUPPORTED_BODY)?;
                if let Some(site) = proof_site {
                    self.check_construction_proof(&self.tree(site)?, site.node, &fields)?;
                }
                execution.values.push(match case {
                    Some(case) => PreparedValue::Sum { owner, case, fields },
                    None => PreparedValue::Record { owner, fields },
                });
            }
            Step::Leave { declaration, first_activation } => {
                // A recursive return leaves the outer activation active.
                if first_activation {
                    execution.active.remove(&declaration);
                }
                execution.chain.pop();
            }
            Step::Require(reason) => {
                if !execution.pop_bool() {
                    return Err(reason.into());
                }
            }
            Step::CrashGuard(reason) => {
                if execution.pop_bool() {
                    return Err(reason.into());
                }
            }
            Step::ApplyInteger(range, token) => {
                let right = execution.pop_integer();
                let left = execution.pop_integer();
                execution
                    .values
                    .push(PreparedValue::Integer(range.apply(&token, left, right)?));
            }
            Step::ApplyFloat { site, format, operation } => self.apply_float(site, format, operation, execution)?,
            Step::CoerceFloat { site, carrier } => self.coerce_float(site, &carrier, execution)?,
            Step::ReadArray => {
                let index = execution.pop_integer();
                let Some(PreparedValue::Array(elements)) = execution.values.pop() else {
                    return Err(UNSUPPORTED_BODY.into());
                };
                let element = usize::try_from(index)
                    .ok()
                    .and_then(|position| elements.get(position))
                    .ok_or("selected array read requires index < items.len")?;
                execution.values.push(element.clone());
            }
            Step::ArrayLength => {
                let Some(PreparedValue::Array(elements)) = execution.values.pop() else {
                    return Err(UNSUPPORTED_BODY.into());
                };
                execution
                    .values
                    .push(PreparedValue::Integer(elements.len().into()));
            }
            Step::ProjectFields(fields) => self.project_owned_fields(&fields, execution)?,
            Step::ProduceZero { site, carrier } => self.produce_zero(site, &carrier, execution)?,
            Step::Compare(token) => {
                let right = execution.pop_integer();
                let left = execution.pop_integer();
                execution.values.push(PreparedValue::Bool(match token.as_str() {
                    "==" => left == right,
                    "!=" => left != right,
                    "<" => left < right,
                    "<=" => left <= right,
                    ">" => left > right,
                    ">=" => left >= right,
                    _ => return Err(UNSUPPORTED_CONTRACT.into()),
                }));
            }
            Step::ShortCircuit { right, scope, stop_at } => {
                let left = execution.pop_bool();
                if left == stop_at {
                    execution.values.push(PreparedValue::Bool(left));
                } else {
                    execution
                        .steps
                        .push(Step::Evaluate(right, ValueType::Bool, scope));
                }
            }
            Step::Negate => {
                let value = execution.pop_bool();
                execution.values.push(PreparedValue::Bool(!value));
            }
            Step::Equal(expected) => {
                let right = execution.pop_bool();
                let left = execution.pop_bool();
                execution
                    .values
                    .push(PreparedValue::Bool((left == right) == expected));
            }
        }
        Ok(())
    }

    fn tree(
        &self,
        site: TreeNodeId,
    ) -> Result<TypedTree<'_>, EvaluationError> {
        self.trees
            .trees()
            .nth(site.file)
            .ok_or_else(|| "the evaluation request has no typed source file".into())
    }

    fn call_refusal(
        &self,
        reason: &str,
        chain: &[(TreeNodeId, bool)],
        cycle: &[&TypedCall],
    ) -> EvaluationError {
        EvaluationError::AtCall(self.call_diagnostic(reason, chain, cycle))
    }

    fn call_diagnostic(
        &self,
        reason: &str,
        chain: &[(TreeNodeId, bool)],
        cycle: &[&TypedCall],
    ) -> omega_typer::TypeError {
        let site = cycle
            .last()
            .map_or_else(|| chain.last().expect("visited call").0, |call| call.node);
        let tree = self.tree(site).expect("visited call file");
        let file = tree.resolved().file;
        let site = omega_typer::Site {
            package: file.package.into(),
            file: file.path.into(),
            span: file.span(site.node),
        };
        let mut error = crate::refusal(&site, reason);
        if chain.len() > 1 {
            error.message.push_str("\ncall chain: ");
            for (index, (occurrence, _)) in chain.iter().enumerate() {
                if index != 0 {
                    error.message.push_str(" -> ");
                }
                let tree = self.tree(*occurrence).expect("visited call file");
                let name = tree
                    .semantics_at(occurrence.node)
                    .find_map(|record| match record {
                        NodeSemantics::Call(call) => Some(call.machine.path.as_str()),
                        _ => None,
                    });
                error
                    .message
                    .push_str(name.unwrap_or("<indirect or unsupported call>"));
            }
        }
        if let Some(first) = cycle.first() {
            error.message.push_str("\nrecursive component: ");
            error.message.push_str(
                &first
                    .caller
                    .as_ref()
                    .expect("local call edge has a caller")
                    .path,
            );
            for call in cycle {
                error.message.push_str(" -> ");
                error.message.push_str(&call.machine.path);
            }
        }
        error
    }
}

fn selected_boolean_operation<'a>(
    tree: &'a TypedTree<'_>,
    node: usize,
) -> Result<&'a omega_typer::TypedOperation, EvaluationError> {
    let mut operation = None;
    for record in tree.semantics_at(node) {
        match record {
            NodeSemantics::Operation(selected) if selected.result == ValueType::Bool && operation.is_none() => operation = Some(selected),
            NodeSemantics::Obligation(o) if matches!(o.kind, ObligationKind::TotalCrashGuard) => {}
            NodeSemantics::Expression(_) => {}
            _ => return Err(UNSUPPORTED_CONTRACT.into()),
        }
    }
    operation.ok_or_else(|| UNSUPPORTED_CONTRACT.into())
}

fn is_aggregate(carrier: &ValueType) -> bool {
    matches!(
        carrier,
        ValueType::Array(..) | ValueType::Tuple(_) | ValueType::Nominal(_) | ValueType::Applied(..)
    )
}

fn checked_carrier_in_scope(
    tree: &TypedTree<'_>,
    node: usize,
    application: Option<&TypedCall>,
) -> Result<ValueType, EvaluationError> {
    let mut expression_type = None;
    let mut landing_type = None;
    for record in tree.semantics_at(node) {
        let (selected, ty) = match record {
            NodeSemantics::Expression(expression) => (&mut expression_type, &expression.ty),
            NodeSemantics::Landing(landing) => (&mut landing_type, &landing.ty),
            _ => continue,
        };
        if selected.replace(ty).is_some_and(|previous| previous != ty) {
            return Err(UNSUPPORTED_BODY.into());
        }
    }
    let carrier = landing_type.or(expression_type).ok_or(UNSUPPORTED_BODY)?;
    let specialized;
    let carrier = if let Some(call) = application {
        specialized = call.instantiate(carrier);
        &specialized
    } else {
        carrier
    };
    if carrier != &ValueType::Bool && !is_aggregate(carrier) && IntegerRange::for_type(carrier).is_none() && FloatFormat::for_type(carrier).is_none() {
        return Err(UNSUPPORTED_BODY.into());
    }
    Ok(carrier.clone())
}

fn landed_value(
    tree: &TypedTree<'_>,
    node: usize,
    carrier: &ValueType,
) -> Result<Option<BigInt>, EvaluationError> {
    let mut value = None;
    for record in tree.semantics_at(node) {
        if let NodeSemantics::Landing(landing) = record
            && &landing.ty == carrier
        {
            if !landing.value.is_integer() {
                return Err("the typed landing does not supply an integral value".into());
            }
            let integer = fits(landing.value.to_integer(), carrier).ok_or("the typed landing exceeds its carrier")?;
            if value.as_ref().is_some_and(|previous| previous != &integer) {
                return Err("conflicting integer landings at one typed occurrence".into());
            }
            value = Some(integer);
        }
    }
    if value.is_some() {
        check_landing(tree, node)?;
    }
    Ok(value)
}

fn check_landing(
    tree: &TypedTree<'_>,
    node: usize,
) -> Result<(), EvaluationError> {
    // A landing gives a value, not permission to skip the evaluation of a
    // block or call that happens to produce it.
    let file = tree.resolved().file;
    if (node..file.nodes[node].end).any(|child| {
        !matches!(file.nodes[child].kind, NodeKind::Literal | NodeKind::Paren | NodeKind::Binary | NodeKind::Unary)
            || tree.semantics_at(child).any(|record| match record {
                NodeSemantics::Expression(_) | NodeSemantics::Landing(_) => false,
                NodeSemantics::Operation(operation) => {
                    !matches!(operation.result, ValueType::Exact { .. })
                        || !operation
                            .operands
                            .iter()
                            .all(|operand| matches!(operand, ValueType::Exact { .. }))
                }
                _ => true,
            })
    }) {
        return Err(UNSUPPORTED_BODY.into());
    }
    Ok(())
}

fn selected_call<'a>(
    tree: &'a TypedTree<'_>,
    node: usize,
    use_site: CallUse,
) -> Result<Option<&'a TypedCall>, EvaluationError> {
    let mut selected = None;
    for record in tree.semantics_at(node) {
        if let NodeSemantics::Call(call) = record
            && (call.use_site != use_site || selected.replace(call).is_some_and(|previous| previous != call))
        {
            return Err("the request does not select one ordinary static invocation".into());
        }
    }
    Ok(selected)
}

fn selected_arithmetic<'a>(
    tree: &'a TypedTree<'_>,
    node: usize,
    carrier: &ValueType,
    application: Option<&TypedCall>,
) -> Result<(&'a str, ValueType), EvaluationError> {
    let instantiate = |ty: &ValueType| application.map_or_else(|| ty.clone(), |call| call.instantiate(ty));
    let right_carrier = |token: &str, operands: &[ValueType]| {
        let [left, right] = operands else { return Err(UNSUPPORTED_BODY) };
        let right = instantiate(right);
        let shift = token.starts_with("<<") || token.starts_with(">>");
        if &instantiate(left) != carrier || (&right != carrier && !(shift && matches!(right, ValueType::Integer { .. }))) {
            return Err(UNSUPPORTED_BODY);
        }
        Ok(right)
    };
    let mut operation = None;
    let mut covers_trap = false;
    let mut contract = None;
    for record in tree.semantics_at(node) {
        match record {
            NodeSemantics::Operation(selected) if &instantiate(&selected.result) == carrier => {
                let right = right_carrier(&selected.token, &selected.operands)?;
                if operation.replace((selected.token.as_str(), right)).is_some() {
                    return Err(UNSUPPORTED_BODY.into());
                }
            }
            NodeSemantics::Obligation(o)
                if matches!(
                    o.kind,
                    ObligationKind::CrashCoverage {
                        cause: omega_typer::CrashCause::Trap,
                        ..
                    }
                ) =>
            {
                covers_trap = true;
            }
            NodeSemantics::Obligation(obligation) => {
                let ObligationKind::OperationContract { token, operands } = &obligation.kind else {
                    return Err(UNSUPPORTED_BODY.into());
                };
                let right = right_carrier(token, operands)?;
                if contract.replace((token.as_str(), right)).is_some() {
                    return Err(UNSUPPORTED_BODY.into());
                }
            }
            NodeSemantics::Expression(_) | NodeSemantics::Landing(_) => {}
            _ => return Err(UNSUPPORTED_BODY.into()),
        }
    }
    operation
        .filter(|selection| Some(selection) == contract.as_ref() && (!covers_trap || selection.0.ends_with('!')))
        .ok_or_else(|| UNSUPPORTED_BODY.into())
}

fn fits(
    value: BigInt,
    carrier: &ValueType,
) -> Option<BigInt> {
    IntegerRange::for_type(carrier)?.fits(&value).then_some(value)
}

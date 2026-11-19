//! The evaluator executes immutable reads, owned constructions, exact landings
//! and admitted scalar operations. A local invocation checks every premise
//! before its value enters the prepared product.
mod admission;
mod statements;
pub(super) mod termination;
mod values;

use crate::PreparedValue;
use crate::constant_evaluation::ConstantEvaluation;
use crate::integer_arithmetic::IntegerRange;
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
}

pub(super) struct CallBindings(RefCell<Vec<BindingValue>>);

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
            kind: NodeKind::Param | NodeKind::BindingPattern,
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
    Enter(&'a TypedSignature),
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
    },
    Select {
        site: TreeNodeId,
        carrier: ValueType,
        scope: Scope<'a>,
    },
    Reject(EvaluationError),
    Push(PreparedValue),
    FinishArray(usize),
    FinishConstruction {
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
    ApplyInteger(IntegerRange, String),
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
                let result = if matches!(scope, Scope::Body(_)) && file.nodes[node].kind == NodeKind::Match {
                    if checked_carrier(&tree, node)? != carrier {
                        return Err(UNSUPPORTED_BODY.into());
                    }
                    self.schedule_boolean_dispatch(&tree, node, &carrier, scope.clone(), execution)
                } else if carrier == ValueType::Bool {
                    self.evaluate_boolean(&tree, site.node, &scope, execution)
                } else if is_aggregate(&carrier) {
                    self.schedule_aggregate(&tree, site.node, &carrier, &scope, execution)
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
                let call = selected_call(&tree, site.node, scope.call_use())?.ok_or(UNSUPPORTED_BODY)?;
                self.schedule_call(&tree, call, &scope, &carrier, execution)?;
            }
            Step::Enter(signature) => self.enter_call(signature, execution)?,
            Step::Block {
                site,
                statement,
                carrier,
                bindings,
            } => self.schedule_statement(site, statement, carrier, bindings, execution)?,
            Step::Bind { binder, carrier, bindings } => {
                let value = execution.values.pop().expect("visited binding initializer");
                bindings
                    .0
                    .borrow_mut()
                    .push(BindingValue { binder, carrier, value });
            }
            Step::Select { site, carrier, scope } => self.select_boolean_arm(site, carrier, scope, execution)?,
            Step::Reject(error) => return Err(error),
            Step::Push(value) => execution.values.push(value),
            Step::FinishArray(count) => {
                let elements = execution.values.split_off(execution.values.len() - count);
                execution.values.push(PreparedValue::Array(elements.into()));
            }
            Step::FinishConstruction { owner, case, positions } => {
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
                let fields = ordered
                    .into_iter()
                    .collect::<Option<_>>()
                    .ok_or(UNSUPPORTED_BODY)?;
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
        EvaluationError::AtCall(error)
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
    matches!(carrier, ValueType::Array(..) | ValueType::Nominal(_) | ValueType::Applied(..))
}

fn checked_carrier(
    tree: &TypedTree<'_>,
    node: usize,
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
    if carrier != &ValueType::Bool && !is_aggregate(carrier) && IntegerRange::for_type(carrier).is_none() {
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
    }
    Ok(value)
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
) -> Result<(&'a str, &'a ValueType), EvaluationError> {
    let operands_match = |token: &str, operands: &[ValueType]| {
        matches!(operands, [left, right] if left == carrier && (right == carrier
            || (token.starts_with("<<") || token.starts_with(">>")) && matches!(right, ValueType::Integer { .. })))
    };
    let mut operation = None;
    let mut covers_trap = false;
    let mut contract = None;
    for record in tree.semantics_at(node) {
        match record {
            NodeSemantics::Operation(selected) if operands_match(&selected.token, &selected.operands) && &selected.result == carrier => {
                if operation
                    .replace((selected.token.as_str(), &selected.operands[1]))
                    .is_some()
                {
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
                if !operands_match(token, operands) || contract.replace((token.as_str(), &operands[1])).is_some() {
                    return Err(UNSUPPORTED_BODY.into());
                }
            }
            NodeSemantics::Expression(_) | NodeSemantics::Landing(_) => {}
            _ => return Err(UNSUPPORTED_BODY.into()),
        }
    }
    operation
        .filter(|token| Some(*token) == contract && (!covers_trap || token.0.ends_with('!')))
        .ok_or_else(|| UNSUPPORTED_BODY.into())
}

fn fits(
    value: BigInt,
    carrier: &ValueType,
) -> Option<BigInt> {
    IntegerRange::for_type(carrier)?.fits(&value).then_some(value)
}

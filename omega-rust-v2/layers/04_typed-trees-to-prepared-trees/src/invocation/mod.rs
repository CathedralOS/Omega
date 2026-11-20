//! The evaluator executes immutable reads, owned constructions, exact landings
//! and admitted scalar operations. A local invocation checks every premise
//! before its value enters the prepared product.
use crate::evaluation::floating_point::FloatFormatEvaluation;
pub(super) mod admission;
mod collection_reads;
mod floating_point;
mod statements;
pub(crate) mod static_callbacks;
pub(super) mod termination;
mod values;

use crate::evaluation::constant_evaluation::ConstantEvaluation;
use crate::evaluation::integer_arithmetic::IntegerRange;
use crate::reasons::{PreparationReason, PreparationRefusal, RefusalCause};
use crate::{FloatFormat, FloatOperation, PreparedValue};
use num_bigint::BigInt;
use omega_resolved::{DeclarationId, Symbol, TreeNodeId};
use omega_syntax::NodeKind;
use omega_typed::{AppliedOperand, CallUse, NodeSemantics, ObligationKind, StaticValueRequest, TypedCall, TypedSignature, TypedTree, ValueType};
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

fn matches_source_occurrence(source: &omega_resolved::IndexedSource<'_>, node: TreeNodeId, site: &omega_typed::Site) -> bool {
    node.file == source.ordinal()
        && node.node < source.nodes.len()
        && source.package == site.package
        && source.path == site.file
        && source.span(node.node) == site.span
}

#[derive(Debug)]
pub(super) enum EvaluationError {
    Refused(RefusalCause, Option<TreeNodeId>),
    NeedsConstant { declaration: DeclarationId, reference: TreeNodeId },
    Located(PreparationRefusal),
    OpenTarget(PreparationRefusal),
}

impl EvaluationError {
    pub(crate) fn at_node(self, node: TreeNodeId) -> Self {
        match self {
            Self::Refused(cause, None) => Self::Refused(cause, Some(node)),
            error => error,
        }
    }
}

// Only a typed reason can refuse evaluation, so every owned refusal is in
// `PreparationReason::ALL`. Forwarded reasons name their owner explicitly.
impl From<PreparationReason> for EvaluationError {
    fn from(reason: PreparationReason) -> Self {
        Self::Refused(RefusalCause::Preparation(reason), None)
    }
}

impl From<RefusalCause> for EvaluationError {
    fn from(cause: RefusalCause) -> Self {
        Self::Refused(cause, None)
    }
}

pub(super) const UNSUPPORTED_BODY: PreparationReason = PreparationReason::UnsupportedBody;
const UNSUPPORTED_ARGUMENT: PreparationReason = PreparationReason::UnsupportedArgument;
const UNSUPPORTED_CONTRACT: PreparationReason = PreparationReason::UnsupportedContract;

struct BindingValue {
    binder: usize,
    carrier: ValueType,
    value: PreparedValue,
    writable: bool,
}

#[derive(Default)]
pub(super) struct CallBindings {
    values: RefCell<Vec<BindingValue>>,
    application: Option<Rc<TypedCall>>,
    callback_bindings: Rc<[static_callbacks::CallbackBinding]>,
}

#[derive(Clone)]
pub(super) enum Scope<'a> {
    Initializer {
        owner_arguments: &'a [ValueType],
        owner_scopes: &'a [(DeclarationId, usize)],
        bindings: Rc<CallBindings>,
        in_argument: bool,
    },
    Body(Rc<CallBindings>),
    Fact(Rc<CallBindings>),
}

/// L3's binder partition of a specialized request's closed argument tuple.
#[derive(Clone, Copy)]
struct InitializerArguments<'a> {
    scopes: &'a [(DeclarationId, usize)],
    values: &'a [ValueType],
}

impl<'a> InitializerArguments<'a> {
    fn bindings(self) -> impl Iterator<Item = (&'a DeclarationId, &'a [ValueType])> {
        self.scopes.iter().scan(self.values, |rest, (owner, arity)| {
            let (arguments, tail) = rest.split_at(*arity);
            *rest = tail;
            Some((owner, arguments))
        })
    }

    fn instantiate(self, value: &ValueType) -> ValueType {
        self.bindings().fold(value.clone(), |value, (owner, args)| value.substitute(owner, args))
    }
}

impl Scope<'_> {
    fn bindings(&self) -> &Rc<CallBindings> {
        match self {
            Self::Initializer { bindings, .. } | Self::Body(bindings) | Self::Fact(bindings) => bindings,
        }
    }

    fn application(&self) -> Option<&TypedCall> {
        match self {
            Self::Body(bindings) | Self::Fact(bindings) => bindings.application.as_deref(),
            Self::Initializer { .. } => None,
        }
    }

    pub(super) fn instantiate(
        &self,
        carrier: &ValueType,
    ) -> ValueType {
        match self {
            Self::Initializer {
                owner_arguments, owner_scopes, ..
            } => InitializerArguments { scopes: owner_scopes, values: owner_arguments }.instantiate(carrier),
            Self::Body(_) | Self::Fact(_) => self
                .application()
                .map_or_else(|| carrier.clone(), |call| call.instantiate(carrier)),
        }
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
        let Symbol::Binding {
            binder_node,
            kind: NodeKind::Param | NodeKind::BindingPattern | NodeKind::FieldPattern,
            ..
        } = tree.resolved().symbol(node)?
        else {
            return None;
        };
        self.bindings()
            .values
            .borrow()
            .iter()
            .rev()
            .find(|binding| binding.binder == *binder_node && &binding.carrier == carrier)
            .map(|binding| binding.value.clone())
    }
}

struct OwnedFieldRead {
    site: TreeNodeId,
    field: DeclarationId,
    result: ValueType,
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
        callback_bindings: Rc<[static_callbacks::CallbackBinding]>,
    },
    EnterCheckedCallback {
        request: static_callbacks::CallableUse,
        signature: &'a TypedSignature,
        application: Option<Rc<TypedCall>>,
        callback_bindings: Rc<[static_callbacks::CallbackBinding]>,
    },
    CheckCallableGuarantee {
        request: static_callbacks::CallableUse,
        operands: Vec<PreparedValue>,
    },
    Block {
        site: TreeNodeId,
        statement: usize,
        carrier: ValueType,
        scope: Scope<'a>,
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
    Discard,
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
    FinishZeroArray { site: TreeNodeId, count: usize },
    FinishConstruction {
        proof: Option<(&'a omega_typed::TypingObligation, ValueType)>,
        owner: DeclarationId,
        case: Option<DeclarationId>,
        positions: Vec<usize>,
    },
    Leave(TreeNodeId),
    Require(PreparationReason),
    CrashGuard(PreparationReason),
    CheckInvocationProof(admission::ProofRequest<'a>, Rc<CallBindings>),
    ApplyInteger { initializer_site: Option<TreeNodeId>, range: IntegerRange, token: String },
    ApplyFloat {
        site: TreeNodeId,
        format: FloatFormat,
        operation: FloatOperation,
    },
    CoerceFloat {
        site: TreeNodeId,
        carrier: ValueType,
    },
    ProjectFields { receiver: ValueType, fields: Vec<OwnedFieldRead> },
    ReadArray { initializer_site: Option<TreeNodeId> },
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
    active: termination::ActiveApplications,
    checked_aggregates: HashSet<ValueType>,
    chain: Vec<TreeNodeId>,
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
        let tree = self.tree(request.node)?;
        let source = tree.resolved().file;
        if !matches_source_occurrence(source, request.node, &request.expression) {
            return Err(PreparationReason::RequestSiteMismatch.into());
        }
        let (owner_arguments, owner_scopes) = match owner_arguments {
            Some(arguments) => {
                let scopes = self
                    .trees
                    .static_request_scopes
                    .get(&request.node)
                    .ok_or(UNSUPPORTED_ARGUMENT)?;
                if scopes.iter().map(|(_, arity)| arity).sum::<usize>() != arguments.len() || arguments.iter().any(ValueType::contains_parameter) {
                    return Err(UNSUPPORTED_ARGUMENT.into());
                }
                (arguments, scopes.as_slice())
            }
            None => (&[][..], &[][..]),
        };
        let scope = Scope::Initializer {
            owner_arguments,
            owner_scopes,
            bindings: Rc::new(CallBindings::default()),
            in_argument: false,
        };
        self.execute(request.node, &scope.instantiate(&request.carrier), &scope)
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
            // A block or match arm in an initializer operand is still computing
            // that operand. Entering a called machine starts a separate body scope.
            let in_argument = match &step {
                Step::Evaluate(_, _, scope) | Step::Block { scope, .. } | Step::Select { scope, .. } => {
                    matches!(scope, Scope::Initializer { in_argument: true, .. })
                }
                _ => false,
            };
            if let Err(error) = self.evaluate_step(step, &mut execution) {
                let error = match error {
                    EvaluationError::Refused(RefusalCause::Preparation(UNSUPPORTED_BODY), None) if in_argument => UNSUPPORTED_ARGUMENT.into(),
                    other => other,
                };
                return Err(match error {
                    EvaluationError::Refused(reason, Some(node)) => {
                        EvaluationError::Located(self.diagnostic_at(&self.visited_site(node), reason, &execution.chain, &[]))
                    }
                    EvaluationError::Refused(reason, None) if !execution.chain.is_empty() => {
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
                if file.nodes[node].kind == NodeKind::Match {
                    if checked_carrier_in_scope(&tree, node, &scope)? != carrier {
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
                }?;
            }
            Step::Invoke(site, carrier, mut scope) => {
                let tree = self.tree(site)?;
                execution.chain.push(site);
                // Further evaluation in the initializer frame computes an operand,
                // regardless of its carrier. Contracts and bodies get their own frames.
                if let Scope::Initializer { in_argument, .. } = &mut scope {
                    *in_argument = true;
                }
                let Some(call) = selected_call(&tree, site.node, scope.call_use())? else {
                    return self.schedule_static_callback(&tree, site, &scope, &carrier, execution);
                };
                if let Some(query) = call.layout_query {
                    if omega_toolchain::core_interface().layout_query(&call.machine) != Some(query)
                        || carrier != collection_reads::COUNT_CARRIER
                        || scope.instantiate(&call.result) != carrier
                        || !call.operands.is_empty()
                        || !call.enclosing_arguments.is_empty()
                        || call.evidence.is_some()
                        || call.acknowledgements.suspend
                        || call.acknowledgements.block
                    {
                        return Err(PreparationReason::NotOneLayoutQuery.into());
                    }

                    let [subject] = call.type_arguments.as_slice() else {
                        return Err(PreparationReason::NotOneLayoutQuery.into());
                    };
                    let subject = scope.instantiate(subject);
                    if subject.contains_parameter() {
                        return Err(PreparationReason::LayoutQueryNeedsClosedType.into());
                    }
                    if self.boundary_slots.iter().any(|slot| {
                        slot.requirement.declaration == call.machine
                            && slot.requirement.trait_arguments.is_empty()
                            && (slot.arguments.is_empty() || slot.arguments.as_slice() == std::slice::from_ref(&subject))
                    }) {
                        return Err(PreparationReason::UnselectedBoundarySlot.into());
                    }
                    let target = self
                        .target
                        .ok_or_else(|| EvaluationError::OpenTarget(self.call_diagnostic(PreparationReason::LayoutQueryNeedsTarget, &execution.chain, &[])))?;
                    let (value, observation) = target
                        .observe_layout(query, &subject)
                        .map_err(RefusalCause::TargetLayout)?;
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
                callback_bindings,
            } => self.enter_call(call, signature, requirement, application, callback_bindings, execution)?,
            Step::EnterCheckedCallback {
                request,
                signature,
                application,
                callback_bindings,
            } => self.enter_checked_callback(request, signature, application, callback_bindings, execution)?,
            Step::CheckCallableGuarantee { request, operands } => {
                let result = execution.values.last().ok_or(UNSUPPORTED_BODY)?;
                self.admit_callable_use(&request, &operands, Some(result))?;
            }
            Step::CheckInvocationProof(request, arguments) => self.admit_proof(request, &arguments, &execution.values)?,
            Step::Block {
                site,
                statement,
                carrier,
                scope,
            } => self.schedule_statement(site, statement, carrier, scope, execution)?,
            Step::Bind {
                binder,
                carrier,
                bindings,
                writable,
            } => {
                let value = execution.values.pop().expect("visited binding initializer");
                bindings.values.borrow_mut().push(BindingValue {
                    binder,
                    carrier,
                    value,
                    writable,
                });
            }
            Step::Store { local, bindings } => {
                let value = execution.values.pop().expect("visited assignment value");
                bindings.values.borrow_mut()[local].value = value;
            }
            Step::Discard => {
                execution.values.pop().expect("visited discarded operand");
            }
            Step::Select {
                site,
                carrier,
                subject_type,
                scope,
            } => self.select_arm(site, carrier, &subject_type, scope, execution)?,
            Step::RestoreBindings { count, bindings } => bindings.values.borrow_mut().truncate(count),
            Step::Reject(error) => return Err(error),
            Step::Push(value) => execution.values.push(value),
            Step::FinishZeroArray { site, count } => {
                let element = execution.values.pop().expect("visited zero array element");
                let mut elements = Vec::new();
                elements
                    .try_reserve_exact(count)
                    .map_err(|_| EvaluationError::from(PreparationReason::ZeroArrayNeedsStorage).at_node(site))?;
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
            Step::FinishConstruction { proof, owner, case, positions } => {
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
                if let Some((obligation, carrier)) = proof {
                    self.check_construction_proof(obligation, &carrier, &fields)?;
                }
                execution.values.push(match case {
                    Some(case) => PreparedValue::Sum { owner, case, fields },
                    None => PreparedValue::Record { owner, fields },
                });
            }
            Step::Leave(declaration) => {
                execution.active.leave(declaration);
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
            Step::ApplyInteger { initializer_site, range, token } => {
                let right = execution.pop_integer();
                let left = execution.pop_integer();
                let value = range.apply(&token, left, right).map_err(|cause| {
                    let error = EvaluationError::from(cause);
                    // Operand evaluation can fail before a pending call enters its body.
                    match initializer_site {
                        Some(site) => error.at_node(site),
                        None => error,
                    }
                })?;
                execution.values.push(PreparedValue::Integer(value));
            }
            Step::ApplyFloat { site, format, operation } => self.apply_float(site, format, operation, execution)?,
            Step::CoerceFloat { site, carrier } => self.coerce_float(site, &carrier, execution)?,
            Step::ReadArray { initializer_site } => {
                let index = execution.pop_integer();
                let Some(PreparedValue::Array(elements)) = execution.values.pop() else {
                    return Err(UNSUPPORTED_BODY.into());
                };
                let element = usize::try_from(index)
                    .ok()
                    .and_then(|position| elements.get(position))
                    .ok_or_else(|| {
                        let error = EvaluationError::from(PreparationReason::ArrayIndexOutOfBounds);
                        match initializer_site {
                            Some(site) => error.at_node(site),
                            None => error,
                        }
                    })?;
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
            Step::ProjectFields { receiver, fields } => self.project_owned_fields(&receiver, &fields, execution)?,
            Step::ProduceZero { site, carrier } => self.produce_zero(site, &carrier, execution).map_err(|error| error.at_node(site))?,
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
        let tree = self.trees.trees().nth(site.file).ok_or(PreparationReason::RequestWithoutTypedFile)?;
        if site.node >= tree.resolved().file.nodes.len() {
            return Err(PreparationReason::RequestWithoutTypedNode.into());
        }
        Ok(tree)
    }

    pub(crate) fn visited_site(&self, node: TreeNodeId) -> omega_typed::Site {
        let tree = self.tree(node).expect("evaluation visited this file");
        omega_typed::site_at(tree.resolved().file, node.node)
    }

    fn call_refusal(
        &self,
        reason: impl Into<RefusalCause>,
        chain: &[TreeNodeId],
        cycle: &[&TypedCall],
    ) -> EvaluationError {
        EvaluationError::Located(self.call_diagnostic(reason, chain, cycle))
    }

    fn call_diagnostic(
        &self,
        reason: impl Into<RefusalCause>,
        chain: &[TreeNodeId],
        cycle: &[&TypedCall],
    ) -> PreparationRefusal {
        if let Some(site) = self.cycle_site(cycle) {
            return self.diagnostic_at(site, reason, chain, cycle);
        }
        let site = self.visited_site(*chain.last().expect("visited call"));
        self.diagnostic_at(&site, reason, chain, cycle)
    }

    fn diagnostic_at(
        &self,
        site: &omega_typed::Site,
        reason: impl Into<RefusalCause>,
        chain: &[TreeNodeId],
        cycle: &[&TypedCall],
    ) -> PreparationRefusal {
        let mut refusal = crate::evaluation::refusal(site, reason);
        let error = &mut refusal.diagnostic;
        if chain.len() > 1 {
            error.message.push_str("\ncall chain: ");
            for (index, occurrence) in chain.iter().enumerate() {
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
        refusal
    }
}

fn selected_boolean_operation<'a>(
    tree: &'a TypedTree<'_>,
    node: usize,
) -> Result<&'a omega_typed::TypedOperation, EvaluationError> {
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
    scope: &Scope<'_>,
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
    let carrier = scope.instantiate(landing_type.or(expression_type).ok_or(UNSUPPORTED_BODY)?);
    if !matches!(carrier, ValueType::Bool | ValueType::NoResult)
        && !is_aggregate(&carrier)
        && IntegerRange::for_type(&carrier).is_none()
        && FloatFormat::for_type(&carrier).is_none()
    {
        return Err(UNSUPPORTED_BODY.into());
    }
    Ok(carrier)
}

fn landed_value(
    tree: &TypedTree<'_>,
    node: usize,
    carrier: &ValueType,
) -> Result<Option<BigInt>, EvaluationError> {
    let at_literal = |reason: PreparationReason| EvaluationError::from(reason).at_node(tree.resolved().file.node_id(node));
    let mut value = None;
    for record in tree.semantics_at(node) {
        if let NodeSemantics::Landing(landing) = record
            && &landing.ty == carrier
        {
            if !landing.value.is_integer() {
                return Err(at_literal(PreparationReason::LandingNotIntegral));
            }
            let integer = fits(landing.value.to_integer(), carrier).ok_or_else(|| at_literal(PreparationReason::LandingOutsideCarrier))?;
            if value.as_ref().is_some_and(|previous| previous != &integer) {
                return Err(at_literal(PreparationReason::ConflictingIntegerLandings));
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
            return Err(PreparationReason::NotOneOrdinaryInvocation.into());
        }
    }
    Ok(selected)
}

fn selected_arithmetic<'a>(
    tree: &'a TypedTree<'_>,
    node: usize,
    carrier: &ValueType,
    instantiate: impl Fn(&ValueType) -> ValueType,
) -> Result<(&'a str, ValueType), EvaluationError> {
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
                        cause: omega_typed::CrashCause::Trap,
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

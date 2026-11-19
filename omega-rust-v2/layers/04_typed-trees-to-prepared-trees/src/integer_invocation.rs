//! The integer fragment executes immutable scalar reads, exact landings, and
//! admitted integer arithmetic. A local invocation must close every premise before a
//! value can enter the prepared product.
use crate::integer_arithmetic::IntegerRange;
use crate::integer_constants::IntegerEvaluation;
use num_bigint::BigInt;
use omega_parser::NodeKind;
use omega_resolver::{DeclarationId, Symbol};
use omega_typer::{AppliedOperand, CallUse, NodeSemantics, ObligationKind, StaticIntegerRequest, TypedCall, TypedTree, ValueType};
use std::collections::HashSet;

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

const UNSUPPORTED_BODY: &str = "the selected body is outside the total integer fragment";
const UNSUPPORTED_ARGUMENT: &str = "the static call has an argument without a supported closed value";
const UNSUPPORTED_CONTRACT: &str = "the selected contract needs proof outside the total integer fragment";

struct ParameterValue<'a> {
    binder: usize,
    carrier: &'a ValueType,
    value: BigInt,
}

enum CallResult<'a> {
    Value(BigInt),
    Tail {
        tree: TypedTree<'a>,
        node: usize,
        arguments: Vec<ParameterValue<'a>>,
    },
}

enum Scope<'a> {
    Initializer { root: usize, owner_arguments: Option<&'a [ValueType]> },
    CallArgument,
    Body(&'a [ParameterValue<'a>]),
}

impl IntegerEvaluation<'_, '_> {
    pub(super) fn evaluate_request(
        &self,
        request: &StaticIntegerRequest,
        owner_arguments: Option<&[ValueType]>,
    ) -> Result<BigInt, EvaluationError> {
        IntegerRange::for_type(&request.carrier).ok_or("the requested carrier is outside the bounded integer fragment")?;
        let tree = self
            .trees
            .trees()
            .nth(request.node.file)
            .ok_or("the evaluation request has no typed source file")?;
        self.expression(
            &tree,
            request.node.node,
            &request.carrier,
            &Scope::Initializer {
                root: request.node.node,
                owner_arguments,
            },
        )
    }

    fn expression(
        &self,
        tree: &TypedTree<'_>,
        node: usize,
        carrier: &ValueType,
        scope: &Scope<'_>,
    ) -> Result<BigInt, EvaluationError> {
        enum Step<'tree> {
            Operand(usize),
            Apply(&'tree str),
        }
        let range = IntegerRange::for_type(carrier).ok_or(UNSUPPORTED_BODY)?;
        let file = tree.resolved().file;
        let mut steps = vec![Step::Operand(node)];
        let mut values = Vec::new();
        while let Some(step) = steps.pop() {
            match step {
                Step::Operand(node) => {
                    if let Some(value) = landed_value(tree, node, carrier)? {
                        values.push(value);
                        continue;
                    }
                    let node = file.ungroup(node);
                    if file.nodes[node].kind != NodeKind::Binary {
                        values.push(self.term(tree, node, carrier, scope)?);
                        continue;
                    }
                    let [left, right] = *file.nodes[node].children.as_slice() else {
                        return Err(UNSUPPORTED_BODY.into());
                    };
                    let token = selected_arithmetic(tree, node, carrier)?;
                    steps.extend([Step::Apply(token), Step::Operand(right), Step::Operand(left)]);
                }
                Step::Apply(token) => {
                    let right = values.pop().expect("visited right operand");
                    let left = values.pop().expect("visited left operand");
                    values.push(range.apply(token, left, right)?);
                }
            }
        }
        Ok(values.pop().expect("visited root operand"))
    }

    fn term(
        &self,
        tree: &TypedTree<'_>,
        node: usize,
        carrier: &ValueType,
        scope: &Scope<'_>,
    ) -> Result<BigInt, EvaluationError> {
        if let Scope::Initializer { root, owner_arguments } = scope
            && let Some(call) = selected_call(tree, tree.resolved().file.ungroup(node), CallUse::StaticArgument)?
        {
            return self.invoke(tree, call, *root, *owner_arguments, carrier);
        }
        self.atom(tree, node, carrier, scope)
    }

    fn invoke(
        &self,
        request_tree: &TypedTree<'_>,
        call: &TypedCall,
        root: usize,
        owner_arguments: Option<&[ValueType]>,
        carrier: &ValueType,
    ) -> Result<BigInt, EvaluationError> {
        let mut visited = HashSet::new();
        let scope = Scope::Initializer { root, owner_arguments };
        let mut result = self.run_call(request_tree, call, &scope, carrier, &mut visited)?;
        let mut chain = vec![call.node];
        loop {
            let (tree, node, arguments) = match result {
                CallResult::Value(value) => return Ok(value),
                CallResult::Tail { tree, node, arguments } => (tree, node, arguments),
            };
            let file = tree.resolved().file;
            chain.push(file.node_id(node));
            let next = selected_call(&tree, node, CallUse::Execution)
                .and_then(|helper| self.run_call(&tree, helper.ok_or(UNSUPPORTED_BODY)?, &Scope::Body(&arguments), carrier, &mut visited));
            result = next.map_err(|error| match error {
                EvaluationError::Refused(reason) => self.tail_refusal(file, node, reason, &chain),
                other => other,
            })?;
        }
    }

    fn tail_refusal(
        &self,
        file: &omega_resolver::IndexedSource<'_>,
        node: usize,
        reason: &str,
        chain: &[omega_resolver::TreeNodeId],
    ) -> EvaluationError {
        let site = omega_typer::Site {
            package: file.package.into(),
            file: file.path.into(),
            span: file.span(node),
        };
        let mut error = crate::refusal(&site, reason);
        error.message.push_str("\ncall chain: ");
        for (index, occurrence) in chain.iter().enumerate() {
            if index != 0 {
                error.message.push_str(" -> ");
            }
            let tree = self
                .trees
                .trees()
                .nth(occurrence.file)
                .expect("visited call file");
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
        EvaluationError::AtCall(error)
    }

    fn run_call<'a>(
        &'a self,
        request_tree: &TypedTree<'_>,
        call: &TypedCall,
        scope: &Scope<'_>,
        carrier: &ValueType,
        visited: &mut HashSet<omega_resolver::TreeNodeId>,
    ) -> Result<CallResult<'a>, EvaluationError> {
        if call.machine.kind != NodeKind::Machine
            || call.evidence.is_some()
            || !call.type_arguments.is_empty()
            || !call.enclosing_arguments.is_empty()
            || call.acknowledgements.suspend
            || call.acknowledgements.block
            || &call.result != carrier
        {
            return Err("the request does not select one ordinary integer invocation".into());
        }

        let signature = self
            .signature(&call.machine)
            .ok_or("the selected declaration has no typed signature")?;
        let body_tree = self
            .trees
            .trees()
            .nth(signature.node.file)
            .ok_or("the selected declaration has no local body")?;
        let declaration = signature.node.node;
        let resolved = body_tree.resolved();
        let file = resolved.file;
        if resolved.declaration_at(declaration) != Some(&call.machine) {
            return Err("the selected declaration has no local body".into());
        }
        if file.nodes[declaration].has_public_modifier() && !signature.terminates {
            return Err("the selected public machine does not declare termination".into());
        }
        if signature.has_guarantee
            || signature.satisfies.is_some()
            || signature.reach.as_ref().is_some_and(|reach| !reach.is_empty())
            || signature
                .crash_ceiling
                .as_ref()
                .is_some_and(|crashes| !crashes.is_empty())
            || signature.pause.suspend == Some(true)
            || signature.pause.block == Some(true)
            || &signature.result != carrier
            || !signature.multiplicity_bounds.is_empty()
            || file.child(declaration, NodeKind::GenericParams).is_some()
            || file.child(declaration, NodeKind::Hypothesis).is_some()
            || file.nodes[declaration].children.iter().any(|&node| {
                file.nodes[node].kind == NodeKind::Terminates && file.nodes[node].termination_clause() != Some(omega_parser::TerminationClause::Bare)
            })
        {
            return Err(UNSUPPORTED_CONTRACT.into());
        }

        let parameters = file
            .child(declaration, NodeKind::Params)
            .map_or(&[][..], |list| file.nodes[list].children.as_slice());
        if signature.parameters.len() != call.operands.len() || parameters.len() != call.operands.len() {
            return Err("the typed call and selected parameters disagree".into());
        }
        if !visited.insert(signature.node) {
            return Err("a cyclic static invocation requires a checked termination proof".into());
        }
        let arguments: Vec<_> = parameters
            .iter()
            .zip(&signature.parameters)
            .zip(&call.operands)
            .map(|((&binder, (_, carrier)), operand)| {
                IntegerRange::for_type(carrier).ok_or("a selected parameter is outside the bounded integer fragment")?;
                let value = self.argument(request_tree, operand, carrier, scope)?;
                let value = fits(value, carrier).ok_or("a closed argument is outside its selected carrier")?;
                Ok(ParameterValue { binder, carrier, value })
            })
            .collect::<Result<_, EvaluationError>>()?;

        let bound = Scope::Body(&arguments);
        let mut requirements = file.nodes[declaration]
            .children
            .iter()
            .copied()
            .filter(|&node| file.nodes[node].kind == NodeKind::Requires)
            .peekable();
        if signature.requires != requirements.peek().is_some() {
            return Err(UNSUPPORTED_CONTRACT.into());
        }
        for clause in requirements {
            let [predicate] = *file.nodes[clause].children.as_slice() else {
                return Err(UNSUPPORTED_CONTRACT.into());
            };
            if !self.integer_precondition(&body_tree, predicate, &bound)? {
                return Err("the selected integer precondition is false for the supplied arguments".into());
            }
        }

        let body = file
            .child(declaration, NodeKind::Body)
            .ok_or(UNSUPPORTED_BODY)?;
        let [tail] = *file.nodes[body].children.as_slice() else {
            return Err(UNSUPPORTED_BODY.into());
        };
        let [returned] = *file.nodes[tail].children.as_slice() else {
            return Err(UNSUPPORTED_BODY.into());
        };
        if file.nodes[tail].kind != NodeKind::Tail {
            return Err(UNSUPPORTED_BODY.into());
        }
        let node = file.ungroup(returned);
        if file.nodes[node].kind == NodeKind::Call {
            // Only the direct return transfers control. Its integer operands and
            // all preconditions are checked in the environments that own them.
            return Ok(CallResult::Tail {
                tree: body_tree,
                node,
                arguments,
            });
        }
        self.expression(&body_tree, returned, carrier, &bound)
            .map(CallResult::Value)
    }

    fn integer_precondition(
        &self,
        tree: &TypedTree<'_>,
        predicate: usize,
        scope: &Scope<'_>,
    ) -> Result<bool, EvaluationError> {
        let file = tree.resolved().file;
        let node = file.ungroup(predicate);
        let [left, right] = *file.nodes[node].children.as_slice() else {
            return Err(UNSUPPORTED_CONTRACT.into());
        };
        if file.nodes[node].kind != NodeKind::Binary {
            return Err(UNSUPPORTED_CONTRACT.into());
        }
        let mut operation = None;
        for record in tree.semantics_at(node) {
            match record {
                NodeSemantics::Operation(selected) if selected.result == ValueType::Bool && operation.is_none() => operation = Some(selected),
                NodeSemantics::Expression(_) => {}
                _ => return Err(UNSUPPORTED_CONTRACT.into()),
            }
        }
        let selected = operation.ok_or(UNSUPPORTED_CONTRACT)?;
        let [left_type, right_type] = selected.operands.as_slice() else {
            return Err(UNSUPPORTED_CONTRACT.into());
        };
        if IntegerRange::for_type(left_type).is_none() || IntegerRange::for_type(right_type).is_none() {
            return Err(UNSUPPORTED_CONTRACT.into());
        }
        // Each operand has its own selected carrier. Compare mathematical
        // values directly even when width or signedness differs.
        let left = self.expression(tree, left, left_type, scope)?;
        let right = self.expression(tree, right, right_type, scope)?;
        Ok(match selected.token.as_str() {
            "==" => left == right,
            "!=" => left != right,
            "<" => left < right,
            "<=" => left <= right,
            ">" => left > right,
            ">=" => left >= right,
            _ => return Err(UNSUPPORTED_CONTRACT.into()),
        })
    }

    fn argument(
        &self,
        tree: &TypedTree<'_>,
        operand: &AppliedOperand,
        carrier: &ValueType,
        scope: &Scope<'_>,
    ) -> Result<BigInt, EvaluationError> {
        let file = tree.resolved().file;
        if operand.node.file != file.node_id(0).file || operand.callable.is_some() {
            return Err("the static call does not have a local scalar operand".into());
        }
        if let Scope::Body(_) = scope {
            return self.expression(tree, operand.node.node, carrier, scope);
        }
        match &operand.subject {
            ValueType::StaticInteger { value, carrier: selected } if selected.as_ref() == carrier => Ok(value.clone()),
            ValueType::Parameter { owner, index: 0 } => {
                let Scope::Initializer {
                    root,
                    owner_arguments: Some([ValueType::StaticInteger { value, carrier: selected }]),
                } = scope
                else {
                    return Err(UNSUPPORTED_ARGUMENT.into());
                };
                let resolved = tree.resolved();
                let (node, declaration) = file
                    .ancestors(*root)
                    .find_map(|node| {
                        resolved
                            .declaration_at(node)
                            .map(|declaration| (node, declaration))
                    })
                    .ok_or("the specialization has no owning machine")?;
                if declaration != owner.as_ref()
                    || declaration.kind != NodeKind::Machine
                    || file
                        .generic_parameters(node)
                        .map(|(_, _, kind)| kind)
                        .ne([NodeKind::ConstParam])
                    || selected.as_ref() != carrier
                {
                    return Err("the specialization does not select its owner's single integer binder".into());
                }
                Ok(value.clone())
            }
            _ => self
                .expression(tree, operand.node.node, carrier, &Scope::CallArgument)
                .map_err(|error| match error {
                    EvaluationError::Refused(UNSUPPORTED_BODY) => UNSUPPORTED_ARGUMENT.into(),
                    error => error,
                }),
        }
    }

    fn atom(
        &self,
        tree: &TypedTree<'_>,
        node: usize,
        carrier: &ValueType,
        scope: &Scope<'_>,
    ) -> Result<BigInt, EvaluationError> {
        if let Some(value) = landed_value(tree, node, carrier)? {
            return Ok(value);
        }
        let resolved = tree.resolved();
        let node = resolved.file.ungroup(node);
        let mut selected_constant = None;
        for record in tree.semantics_at(node) {
            match record {
                NodeSemantics::Constant(declaration) => selected_constant = Some(declaration),
                NodeSemantics::Expression(_) | NodeSemantics::Landing(_) => {}
                _ => return Err(UNSUPPORTED_BODY.into()),
            }
        }
        if let Some(declaration) = selected_constant {
            let Some(constant) = self.definitions.get(declaration) else {
                return Err("the selected constant has no local typed initializer".into());
            };
            if &constant.initializer.carrier != carrier {
                return Err("the selected constant has a different carrier".into());
            }
            let value = self
                .values
                .get(declaration)
                .ok_or_else(|| EvaluationError::NeedsConstant(declaration.clone()))?;
            return fits(value.clone(), carrier).ok_or_else(|| "the frozen constant exceeds its carrier".into());
        }
        if let Scope::Body(arguments) = scope
            && let Some(Symbol::Binding {
                binder_node,
                kind: NodeKind::Param,
                ..
            }) = resolved.symbol(node)
            && let Some(parameter) = arguments
                .iter()
                .find(|parameter| parameter.binder == *binder_node && parameter.carrier == carrier)
        {
            return Ok(parameter.value.clone());
        }
        Err(UNSUPPORTED_BODY.into())
    }
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
            return Err("the request does not select one ordinary integer invocation".into());
        }
    }
    if use_site == CallUse::Execution {
        let call = selected.ok_or(UNSUPPORTED_BODY)?;
        for record in tree.semantics_at(node) {
            match record {
                NodeSemantics::Call(_) | NodeSemantics::Expression(_) => {}
                NodeSemantics::Obligation(obligation) if matches!(&obligation.kind, ObligationKind::CallPrecondition(target) if target == &call.machine) => {}
                _ => return Err(UNSUPPORTED_BODY.into()),
            }
        }
    }
    Ok(selected)
}

fn selected_arithmetic<'a>(
    tree: &'a TypedTree<'_>,
    node: usize,
    carrier: &ValueType,
) -> Result<&'a str, EvaluationError> {
    let same_carrier = |operands: &[ValueType]| matches!(operands, [left, right] if left == carrier && right == carrier);
    let mut operation = None;
    let mut contract = None;
    for record in tree.semantics_at(node) {
        match record {
            NodeSemantics::Operation(selected) if same_carrier(&selected.operands) && &selected.result == carrier => {
                if operation.replace(selected.token.as_str()).is_some() {
                    return Err(UNSUPPORTED_BODY.into());
                }
            }
            NodeSemantics::Obligation(obligation) => {
                let ObligationKind::OperationContract { token, operands } = &obligation.kind else {
                    return Err(UNSUPPORTED_BODY.into());
                };
                if !same_carrier(operands) || contract.replace(token.as_str()).is_some() {
                    return Err(UNSUPPORTED_BODY.into());
                }
            }
            NodeSemantics::Expression(_) | NodeSemantics::Landing(_) => {}
            _ => return Err(UNSUPPORTED_BODY.into()),
        }
    }
    operation
        .filter(|token| Some(*token) == contract)
        .ok_or_else(|| UNSUPPORTED_BODY.into())
}

fn fits(
    value: BigInt,
    carrier: &ValueType,
) -> Option<BigInt> {
    IntegerRange::for_type(carrier)?.fits(&value).then_some(value)
}

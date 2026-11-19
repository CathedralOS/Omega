use super::*;
use crate::TargetObservation;
use omega_typer::{Site, TypedOperation};

impl ConstantEvaluation<'_, '_> {
    pub(super) fn evaluate_float<'a>(
        &self,
        tree: &TypedTree<'_>,
        node: usize,
        carrier: &ValueType,
        scope: &Scope<'a>,
        execution: &mut Execution<'a>,
    ) -> Result<(), EvaluationError> {
        let format = FloatFormat::for_type(carrier).ok_or(UNSUPPORTED_BODY)?;
        let file = tree.resolved().file;
        let mut landed = None;
        for record in tree.semantics_at(node) {
            if let NodeSemantics::Landing(landing) = record
                && &landing.ty == carrier
            {
                if landed.is_some_and(|previous| previous != &landing.value) {
                    return Err("conflicting floating landings at one typed occurrence".into());
                }
                landed = Some(&landing.value);
            }
        }
        if let Some(value) = landed {
            check_landing(tree, node)?;
            execution.values.push(PreparedValue::Float {
                value: format.land(value),
                origin: origin(tree, node),
            });
            return Ok(());
        }
        let ungrouped = file.ungroup(node);
        if ungrouped != node {
            return self.evaluate_float(tree, ungrouped, carrier, scope, execution);
        }
        if selected_call(tree, node, scope.call_use())?.is_some() || file.nodes[node].kind == NodeKind::Call {
            execution
                .steps
                .push(Step::Invoke(file.node_id(node), carrier.clone(), scope.clone()));
        } else if let Some(value) = self.constant_reference(tree, node, carrier)? {
            execution.values.push(value.clone());
        } else if self.schedule_float_cast(tree, node, carrier, scope, execution)? {
            // The cast schedules its operand using the selected source carrier.
        } else if matches!(file.nodes[node].kind, NodeKind::Binary | NodeKind::Unary) {
            let operation = float_arithmetic(tree, node, carrier)?;
            execution.steps.push(Step::ApplyFloat {
                site: file.node_id(node),
                format,
                operation,
            });
            execution.steps.extend(
                file.nodes[node]
                    .children
                    .iter()
                    .rev()
                    .map(|&operand| Step::Evaluate(file.node_id(operand), carrier.clone(), scope.clone())),
            );
        } else if tree.semantics_at(node).all(|record| match record {
            NodeSemantics::Expression(_) => true,
            NodeSemantics::Obligation(o) => self.admits_owned_return(scope, &o.kind, carrier),
            _ => false,
        }) && let Some(value) = scope.binding(tree, node, carrier)
        {
            execution.values.push(value);
        } else {
            return Err(UNSUPPORTED_BODY.into());
        }
        Ok(())
    }

    pub(super) fn schedule_float_comparison<'a>(
        &self,
        tree: &TypedTree<'_>,
        node: usize,
        selected: &TypedOperation,
        scope: &Scope<'a>,
        execution: &mut Execution<'a>,
    ) -> Result<bool, EvaluationError> {
        let [left_type, right_type] = selected.operands.as_slice() else {
            return Err(UNSUPPORTED_CONTRACT.into());
        };
        let Some(format) = FloatFormat::for_type(left_type) else { return Ok(false) };
        if FloatFormat::for_type(right_type) != Some(format) {
            return Err(UNSUPPORTED_CONTRACT.into());
        }
        let operation = match selected.token.as_str() {
            "==" => FloatOperation::Equal,
            "!=" => FloatOperation::NotEqual,
            "<" => FloatOperation::Less,
            "<=" => FloatOperation::LessEqual,
            ">" => FloatOperation::Greater,
            ">=" => FloatOperation::GreaterEqual,
            _ => return Err(UNSUPPORTED_CONTRACT.into()),
        };
        let file = tree.resolved().file;
        let [left, right] = *file.nodes[node].children.as_slice() else {
            return Err(UNSUPPORTED_CONTRACT.into());
        };
        execution.steps.extend([
            Step::ApplyFloat {
                site: file.node_id(node),
                format,
                operation,
            },
            Step::Evaluate(file.node_id(right), right_type.clone(), scope.clone()),
            Step::Evaluate(file.node_id(left), left_type.clone(), scope.clone()),
        ]);
        Ok(true)
    }

    pub(super) fn apply_float(
        &self,
        site: TreeNodeId,
        format: FloatFormat,
        operation: FloatOperation,
        execution: &mut Execution<'_>,
    ) -> Result<(), EvaluationError> {
        let site = origin(&self.tree(site)?, site.node);
        let target = self
            .target
            .ok_or_else(|| EvaluationError::OpenTarget(crate::refusal(&site, "the floating operation needs a selected target realization")))?;
        let realization = target.float_realization(format, operation).ok_or_else(|| {
            EvaluationError::AtCall(omega_typer::TypeError::new(
                &site.package,
                &site.file,
                site.span,
                format!(
                    "the selected target does not support this floating operation ({format:?} {operation:?} on {})",
                    target.identity().profile
                ),
            ))
        })?;
        let right = pop_float(execution)?;
        if right.format() != format {
            return Err(UNSUPPORTED_BODY.into());
        }
        let semantics = realization.semantics;
        let result = if operation == FloatOperation::Negate {
            PreparedValue::Float {
                value: semantics.negate(right),
                origin: site,
            }
        } else {
            let left = pop_float(execution)?;
            if left.format() != format {
                return Err(UNSUPPORTED_BODY.into());
            }
            match operation {
                FloatOperation::Add | FloatOperation::Subtract | FloatOperation::Multiply | FloatOperation::Divide => PreparedValue::Float {
                    value: semantics.arithmetic(operation, left, right),
                    origin: site,
                },
                _ => PreparedValue::Bool(semantics.compare(operation, left, right)),
            }
        };
        execution.values.push(result);
        self.record_observation(TargetObservation::FloatRealization {
            target: target.identity().clone(),
            realization: *realization,
        });
        Ok(())
    }

    pub(super) fn schedule_float_cast<'a>(
        &self,
        tree: &TypedTree<'_>,
        node: usize,
        carrier: &ValueType,
        scope: &Scope<'a>,
        execution: &mut Execution<'a>,
    ) -> Result<bool, EvaluationError> {
        let file = tree.resolved().file;
        if file.nodes[node].kind != NodeKind::Cast {
            return Ok(false);
        }
        let mut operation = None;
        let mut exact = None;
        for record in tree.semantics_at(node) {
            match record {
                NodeSemantics::Operation(selected) if selected.token == "as" && &selected.result == carrier && operation.is_none() => {
                    operation = Some(selected);
                }
                NodeSemantics::Obligation(obligation) => match &obligation.kind {
                    ObligationKind::ExactCast { source, destination } if exact.is_none() => exact = Some((source, destination)),
                    _ => return Err(UNSUPPORTED_BODY.into()),
                },
                NodeSemantics::Expression(_) => {}
                _ => return Err(UNSUPPORTED_BODY.into()),
            }
        }
        let operation = operation.ok_or(UNSUPPORTED_BODY)?;
        let [source] = operation.operands.as_slice() else {
            return Err(UNSUPPORTED_BODY.into());
        };
        let [operand, _] = *file.nodes[node].children.as_slice() else {
            return Err(UNSUPPORTED_BODY.into());
        };
        if FloatFormat::for_type(carrier).is_none() && FloatFormat::for_type(source).is_none() {
            return Ok(false);
        }
        let operand_carrier = if matches!(source, ValueType::Exact { .. }) {
            // An anonymous numeric operand lands directly in its destination.
            carrier
        } else if source == carrier {
            source
        } else {
            if exact != Some((source, carrier)) {
                return Err(UNSUPPORTED_BODY.into());
            }
            execution.steps.push(Step::CoerceFloat {
                site: file.node_id(node),
                carrier: carrier.clone(),
            });
            source
        };
        execution
            .steps
            .push(Step::Evaluate(file.node_id(operand), operand_carrier.clone(), scope.clone()));
        Ok(true)
    }

    pub(super) fn coerce_float(
        &self,
        site: TreeNodeId,
        carrier: &ValueType,
        execution: &mut Execution<'_>,
    ) -> Result<(), EvaluationError> {
        let operand = execution.values.pop().ok_or(UNSUPPORTED_BODY)?;
        let not_exact = "the floating carrier cast does not preserve the exact value";
        let output = if let Some(format) = FloatFormat::for_type(carrier) {
            let value = match operand {
                PreparedValue::Float { value, .. } => value.exactly_in(format),
                PreparedValue::Integer(value) => format.land_exact_integer(value),
                _ => return Err(UNSUPPORTED_BODY.into()),
            }
            .ok_or(not_exact)?;
            PreparedValue::Float {
                value,
                origin: origin(&self.tree(site)?, site.node),
            }
        } else {
            let integer = operand
                .float()
                .ok_or(UNSUPPORTED_BODY)?
                .exact_integer()
                .ok_or(not_exact)?;
            PreparedValue::Integer(fits(integer, carrier).ok_or(not_exact)?)
        };
        execution.values.push(output);
        Ok(())
    }
}

pub(super) fn origin(
    tree: &TypedTree<'_>,
    node: usize,
) -> Site {
    let file = tree.resolved().file;
    Site {
        package: file.package.into(),
        file: file.path.into(),
        span: file.span(node),
    }
}

fn pop_float(execution: &mut Execution<'_>) -> Result<crate::PreparedFloat, EvaluationError> {
    execution
        .values
        .pop()
        .and_then(|value| value.float())
        .ok_or_else(|| UNSUPPORTED_BODY.into())
}

fn float_arithmetic(
    tree: &TypedTree<'_>,
    node: usize,
    carrier: &ValueType,
) -> Result<FloatOperation, EvaluationError> {
    let mut operation = None;
    let mut contract = None;
    for record in tree.semantics_at(node) {
        match record {
            NodeSemantics::Operation(selected) if &selected.result == carrier && operation.is_none() => operation = Some(selected),
            NodeSemantics::Obligation(obligation) => match &obligation.kind {
                ObligationKind::OperationContract { token, operands } if contract.is_none() => contract = Some((token, operands)),
                _ => return Err(UNSUPPORTED_BODY.into()),
            },
            NodeSemantics::Expression(_) => {}
            _ => return Err(UNSUPPORTED_BODY.into()),
        }
    }
    let selected = operation.ok_or(UNSUPPORTED_BODY)?;
    let count = tree.resolved().file.nodes[node].children.len();
    if selected.operands.len() != count || !selected.operands.iter().all(|operand| operand == carrier) {
        return Err(UNSUPPORTED_BODY.into());
    }
    Ok(match (selected.token.as_str(), count) {
        ("-", 1) if contract.is_none() => FloatOperation::Negate,
        (token, 2) if contract == Some((&selected.token, &selected.operands)) => match token {
            "+" => FloatOperation::Add,
            "-" => FloatOperation::Subtract,
            "*" => FloatOperation::Multiply,
            "/" => FloatOperation::Divide,
            _ => return Err(UNSUPPORTED_BODY.into()),
        },
        _ => return Err(UNSUPPORTED_BODY.into()),
    })
}

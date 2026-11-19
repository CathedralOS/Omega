use super::*;

impl ConstantEvaluation<'_, '_> {
    pub(super) fn schedule_aggregate<'a>(
        &self,
        tree: &TypedTree<'_>,
        node: usize,
        carrier: &ValueType,
        scope: &Scope<'a>,
        execution: &mut Execution<'a>,
    ) -> Result<(), EvaluationError> {
        if !execution.checked_aggregates.contains(carrier) {
            self.check_owned_type(carrier, matches!(carrier, ValueType::Array(..)))?;
            execution.checked_aggregates.insert(carrier.clone());
        }
        let file = tree.resolved().file;
        let node = file.ungroup(node);
        if selected_call(tree, node, scope.call_use())?.is_some() {
            execution
                .steps
                .push(Step::Invoke(file.node_id(node), carrier.clone(), scope.clone()));
            return Ok(());
        }
        if let Some(value) = self.constant_reference(tree, node, carrier)? {
            execution.values.push(value.clone());
            return Ok(());
        }
        if let Some(value) = scope.binding(tree, node, carrier) {
            if tree
                .semantics_at(node)
                .any(|record| !matches!(record, NodeSemantics::Expression(_)))
            {
                return Err(UNSUPPORTED_BODY.into());
            }
            execution.values.push(value);
            return Ok(());
        }
        match carrier {
            ValueType::Array(element, length) => {
                if tree
                    .semantics_at(node)
                    .any(|record| !matches!(record, NodeSemantics::Expression(_)))
                {
                    return Err(UNSUPPORTED_BODY.into());
                }
                if let Some(bytes) = tree.owned_byte_literal(node) {
                    if *length != bytes.len() as u64 || !matches!(element.as_ref(), ValueType::Integer { signed: false, width: Some(8) }) {
                        return Err("the selected byte literal has a different array type".into());
                    }
                    execution.values.push(PreparedValue::Array(
                        bytes
                            .iter()
                            .map(|&byte| PreparedValue::Integer(byte.into()))
                            .collect(),
                    ));
                } else {
                    let literal = &file.nodes[node];
                    if literal.kind != NodeKind::ArrayLiteral || literal.children.len() as u64 != *length {
                        return Err("the selected array value has no supported initializer".into());
                    }
                    execution.steps.push(Step::FinishArray(literal.children.len()));
                    execution.steps.extend(
                        literal
                            .children
                            .iter()
                            .rev()
                            .map(|&item| Step::Evaluate(file.node_id(item), element.as_ref().clone(), scope.clone())),
                    );
                }
            }
            ValueType::Nominal(owner) | ValueType::Applied(owner, _) => {
                let (construction, fields) = self.construction(tree, node, carrier, owner)?;
                execution.steps.push(Step::FinishConstruction {
                    owner: owner.as_ref().clone(),
                    case: construction.case.clone(),
                    positions: construction
                        .initializers
                        .iter()
                        .map(|field| field.position)
                        .collect(),
                });
                // Run the initializers in source order, then place their values
                // into declaration order when finishing the construction.
                for init in construction.initializers.iter().rev() {
                    let field = fields.get(init.position).ok_or(UNSUPPORTED_BODY)?;
                    execution
                        .steps
                        .push(Step::Evaluate(file.node_id(init.expression), field.clone(), scope.clone()));
                }
            }
            _ => return Err(UNSUPPORTED_BODY.into()),
        }
        Ok(())
    }

    pub(super) fn evaluate_integer<'a>(
        &self,
        tree: &TypedTree<'_>,
        node: usize,
        carrier: &ValueType,
        scope: &Scope<'a>,
        execution: &mut Execution<'a>,
    ) -> Result<(), EvaluationError> {
        let range = IntegerRange::for_type(carrier).ok_or(UNSUPPORTED_BODY)?;
        if let Some(value) = landed_value(tree, node, carrier)? {
            execution.values.push(PreparedValue::Integer(value));
            return Ok(());
        }
        let file = tree.resolved().file;
        let node = file.ungroup(node);
        if file.nodes[node].kind == NodeKind::Binary {
            let [left, right] = *file.nodes[node].children.as_slice() else {
                return Err(UNSUPPORTED_BODY.into());
            };
            let (token, right_carrier) = selected_arithmetic(tree, node, carrier)?;
            execution.steps.extend([
                Step::ApplyInteger(range, token.into()),
                Step::Evaluate(file.node_id(right), right_carrier.clone(), scope.clone()),
                Step::Evaluate(file.node_id(left), carrier.clone(), scope.clone()),
            ]);
        } else if selected_call(tree, node, scope.call_use())?.is_some() {
            execution
                .steps
                .push(Step::Invoke(file.node_id(node), carrier.clone(), scope.clone()));
        } else if file.nodes[node].kind == NodeKind::Unary && matches!(carrier, ValueType::Integer { signed: true, .. }) {
            let [operand] = *file.nodes[node].children.as_slice() else {
                return Err(UNSUPPORTED_BODY.into());
            };
            let mut selected = false;
            let mut contract = false;
            for record in tree.semantics_at(node) {
                match record {
                    NodeSemantics::Operation(op) if op.token == "-" && op.operands == [carrier.clone()] && &op.result == carrier && !selected => {
                        selected = true;
                    }
                    NodeSemantics::Obligation(obligation)
                        if matches!(&obligation.kind, ObligationKind::OperationContract { token, operands }
                            if token == "-" && operands.as_slice() == [carrier.clone()])
                            && !contract =>
                    {
                        contract = true;
                    }
                    NodeSemantics::Expression(_) => {}
                    _ => return Err(UNSUPPORTED_BODY.into()),
                }
            }
            if !selected || !contract {
                return Err(UNSUPPORTED_BODY.into());
            }
            execution.steps.extend([
                Step::ApplyInteger(range, "-".into()),
                Step::Evaluate(file.node_id(operand), carrier.clone(), scope.clone()),
                Step::Push(PreparedValue::Integer(0.into())),
            ]);
        } else {
            execution
                .values
                .push(PreparedValue::Integer(self.atom(tree, node, carrier, scope)?));
        }
        Ok(())
    }

    pub(super) fn evaluate_boolean<'a>(
        &self,
        tree: &TypedTree<'_>,
        node: usize,
        scope: &Scope<'a>,
        execution: &mut Execution<'a>,
    ) -> Result<(), EvaluationError> {
        let file = tree.resolved().file;
        let node = file.ungroup(node);
        if let Some(value) = self.constant_reference(tree, node, &ValueType::Bool)? {
            let PreparedValue::Bool(_) = value else {
                return Err(UNSUPPORTED_CONTRACT.into());
            };
            execution.values.push(value.clone());
        } else if file.nodes[node].kind == NodeKind::Call {
            execution
                .steps
                .push(Step::Invoke(file.node_id(node), ValueType::Bool, scope.clone()));
        } else if file.nodes[node].kind == NodeKind::Unary {
            let [operand] = *file.nodes[node].children.as_slice() else {
                return Err(UNSUPPORTED_CONTRACT.into());
            };
            let operation = selected_boolean_operation(tree, node)?;
            if operation.token != "!" || operation.operands != [ValueType::Bool] {
                return Err(UNSUPPORTED_CONTRACT.into());
            }
            execution.steps.extend([
                Step::Negate,
                Step::Evaluate(file.node_id(operand), ValueType::Bool, scope.clone()),
            ]);
        } else if file.nodes[node].kind != NodeKind::Binary {
            for record in tree.semantics_at(node) {
                match record {
                    NodeSemantics::Expression(_) => {}
                    NodeSemantics::Obligation(o) if matches!(o.kind, ObligationKind::TotalCrashGuard) => {}
                    _ => return Err(UNSUPPORTED_CONTRACT.into()),
                }
            }
            let value = if let Some(argument) = self.specialized_argument(tree, node, &ValueType::Bool, scope)? {
                let ValueType::StaticValue {
                    value: PreparedValue::Bool(value),
                    carrier,
                } = argument
                else {
                    return Err(UNSUPPORTED_ARGUMENT.into());
                };
                if carrier.as_ref() != &ValueType::Bool {
                    return Err(UNSUPPORTED_ARGUMENT.into());
                }
                *value
            } else if let Some(Symbol::Builtin(name)) = tree.resolved().symbol(node)
                && matches!(name.as_str(), "true" | "false")
            {
                name == "true"
            } else {
                let Some(PreparedValue::Bool(value)) = scope.binding(tree, node, &ValueType::Bool) else {
                    return Err(UNSUPPORTED_CONTRACT.into());
                };
                value
            };
            execution.values.push(PreparedValue::Bool(value));
        } else {
            let [left, right] = *file.nodes[node].children.as_slice() else {
                return Err(UNSUPPORTED_CONTRACT.into());
            };
            let selected = selected_boolean_operation(tree, node)?;
            let [left_type, right_type] = selected.operands.as_slice() else {
                return Err(UNSUPPORTED_CONTRACT.into());
            };
            let token = selected.token.as_str();
            let right = file.node_id(right);
            if left_type == &ValueType::Bool && right_type == &ValueType::Bool {
                match token {
                    "&&" | "||" => execution.steps.push(Step::ShortCircuit {
                        right,
                        scope: scope.clone(),
                        stop_at: token == "||",
                    }),
                    "==" | "!=" => execution.steps.extend([
                        Step::Equal(token == "=="),
                        Step::Evaluate(right, ValueType::Bool, scope.clone()),
                    ]),
                    _ => return Err(UNSUPPORTED_CONTRACT.into()),
                }
            } else {
                // A mixed or unsupported selection must not reach integer operand reads.
                IntegerRange::for_type(left_type).ok_or(UNSUPPORTED_BODY)?;
                IntegerRange::for_type(right_type).ok_or(UNSUPPORTED_BODY)?;
                execution.steps.extend([
                    Step::Compare(token.into()),
                    Step::Evaluate(right, right_type.clone(), scope.clone()),
                ]);
            }
            execution
                .steps
                .push(Step::Evaluate(file.node_id(left), left_type.clone(), scope.clone()));
        }
        Ok(())
    }

    fn specialized_argument<'a>(
        &self,
        tree: &TypedTree<'_>,
        node: usize,
        carrier: &ValueType,
        scope: &Scope<'a>,
    ) -> Result<Option<&'a ValueType>, EvaluationError> {
        let mut slot = None;
        for record in tree.semantics_at(node) {
            if let NodeSemantics::Expression(expression) = record
                && let Some(argument) = expression.static_argument_slot
                && (&expression.ty != carrier || slot.replace(argument).is_some_and(|old| old != argument))
            {
                return Err(UNSUPPORTED_ARGUMENT.into());
            }
        }
        let Some(slot) = slot else { return Ok(None) };
        let Scope::Initializer {
            owner_arguments: Some(arguments),
            ..
        } = scope
        else {
            return Err(UNSUPPORTED_ARGUMENT.into());
        };
        Ok(Some(arguments.get(slot).ok_or(UNSUPPORTED_ARGUMENT)?))
    }

    pub(super) fn atom(
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
        for record in tree.semantics_at(node) {
            match record {
                NodeSemantics::Constant(_) | NodeSemantics::Expression(_) | NodeSemantics::Landing(_) => {}
                _ => return Err(UNSUPPORTED_BODY.into()),
            }
        }
        if let Some(value) = self.constant_reference(tree, node, carrier)? {
            let PreparedValue::Integer(value) = value else {
                return Err(UNSUPPORTED_BODY.into());
            };
            return fits(value.clone(), carrier).ok_or_else(|| "the frozen constant exceeds its carrier".into());
        }
        if let Some(argument) = self.specialized_argument(tree, node, carrier, scope)? {
            let ValueType::StaticInteger { value, carrier: actual } = argument else {
                return Err(UNSUPPORTED_ARGUMENT.into());
            };
            if actual.as_ref() != carrier {
                return Err(UNSUPPORTED_ARGUMENT.into());
            }
            return fits(value.clone(), carrier).ok_or_else(|| UNSUPPORTED_ARGUMENT.into());
        }
        if let Some(PreparedValue::Integer(value)) = scope.binding(tree, node, carrier) {
            return Ok(value);
        }
        Err(UNSUPPORTED_BODY.into())
    }
}

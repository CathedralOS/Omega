use super::*;

impl ConstantEvaluation<'_, '_> {
    pub(super) fn schedule_statement<'a>(
        &self,
        site: TreeNodeId,
        statement: usize,
        carrier: ValueType,
        bindings: Rc<CallBindings>,
        execution: &mut Execution<'a>,
    ) -> Result<(), EvaluationError> {
        let tree = self.tree(site)?;
        let file = tree.resolved().file;
        let statements = &file.nodes[site.node].children;
        let &node = statements.get(statement).ok_or(UNSUPPORTED_BODY)?;
        let scope = Scope::Body(bindings.clone());
        match file.nodes[node].kind {
            NodeKind::Tail if statement + 1 == statements.len() => {
                let [expression] = *file.nodes[node].children.as_slice() else {
                    return Err(UNSUPPORTED_BODY.into());
                };
                execution
                    .steps
                    .push(Step::Evaluate(file.node_id(expression), carrier, scope));
            }
            NodeKind::Transition if statement + 1 == statements.len() => {
                self.schedule_dispatch(&tree, node, &carrier, scope, execution)?;
            }
            NodeKind::Let => {
                if !file.nodes[node].leaves.iter().any(|leaf| leaf.text == "=")
                    || tree
                        .semantics_at(node)
                        .any(|record| !matches!(record, NodeSemantics::Expression(_)))
                {
                    return Err(UNSUPPORTED_BODY.into());
                }
                let (binder, initializer) = match *file.nodes[node].children.as_slice() {
                    [binder, initializer] | [binder, _, initializer] => (binder, initializer),
                    _ => return Err(UNSUPPORTED_BODY.into()),
                };
                if file.nodes[binder].kind != NodeKind::BindingPattern
                    || tree.resolved().pattern_binding(binder).is_none()
                    || tree
                        .semantics_at(binder)
                        .any(|record| !matches!(record, NodeSemantics::Expression(_)))
                {
                    return Err(UNSUPPORTED_BODY.into());
                }
                let local_carrier = checked_carrier_in_scope(&tree, binder, scope.application())?;
                let writable = file.nodes[node].words().any(|word| word.text == "mut");
                if writable && local_carrier != ValueType::Bool && IntegerRange::for_type(&local_carrier).is_none() {
                    return Err(UNSUPPORTED_BODY.into());
                }
                execution.steps.extend([
                    Step::Block {
                        site,
                        statement: statement + 1,
                        carrier,
                        bindings: bindings.clone(),
                    },
                    Step::Bind {
                        binder,
                        carrier: local_carrier.clone(),
                        bindings,
                        writable,
                    },
                    Step::Evaluate(file.node_id(initializer), local_carrier, scope),
                ]);
            }
            NodeKind::Assign => {
                let [place, right] = *file.nodes[node].children.as_slice() else {
                    return Err(UNSUPPORTED_BODY.into());
                };
                let place = file.ungroup(place);
                let Some(Symbol::Binding {
                    binder_node,
                    kind: NodeKind::BindingPattern,
                    ..
                }) = tree.resolved().symbol(place)
                else {
                    return Err(UNSUPPORTED_BODY.into());
                };
                if file.nodes[place].kind != NodeKind::PathExpression
                    || tree
                        .semantics_at(place)
                        .any(|record| !matches!(record, NodeSemantics::Expression(_)))
                    || tree.semantics_at(node).next().is_some()
                {
                    return Err(UNSUPPORTED_BODY.into());
                }
                // Only this frame's own scalar binding can be replaced. A
                // field, computed place, or alias needs checked storage access.
                let place_carrier = checked_carrier_in_scope(&tree, place, scope.application())?;
                let local = bindings
                    .0
                    .borrow()
                    .iter()
                    .rposition(|binding| binding.binder == *binder_node && binding.writable && binding.carrier == place_carrier)
                    .ok_or(UNSUPPORTED_BODY)?;
                execution.steps.extend([
                    Step::Block {
                        site,
                        statement: statement + 1,
                        carrier,
                        bindings: bindings.clone(),
                    },
                    Step::Store { local, bindings },
                    Step::Evaluate(file.node_id(right), place_carrier, scope),
                ]);
            }
            _ => return Err(UNSUPPORTED_BODY.into()),
        }
        Ok(())
    }

    pub(super) fn schedule_dispatch<'a>(
        &self,
        tree: &TypedTree<'_>,
        node: usize,
        carrier: &ValueType,
        scope: Scope<'a>,
        execution: &mut Execution<'a>,
    ) -> Result<(), EvaluationError> {
        let file = tree.resolved().file;
        if tree
            .semantics_at(node)
            .any(|record| !matches!(record, NodeSemantics::Expression(_)))
        {
            return Err(UNSUPPORTED_BODY.into());
        }
        let [subject, ..] = *file.nodes[node].children.as_slice() else {
            return Err(UNSUPPORTED_BODY.into());
        };
        let subject_type = checked_carrier_in_scope(tree, subject, scope.application())?;
        if !matches!(subject_type, ValueType::Bool | ValueType::Nominal(_) | ValueType::Applied(_, _)) {
            return Err(UNSUPPORTED_BODY.into());
        }
        execution.steps.extend([
            Step::Select {
                site: file.node_id(node),
                carrier: carrier.clone(),
                subject_type: subject_type.clone(),
                scope: scope.clone(),
            },
            Step::Evaluate(file.node_id(subject), subject_type, scope),
        ]);
        Ok(())
    }

    pub(super) fn select_arm<'a>(
        &self,
        site: TreeNodeId,
        carrier: ValueType,
        subject_type: &ValueType,
        scope: Scope<'a>,
        execution: &mut Execution<'a>,
    ) -> Result<(), EvaluationError> {
        let subject = execution.values.pop().ok_or(UNSUPPORTED_BODY)?;
        let tree = self.tree(site)?;
        let file = tree.resolved().file;
        let subject_node = file.nodes[site.node].children[0];
        for &arm in &file.nodes[site.node].children[1..] {
            let [pattern, expression] = *file.nodes[arm].children.as_slice() else {
                return Err(UNSUPPORTED_BODY.into());
            };
            let Some(values) = self
                .dispatch_pattern(&tree, pattern, subject_node, subject_type, &subject)
                .map_err(|error| match error {
                    EvaluationError::Refused(UNSUPPORTED_BODY) => "the selected pattern has no supported owned-value selection".into(),
                    other => other,
                })?
            else {
                continue;
            };
            if !values.is_empty() {
                let (Scope::Body(bindings) | Scope::Fact(bindings)) = &scope else {
                    return Err(UNSUPPORTED_BODY.into());
                };
                execution.steps.push(Step::RestoreBindings {
                    count: bindings.0.borrow().len(),
                    bindings: bindings.clone(),
                });
                bindings.0.borrow_mut().extend(values);
            }
            execution
                .steps
                .push(Step::Evaluate(file.node_id(expression), carrier, scope));
            return Ok(());
        }
        Err(UNSUPPORTED_BODY.into())
    }

    fn dispatch_pattern(
        &self,
        tree: &TypedTree<'_>,
        mut pattern: usize,
        subject_node: usize,
        subject_type: &ValueType,
        subject: &PreparedValue,
    ) -> Result<Option<Vec<BindingValue>>, EvaluationError> {
        let file = tree.resolved().file;
        while file.nodes[pattern].kind == NodeKind::ParenPattern {
            let [inner] = *file.nodes[pattern].children.as_slice() else {
                return Err(UNSUPPORTED_BODY.into());
            };
            pattern = inner;
        }
        match file.nodes[pattern].kind {
            NodeKind::WildcardPattern => Ok(Some(Vec::new())),
            NodeKind::BindingPattern if tree.resolved().pattern_binding(pattern).is_some() => Ok(Some(vec![BindingValue {
                binder: pattern,
                carrier: subject_type.clone(),
                value: subject.clone(),
                writable: false,
            }])),
            NodeKind::BindingPattern => match (tree.resolved().symbol(pattern), subject) {
                (Some(Symbol::Builtin(literal)), PreparedValue::Bool(value)) if matches!(literal.as_str(), "true" | "false") => {
                    Ok(((literal == "true") == *value).then(Vec::new))
                }
                _ => Err(UNSUPPORTED_BODY.into()),
            },
            NodeKind::ValuePattern => {
                let [value] = *file.nodes[pattern].children.as_slice() else {
                    return Err(UNSUPPORTED_BODY.into());
                };
                match (tree.resolved().symbol(value), subject) {
                    (Some(Symbol::Declaration(selected)), PreparedValue::Sum { case, .. }) if selected.kind == NodeKind::Case => {
                        Ok((selected == case).then(Vec::new))
                    }
                    _ => Err(UNSUPPORTED_BODY.into()),
                }
            }
            NodeKind::RecordPattern => {
                let [head, parts @ ..] = file.nodes[pattern].children.as_slice() else {
                    return Err(UNSUPPORTED_BODY.into());
                };
                let Some(Symbol::Declaration(selected)) = tree.resolved().symbol(*head) else {
                    return Err(UNSUPPORTED_BODY.into());
                };
                let shape = self
                    .trees
                    .static_data_shapes
                    .get(subject_type)
                    .ok_or(UNSUPPORTED_BODY)?;
                let (fields, types) = match subject {
                    PreparedValue::Record { owner, fields } if owner == selected => (fields, &shape.fields),
                    PreparedValue::Sum { case, fields, .. } if case == selected => {
                        let (_, types) = shape
                            .cases
                            .iter()
                            .find(|(id, _)| id == case)
                            .ok_or(UNSUPPORTED_BODY)?;
                        (fields, types)
                    }
                    PreparedValue::Sum { .. } => return Ok(None),
                    _ => return Err(UNSUPPORTED_BODY.into()),
                };
                let mut bindings = Vec::new();
                for &part in parts {
                    if file.nodes[part].kind == NodeKind::RestPattern {
                        continue;
                    }
                    if file.nodes[part].kind != NodeKind::FieldPattern || tree.resolved().pattern_binding(part).is_none() {
                        return Err(UNSUPPORTED_BODY.into());
                    }
                    let (_, path) = self.trees.place_path(file.node_id(part));
                    let Some(step) = path.last() else { return Err(UNSUPPORTED_BODY.into()) };
                    let Some(omega_typer::Projection::Field(field)) = &step.projection else {
                        return Err(UNSUPPORTED_BODY.into());
                    };
                    if step.base != file.node_id(file.ungroup(subject_node)) || step.through.is_some() {
                        return Err(UNSUPPORTED_BODY.into());
                    }
                    let slot = self.owned_field_position(selected, field)?;
                    bindings.push(BindingValue {
                        binder: part,
                        carrier: types.get(slot).ok_or(UNSUPPORTED_BODY)?.clone(),
                        value: fields.get(slot).ok_or(UNSUPPORTED_BODY)?.clone(),
                        writable: false,
                    });
                }
                Ok(Some(bindings))
            }
            _ => Err(UNSUPPORTED_BODY.into()),
        }
    }
}

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
                self.schedule_boolean_dispatch(&tree, node, &carrier, scope, execution)?;
            }
            NodeKind::Let => {
                if file.nodes[node].words().any(|word| word.text == "mut")
                    || !file.nodes[node].leaves.iter().any(|leaf| leaf.text == "=")
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
                let local_carrier = checked_carrier(&tree, binder)?;
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
                    },
                    Step::Evaluate(file.node_id(initializer), local_carrier, scope),
                ]);
            }
            _ => return Err(UNSUPPORTED_BODY.into()),
        }
        Ok(())
    }

    pub(super) fn schedule_boolean_dispatch<'a>(
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
        if checked_carrier(tree, subject)? != ValueType::Bool {
            return Err(UNSUPPORTED_BODY.into());
        }
        execution.steps.extend([
            Step::Select {
                site: file.node_id(node),
                carrier: carrier.clone(),
                scope: scope.clone(),
            },
            Step::Evaluate(file.node_id(subject), ValueType::Bool, scope),
        ]);
        Ok(())
    }

    pub(super) fn select_boolean_arm<'a>(
        &self,
        site: TreeNodeId,
        carrier: ValueType,
        scope: Scope<'a>,
        execution: &mut Execution<'a>,
    ) -> Result<(), EvaluationError> {
        let subject = execution.pop_bool();
        let tree = self.tree(site)?;
        let file = tree.resolved().file;
        for &arm in &file.nodes[site.node].children[1..] {
            let [pattern, expression] = *file.nodes[arm].children.as_slice() else {
                return Err(UNSUPPORTED_BODY.into());
            };
            let matches = match (file.nodes[pattern].kind, tree.resolved().symbol(pattern)) {
                (NodeKind::WildcardPattern, _) => true,
                (NodeKind::BindingPattern, Some(Symbol::Builtin(literal))) if matches!(literal.as_str(), "true" | "false") => (literal == "true") == subject,
                _ => return Err(UNSUPPORTED_BODY.into()),
            };
            if matches {
                execution
                    .steps
                    .push(Step::Evaluate(file.node_id(expression), carrier, scope));
                return Ok(());
            }
        }
        Err(UNSUPPORTED_BODY.into())
    }
}

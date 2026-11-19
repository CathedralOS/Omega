use super::*;

impl ConstantEvaluation<'_, '_> {
    pub(super) fn schedule_call<'a>(
        &'a self,
        request_tree: &TypedTree<'_>,
        call: &TypedCall,
        scope: &Scope<'a>,
        carrier: &ValueType,
        execution: &mut Execution<'a>,
    ) -> Result<(), EvaluationError> {
        // Only unapplied local calls can execute here. A Build-selected
        // boundary has runtime meaning even when its authored effects are empty.
        if call.type_arguments.is_empty()
            && call.enclosing_arguments.iter().all(|(_, args)| args.is_empty())
            && self
                .boundary_slots
                .iter()
                .any(|slot| slot.requirement.declaration == call.machine && slot.arguments.is_empty() && slot.requirement.trait_arguments.is_empty())
        {
            return Err("the selected runtime boundary slot cannot execute during preparation".into());
        }
        if request_tree.resolved().file.node_id(call.node.node) != call.node
            || call.use_site != scope.call_use()
            || call.machine.kind != NodeKind::Machine
            || call.evidence.is_some()
            || !call.type_arguments.is_empty()
            || !call.enclosing_arguments.is_empty()
            || call.acknowledgements.suspend
            || call.acknowledgements.block
            || &call.result != carrier
        {
            return Err("the request does not select one ordinary static invocation".into());
        }

        let &(index, signature) = self
            .signatures()
            .get(&call.machine)
            .ok_or("the selected declaration has no typed signature")?;
        for record in request_tree.semantics_at(call.node.node) {
            match record {
                NodeSemantics::Call(_) | NodeSemantics::Expression(_) => {}
                NodeSemantics::Obligation(o) if call.use_site == CallUse::Fact && matches!(o.kind, ObligationKind::TotalCrashGuard) => {}
                NodeSemantics::Obligation(obligation) if matches!(&obligation.kind, ObligationKind::CallPrecondition(target) if target == &call.machine) => {}
                // Each declared cause is checked against the bound operands
                // before the selected machine body can run.
                NodeSemantics::Obligation(o) if matches!(&o.kind, ObligationKind::CrashCoverage { machine, cause } if call.caller.as_ref() == Some(machine) && signature.crash_ceiling.as_ref().is_some_and(|causes| causes.contains(cause))) =>
                    {}
                _ => return Err(UNSUPPORTED_BODY.into()),
            }
        }
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
            || signature.pause.suspend == Some(true)
            || signature.pause.block == Some(true)
            || &signature.result != carrier
            || !signature.multiplicity_bounds.is_empty()
            || file.child(declaration, NodeKind::GenericParams).is_some()
            || file.child(declaration, NodeKind::Hypothesis).is_some()
        {
            return Err(UNSUPPORTED_CONTRACT.into());
        }

        let parameters = file
            .child(declaration, NodeKind::Params)
            .map_or(&[][..], |list| file.nodes[list].children.as_slice());
        if signature.parameters.len() != call.operands.len() || parameters.len() != call.operands.len() {
            return Err("the typed call and selected parameters disagree".into());
        }
        self.admit_termination(index, signature, call, scope, &body_tree, execution)?;
        // The callee is active while evaluating its contract and body. Its
        // arguments belong to the caller, so keep id(id(true)) acyclic.
        execution.steps.push(Step::Enter(signature));
        for ((_, carrier), operand) in signature.parameters.iter().zip(&call.operands).rev() {
            let argument = self
                .argument_step(request_tree, operand, carrier, scope)
                .unwrap_or_else(Step::Reject);
            execution.steps.push(argument);
        }
        Ok(())
    }

    pub(super) fn enter_call<'a>(
        &'a self,
        signature: &'a TypedSignature,
        execution: &mut Execution<'a>,
    ) -> Result<(), EvaluationError> {
        let tree = self.tree(signature.node)?;
        let file = tree.resolved().file;
        let declaration = signature.node.node;
        let parameters = file
            .child(declaration, NodeKind::Params)
            .map_or(&[][..], |list| file.nodes[list].children.as_slice());
        let values = execution
            .values
            .split_off(execution.values.len() - parameters.len());
        let arguments = parameters
            .iter()
            .zip(&signature.parameters)
            .zip(values)
            .map(|((&binder, (_, carrier)), value)| {
                let value = match value {
                    PreparedValue::Integer(value) => PreparedValue::Integer(fits(value, carrier).ok_or("a closed argument is outside its selected carrier")?),
                    other => other,
                };
                Ok(BindingValue {
                    binder,
                    carrier: carrier.clone(),
                    value,
                })
            })
            .collect::<Result<Vec<_>, EvaluationError>>()?;
        let arguments = Rc::new(CallBindings(RefCell::new(arguments)));
        let first_activation = execution.active.insert(signature.node);
        execution.steps.push(Step::Leave {
            declaration: signature.node,
            first_activation,
        });

        let body = file
            .child(declaration, NodeKind::Body)
            .ok_or(UNSUPPORTED_BODY)?;
        execution.steps.push(Step::Block {
            site: file.node_id(body),
            statement: 0,
            carrier: signature.result.clone(),
            bindings: arguments.clone(),
        });
        let facts = Scope::Fact(arguments);
        self.schedule_crash_routes(signature, &tree, &facts, execution)?;
        let mut requirements = file.nodes[declaration]
            .children
            .iter()
            .copied()
            .rev()
            .filter(|&node| file.nodes[node].kind == NodeKind::Requires)
            .peekable();
        if signature.requires != requirements.peek().is_some() {
            return Err(UNSUPPORTED_CONTRACT.into());
        }
        let reason = match &signature.result {
            ValueType::Bool => "the selected Boolean precondition is false for the supplied arguments",
            ValueType::Integer { .. } => "the selected integer precondition is false for the supplied arguments",
            _ => "the selected precondition is false for the supplied arguments",
        };
        for clause in requirements {
            let [predicate] = *file.nodes[clause].children.as_slice() else {
                return Err(UNSUPPORTED_CONTRACT.into());
            };
            execution.steps.extend([
                Step::Require(reason),
                Step::Evaluate(file.node_id(predicate), ValueType::Bool, facts.clone()),
            ]);
        }
        Ok(())
    }

    pub(super) fn schedule_crash_routes<'a>(
        &self,
        signature: &TypedSignature,
        tree: &TypedTree<'_>,
        scope: &Scope<'a>,
        execution: &mut Execution<'a>,
    ) -> Result<(), EvaluationError> {
        for cause in signature
            .crash_ceiling
            .as_deref()
            .unwrap_or_default()
            .iter()
            .rev()
        {
            let mut routes = self
                .trees
                .crash_routes
                .iter()
                .rev()
                .filter(|route| route.contract == signature.site && route.cause == *cause)
                .peekable();
            if routes.peek().is_none() {
                return Err("the selected crash cause has no typed route".into());
            }
            let reason = match cause {
                omega_typer::CrashCause::Trap => "the selected Trap route remains possible for the supplied arguments",
                omega_typer::CrashCause::Abort => "the selected Abort route remains possible for the supplied arguments",
            };
            for route in routes {
                let guard = if let Some(guard) = &route.guard {
                    let obligation = self
                        .trees
                        .obligations
                        .iter()
                        .find(|obligation| {
                            obligation.site == *guard
                                && obligation.node.file == signature.node.file
                                && matches!(obligation.kind, ObligationKind::TotalCrashGuard)
                        })
                        .ok_or("the selected crash guard has no typed predicate")?;
                    Step::Evaluate(tree.resolved().file.node_id(obligation.node.node), ValueType::Bool, scope.clone())
                } else {
                    Step::Push(PreparedValue::Bool(true))
                };
                execution.steps.extend([Step::CrashGuard(reason), guard]);
            }
        }
        Ok(())
    }

    pub(super) fn argument_step<'a>(
        &self,
        tree: &TypedTree<'_>,
        operand: &AppliedOperand,
        carrier: &ValueType,
        scope: &Scope<'a>,
    ) -> Result<Step<'a>, EvaluationError> {
        let file = tree.resolved().file;
        if operand.node.file != file.node_id(0).file || operand.callable.is_some() {
            return Err("the static call does not have a local scalar operand".into());
        }
        let step = if carrier == &ValueType::Bool || is_aggregate(carrier) {
            Step::Evaluate(operand.node, carrier.clone(), scope.clone())
        } else {
            IntegerRange::for_type(carrier).ok_or("a selected parameter is outside the bounded integer fragment")?;
            if matches!(scope, Scope::Body(_) | Scope::Fact(_)) {
                Step::Evaluate(operand.node, carrier.clone(), scope.clone())
            } else {
                match &operand.subject {
                    ValueType::StaticInteger { value, carrier: selected } if selected.as_ref() == carrier => Step::Push(PreparedValue::Integer(value.clone())),
                    _ => {
                        let Scope::Initializer { owner_arguments, .. } = scope else { unreachable!() };
                        Step::Evaluate(
                            operand.node,
                            carrier.clone(),
                            Scope::Initializer {
                                owner_arguments: *owner_arguments,
                                in_argument: true,
                            },
                        )
                    }
                }
            }
        };
        Ok(step)
    }
}

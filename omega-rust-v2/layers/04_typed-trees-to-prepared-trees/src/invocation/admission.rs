use super::*;

mod proofs;
mod providers;
pub use proofs::CheckedInvocationFact;
pub(crate) use proofs::InvocationProofs;
pub(super) use proofs::ProofRequest;
use proofs::{InvocationClaim, InvocationSite};
pub(crate) use providers::ProviderAdmission;
pub use providers::{CheckedProvider, SelectedRealization};

use omega_typer::CrashCause;

impl ConstantEvaluation<'_, '_> {
    pub(super) fn schedule_call<'a>(
        &'a self,
        request_tree: &TypedTree<'_>,
        call: &TypedCall,
        scope: &Scope<'a>,
        carrier: &ValueType,
        execution: &mut Execution<'a>,
    ) -> Result<(), EvaluationError> {
        // A runtime slot requires a checked, build-admissible selection on
        // this target before it can dispatch to a local provider body.
        let typed_slot = self
            .trees
            .boundary_calls
            .iter()
            .find(|(site, _)| site == &call.site)
            .map(|(_, slot)| slot);
        let mut trait_scopes = call
            .enclosing_arguments
            .iter()
            .filter(|(owner, _)| owner.kind == NodeKind::Trait);
        let trait_arguments = trait_scopes
            .next()
            .map_or(&[][..], |(_, arguments)| arguments.as_slice());
        let ambiguous_trait_scope = trait_scopes.next().is_some();
        let mut supplied = self.boundary_slots.iter().filter(|slot| {
            slot.requirement.declaration == call.machine
                && call
                    .type_arguments
                    .iter()
                    .map(|argument| scope.instantiate(argument))
                    .eq(slot.arguments.iter().cloned())
                && (ambiguous_trait_scope
                    || trait_arguments
                        .iter()
                        .map(|argument| scope.instantiate(argument))
                        .eq(slot.requirement.trait_arguments.iter().cloned()))
        });
        let supplied_slot = supplied.next();
        if supplied_slot.is_some() && (ambiguous_trait_scope || supplied.next().is_some()) {
            return Err("the selected invocation has competing runtime boundary slots".into());
        }
        if typed_slot.is_some() && typed_slot != supplied_slot {
            return Err("the selected runtime boundary slot cannot execute under different build selections".into());
        }
        let selected_application = (!call.type_arguments.is_empty()
            || call
                .enclosing_arguments
                .iter()
                .any(|(_, args)| !args.is_empty()))
        .then(|| {
            Rc::new(
                scope
                    .application()
                    .map_or_else(|| call.clone(), |outer| call.in_caller_application(outer)),
            )
        });
        let provider = typed_slot
            .or(supplied_slot)
            .map(|slot| self.checked_provider(call, selected_application.as_deref(), slot))
            .transpose()?;
        if request_tree.resolved().file.node_id(call.node.node) != call.node
            || call.use_site != scope.call_use()
            || call.machine.kind != NodeKind::Machine
            || call.evidence.is_some()
            || &scope.instantiate(&call.result) != carrier
        {
            return Err("the request does not select one ordinary static invocation".into());
        }

        let &(contract_index, contract) = self
            .signatures()
            .get(&call.machine)
            .ok_or("the selected declaration has no typed signature")?;
        for record in request_tree.semantics_at(call.node.node) {
            match record {
                NodeSemantics::Call(_) | NodeSemantics::Expression(_) => {}
                NodeSemantics::Obligation(obligation) if let ObligationKind::RefineCallable { contract, machine, arguments } = &obligation.kind => {
                    self.admit_static_callable_refinement(obligation, contract, machine, arguments, selected_application.as_deref(), scope, execution)?;
                }
                NodeSemantics::Obligation(o) if call.use_site == CallUse::Fact && matches!(o.kind, ObligationKind::TotalCrashGuard) => {}
                NodeSemantics::Obligation(obligation) if matches!(&obligation.kind, ObligationKind::CallPrecondition(target) if target == &call.machine) => {}
                // Each declared cause is checked against the bound operands
                // before the selected machine body can run.
                NodeSemantics::Obligation(o) if matches!(&o.kind, ObligationKind::CrashCoverage { machine, cause } if call.caller.as_ref() == Some(machine) && contract.crash_ceiling.as_ref().is_some_and(|causes| causes.contains(cause))) =>
                    {}
                NodeSemantics::Obligation(o) if self.admits_owned_return(scope, &o.kind, carrier) => {}
                _ => return Err(UNSUPPORTED_BODY.into()),
            }
        }
        let (index, signature) = if let Some(provider) = &provider {
            *self
                .signatures()
                .get(provider.implementation)
                .ok_or("the checked provider has no typed signature")?
        } else {
            (contract_index, contract)
        };
        let application = if let Some(provider) = &provider {
            provider.application.cloned().map(Rc::new)
        } else {
            selected_application.clone()
        };
        let specialize = |ty: &ValueType| {
            application
                .as_ref()
                .map_or_else(|| ty.clone(), |applied| applied.instantiate(ty))
        };
        let body_tree = self
            .trees
            .trees()
            .nth(signature.node.file)
            .ok_or("the selected declaration has no local body")?;
        let declaration = signature.node.node;
        let resolved = body_tree.resolved();
        let file = resolved.file;
        if resolved.declaration_at(declaration) != Some(&signature.machine) {
            return Err("the selected declaration has no local body".into());
        }
        if file.nodes[declaration].has_public_modifier() && !signature.terminates {
            return Err("the selected public machine does not declare termination".into());
        }
        let effects = self
            .machine_effects(index)
            .ok_or("the selected body has no complete typed effect row")?;
        if !effects.reach.is_empty() || effects.fixed_pause.suspend || effects.fixed_pause.block {
            return Err("the selected invocation is not effect-free during preparation".into());
        }
        self.admit_callback_effects(effects, application.as_deref(), execution)?;
        if signature.satisfies.is_some()
            || &specialize(&signature.result) != carrier
            || !signature.multiplicity_bounds.is_empty()
            || (file.child(declaration, NodeKind::GenericParams).is_some() && application.is_none())
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
        let requirement = if let Some(provider) = provider {
            self.check_provider_contract(contract_index, (contract, selected_application.as_deref()), (signature, application.as_deref()))?;
            self.record_observation(provider.observation);
            Some((contract, selected_application))
        } else {
            None
        };
        // The callee is active while evaluating its contract and body. Its
        // arguments belong to the caller, so keep id(id(true)) acyclic.
        execution.steps.push(Step::Enter {
            call: call.node,
            signature,
            requirement,
            application: application.clone(),
        });
        for ((_, carrier), operand) in signature.parameters.iter().zip(&call.operands).rev() {
            let argument = self
                .argument_step(request_tree, operand, &specialize(carrier), scope)
                .unwrap_or_else(Step::Reject);
            execution.steps.push(argument);
        }
        Ok(())
    }

    pub(super) fn enter_call<'a>(
        &'a self,
        call: TreeNodeId,
        signature: &'a TypedSignature,
        requirement: Option<(&'a TypedSignature, Option<Rc<TypedCall>>)>,
        application: Option<Rc<TypedCall>>,
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
            .map(|((&binder, (_, declared)), value)| {
                let carrier = application
                    .as_ref()
                    .map_or_else(|| declared.clone(), |applied| applied.instantiate(declared));
                let value = match value {
                    PreparedValue::Integer(value) => PreparedValue::Integer(fits(value, &carrier).ok_or("a closed argument is outside its selected carrier")?),
                    other => other,
                };
                Ok(BindingValue {
                    binder,
                    carrier: carrier.clone(),
                    value,
                    writable: false,
                })
            })
            .collect::<Result<Vec<_>, EvaluationError>>()?;
        let result = application
            .as_ref()
            .map_or_else(|| signature.result.clone(), |applied| applied.instantiate(&signature.result));
        let arguments = Rc::new(CallBindings(RefCell::new(arguments), application));
        let first_activation = execution.active.insert(signature.node);
        execution.steps.push(Step::Leave {
            declaration: signature.node,
            first_activation,
        });

        let body = file
            .child(declaration, NodeKind::Body)
            .ok_or(UNSUPPORTED_BODY)?;
        let body = file.node_id(body);
        let invocation = InvocationSite { call, signature, body };
        let requirement = requirement
            .map(|(signature, application)| {
                let tree = self.tree(signature.node)?;
                let bindings = providers::bind_requirement(signature, &tree, &arguments, application)?;
                Ok::<_, EvaluationError>((InvocationSite { call, signature, body }, tree, bindings))
            })
            .transpose()?;
        if let Some((invocation, tree, bindings)) = &requirement {
            self.schedule_guarantees(*invocation, tree, bindings, execution)?;
        }
        self.schedule_guarantees(invocation, &tree, &arguments, execution)?;
        execution.steps.push(Step::Block {
            site: body,
            statement: 0,
            carrier: result.clone(),
            bindings: arguments.clone(),
        });
        self.schedule_entry_contract(invocation, &tree, &arguments, &result, execution)?;
        if let Some((invocation, tree, bindings)) = &requirement {
            self.schedule_entry_contract(*invocation, tree, bindings, &result, execution)?;
        }
        Ok(())
    }

    fn schedule_entry_contract<'a>(
        &'a self,
        invocation: InvocationSite<'a>,
        tree: &TypedTree<'_>,
        arguments: &Rc<CallBindings>,
        result: &ValueType,
        execution: &mut Execution<'a>,
    ) -> Result<(), EvaluationError> {
        self.schedule_crash_routes(invocation, tree, arguments, execution)?;
        let file = tree.resolved().file;
        let signature = invocation.signature;
        let declaration = signature.node.node;
        let facts = Scope::Fact(Rc::clone(arguments));
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
        let reason = match result {
            ValueType::Bool => "the selected Boolean precondition is false for the supplied arguments",
            ValueType::Integer { .. } => "the selected integer precondition is false for the supplied arguments",
            _ => "the selected precondition is false for the supplied arguments",
        };
        for clause in requirements {
            let [predicate] = *file.nodes[clause].children.as_slice() else {
                return Err(UNSUPPORTED_CONTRACT.into());
            };
            let predicate_id = file.node_id(predicate);
            let request = ProofRequest {
                invocation,
                claim: InvocationClaim::Requires(predicate_id),
            };
            if self.has_invocation_proof(request, arguments.1.as_deref()) || needs_proof_evaluation(tree, predicate) {
                execution
                    .steps
                    .push(Step::CheckInvocationProof(request, Rc::clone(arguments)));
            } else {
                execution.steps.extend([
                    Step::Require(reason),
                    Step::Evaluate(predicate_id, ValueType::Bool, facts.clone()),
                ]);
            }
        }
        Ok(())
    }

    fn schedule_guarantees<'a>(
        &'a self,
        invocation: InvocationSite<'a>,
        tree: &TypedTree<'_>,
        arguments: &Rc<CallBindings>,
        execution: &mut Execution<'a>,
    ) -> Result<(), EvaluationError> {
        let signature = invocation.signature;
        if !signature.has_guarantee {
            return Ok(());
        }
        let file = tree.resolved().file;
        let guarantees = file.nodes[signature.node.node]
            .children
            .iter()
            .copied()
            .rev()
            .filter(|&node| file.nodes[node].kind == NodeKind::Ensures);
        for clause in guarantees {
            let [predicate] = *file.nodes[clause].children.as_slice() else {
                return Err(UNSUPPORTED_CONTRACT.into());
            };
            let obligation = self
                .trees
                .obligations
                .iter()
                .find(|obligation| {
                    obligation.node == file.node_id(predicate)
                        && matches!(&obligation.kind,
                    ObligationKind::ProveGuarantee { machine, predicate } if machine == &signature.machine && predicate == &obligation.site)
                })
                .ok_or("the selected postcondition has no typed proposition")?;
            let request = ProofRequest {
                invocation,
                claim: InvocationClaim::Ensures(obligation),
            };
            execution
                .steps
                .push(Step::CheckInvocationProof(request, Rc::clone(arguments)));
        }
        Ok(())
    }

    fn schedule_crash_routes<'a>(
        &'a self,
        invocation: InvocationSite<'a>,
        tree: &TypedTree<'_>,
        arguments: &Rc<CallBindings>,
        execution: &mut Execution<'a>,
    ) -> Result<(), EvaluationError> {
        let signature = invocation.signature;
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
                CrashCause::Trap => "the selected Trap route remains possible for the supplied arguments",
                CrashCause::Abort => "the selected Abort route remains possible for the supplied arguments",
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
                    let request = ProofRequest {
                        invocation,
                        claim: InvocationClaim::CrashDisproved { route, guard: obligation },
                    };
                    if self.has_invocation_proof(request, arguments.1.as_deref()) || needs_proof_evaluation(tree, obligation.node.node) {
                        // The stack runs all preconditions before this step. The guard
                        // uses the immutable entry operands before the body can run.
                        execution
                            .steps
                            .push(Step::CheckInvocationProof(request, Rc::clone(arguments)));
                        continue;
                    }
                    Step::Evaluate(obligation.node, ValueType::Bool, Scope::Fact(Rc::clone(arguments)))
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
        let step = if carrier == &ValueType::Bool || is_aggregate(carrier) || FloatFormat::for_type(carrier).is_some() {
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

fn needs_proof_evaluation(
    tree: &TypedTree<'_>,
    guard: usize,
) -> bool {
    let file = tree.resolved().file;
    (guard..file.nodes[guard].end).any(|node| {
        tree.semantics_at(node).any(|record| {
            matches!(record, NodeSemantics::Obligation(obligation)
                if matches!(obligation.kind, ObligationKind::MathematicalApplication { .. } | ObligationKind::MathematicalFact { .. }))
        })
    })
}

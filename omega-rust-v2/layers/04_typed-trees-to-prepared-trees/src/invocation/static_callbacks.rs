//! The evaluator binds a typed static callable to its exact selected body
//! and checks the published invocation dependencies before a branch can skip one.

use super::*;
use crate::TargetObservation;
use omega_typer::{CallableType, IndirectCall, InvocationTarget, MachineEffects, TypedTrees, TypingObligation};

#[derive(Clone, Debug, Eq, PartialEq)]
struct CallbackCaller {
    machine: DeclarationId,
    arguments: Vec<ValueType>,
    enclosing: Vec<(DeclarationId, Vec<ValueType>)>,
    evidence: Option<ValueType>,
}

impl CallbackCaller {
    fn from_application(call: &TypedCall) -> Self {
        Self {
            machine: call.machine.clone(),
            arguments: call.type_arguments.clone(),
            enclosing: call.enclosing_arguments.clone(),
            evidence: call.evidence.clone(),
        }
    }

    fn matches(
        &self,
        call: &TypedCall,
    ) -> bool {
        self.machine == call.machine && self.arguments == call.type_arguments && self.enclosing == call.enclosing_arguments && self.evidence == call.evidence
    }
}

#[derive(Default)]
pub(crate) struct CallableRefinements<'trees, 'text> {
    pub(crate) checked: Vec<CheckedCallableRefinement<'trees, 'text>>,
    pub(crate) uses: Vec<CheckedCallableUse<'trees, 'text>>,
}

/// A verifier's callable refinement judgment for one typed occurrence.
pub struct CheckedCallableRefinement<'trees, 'text> {
    trees: &'trees TypedTrees<'text>,
    obligation: &'trees TypingObligation,
    caller: CallbackCaller,
    bound: ValueType,
    arguments: Vec<ValueType>,
    observations: Vec<TargetObservation>,
}

/// The checked entry requirements or result guarantees at one indirect call.
/// A result-free judgment covers the callable bound's entry requirements.
pub struct CheckedCallableUse<'trees, 'text> {
    trees: &'trees TypedTrees<'text>,
    call: &'trees IndirectCall,
    caller: CallbackCaller,
    bound: ValueType,
    machine: DeclarationId,
    arguments: Vec<ValueType>,
    operands: Vec<PreparedValue>,
    result: Option<PreparedValue>,
    observations: Vec<TargetObservation>,
}

pub(super) struct CallableUse {
    site: TreeNodeId,
    use_site: CallUse,
    caller: CallbackCaller,
    bound: ValueType,
    machine: DeclarationId,
    arguments: Vec<ValueType>,
}

struct CallbackSelection<'a> {
    machine: &'a DeclarationId,
    arguments: Vec<ValueType>,
}

impl CallbackSelection<'_> {
    fn instantiate(
        &self,
        value: &ValueType,
    ) -> ValueType {
        value.substitute(self.machine, &self.arguments)
    }

    fn enter_application(
        &self,
        call: &IndirectCall,
        signature: &TypedSignature,
        scope: &Scope<'_>,
    ) -> Option<Rc<TypedCall>> {
        if self.arguments.is_empty() {
            return None;
        }
        let operands = call
            .operands
            .iter()
            .zip(&signature.parameters)
            .map(|(operand, (span, _))| {
                let mut parameter = signature.site.clone();
                parameter.span = *span;
                AppliedOperand {
                    node: operand.node,
                    site: operand.site.clone(),
                    parameter,
                    subject: scope.instantiate(&operand.subject),
                    callable: operand.callable.clone(),
                }
            })
            .collect();
        Some(Rc::new(TypedCall {
            node: call.node,
            use_site: call.use_site,
            site: call.site.clone(),
            machine: self.machine.clone(),
            type_arguments: self.arguments.clone(),
            operands,
            enclosing_arguments: vec![],
            caller: call.caller.clone(),
            acknowledgements: Default::default(),
            evidence: None,
            result: self.instantiate(&signature.result),
            layout_query: None,
        }))
    }
}

impl ConstantEvaluation<'_, '_> {
    #[allow(
        clippy::too_many_arguments,
        reason = "refinement identifies its typed obligation and selected callback application"
    )]
    pub(super) fn admit_static_callable_refinement(
        &self,
        obligation: &TypingObligation,
        bound: &CallableType,
        machine: &DeclarationId,
        arguments: &[ValueType],
        caller: Option<&TypedCall>,
        scope: &Scope<'_>,
        execution: &Execution<'_>,
    ) -> Result<(), EvaluationError> {
        let selection = CallbackSelection {
            machine,
            arguments: arguments
                .iter()
                .map(|argument| scope.instantiate(argument))
                .collect(),
        };
        let Some(caller) = caller else {
            return Err("the static callable refinement has no closed caller application".into());
        };
        if !caller.type_arguments.iter().any(
            |selected| matches!(selected, ValueType::MachineSelection(declaration, applied) if declaration.as_ref() == machine && applied == &selection.arguments),
        ) {
            return Err("the static callable refinement does not identify the selected application".into());
        }
        let signature = self.admit_callback_body(&selection, execution)?;
        admit_callback_contract(bound, scope.application(), &selection, signature)?;
        self.admit_callback_refinement(Some(obligation), Some(caller), scope.application(), bound, &selection, signature)
    }

    pub(super) fn admit_callback_effects(
        &self,
        effects: &MachineEffects,
        application: Option<&TypedCall>,
        execution: &Execution<'_>,
    ) -> Result<(), EvaluationError> {
        for entry in &effects.invokes {
            self.admit_callback_entry(entry, application, execution)?;
        }
        for &index in effects.callback_pause.keys() {
            self.admit_callback_entry(
                &InvocationTarget::StaticParameter {
                    owner: effects.machine.clone(),
                    index,
                },
                application,
                execution,
            )?;
        }
        Ok(())
    }

    fn admit_callback_entry(
        &self,
        entry: &InvocationTarget,
        application: Option<&TypedCall>,
        execution: &Execution<'_>,
    ) -> Result<(), EvaluationError> {
        let selection = selected_callback(entry, application)?;
        let signature = self.admit_callback_body(&selection, execution)?;
        if let InvocationTarget::StaticParameter { owner, index } = entry {
            let contract = self
                .trees
                .callable_bounds
                .iter()
                .find(|(binder, position, _)| binder == owner && position == index)
                .map(|(_, _, contract)| contract)
                .ok_or("the static callback has no typed bound")?;
            admit_callback_contract(contract, application, &selection, signature)?;
            self.admit_callback_refinement(None, application, application, contract, &selection, signature)?;
        }
        Ok(())
    }

    pub(super) fn schedule_static_callback<'a>(
        &'a self,
        tree: &TypedTree<'_>,
        site: TreeNodeId,
        scope: &Scope<'a>,
        carrier: &ValueType,
        execution: &mut Execution<'a>,
    ) -> Result<(), EvaluationError> {
        let mut selected = None;
        for record in tree.semantics_at(site.node) {
            match record {
                NodeSemantics::Expression(_) => {}
                NodeSemantics::IndirectCall(call) if call.use_site == scope.call_use() && selected.is_none() => selected = Some(call),
                NodeSemantics::Obligation(obligation) if scope.call_use() == CallUse::Fact && matches!(obligation.kind, ObligationKind::TotalCrashGuard) => {}
                _ => return Err(UNSUPPORTED_CONTRACT.into()),
            }
        }
        let call = selected.ok_or(UNSUPPORTED_BODY)?;
        let InvocationTarget::StaticParameter { .. } = &call.entry else {
            return Err("preparation requires an exact static callback selection".into());
        };
        let caller = scope
            .application()
            .ok_or("the static callback has no closed caller application")?;
        let selection = selected_callback(&call.entry, Some(caller))?;
        let signature = self.admit_callback_body(&selection, execution)?;
        admit_callback_contract(&call.contract, Some(caller), &selection, signature)?;
        self.admit_callback_refinement(None, Some(caller), Some(caller), &call.contract, &selection, signature)?;
        if &selection.instantiate(&signature.result) != carrier || call.operands.len() != signature.parameters.len() {
            return Err("the selected static callback disagrees with the typed call".into());
        }
        let application = selection.enter_application(call, signature, scope);
        if call.contract.requires || call.contract.has_guarantee {
            execution.steps.push(Step::EnterCheckedCallback {
                request: CallableUse {
                    site,
                    use_site: call.use_site,
                    caller: CallbackCaller::from_application(caller),
                    bound: scope.instantiate(&ValueType::Callable(Box::new(call.contract.clone()))),
                    machine: selection.machine.clone(),
                    arguments: selection.arguments.clone(),
                },
                signature,
                application,
            });
        } else {
            execution.steps.push(Step::Enter {
                call: site,
                signature,
                requirement: None,
                application,
            });
        }
        for (argument, (_, parameter)) in call.operands.iter().zip(&signature.parameters).rev() {
            if argument.callable.is_some() {
                return Err("preparation cannot materialize a runtime callable argument".into());
            }
            execution
                .steps
                .push(Step::Evaluate(argument.node, selection.instantiate(parameter), scope.clone()));
        }
        Ok(())
    }

    pub(super) fn enter_checked_callback<'a>(
        &'a self,
        request: CallableUse,
        signature: &'a TypedSignature,
        application: Option<Rc<TypedCall>>,
        execution: &mut Execution<'a>,
    ) -> Result<(), EvaluationError> {
        let ValueType::Callable(bound) = &request.bound else {
            unreachable!("checked callback has a callable bound")
        };
        let first = execution.values.len() - signature.parameters.len();
        let operands = &execution.values[first..];
        if bound.requires {
            self.admit_callable_use(&request, operands, None)?;
        }
        let site = request.site;
        if bound.has_guarantee {
            let operands = operands.to_vec();
            execution
                .steps
                .push(Step::CheckCallableGuarantee { request, operands });
        }
        self.enter_call(site, signature, None, application, execution)
    }

    pub(super) fn admit_callable_use(
        &self,
        request: &CallableUse,
        operands: &[PreparedValue],
        result: Option<&PreparedValue>,
    ) -> Result<(), EvaluationError> {
        let tree = self.tree(request.site)?;
        for proof in &self.callable_refinements.uses {
            if !std::ptr::eq(proof.trees, self.trees)
                || proof.call.node != request.site
                || proof.call.use_site != request.use_site
                || proof.caller != request.caller
                || proof.bound != request.bound
                || proof.machine != request.machine
                || proof.arguments != request.arguments
                || proof.operands != operands
                || proof.result.as_ref() != result
            {
                continue;
            }
            if !tree
                .semantics_at(request.site.node)
                .any(|record| matches!(record, NodeSemantics::IndirectCall(call) if std::ptr::eq(call, proof.call)))
            {
                continue;
            }
            if !proof.observations.iter().all(|used| {
                self.target
                    .is_some_and(|target| target.matches_observation(used))
            }) {
                continue;
            }
            for used in &proof.observations {
                self.record_observation(used.clone());
            }
            return Ok(());
        }
        let call = tree
            .semantics_at(request.site.node)
            .find_map(|record| match record {
                NodeSemantics::IndirectCall(call) if call.use_site == request.use_site => Some(call),
                _ => None,
            })
            .ok_or(UNSUPPORTED_CONTRACT)?;
        let reason = if result.is_some() {
            "the callable bound's guarantee needs a checked judgment for the arguments and result"
        } else {
            "the callable bound's precondition needs a checked judgment for the supplied arguments"
        };
        Err(EvaluationError::AtCall(crate::refusal(&call.site, reason)))
    }

    fn admit_callback_refinement(
        &self,
        typed: Option<&TypingObligation>,
        caller: Option<&TypedCall>,
        application: Option<&TypedCall>,
        bound: &CallableType,
        selection: &CallbackSelection<'_>,
        signature: &TypedSignature,
    ) -> Result<(), EvaluationError> {
        if !bound.requires && !bound.has_guarantee && self.unrestricted_entry(signature)? {
            return Ok(());
        }
        let Some(caller) = caller else {
            return Err("the exact static callback needs a checked refinement of its callable contract".into());
        };
        let origin = caller.node;
        let closed_bound = ValueType::Callable(Box::new(bound.clone()));
        let closed_bound = application.map_or_else(|| closed_bound.clone(), |applied| applied.instantiate(&closed_bound));
        let tree = self.tree(origin)?;
        for proof in &self.callable_refinements.checked {
            if !std::ptr::eq(proof.trees, self.trees)
                || proof.obligation.node != origin
                || !proof.caller.matches(caller)
                || typed.is_some_and(|obligation| !std::ptr::eq(obligation, proof.obligation))
                || proof.bound != closed_bound
                || proof.arguments != selection.arguments
                || !matches!(&proof.obligation.kind, ObligationKind::RefineCallable { machine, .. } if machine == selection.machine)
            {
                continue;
            }
            if !tree
                .semantics_at(origin.node)
                .any(|record| matches!(record, NodeSemantics::Obligation(obligation) if std::ptr::eq(obligation, proof.obligation)))
            {
                continue;
            }
            if !proof.observations.iter().all(|observation| {
                self.target
                    .is_some_and(|target| target.matches_observation(observation))
            }) {
                continue;
            }
            for observation in &proof.observations {
                self.record_observation(observation.clone());
            }
            return Ok(());
        }
        Err("the exact static callback needs a checked refinement of its callable contract".into())
    }

    fn unrestricted_entry(
        &self,
        signature: &TypedSignature,
    ) -> Result<bool, EvaluationError> {
        if !signature.requires {
            return Ok(true);
        }
        let tree = self.tree(signature.node)?;
        let file = tree.resolved().file;
        let mut requirements = file.nodes[signature.node.node]
            .children
            .iter()
            .filter(|&&node| file.nodes[node].kind == NodeKind::Requires)
            .peekable();
        Ok(requirements.peek().is_some()
            && requirements.all(|&node| {
                let [predicate] = file.nodes[node].children.as_slice() else { return false };
                let predicate = file.ungroup(*predicate);
                matches!(tree.resolved().symbol(predicate), Some(Symbol::Builtin(name)) if name == "true")
                    && tree
                        .semantics_at(predicate)
                        .any(|record| matches!(record, NodeSemantics::Expression(expression) if expression.ty == ValueType::Bool))
            }))
    }
    fn admit_callback_body(
        &self,
        selection: &CallbackSelection<'_>,
        execution: &Execution<'_>,
    ) -> Result<&TypedSignature, EvaluationError> {
        let &(index, signature) = self
            .signatures()
            .get(selection.machine)
            .ok_or("the static callback has no typed signature")?;
        let tree = self.tree(signature.node)?;
        let file = tree.resolved().file;
        let declaration = signature.node.node;
        if tree.resolved().declaration_at(declaration) != Some(selection.machine)
            || file.child(declaration, NodeKind::Body).is_none()
            || file.generic_parameters(declaration).count() != selection.arguments.len()
            || selection.arguments.iter().any(ValueType::depends_on_call_input)
            || file.child(declaration, NodeKind::Hypothesis).is_some()
            || file.nodes[declaration].has_public_modifier() && !signature.terminates
            || signature.satisfies.is_some()
            || !signature.multiplicity_bounds.is_empty()
        {
            return Err("the exact static callback needs checked contract evidence outside the supported fragment".into());
        }
        if self
            .boundary_slots
            .iter()
            .chain(self.trees.boundary_calls.iter().map(|(_, slot)| slot))
            .any(|slot| {
                &slot.requirement.declaration == selection.machine && slot.arguments == selection.arguments && slot.requirement.trait_arguments.is_empty()
            })
        {
            return Err("a runtime boundary selection cannot execute as a static callback".into());
        }
        let effects = self
            .machine_effects(index)
            .ok_or("the exact static callback has no complete operational bounds")?;
        if !effects.reach.is_empty() || effects.pause.suspend || effects.pause.block || !effects.invokes.is_empty() {
            return Err("the exact static callback has unresolved or nonempty operational effects".into());
        }
        self.admit_callback_termination(index, signature, &tree, &selection.arguments, execution)?;
        Ok(signature)
    }
}

fn selected_callback<'a>(
    entry: &'a InvocationTarget,
    application: Option<&'a TypedCall>,
) -> Result<CallbackSelection<'a>, EvaluationError> {
    match entry {
        InvocationTarget::Declaration(slot) if slot.trait_arguments.is_empty() => Ok(CallbackSelection {
            machine: &slot.declaration,
            arguments: vec![],
        }),
        InvocationTarget::AppliedMachine { declaration, arguments } => {
            let arguments = arguments
                .iter()
                .map(|argument| application.map_or_else(|| argument.clone(), |applied| applied.instantiate(argument)))
                .collect();
            Ok(CallbackSelection {
                machine: declaration,
                arguments,
            })
        }
        InvocationTarget::StaticParameter { owner, index } => {
            let ValueType::MachineSelection(machine, arguments) = application
                .filter(|application| &application.machine == owner)
                .and_then(|application| application.type_arguments.get(*index))
                .ok_or("the static callback has no closed selection")?
            else {
                return Err("the static callback has no closed selection".into());
            };
            Ok(CallbackSelection {
                machine,
                arguments: arguments.clone(),
            })
        }
        _ => Err("preparation requires an exact static callback selection".into()),
    }
}

fn admit_callback_contract(
    contract: &CallableType,
    application: Option<&TypedCall>,
    selection: &CallbackSelection<'_>,
    signature: &TypedSignature,
) -> Result<(), EvaluationError> {
    let instantiate = |value: &ValueType| application.map_or_else(|| value.clone(), |applied| applied.instantiate(value));
    if contract.requirement.is_some()
        || contract.crash_ceiling.as_ref().is_some_and(|ceiling| {
            signature
                .crash_ceiling
                .as_ref()
                .is_none_or(|causes| causes.iter().any(|cause| !ceiling.contains(cause)))
        })
        || instantiate(&contract.result) != selection.instantiate(&signature.result)
        || !contract.parameters.iter().map(instantiate).eq(signature
            .parameters
            .iter()
            .map(|(_, parameter)| selection.instantiate(parameter)))
    {
        return Err("the exact static callback needs checked refinement of its callable contract".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests;

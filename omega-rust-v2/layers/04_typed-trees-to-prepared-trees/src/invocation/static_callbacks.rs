//! The evaluator binds a typed static callable to its exact selected body
//! and checks the published invocation dependencies before a branch can skip one.

use super::*;
use crate::TargetObservation;
use omega_typed::{CallableType, IndirectCall, InvocationTarget, MachineApplication, MachineEffects, TypedTrees, TypingObligation};

#[derive(Clone, Debug, Eq, PartialEq)]
struct CallbackCaller {
    node: TreeNodeId,
    machine: DeclarationId,
    arguments: Vec<ValueType>,
    enclosing: Vec<(DeclarationId, Vec<ValueType>)>,
    evidence: Option<ValueType>,
}

impl CallbackCaller {
    fn from_application(call: &TypedCall) -> Self {
        Self {
            node: call.node,
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
        self.node == call.node
            && self.machine == call.machine
            && self.arguments == call.type_arguments
            && self.enclosing == call.enclosing_arguments
            && self.evidence == call.evidence
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
    selection: MachineApplication,
    observations: Vec<TargetObservation>,
}

/// The checked entry requirements or result guarantees at one indirect call.
/// A result-free judgment covers the callable bound's entry requirements.
pub struct CheckedCallableUse<'trees, 'text> {
    trees: &'trees TypedTrees<'text>,
    call: &'trees IndirectCall,
    caller: CallbackCaller,
    bound: ValueType,
    selection: MachineApplication,
    operands: Vec<PreparedValue>,
    result: Option<PreparedValue>,
    observations: Vec<TargetObservation>,
}

pub(super) struct CallableUse {
    site: TreeNodeId,
    use_site: CallUse,
    caller: CallbackCaller,
    bound: ValueType,
    selection: MachineApplication,
}

// A checked refinement belongs to one static formal in one active application.
#[derive(Clone, PartialEq, Eq, Hash)]
pub(super) struct CallbackBinding {
    owner: DeclarationId,
    index: usize,
    bound: ValueType,
    captured: Rc<[CallbackBinding]>,
}

fn callback_application(
    selection: &MachineApplication,
    call: &IndirectCall,
    signature: &TypedSignature,
    scope: &Scope<'_>,
) -> Option<Rc<TypedCall>> {
    if selection.arguments.is_empty() && selection.enclosing_arguments.is_empty() {
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
        machine: selection.machine.as_ref().clone(),
        type_arguments: selection.arguments.clone(),
        operands,
        enclosing_arguments: selection.enclosing_arguments.clone(),
        caller: call.caller.clone(),
        acknowledgements: Default::default(),
        evidence: None,
        result: selection.instantiate(&signature.result),
        layout_query: None,
    }))
}

impl ConstantEvaluation<'_, '_> {
    pub(super) fn forward_callback_bindings(
        template: &TypedCall,
        selected: &TypedCall,
        scope: &Scope<'_>,
    ) -> Vec<CallbackBinding> {
        project_callback_bindings(
            callback_parameters(&template.machine, &template.type_arguments, &template.enclosing_arguments),
            |owner, index| selected.generic_argument(owner, index),
            scope.application().map(CallbackArguments::Call),
            &scope.bindings().callback_bindings,
        )
    }

    pub(super) fn admit_static_callable_refinement(
        &self,
        obligation: &TypingObligation,
        caller: Option<&TypedCall>,
        scope: &Scope<'_>,
        execution: &Execution<'_>,
    ) -> Result<CallbackBinding, EvaluationError> {
        let caller = caller.ok_or(PreparationReason::RefinementWithoutCaller)?;
        let (selection, binding) = self.checked_callback_binding(
            obligation,
            caller,
            scope.application().map(CallbackArguments::Call),
            &scope.bindings().callback_bindings,
        )?;
        self.admit_callback_body(&selection, &binding.captured, execution)?;
        Ok(binding)
    }

    fn checked_callback_binding(
        &self,
        obligation: &TypingObligation,
        caller: &TypedCall,
        parent: Option<CallbackArguments<'_>>,
        bindings: &[CallbackBinding],
    ) -> Result<(MachineApplication, CallbackBinding), EvaluationError> {
        let ObligationKind::RefineCallable {
            formal: Some((owner, index)),
            contract,
            selection: template,
        } = &obligation.kind
        else {
            return Err(UNSUPPORTED_CONTRACT.into());
        };
        let close = |value: &ValueType| parent.map_or_else(|| value.clone(), |application| application.instantiate(value));
        let selection = template.with_substitutions(close);
        if &caller.machine != owner
            || !matches!(caller.type_arguments.get(*index),
                Some(ValueType::MachineSelection(applied)) if applied == &selection)
        {
            return Err(PreparationReason::RefinementApplicationMismatch.into());
        }
        let captured = capture_callback_bindings(template, &selection, parent, bindings);
        let &(_, signature) = self
            .signatures()
            .get(selection.machine.as_ref())
            .ok_or(PreparationReason::CallbackWithoutSignature)?;
        admit_callback_contract(contract, parent, &selection, signature)?;
        let bound = close(&ValueType::Callable(contract.clone()));
        self.check_callback_refinement(obligation, caller, &bound, &selection, signature)?;
        Ok((
            selection,
            CallbackBinding {
                owner: owner.clone(),
                index: *index,
                bound,
                captured,
            },
        ))
    }

    /// Carry forwarded proofs and check fresh selections at the actual direct
    /// call, before two paths can converge on the same closed application.
    pub(super) fn callback_bindings_at_edge(
        &self,
        tree: &TypedTree<'_>,
        call: &TypedCall,
        parent: &CallbackApplication,
        selected: &MachineApplication,
    ) -> Result<Rc<[CallbackBinding]>, EvaluationError> {
        let caller = Some(CallbackArguments::Selection(&parent.selection));
        let mut bindings = project_callback_bindings(
            callback_parameters(&call.machine, &call.type_arguments, &call.enclosing_arguments),
            |owner, index| machine_argument(selected, owner, index),
            caller,
            &parent.bindings,
        );
        let mut closed_call = None;
        for record in tree.semantics_at(call.node.node) {
            if let NodeSemantics::Obligation(obligation) = record
                && matches!(obligation.kind, ObligationKind::RefineCallable { formal: Some(_), .. })
            {
                let closed = closed_call.get_or_insert_with(|| call.with_substitutions(|value| parent.selection.instantiate(value)));
                let (_, binding) = self.checked_callback_binding(obligation, closed, caller, &parent.bindings)?;
                bindings.push(binding);
            }
        }
        Ok(bindings.into())
    }

    pub(super) fn admit_callback_effects(
        &self,
        effects: &MachineEffects,
        application: Option<&TypedCall>,
        bindings: &[CallbackBinding],
        execution: &Execution<'_>,
    ) -> Result<(), EvaluationError> {
        for entry in callback_entries(effects) {
            let (selection, captured) = self.select_callback(&entry, application.map(CallbackArguments::Call), bindings)?;
            self.admit_callback_body(&selection, &captured, execution)?;
        }
        Ok(())
    }

    fn select_callback(
        &self,
        entry: &InvocationTarget,
        application: Option<CallbackArguments<'_>>,
        bindings: &[CallbackBinding],
    ) -> Result<(MachineApplication, Rc<[CallbackBinding]>), EvaluationError> {
        let (selection, captured) = selected_callback(entry, application, bindings)?;
        if let InvocationTarget::StaticParameter { owner, index } = entry {
            let contract = self
                .trees
                .callable_bounds
                .iter()
                .find(|(binder, position, _)| binder == owner && position == index)
                .map(|(_, _, contract)| contract)
                .ok_or(PreparationReason::CallbackWithoutBound)?;
            let &(_, signature) = self
                .signatures()
                .get(selection.machine.as_ref())
                .ok_or(PreparationReason::CallbackWithoutSignature)?;
            admit_callback_contract(contract, application, &selection, signature)?;
            self.admit_bound_callback(bindings, owner, *index, application, contract, signature)?;
        }
        Ok((selection, captured))
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
        let InvocationTarget::StaticParameter { owner, index } = &call.entry else {
            return Err(PreparationReason::InexactCallbackSelection.into());
        };
        let caller = scope
            .application()
            .ok_or(PreparationReason::CallbackWithoutCallerApplication)?;
        let (selection, captured) = selected_callback(&call.entry, Some(CallbackArguments::Call(caller)), &scope.bindings().callback_bindings)?;
        let signature = self.admit_callback_body(&selection, &captured, execution)?;
        admit_callback_contract(&call.contract, Some(CallbackArguments::Call(caller)), &selection, signature)?;
        self.admit_bound_callback(
            &scope.bindings().callback_bindings,
            owner,
            *index,
            Some(CallbackArguments::Call(caller)),
            &call.contract,
            signature,
        )?;
        if &selection.instantiate(&signature.result) != carrier || call.operands.len() != signature.parameters.len() {
            return Err(PreparationReason::CallbackDisagreesWithCall.into());
        }
        let application = callback_application(&selection, call, signature, scope);
        if call.contract.requires || call.contract.has_guarantee {
            execution.steps.push(Step::EnterCheckedCallback {
                request: CallableUse {
                    site,
                    use_site: call.use_site,
                    caller: CallbackCaller::from_application(caller),
                    bound: scope.instantiate(&ValueType::Callable(Box::new(call.contract.clone()))),
                    selection: selection.clone(),
                },
                signature,
                application,
                callback_bindings: captured,
            });
        } else {
            execution.steps.push(Step::Enter {
                call: site,
                signature,
                requirement: None,
                application,
                callback_bindings: captured,
            });
        }
        for (argument, (_, parameter)) in call.operands.iter().zip(&signature.parameters).rev() {
            if argument.callable.is_some() {
                return Err(PreparationReason::RuntimeCallableArgument.into());
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
        callback_bindings: Rc<[CallbackBinding]>,
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
        self.enter_call(site, signature, None, application, callback_bindings, execution)
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
                || proof.selection != request.selection
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
            PreparationReason::CallableGuaranteeNeedsJudgment
        } else {
            PreparationReason::CallablePreconditionNeedsJudgment
        };
        Err(EvaluationError::Located(crate::evaluation::refusal(&call.site, reason)))
    }

    fn admit_bound_callback(
        &self,
        bindings: &[CallbackBinding],
        owner: &DeclarationId,
        index: usize,
        application: Option<CallbackArguments<'_>>,
        bound: &CallableType,
        signature: &TypedSignature,
    ) -> Result<(), EvaluationError> {
        if !bound.requires && !bound.has_guarantee && self.unrestricted_entry(signature)? {
            return Ok(());
        }
        let bound = ValueType::Callable(Box::new(bound.clone()));
        let bound = application.map_or_else(|| bound.clone(), |applied| applied.instantiate(&bound));
        if bindings
            .iter()
            .any(|binding| &binding.owner == owner && binding.index == index && binding.bound == bound)
        {
            return Ok(());
        }
        Err(PreparationReason::CallbackNeedsRefinement.into())
    }

    fn check_callback_refinement(
        &self,
        obligation: &TypingObligation,
        caller: &TypedCall,
        closed_bound: &ValueType,
        selection: &MachineApplication,
        signature: &TypedSignature,
    ) -> Result<(), EvaluationError> {
        if let ValueType::Callable(bound) = closed_bound
            && !bound.requires
            && !bound.has_guarantee
            && self.unrestricted_entry(signature)?
        {
            return Ok(());
        }
        let origin = caller.node;
        let tree = self.tree(origin)?;
        for proof in &self.callable_refinements.checked {
            if !std::ptr::eq(proof.trees, self.trees)
                || proof.obligation.node != origin
                || !proof.caller.matches(caller)
                || !std::ptr::eq(obligation, proof.obligation)
                || &proof.bound != closed_bound
                || &proof.selection != selection
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
        Err(PreparationReason::CallbackNeedsRefinement.into())
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
        selection: &MachineApplication,
        bindings: &Rc<[CallbackBinding]>,
        execution: &Execution<'_>,
    ) -> Result<&TypedSignature, EvaluationError> {
        let root = self.admit_callback_local_body(selection, bindings, execution)?;
        let mut active = termination::ActiveApplications::default();
        active.enter_selection(root.signature.node, selection);
        let mut pending = Vec::new();
        self.schedule_callback_dependencies(&root, selection, bindings, &mut pending)?;
        while let Some(step) = pending.pop() {
            match step {
                CallbackDependency::Leave(node) => active.leave(node),
                CallbackDependency::Enter(selection, bindings, fixed) => {
                    let body = self.admit_callback_local_body(&selection, &bindings, execution)?;
                    if active.would_cycle(body.signature.node, &selection, fixed) {
                        return Err(PreparationReason::CyclicCallbackWithoutTermination.into());
                    }
                    active.enter_selection(body.signature.node, &selection);
                    pending.push(CallbackDependency::Leave(body.signature.node));
                    self.schedule_callback_dependencies(&body, &selection, &bindings, &mut pending)?;
                }
            }
        }
        Ok(root.signature)
    }

    fn schedule_callback_dependencies(
        &self,
        body: &AdmittedCallback<'_>,
        selection: &MachineApplication,
        bindings: &[CallbackBinding],
        pending: &mut Vec<CallbackDependency>,
    ) -> Result<(), EvaluationError> {
        let applications = std::iter::once((body.effects, selection, bindings)).chain(
            body.fact_selections
                .iter()
                .map(|(effects, application)| (*effects, &application.selection, application.bindings.as_ref())),
        );
        for (effects, application, bindings) in applications {
            for entry in callback_entries(effects) {
                let (selected, captured) = self.select_callback(&entry, Some(CallbackArguments::Selection(application)), bindings)?;
                let fixed = match &entry {
                    InvocationTarget::Declaration(_) => true,
                    InvocationTarget::AppliedMachine(template) => !template.all_arguments().any(ValueType::depends_on_call_input),
                    _ => false,
                };
                pending.push(CallbackDependency::Enter(selected, captured, fixed));
            }
        }
        Ok(())
    }

    fn admit_callback_local_body(
        &self,
        selection: &MachineApplication,
        bindings: &Rc<[CallbackBinding]>,
        execution: &Execution<'_>,
    ) -> Result<AdmittedCallback<'_>, EvaluationError> {
        let &(index, signature) = self
            .signatures()
            .get(selection.machine.as_ref())
            .ok_or(PreparationReason::CallbackWithoutSignature)?;
        let tree = self.tree(signature.node)?;
        let file = tree.resolved().file;
        let declaration = signature.node.node;
        if tree.resolved().declaration_at(declaration) != Some(selection.machine.as_ref())
            || file.child(declaration, NodeKind::Body).is_none()
            || file.generic_parameters(declaration).count() != selection.arguments.len()
            || selection.all_arguments().any(ValueType::depends_on_call_input)
            || file.child(declaration, NodeKind::Hypothesis).is_some()
            || tree.resolved().is_exported(declaration) && !signature.terminates
            || signature.satisfies.is_some()
            || !signature.multiplicity_bounds.is_empty()
        {
            return Err(PreparationReason::CallbackContractOutsideFragment.into());
        }
        let mut trait_scopes = selection
            .enclosing_arguments
            .iter()
            .filter(|(owner, _)| owner.kind == NodeKind::Trait);
        let trait_arguments = trait_scopes
            .next()
            .map_or(&[][..], |(_, arguments)| arguments.as_slice());
        let ambiguous_trait_scope = trait_scopes.next().is_some();
        if self
            .boundary_slots
            .iter()
            .chain(self.trees.boundary_calls.iter().map(|(_, slot)| slot))
            .any(|slot| {
                &slot.requirement.declaration == selection.machine.as_ref()
                    && slot.arguments == selection.arguments
                    && (ambiguous_trait_scope || slot.requirement.trait_arguments == trait_arguments)
            })
        {
            return Err(PreparationReason::RuntimeBoundaryCallback.into());
        }
        let effects = self
            .machine_effects(index)
            .ok_or(PreparationReason::CallbackWithoutOperationalBounds)?;
        if !effects.reach.is_empty() || effects.fixed_pause.suspend || effects.fixed_pause.block {
            return Err(PreparationReason::CallbackEffectsNotEmpty.into());
        }
        let fact_selections = self.admit_callback_termination(
            index,
            CallbackApplication {
                selection: selection.clone(),
                bindings: Rc::clone(bindings),
            },
            execution,
        )?;
        Ok(AdmittedCallback {
            signature,
            effects,
            fact_selections,
        })
    }
}

struct AdmittedCallback<'a> {
    signature: &'a TypedSignature,
    effects: &'a MachineEffects,
    fact_selections: Vec<(&'a MachineEffects, CallbackApplication)>,
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub(super) struct CallbackApplication {
    pub(super) selection: MachineApplication,
    pub(super) bindings: Rc<[CallbackBinding]>,
}

enum CallbackDependency {
    Enter(MachineApplication, Rc<[CallbackBinding]>, bool),
    Leave(TreeNodeId),
}

/// Re-entering the same declaration is finite when its closed application
/// is a proper subterm of the previous selection, as in apply::<apply::<bump>>.
/// Merely changing an argument cannot establish descent.
pub(super) fn nested_callback_selection<'a>(
    selected: &MachineApplication,
    arguments: impl Iterator<Item = &'a ValueType>,
) -> bool {
    let mut pending: Vec<_> = arguments.collect();
    while let Some(argument) = pending.pop() {
        if let ValueType::MachineSelection(candidate) = argument {
            if candidate == selected {
                return true;
            }
            pending.extend(candidate.all_arguments());
        }
    }
    false
}

fn callback_entries(effects: &MachineEffects) -> impl Iterator<Item = InvocationTarget> + '_ {
    effects.invokes.iter().cloned().chain(
        effects
            .callback_pause
            .keys()
            .map(|&index| InvocationTarget::StaticParameter {
                owner: effects.machine.clone(),
                index,
            }),
    )
}

#[derive(Clone, Copy)]
enum CallbackArguments<'a> {
    Call(&'a TypedCall),
    Selection(&'a MachineApplication),
}

impl<'a> CallbackArguments<'a> {
    fn instantiate(
        self,
        value: &ValueType,
    ) -> ValueType {
        match self {
            Self::Call(call) => call.instantiate(value),
            Self::Selection(selection) => selection.instantiate(value),
        }
    }

    fn generic_argument(
        self,
        owner: &DeclarationId,
        index: usize,
    ) -> Option<&'a ValueType> {
        match self {
            Self::Call(call) => call.generic_argument(owner, index),
            Self::Selection(selection) => machine_argument(selection, owner, index),
        }
    }
}

fn machine_argument<'a>(
    selection: &'a MachineApplication,
    owner: &DeclarationId,
    index: usize,
) -> Option<&'a ValueType> {
    if owner == selection.machine.as_ref() {
        return selection.arguments.get(index);
    }
    selection
        .enclosing_arguments
        .iter()
        .find(|(enclosing, _)| enclosing == owner)
        .and_then(|(_, arguments)| arguments.get(index))
}

fn callback_parameters<'a>(
    machine: &'a DeclarationId,
    arguments: &'a [ValueType],
    enclosing: &'a [(DeclarationId, Vec<ValueType>)],
) -> impl Iterator<Item = (&'a DeclarationId, usize, &'a ValueType)> {
    enclosing
        .iter()
        .map(|(owner, arguments)| (owner, arguments.as_slice()))
        .chain(std::iter::once((machine, arguments)))
        .flat_map(|(owner, arguments)| {
            arguments
                .iter()
                .enumerate()
                .map(move |(index, argument)| (owner, index, argument))
        })
}

fn project_callback_bindings<'a, 'selected>(
    parameters: impl Iterator<Item = (&'a DeclarationId, usize, &'a ValueType)>,
    selected: impl Fn(&DeclarationId, usize) -> Option<&'selected ValueType>,
    caller: Option<CallbackArguments<'_>>,
    bindings: &[CallbackBinding],
) -> Vec<CallbackBinding> {
    let Some(caller) = caller else { return vec![] };
    // The typed argument supplies the originating formal before substitution
    // erases it. Matching the same concrete selection in another slot carries no proof.
    parameters
        .filter_map(|(destination, index, argument)| {
            let ValueType::Parameter { owner, index: source_index } = argument else {
                return None;
            };
            if caller.generic_argument(owner, *source_index) != selected(destination, index) {
                return None;
            }
            let source = bindings
                .iter()
                .find(|binding| &binding.owner == owner.as_ref() && binding.index == *source_index)?;
            Some(CallbackBinding {
                owner: destination.clone(),
                index,
                bound: source.bound.clone(),
                captured: Rc::clone(&source.captured),
            })
        })
        .collect()
}

fn capture_callback_bindings(
    template: &MachineApplication,
    selected: &MachineApplication,
    caller: Option<CallbackArguments<'_>>,
    bindings: &[CallbackBinding],
) -> Rc<[CallbackBinding]> {
    project_callback_bindings(
        callback_parameters(&template.machine, &template.arguments, &template.enclosing_arguments),
        |owner, index| machine_argument(selected, owner, index),
        caller,
        bindings,
    )
    .into()
}

fn selected_callback(
    entry: &InvocationTarget,
    application: Option<CallbackArguments<'_>>,
    bindings: &[CallbackBinding],
) -> Result<(MachineApplication, Rc<[CallbackBinding]>), EvaluationError> {
    match entry {
        InvocationTarget::Declaration(slot) if slot.trait_arguments.is_empty() => Ok((
            MachineApplication {
                machine: Box::new(slot.declaration.clone()),
                arguments: vec![],
                enclosing_arguments: vec![],
            },
            Rc::default(),
        )),
        InvocationTarget::AppliedMachine(template) => {
            let selected = template.with_substitutions(|argument| application.map_or_else(|| argument.clone(), |applied| applied.instantiate(argument)));
            let captured = capture_callback_bindings(template, &selected, application, bindings);
            Ok((selected, captured))
        }
        InvocationTarget::StaticParameter { owner, index } => {
            let Some(ValueType::MachineSelection(selection)) = application.and_then(|application| application.generic_argument(owner, *index)) else {
                return Err(PreparationReason::CallbackWithoutSelection.into());
            };
            let captured = bindings
                .iter()
                .find(|binding| &binding.owner == owner && binding.index == *index)
                .map_or_else(Rc::default, |binding| Rc::clone(&binding.captured));
            Ok((selection.clone(), captured))
        }
        _ => Err(PreparationReason::InexactCallbackSelection.into()),
    }
}

fn admit_callback_contract(
    contract: &CallableType,
    application: Option<CallbackArguments<'_>>,
    selection: &MachineApplication,
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
        return Err(PreparationReason::ForwardedCallbackNeedsRefinement.into());
    }
    Ok(())
}

#[cfg(test)]
mod tests;

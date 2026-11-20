use crate::contracts::effect_clauses;
use crate::expressions::body_typer::BodyTyper;
use crate::type_error::SourceTypeError;
use crate::{CallableType, IndirectCall, IndirectOperand, MachineApplication, ObligationKind, TypeError, ValueType};
use omega_resolved::DeclarationId;
use omega_tokens::SourceSpan;
use omega_typed::site_at;

impl BodyTyper<'_, '_> {
    pub(super) fn invocation_target(
        &self,
        node_index: usize,
    ) -> crate::InvocationTarget {
        let source = self.unit.file;
        let node_index = source.ungroup(node_index);
        if let Some(omega_resolved::Symbol::Declaration(id)) = self.unit.symbol(node_index)
            && matches!(id.kind, omega_syntax::NodeKind::Machine | omega_syntax::NodeKind::AsmMachine)
        {
            let enclosing_arguments: Vec<_> = self.catalog.types.lexical_machine_arguments(id).collect();
            if !enclosing_arguments.is_empty() {
                return crate::InvocationTarget::AppliedMachine(MachineApplication {
                    machine: Box::new(id.clone()),
                    arguments: Vec::new(),
                    enclosing_arguments,
                });
            }
            return crate::InvocationTarget::Declaration(crate::RequirementSlot {
                declaration: id.clone(),
                trait_arguments: Vec::new(),
            });
        }
        if let Some(omega_resolved::Symbol::Binding { span, .. }) = self.unit.symbol(node_index) {
            if let Some(ValueType::Parameter { owner, index }) = self.unit.parameters.get(span) {
                return crate::InvocationTarget::StaticParameter {
                    owner: *owner.clone(),
                    index: *index,
                };
            }
            if let Some(owner) = &self.caller
                && let Some(index) = self.catalog.signatures[owner]
                    .parameters
                    .iter()
                    .position(|(bind, _)| bind == span)
            {
                return crate::InvocationTarget::RuntimeParameter { owner: owner.clone(), index };
            }
        }
        crate::InvocationTarget::StoredCallable(site_at(source, node_index))
    }

    pub(super) fn static_callable(
        &self,
        span: SourceSpan,
        node_index: usize,
    ) -> Result<CallableType, TypeError> {
        match self
            .unit
            .parameters
            .get(&span)
            .and_then(|p| self.catalog.types.parameter_carrier(p))
        {
            Some(ValueType::Callable(c)) => Ok(*c.clone()),
            _ => Err(TypeError::at(self.unit.file, node_index, "static binder requires a callable contract")),
        }
    }
    pub(super) fn refine_callable(
        &mut self,
        node_index: usize,
        c: &CallableType,
        formal: Option<(DeclarationId, usize)>,
        selection: &MachineApplication,
    ) -> Result<(), TypeError> {
        let machine = selection.machine.as_ref();
        let signature = self
            .catalog
            .signatures
            .get(machine)
            .ok_or_else(|| TypeError::unsupported(self.unit.file, node_index))?;
        let instantiate = |ty: &ValueType| selection.instantiate(ty);
        if self.catalog.types.arity(machine) != selection.arguments.len()
            || !signature
                .parameters
                .iter()
                .map(|(_, t)| instantiate(t))
                .eq(c.parameters.iter().cloned())
            || instantiate(&signature.result) != c.result
        {
            return Err(TypeError::at(
                self.unit.file,
                node_index,
                "selected machine has incompatible callable signature",
            ));
        }
        if let Some(required) = &c.requirement
            && signature.satisfies.as_ref().is_none_or(|selected| {
                selected.declaration != required.declaration
                    || !selected
                        .trait_arguments
                        .iter()
                        .map(instantiate)
                        .eq(required.trait_arguments.iter().cloned())
            })
        {
            return Err(TypeError::at(self.unit.file, node_index, "selected machine lacks the exact satisfaction edge"));
        }
        let pause = signature.pause.declared();
        if pause.suspend && !c.pause.suspend || pause.block && !c.pause.block {
            return Err(TypeError::at(self.unit.file, node_index, "selected machine exceeds callable pause bounds"));
        }
        self.record_obligation(
            node_index,
            ObligationKind::RefineCallable {
                formal,
                contract: Box::new(c.clone()),
                selection: selection.clone(),
            },
        );
        Ok(())
    }
    pub(super) fn project_guarantee(
        &mut self,
        node: usize,
        callee: usize,
        args: usize,
        binder: SourceSpan,
    ) -> Result<ValueType, TypeError> {
        let source = self.unit.file;
        if self.context != super::body_typer::ExpressionContext::Proof {
            return Err(TypeError::at(source, callee, "logical evidence cannot be executed"));
        }
        let subjects = &source.nodes[args].children;
        let operands = subjects.iter().map(|&s| site_at(source, s)).collect();
        let (mut parameters, result, obligation) = if let Some(site) = self.unit.hypotheses.get(&binder) {
            let hypothesis = &self.catalog.hypotheses[site];
            if hypothesis.conclusions.is_empty() {
                return Err(TypeError::at(source, node, "logical guarantee needs its declared subjects and conclusion"));
            }
            let parameters = hypothesis.parameters.iter().map(|(_, ty)| ty.clone()).collect();
            (
                parameters,
                hypothesis.result.clone(),
                ObligationKind::ApplyHypothesis {
                    requirement: site.clone(),
                    operands,
                },
            )
        } else {
            let callable = self.static_callable(binder, callee)?;
            if !callable.has_guarantee {
                return Err(TypeError::at(source, callee, "callback has no declared guarantee"));
            }
            let projection = ObligationKind::ApplyCallbackGuarantee {
                binder: self.unit.parameters[&binder].clone(),
                contract: callable.contract,
                operands,
            };
            (callable.parameters, callable.result, projection)
        };
        if result != ValueType::NoResult {
            parameters.push(result);
        }
        if parameters.len() != subjects.len() {
            return Err(TypeError::at(source, node, "logical guarantee needs its declared subjects and conclusion"));
        }
        for (&subject, ty) in subjects.iter().zip(&parameters) {
            let value = self.type_expression_with_expected_type(subject, Some(ty))?;
            self.check_destination(subject, &value, ty)?;
        }
        self.record_obligation(node, obligation);
        Ok(ValueType::Bool)
    }

    pub(super) fn indirect(
        &mut self,
        node_index: usize,
        callee: usize,
        args: usize,
        c: &CallableType,
    ) -> Result<ValueType, TypeError> {
        let source = self.unit.file;
        let ack = effect_clauses::markers(self.unit, node_index)?;
        if c.pause.suspend && !ack.suspend || c.pause.block && !ack.block {
            return Err(TypeError::at(source, node_index, "callback requires its declared pause acknowledgement"));
        }
        self.unit
            .check_runtime_type(node_index, &c.result, &self.catalog.types)?;
        let params = &c.parameters;
        if params.len() != source.nodes[args].children.len() {
            return Err(TypeError::at(source, node_index, "incorrect callback arity"));
        }
        let mut operands = Vec::with_capacity(params.len());
        for (position, (&arg, p)) in source.nodes[args].children.iter().zip(params).enumerate() {
            self.unit.check_runtime_type(arg, p, &self.catalog.types)?;
            let operand = self.argument_operand(arg, Some(p).into())?;
            self.admit_argument(node_index, position, &operand, p)?;
            operands.push(IndirectOperand {
                node: source.node_id(arg),
                site: site_at(source, arg),
                subject: operand.subject(self.unit, p),
                callable: matches!(operand.value, ValueType::Callable(_)).then(|| self.invocation_target(arg)),
            });
        }
        let entry = self.invocation_target(callee);
        let use_site = self.record_call_use(node_index);
        self.output.indirect_calls.push(IndirectCall {
            node: source.node_id(node_index),
            use_site,
            operands,
            entry,
            acknowledgements: ack,
            site: site_at(source, node_index),
            callee: site_at(source, callee),
            caller: self.caller.clone(),
            contract: c.clone(),
        });
        Ok(c.result.clone())
    }
}

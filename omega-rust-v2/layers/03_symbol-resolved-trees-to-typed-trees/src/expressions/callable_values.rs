use crate::contracts::effect_clauses;
use crate::expressions::body_typer::BodyTyper;
use crate::{CallableType, IndirectCall, ObligationKind, Site, TypeError, ValueType};
use omega_lexer::SourceSpan;
use omega_resolver::DeclarationId;

impl BodyTyper<'_, '_> {
    pub(super) fn invocation_target(
        &self,
        node_index: usize,
    ) -> crate::InvocationTarget {
        let site = Site::at(self.unit.file, node_index);
        if let Some(omega_resolver::Symbol::Binding { span, .. }) = self.unit.symbol(node_index) {
            if let Some(ValueType::Parameter { owner, index }) = self.unit.parameters.get(span) {
                return crate::InvocationTarget::StaticParameter {
                    owner: *owner.clone(),
                    index: *index,
                };
            }
            if let Some(owner) = &self.caller
                && let Some(index) = self.catalog.signatures[owner]
                    .params
                    .iter()
                    .position(|(bind, _)| bind == span)
            {
                return crate::InvocationTarget::RuntimeParameter { owner: owner.clone(), index };
            }
        }
        crate::InvocationTarget::StoredCallable(site)
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
        machine: &DeclarationId,
    ) -> Result<(), TypeError> {
        let signature = self
            .catalog
            .signatures
            .get(machine)
            .ok_or_else(|| TypeError::unsupported(self.unit.file, node_index))?;
        if !signature.params.iter().map(|(_, t)| t).eq(&c.parameters) || signature.result != c.result || self.catalog.types.arities[machine] != 0 {
            return Err(TypeError::at(
                self.unit.file,
                node_index,
                "selected machine has incompatible callable signature",
            ));
        }
        if let Some(slot) = &c.requirement
            && !signature.satisfies.contains(slot)
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
                contract: Box::new(c.clone()),
                machine: machine.clone(),
            },
        );
        Ok(())
    }
    pub(super) fn indirect(
        &mut self,
        node_index: usize,
        callee: usize,
        args: usize,
        c: &CallableType,
        guarantee: bool,
    ) -> Result<ValueType, TypeError> {
        let source = self.unit.file;
        let mut params = c.parameters.clone();
        let result = if guarantee {
            if !c.has_guarantee {
                return Err(TypeError::at(source, callee, "callback has no declared guarantee"));
            }
            if c.result != ValueType::NoResult {
                params.push(c.result.clone());
            }
            ValueType::Bool
        } else {
            let ack = effect_clauses::markers(self.unit, node_index)?;
            if c.pause.suspend && !ack.suspend || c.pause.block && !ack.block {
                return Err(TypeError::at(source, node_index, "callback requires its declared pause acknowledgement"));
            }
            c.result.clone()
        };
        if params.len() != source.nodes[args].children.len() {
            return Err(TypeError::at(source, node_index, "incorrect callback arity"));
        }
        for (&arg, p) in source.nodes[args].children.iter().zip(&params) {
            let ty = self.type_expression_with_expected_type(arg, Some(p))?;
            self.admit_argument(arg, &ty, p)?;
        }
        self.output.indirect_calls.push(IndirectCall {
            node: source.node_id(node_index),
            use_site: self.call_use(node_index),
            operands: source.nodes[args]
                .children
                .iter()
                .map(|&a| Site::at(source, a))
                .collect(),
            entry: self.invocation_target(callee),
            site: Site::at(source, node_index),
            callee: Site::at(source, callee),
            caller: self.caller.clone(),
            contract: c.clone(),
            binder: match self.unit.symbol(callee) {
                Some(omega_resolver::Symbol::Binding { span, .. }) => self.unit.parameters.get(span).cloned(),
                _ => None,
            },
            guarantee,
        });
        if !guarantee && self.call_use(node_index) == crate::CallUse::Execution {
            self.version = Some(Site::at(source, node_index));
            self.cases.clear();
        }
        Ok(result)
    }
}

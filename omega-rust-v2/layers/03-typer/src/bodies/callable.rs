use super::{Body, ObligationKind, Site, SourceSpan, TypeError, ValueType};
use crate::{CallableType, IndirectCall};
use omega_resolver::DeclarationId;
impl Body<'_, '_> {
    pub(super) fn invocation_target(
        &self,
        n: usize,
    ) -> crate::InvocationTarget {
        let site = Site::at(&self.unit.file, n);
        if let Some(omega_resolver::Symbol::Binding { span, .. }) = self.unit.symbol(n) {
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
        n: usize,
    ) -> Result<CallableType, TypeError> {
        match self
            .unit
            .parameters
            .get(&span)
            .and_then(|p| self.catalog.types.parameter_carrier(p))
        {
            Some(ValueType::Callable(c)) => Ok(*c.clone()),
            _ => Err(TypeError::at(&self.unit.file, n, "static binder requires a callable contract")),
        }
    }
    pub(super) fn refine_callable(
        &mut self,
        n: usize,
        c: &CallableType,
        machine: &DeclarationId,
    ) -> Result<(), TypeError> {
        let sig = self
            .catalog
            .signatures
            .get(machine)
            .ok_or_else(|| TypeError::unsupported(&self.unit.file, n))?;
        if !sig.params.iter().map(|(_, t)| t).eq(&c.parameters) || sig.result != c.result || self.catalog.types.arities[machine] != 0 {
            return Err(TypeError::at(&self.unit.file, n, "selected machine has incompatible callable signature"));
        }
        if let Some(slot) = &c.requirement
            && !sig.satisfies.contains(slot)
        {
            return Err(TypeError::at(&self.unit.file, n, "selected machine lacks the exact satisfaction edge"));
        }
        let pause = sig.pause.declared();
        if pause.suspend && !c.pause.suspend || pause.block && !c.pause.block {
            return Err(TypeError::at(&self.unit.file, n, "selected machine exceeds callable pause bounds"));
        }
        self.obligation(
            n,
            ObligationKind::RefineCallable {
                contract: Box::new(c.clone()),
                machine: machine.clone(),
            },
        );
        Ok(())
    }
    pub(super) fn indirect(
        &mut self,
        n: usize,
        callee: usize,
        args: usize,
        c: &CallableType,
        guarantee: bool,
    ) -> Result<ValueType, TypeError> {
        let f = &self.unit.file;
        let mut params = c.parameters.clone();
        let result = if guarantee {
            if !c.has_guarantee {
                return Err(TypeError::at(f, callee, "callback has no declared guarantee"));
            }
            if c.result != ValueType::NoResult {
                params.push(c.result.clone());
            }
            ValueType::Bool
        } else {
            let ack = crate::effects::markers(self.unit, n)?;
            if c.pause.suspend && !ack.suspend || c.pause.block && !ack.block {
                return Err(TypeError::at(f, n, "callback requires its declared pause acknowledgement"));
            }
            c.result.clone()
        };
        if params.len() != f.nodes[args].children.len() {
            return Err(TypeError::at(f, n, "incorrect callback arity"));
        }
        for (&arg, p) in f.nodes[args].children.iter().zip(&params) {
            let ty = self.expected(arg, Some(p))?;
            self.admit_argument(arg, &ty, p)?;
        }
        self.output.indirect_calls.push(IndirectCall {
            entry: self.invocation_target(callee),
            site: Site::at(f, n),
            callee: Site::at(f, callee),
            caller: self.caller.clone(),
            contract: c.clone(),
            binder: match self.unit.symbol(callee) {
                Some(omega_resolver::Symbol::Binding { span, .. }) => self.unit.parameters.get(span).cloned(),
                _ => None,
            },
            guarantee,
        });
        if !guarantee {
            self.version = Some(Site::at(f, n));
            self.cases.clear();
        }
        Ok(result)
    }
}

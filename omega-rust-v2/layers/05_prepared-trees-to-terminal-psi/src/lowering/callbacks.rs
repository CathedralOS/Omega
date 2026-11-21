use crate::{LoweringError, LoweringTypes};
use omega_resolved::DeclarationId;
use omega_typed::{
    AppliedOperand, CallableType, IndirectCall, InvocationTarget, MachineApplication, MachineEffects, PauseEffects, TypedCall, TypedSignature, ValueType,
};
use std::collections::HashMap;

pub(crate) struct Callback<'a> {
    pub slot: usize,
    pub selection: &'a TypedSignature,
    pub bound: &'a CallableType,
}

impl<'a> Callback<'a> {
    pub fn bind(
        slot: usize,
        selection: &MachineApplication,
        bound: &'a CallableType,
        templates: &HashMap<&DeclarationId, &'a TypedSignature>,
    ) -> Result<Self, LoweringError> {
        let bad = || {
            LoweringError::new(
                &bound.contract,
                "static callback needs a local closed target and a supported structural contract",
            )
        };
        let sig = *templates.get(selection.machine.as_ref()).ok_or_else(bad)?;
        if !selection.arguments.is_empty()
            || !selection.enclosing_arguments.is_empty()
            || bound.requirement.is_some()
            || bound.has_guarantee
            || bound.requires
            || bound.reach.as_ref().is_some_and(|r| !r.is_empty())
            || bound.crash_ceiling.as_ref().is_some_and(|r| !r.is_empty())
            || sig.requires
            || sig.has_guarantee
            || sig.satisfies.is_some()
            || sig.parameters.iter().map(|(_, v)| v).ne(&bound.parameters)
            || sig.result != bound.result
            || !sig.result.domains().is_empty()
            || sig.parameters.iter().any(|(_, v)| !v.domains().is_empty())
        {
            return Err(bad());
        }
        Ok(Self { slot, selection: sig, bound })
    }
    pub fn lower_call(
        &self,
        call: &IndirectCall,
        app: &MachineApplication,
    ) -> Option<TypedCall> {
        // The occurrence must belong to this application's own body and invoke
        // exactly this binder; its typed acknowledgement must cover the binder.
        let owned = call.caller.as_ref() == Some(&*app.machine)
            && matches!(&call.entry, InvocationTarget::StaticParameter { owner, index }
                if *owner == *app.machine && *index == self.slot);
        let acknowledged = (!self.bound.pause.suspend || call.acknowledgements.suspend) && (!self.bound.pause.block || call.acknowledgements.block);
        if !owned || !acknowledged || call.contract != *self.bound || call.operands.len() != self.selection.parameters.len() {
            return None;
        }
        let mut operands = Vec::with_capacity(call.operands.len());
        for (operand, (span, _)) in call.operands.iter().zip(&self.selection.parameters) {
            let mut parameter = self.selection.site.clone();
            parameter.span = *span;
            operands.push(AppliedOperand {
                node: operand.node,
                site: operand.site.clone(),
                parameter,
                subject: app.instantiate(&operand.subject),
                callable: operand.callable.clone(),
            });
        }
        Some(TypedCall {
            node: call.node,
            site: call.site.clone(),
            caller: call.caller.clone(),
            machine: self.selection.machine.clone(),
            use_site: call.use_site,
            type_arguments: vec![],
            enclosing_arguments: vec![],
            operands,
            result: self.selection.result.clone(),
            acknowledgements: call.acknowledgements,
            evidence: None,
            layout_query: None,
        })
    }
    pub fn lower_bound(
        &self,
        types: &LoweringTypes,
        machine: crate::MachineId,
    ) -> Result<crate::StaticArgument, LoweringError> {
        let id = |v: &ValueType| {
            types
                .id_for(v)
                .map_err(|err| LoweringError::new(&self.bound.contract, err.to_string()))
        };
        Ok(crate::StaticArgument::Callable(Box::new(crate::StaticCallableSelection {
            machine,
            parameters: self.bound.parameters.iter().map(id).collect::<Result<_, _>>()?,
            result: (self.bound.result != ValueType::NoResult)
                .then(|| id(&self.bound.result))
                .transpose()?,
            pause: self.bound.pause,
            terminates: self.bound.terminates,
            empty_reach: self.bound.reach.is_some(),
            empty_crash_ceiling: self.bound.crash_ceiling.is_some(),
            site: self.bound.contract.clone(),
        })))
    }
}

/// A runtime callable's declared ceiling: no pause, and empty service and crash rows.
fn runtime_ceiling_supported(callable: &CallableType) -> bool {
    callable.closed_runtime
        && callable.pause == PauseEffects::default()
        && callable.reach.as_ref().is_some_and(Vec::is_empty)
        && callable.crash_ceiling.as_ref().is_some_and(Vec::is_empty)
}

/// The finite envelope of a stored descriptor follows its typed occurrence. All
/// contracts attached to that occurrence must admit the same nonpausing lowering.
pub(crate) fn stored_ceilings<'a>(typed: &'a omega_typed::TypedTrees<'_>) -> HashMap<&'a omega_typed::Site, bool> {
    let mut rows: HashMap<_, bool> = HashMap::new();
    for (site, contract) in typed
        .expressions
        .iter()
        .filter_map(|e| match &e.ty {
            ValueType::Callable(c) => Some((&e.site, c.as_ref())),
            _ => None,
        })
        .chain(typed.indirect_calls.iter().filter_map(|c| match &c.entry {
            InvocationTarget::StoredCallable(site) => Some((site, &c.contract)),
            _ => None,
        }))
    {
        *rows.entry(site).or_insert(true) &= runtime_ceiling_supported(contract);
    }
    rows
}

/// Specialize the L3 pause formula of one instance: its fixed term united with,
/// per callback binder, the selected closed callback's own demand intersected
/// with that binder's term. A call can carry reach while retaining this envelope.
/// `None` keeps unsupported callback routes closed: an unbound or unknown target,
/// a blocking callback, or a runtime or stored descriptor that may pause. Only synchronous
/// suspension of known, locally selected targets is admitted.
pub(crate) fn realized_pause(
    effects: &MachineEffects,
    callbacks: &[Callback<'_>],
    signatures: &HashMap<&DeclarationId, &TypedSignature>,
    summaries: &HashMap<&DeclarationId, &MachineEffects>,
    stored: &HashMap<&omega_typed::Site, bool>,
) -> Option<PauseEffects> {
    // Crash routes have their own invocation checks and instance dependency
    // rows. A crash ceiling is not a pause demand.
    let known = |sig: &TypedSignature| {
        summaries.get(&sig.machine).copied().filter(|row| {
            !sig.pause.declared().block
                && !row.pause.block
                && row.reach.is_empty()
                && sig.reach.as_ref().is_none_or(Vec::is_empty)
        })
    };
    // A selected callback is closed, so its whole demand is its fixed term.
    let demand = |slot: usize| {
        let callback = callbacks.iter().find(|c| c.slot == slot)?;
        known(callback.selection)
            .filter(|row| row.callback_pause.is_empty())
            .map(|row| row.fixed_pause)
    };
    if !effects.invokes.iter().all(|target| match target {
        InvocationTarget::StaticParameter { owner, index } if owner == &effects.machine => demand(*index).is_some(),
        InvocationTarget::Declaration(slot) if slot.trait_arguments.is_empty() => signatures
            .get(&slot.declaration)
            .is_some_and(|sig| known(sig).is_some()),
        // An unknown runtime selection is bounded only by the declared ceiling of
        // this machine's own exact callable formal; no caller set strengthens it.
        InvocationTarget::RuntimeParameter { owner, index } if owner == &effects.machine => signatures
            .get(owner)
            .and_then(|sig| sig.parameters.get(*index))
            .is_some_and(|(_, formal)| matches!(formal, ValueType::Callable(c) if runtime_ceiling_supported(c))),
        InvocationTarget::StoredCallable(site) => stored.get(site).copied().unwrap_or(false),
        _ => false,
    }) {
        return None;
    }
    let mut realized = effects.fixed_pause;
    for (&slot, &bound) in &effects.callback_pause {
        realized.union(demand(slot)?.intersect(bound));
    }
    // A declaration may publish a blocking envelope even if it is not called.
    // The call consumer still refuses an actual crossing with that envelope.
    Some(realized)
}

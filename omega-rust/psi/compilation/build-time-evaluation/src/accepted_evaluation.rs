//! Permission for compiler evaluation is read from an accepted executable.
//! This does not publish a stronger source contract: known code may be finite
//! even when its public interface makes no termination promise.

use psi_terminal_production::CheckedInvocationSource;
use semantic_vocabulary::{BlockId, MachineId};
use terminal_psi::{
    TargetMachineCallable, TerminalCallableEnvelope, TerminalCallableTermination, TerminalMachine,
    TerminalModule,
};

pub(crate) fn entry(source: &CheckedInvocationSource) -> Result<MachineId, String> {
    match source.publication().artifact.semantic_module().entry {
        Some(TargetMachineCallable::Machine(machine)) => Ok(machine),
        _ => Err("compiler evaluation requires one exact executable machine entry".into()),
    }
}

pub(crate) fn require_unconditional_entry(source: &CheckedInvocationSource) -> Result<(), String> {
    let module = source.publication().artifact.semantic_module();
    let machine = module
        .machine(entry(source)?)
        .ok_or("compiler evaluation publication lost its entry")?;
    if !machine.contract.requires.is_empty() || !machine.contract.source_checked_requires.is_empty()
    {
        return Err("compiler invocation still has unproved entry requirements; its exact actuals must be checked in a caller".into());
    }
    if !machine.contract.crash_routes.is_empty() {
        let mut causes = machine
            .contract
            .crash_routes
            .iter()
            .map(|route| route.cause)
            .collect::<Vec<_>>();
        causes.sort();
        causes.dedup();
        return Err(format!(
            "compiler invocation retains unhandled {causes:?} public crash routes at its exact actuals before execution"
        ));
    }
    Ok(())
}

pub(crate) fn require_floor(source: &CheckedInvocationSource) -> Result<Vec<MachineId>, String> {
    let module = source.publication().artifact.semantic_module();
    let root = entry(source)?;
    let mut walk = EvaluationClosure {
        module,
        active: Vec::new(),
        complete: Vec::new(),
    };
    walk.machine(root)?;
    Ok(walk.complete)
}

struct EvaluationClosure<'a> {
    module: &'a TerminalModule,
    active: Vec<MachineId>,
    complete: Vec<MachineId>,
}

impl EvaluationClosure<'_> {
    fn machine(&mut self, id: MachineId) -> Result<(), String> {
        if self.complete.contains(&id) {
            return Ok(());
        }
        let machine = self
            .module
            .machine(id)
            .ok_or_else(|| format!("compiler evaluation call names absent machine {id:?}"))?;
        let declaration = machine
            .declaration
            .as_ref()
            .ok_or("compiler evaluation machine has no retained declaration")?;
        let name = self
            .module
            .identities
            .text(declaration.semantic_declaration);
        if !machine.declared_service_reach.is_empty()
            || !machine.published_service_ceiling.is_empty()
            || declaration.published_may_suspend
            || declaration.published_may_block
        {
            return Err(format!(
                "compiler evaluation machine {name:?} permits services, suspension or blocking"
            ));
        }
        self.require_unrestricted_storage(machine)?;
        let envelope = self.module.callables.machine_envelope(id);
        if let Some(envelope) = envelope {
            self.effects(envelope)?;
        }
        if self.active.contains(&id) {
            return Self::unconditional_progress(envelope, "recursive machine invocation");
        }
        self.active.push(id);
        if has_control_cycle(machine)? && machine.ranked_scc.is_none() {
            Self::unconditional_progress(envelope, "cyclic machine control flow")?;
        }
        for invocation in machine.module_invocations() {
            self.machine(invocation.inputs.callee)?;
        }
        for invocation in machine.callable_invocations() {
            let contract = self
                .module
                .callables
                .contract(invocation.inputs.contract)
                .ok_or("compiler evaluation invocation has a missing callable contract")?;
            self.effects(&contract.envelope)?;
            Self::unconditional_progress(Some(&contract.envelope), "unknown callable invocation")?;
        }
        for invocation in machine.boundary_invocations() {
            let boundary = self
                .module
                .boundary_machine(invocation.boundary)
                .ok_or("compiler evaluation invocation has a missing boundary contract")?;
            let declaration = boundary
                .declaration
                .as_ref()
                .ok_or("compiler evaluation boundary has no exact public declaration")?;
            if !boundary.fixed_service_reach.is_empty()
                || !boundary.published_service_ceiling.is_empty()
                || declaration.source.published_may_suspend
                || declaration.source.published_may_block
            {
                return Err(
                    "compiler evaluation boundary permits services, suspension or blocking".into(),
                );
            }
            if !boundary.unconditional_termination {
                return Err(
                    "compiler evaluation boundary publishes no unconditional termination guarantee"
                        .into(),
                );
            }
        }
        self.active.pop();
        self.complete.push(id);
        Ok(())
    }

    fn effects(&self, envelope: &TerminalCallableEnvelope) -> Result<(), String> {
        if !envelope.service_reach.is_empty() || envelope.suspends || envelope.blocks {
            return Err(
                "compiler evaluation callable permits services, suspension or blocking".into(),
            );
        }
        if !envelope.invocations.is_empty() {
            return Err(
                "compiler evaluation callable retains an uninstantiated invocation capability"
                    .into(),
            );
        }
        Ok(())
    }

    fn unconditional_progress(
        envelope: Option<&TerminalCallableEnvelope>,
        operation: &str,
    ) -> Result<(), String> {
        match envelope.map(|envelope| &envelope.termination) {
            Some(TerminalCallableTermination::Terminates { premises }) if premises.is_empty() => {
                Ok(())
            }
            Some(TerminalCallableTermination::Terminates { .. }) => Err(format!(
                "compiler evaluation {operation} retains termination premises that require exact invocation evidence"
            )),
            Some(TerminalCallableTermination::NotGuaranteed) | None => Err(format!(
                "compiler evaluation {operation} has neither checked finite code nor an unconditional termination guarantee"
            )),
        }
    }

    fn require_unrestricted_storage(&self, machine: &TerminalMachine) -> Result<(), String> {
        for place in &machine.structural_places {
            let ty = machine
                .structural_place_type(place.id)
                .ok_or("compiler evaluation storage has no exact retained type")?;
            let declaration = self
                .module
                .structural_type(ty)
                .ok_or("compiler evaluation storage has no retained type declaration")?;
            if declaration.owns_linear_claim {
                return Err(
                    "compiler evaluation has no admission for a linear runtime carrier".into(),
                );
            }
        }
        Ok(())
    }
}

fn has_control_cycle(machine: &TerminalMachine) -> Result<bool, String> {
    fn visit(
        machine: &TerminalMachine,
        id: BlockId,
        active: &mut Vec<BlockId>,
        complete: &mut Vec<BlockId>,
    ) -> Result<bool, String> {
        if active.contains(&id) {
            return Ok(true);
        }
        if complete.contains(&id) {
            return Ok(false);
        }
        let block = machine
            .blocks
            .iter()
            .find(|block| block.id == id)
            .ok_or("compiler evaluation control flow names an absent block")?;
        active.push(id);
        for successor in block.terminator.successor_targets() {
            if visit(machine, successor, active, complete)? {
                return Ok(true);
            }
        }
        active.pop();
        complete.push(id);
        Ok(false)
    }
    visit(machine, machine.entry, &mut Vec::new(), &mut Vec::new())
}

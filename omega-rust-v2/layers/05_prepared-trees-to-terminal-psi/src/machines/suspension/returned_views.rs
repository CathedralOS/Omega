//! Recover which input positions can back a completed shared-view result.
//! The path-specific evidence stays on the return and call in Psi. Lowering uses
//! these sets only to decide whether it can describe every possible backing.
use crate::{
    BlockTerminator, CallArgument, CallTarget, LoweredMachine, LoweringTypes, MachineBody, OperationKind, OperationResult, Parameter, PlaceId, ReturnEdge,
    SuccessorEdge, TypeShape,
};

pub(super) struct ReturnInputs<'a> {
    slots: Vec<Option<Vec<usize>>>,
    members: &'a [crate::MemberCallSelection],
}

impl<'a> ReturnInputs<'a> {
    pub(super) fn from_machines(
        machines: &[LoweredMachine],
        types: &LoweringTypes,
        members: &'a [crate::MemberCallSelection],
    ) -> Self {
        let mut returns = Self {
            slots: vec![None; machines.len()],
            members,
        };
        let mut callers = vec![Vec::new(); machines.len()];
        let mut pending = Vec::new();
        let mut queued = vec![false; machines.len()];
        for (index, machine) in machines.iter().enumerate() {
            if !matches!(
                machine.result_type.and_then(|ty| types.get(ty)),
                Some(TypeShape::SharedByteView | TypeShape::SharedElementView { .. })
            ) {
                continue;
            }
            pending.push(index);
            queued[index] = true;
            let mut depends_on = |target| {
                if let Some(id) = returns.local_machine(target) {
                    let dependents = &mut callers[(id.get() - 1) as usize];
                    if dependents.last() != Some(&index) {
                        dependents.push(index);
                    }
                }
            };
            for op in &machine.body.operations {
                if let (OperationKind::Call { callee, .. }, OperationResult::Structural(_)) = (&op.kind, op.result) {
                    depends_on(*callee);
                }
            }
            for block in &machine.body.blocks {
                if let BlockTerminator::TerminalCall { callee, .. } = block.terminator {
                    depends_on(callee);
                }
            }
        }
        // Publish a summary only after every normal return can be followed.
        // A recursive dependency cannot establish its own backing.
        while let Some(index) = pending.pop() {
            queued[index] = false;
            if let Some(slots) = returns.normal_returns(&machines[index].body) {
                returns.slots[index] = Some(slots);
                for &caller in &callers[index] {
                    if returns.slots[caller].is_none() && !queued[caller] {
                        pending.push(caller);
                        queued[caller] = true;
                    }
                }
            }
        }
        returns
    }

    fn local_machine(
        &self,
        target: CallTarget,
    ) -> Option<crate::MachineId> {
        match target {
            CallTarget::Machine(id) => Some(id),
            CallTarget::Member(slot) => match self.members.get(slot)?.target {
                crate::MemberRealization::Conformance { machine, .. } => Some(machine),
                crate::MemberRealization::Boundary { .. } => None,
            },
            CallTarget::Runtime { .. } => None,
        }
    }

    fn actuals(
        &self,
        target: CallTarget,
        arguments: &[CallArgument],
    ) -> Option<Vec<PlaceId>> {
        let id = self.local_machine(target)?;
        self.slots[(id.get() - 1) as usize]
            .as_ref()?
            .iter()
            .map(|&slot| match arguments.get(slot)? {
                CallArgument::CopyWhole(input) => Some(*input),
                _ => None,
            })
            .collect()
    }

    fn normal_returns(
        &self,
        body: &MachineBody,
    ) -> Option<Vec<usize>> {
        let origins = input_origins(body, self);
        let mut slots = Vec::new();
        for block in &body.blocks {
            match &block.terminator {
                BlockTerminator::Return {
                    result: ReturnEdge::Structural { source, .. },
                    ..
                } => slots.extend(origins[source.index()].as_ref()?),
                BlockTerminator::TerminalCall { callee, arguments, .. } => {
                    for actual in self.actuals(*callee, arguments)? {
                        slots.extend(origins[actual.index()].as_ref()?);
                    }
                }
                BlockTerminator::Return { .. } => return None,
                _ => {}
            }
        }
        slots.sort_unstable();
        slots.dedup();
        (!slots.is_empty()).then_some(slots)
    }
}

/// Each edge supplies one possible source of the destination. A call supplies
/// only the actuals corresponding to its callee's possible returned formals.
pub(super) fn input_origins(
    body: &MachineBody,
    returns: &ReturnInputs<'_>,
) -> Vec<Option<Vec<usize>>> {
    let mut awaiting = vec![0usize; body.places.len()];
    let mut destinations = vec![Vec::new(); body.places.len()];
    let mut arrival = |target: PlaceId, source: Option<PlaceId>| {
        awaiting[target.index()] += 1;
        if let Some(source) = source
            && body.places[source.index()].ty == body.places[target.index()].ty
        {
            destinations[source.index()].push(target.index());
        }
    };
    for op in &body.operations {
        if let (OperationKind::Call { callee, arguments, .. }, OperationResult::Structural(result)) = (&op.kind, op.result)
            && let Some(sources) = returns.actuals(*callee, arguments)
        {
            for source in sources {
                arrival(result, Some(source));
            }
        }
    }
    let mut edge = |edge: &SuccessorEdge| {
        for (slot, &target) in body.blocks[edge.target.get() - 1]
            .structural_parameters
            .iter()
            .enumerate()
        {
            let source = match edge.structural_arguments.get(slot) {
                Some(CallArgument::CopyWhole(p)) => Some(*p),
                _ => None,
            };
            arrival(target, source);
        }
    };
    for block in &body.blocks {
        match &block.terminator {
            BlockTerminator::Conditional { when_true, when_false, .. } => {
                edge(when_true);
                edge(when_false);
            }
            BlockTerminator::Jump(successor) => edge(successor),
            _ => {}
        }
    }
    let mut origins = vec![None; body.places.len()];
    let mut candidates = vec![Vec::new(); body.places.len()];
    let mut ready = Vec::new();
    for (slot, param) in body.parameters.iter().enumerate() {
        if let Parameter::Structural(source) = param {
            origins[source.index()] = Some(vec![slot]);
            ready.push(source.index());
        }
    }
    while let Some(source) = ready.pop() {
        for &target in &destinations[source] {
            candidates[target].extend_from_slice(
                origins[source]
                    .as_ref()
                    .expect("a scheduled descriptor has established origins"),
            );
            awaiting[target] -= 1;
            if awaiting[target] == 0 {
                let mut slots = std::mem::take(&mut candidates[target]);
                slots.sort_unstable();
                slots.dedup();
                origins[target] = Some(slots);
                ready.push(target);
            }
        }
    }
    origins
}

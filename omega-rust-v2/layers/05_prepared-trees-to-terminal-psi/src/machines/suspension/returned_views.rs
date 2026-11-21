//! Which input loans must remain traceable for a completed shared-view result?
//! A local return can forward a descriptor. A boundary completion instead
//! establishes its own result with an input-related validity interval. These
//! sets are dependencies, not spatial provenance. Psi retains the producing
//! call and its applied contract, not a replacement descriptor from this set.
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
        returns.narrow_recursive(machines, types, &callers);
        returns
    }

    /// A recursive call's result cannot be followed to a finished summary,
    /// because the summary is the thing being computed. The elided result
    /// relation instead makes every formal of the result's view type a possible
    /// source (`wiki/spec/language/lifetimes.md#returned-views`). Start each
    /// unresolved view-returning machine from those formals and keep only the
    /// ones its normal returns reach through the current summaries. The sets
    /// only shrink, so the loop stops. The final sets agree with every return
    /// edge: a completed call's result reaches its normal return through a
    /// finite chain of calls, and each call in that chain maps the actuals at
    /// its callee's set back into its caller's set. Any return that still
    /// cannot be followed, such as a boundary result without a recorded
    /// lifetime input, leaves the summary absent.
    fn narrow_recursive(
        &mut self,
        machines: &[LoweredMachine],
        types: &LoweringTypes,
        callers: &[Vec<usize>],
    ) {
        let mut pending = Vec::new();
        let mut queued = vec![false; machines.len()];
        let mut open = vec![false; machines.len()];
        for (index, machine) in machines.iter().enumerate() {
            let Some(result) = machine.result_type else { continue };
            if self.slots[index].is_some() || !matches!(types.get(result), Some(TypeShape::SharedByteView | TypeShape::SharedElementView { .. })) {
                continue;
            }
            let body = &machine.body;
            let formals: Vec<usize> = (body.parameters.iter().enumerate())
                .filter(|(_, parameter)| matches!(parameter, Parameter::Structural(p) if body.places[p.index()].ty == result))
                .map(|(slot, _)| slot)
                .collect();
            if formals.is_empty() {
                continue;
            }
            self.slots[index] = Some(formals);
            open[index] = true;
            pending.push(index);
            queued[index] = true;
        }
        while let Some(index) = pending.pop() {
            queued[index] = false;
            if self.slots[index].is_none() {
                continue;
            }
            let next = self.normal_returns(&machines[index].body);
            if next == self.slots[index] {
                continue;
            }
            self.slots[index] = next;
            for &caller in &callers[index] {
                if open[caller] && self.slots[caller].is_some() && !queued[caller] {
                    pending.push(caller);
                    queued[caller] = true;
                }
            }
        }
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
        let slots: &[usize] = if let CallTarget::Member(slot) = target
            && let member = self.members.get(slot)?
            && matches!(member.target, crate::MemberRealization::Boundary { .. })
        {
            std::slice::from_ref(member.boundary_result_lifetime_input.as_ref()?)
        } else {
            let id = self.local_machine(target)?;
            self.slots[(id.get() - 1) as usize].as_deref()?
        };
        slots.iter()
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
/// only the actuals on which its selected result's lifetime depends.
pub(super) fn input_origins(
    body: &MachineBody,
    returns: &ReturnInputs<'_>,
) -> Vec<Option<Vec<usize>>> {
    let mut awaiting = vec![0usize; body.places.len()];
    let mut destinations = vec![Vec::new(); body.places.len()];
    let mut arrival = |target: PlaceId, source: Option<PlaceId>| {
        awaiting[target.index()] += 1;
        if let Some(source) = source {
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
                // The edge forwards the descriptor itself, unlike a boundary
                // call whose lifetime anchor can denote a different view type.
                Some(CallArgument::CopyWhole(p)) if body.places[p.index()].ty == body.places[target.index()].ty => Some(*p),
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

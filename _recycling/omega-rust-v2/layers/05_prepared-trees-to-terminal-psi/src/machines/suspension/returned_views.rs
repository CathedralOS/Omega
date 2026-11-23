//! Which input loans must remain traceable for a completed shared-view result?
//! A local return can forward a descriptor. A boundary completion instead
//! establishes its own result: with an input-related validity interval, or
//! with a lifetime each caller chooses and no input dependency at all. These
//! sets are dependencies, not spatial provenance. Psi retains the producing
//! call and its applied contract, not a replacement descriptor from this set.
use crate::{
    BlockTerminator, BoundaryResultLifetime, CallArgument, CallTarget, LoweredMachine, LoweringTypes, MachineBody, OperationKind, OperationResult, Parameter,
    PlaceId, ReturnEdge, SuccessorEdge, TypeShape,
};

/// One possible source of a returned view. `Input` is an incoming formal slot
/// or the place an actual names. `Provider` is a boundary completion whose
/// lifetime each caller chooses: the admitted provider establishes its bytes,
/// and no input carries its lifetime
/// (`wiki/spec/language/lifetimes.md#caller-chosen-result-lifetimes`). Psi
/// still names the exact invocation through the producing call.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum ViewSource<T> {
    Input(T),
    Provider,
}

pub(super) struct ReturnInputs<'a> {
    slots: Vec<Option<Vec<ViewSource<usize>>>>,
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
    /// unresolved view-returning machine from those formals, possibly none, and
    /// keep only the ones its normal returns reach through the current
    /// summaries. The formal sets only shrink. A provider source joins a set
    /// once a return reaches a caller-chosen boundary result and is never
    /// removed, because no formal maps to it. So the loop stops. The final sets
    /// agree with every return edge: a completed call's result reaches its
    /// normal return through a finite chain of calls, and each call in that
    /// chain maps the actuals at its callee's set back into its caller's set.
    /// Any return that still cannot be followed, such as a boundary result
    /// whose input frontier has no published relation, leaves the summary absent.
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
            let formals = (body.parameters.iter().enumerate())
                .filter(|(_, parameter)| matches!(parameter, Parameter::Structural(p) if body.places[p.index()].ty == result))
                .map(|(slot, _)| ViewSource::Input(slot))
                .collect();
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

    /// The possible sources of a call's view result, as the actuals they name.
    fn actuals(
        &self,
        target: CallTarget,
        arguments: &[CallArgument],
    ) -> Option<Vec<ViewSource<PlaceId>>> {
        let anchor;
        let slots: &[ViewSource<usize>] = if let CallTarget::Member(slot) = target
            && let member = self.members.get(slot)?
            && matches!(member.target, crate::MemberRealization::Boundary { .. })
        {
            anchor = match member.boundary_result_lifetime? {
                BoundaryResultLifetime::Input(position) => ViewSource::Input(position),
                BoundaryResultLifetime::CallerChosen => ViewSource::Provider,
            };
            std::slice::from_ref(&anchor)
        } else {
            let id = self.local_machine(target)?;
            self.slots[(id.get() - 1) as usize].as_deref()?
        };
        slots
            .iter()
            .map(|&source| match source {
                ViewSource::Input(slot) => match arguments.get(slot)? {
                    CallArgument::CopyWhole(input) => Some(ViewSource::Input(*input)),
                    _ => None,
                },
                ViewSource::Provider => Some(ViewSource::Provider),
            })
            .collect()
    }

    fn normal_returns(
        &self,
        body: &MachineBody,
    ) -> Option<Vec<ViewSource<usize>>> {
        let origins = input_origins(body, self);
        let mut slots = Vec::new();
        for block in &body.blocks {
            match &block.terminator {
                BlockTerminator::Return {
                    result: ReturnEdge::Structural { source, .. },
                    ..
                } => slots.extend(origins[source.index()].as_ref()?),
                BlockTerminator::TerminalCall {
                    callee,
                    arguments,
                    completion: None,
                    ..
                } => {
                    for actual in self.actuals(*callee, arguments)? {
                        match actual {
                            ViewSource::Input(actual) => slots.extend(origins[actual.index()].as_ref()?),
                            ViewSource::Provider => slots.push(ViewSource::Provider),
                        }
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
/// only the actuals on which its selected result's lifetime depends, and a
/// provider source when the result may be a caller-chosen boundary view.
pub(super) fn input_origins(
    body: &MachineBody,
    returns: &ReturnInputs<'_>,
) -> Vec<Option<Vec<ViewSource<usize>>>> {
    let mut awaiting = vec![0usize; body.places.len()];
    let mut destinations = vec![Vec::new(); body.places.len()];
    let mut provided = vec![false; body.places.len()];
    let mut arrival = |target: PlaceId, source: Option<PlaceId>| {
        awaiting[target.index()] += 1;
        if let Some(source) = source {
            destinations[source.index()].push(target.index());
        }
    };
    // Ordinary completions name an operation result. A terminal completion
    // with a cleanup continuation binds that result to the target parameter.
    let ordinary = body
        .operations
        .iter()
        .filter_map(|op| match (&op.kind, op.result) {
            (OperationKind::Call { callee, arguments, .. }, OperationResult::Structural(result)) => Some((*callee, arguments, result)),
            _ => None,
        });
    let terminal = body.blocks.iter().filter_map(|block| {
        let BlockTerminator::TerminalCall {
            callee,
            arguments,
            completion: Some(target),
            ..
        } = &block.terminator
        else {
            return None;
        };
        let [result] = body.blocks[target.get() - 1].structural_parameters.as_slice() else {
            return None;
        };
        Some((*callee, arguments, *result))
    });
    for (callee, arguments, result) in ordinary.chain(terminal) {
        match returns.actuals(callee, arguments) {
            Some(sources) => {
                for source in sources {
                    match source {
                        ViewSource::Input(source) => arrival(result, Some(source)),
                        ViewSource::Provider => provided[result.index()] = true,
                    }
                }
            }
            None => arrival(result, None),
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
    let mut candidates: Vec<Vec<ViewSource<usize>>> = (provided.iter())
        .map(|&provided| if provided { vec![ViewSource::Provider] } else { Vec::new() })
        .collect();
    let mut ready = Vec::new();
    for (slot, param) in body.parameters.iter().enumerate() {
        if let Parameter::Structural(source) = param {
            origins[source.index()] = Some(vec![ViewSource::Input(slot)]);
            ready.push(source.index());
        }
    }
    // A result whose only sources are caller-chosen boundary views awaits no input.
    for (index, provided) in provided.into_iter().enumerate() {
        if provided && awaiting[index] == 0 {
            origins[index] = Some(std::mem::take(&mut candidates[index]));
            ready.push(index);
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

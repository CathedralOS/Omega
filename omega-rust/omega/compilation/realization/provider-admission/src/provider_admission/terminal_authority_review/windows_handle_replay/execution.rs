//! Follow original control edges and finish cleanup before binding arrivals.
//!
//! Every queued state retains its own resource ledger. Numeric snapshots may
//! lose precision at a repeated cut; resource identities, pointers, callers and
//! retirement histories never join by selecting one convenient predecessor.

use super::*;
use semantic_vocabulary::BlockId;
use terminal_semantics::descriptor_lifetimes::DescriptorPosition;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Completion {
    Arrival(CapturedHandleArrival),
    Return(CapturedHandleReturn),
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Control {
    Position(DescriptorPosition),
    Cleanup {
        position: DescriptorPosition,
        edge: EdgeId,
        ordinal: usize,
        completion: Completion,
    },
}

impl Control {
    fn position(&self) -> DescriptorPosition {
        match self {
            Self::Position(position) | Self::Cleanup { position, .. } => *position,
        }
    }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct SuspendedCall {
    frame: HandleReplayFrame,
    continuation: Control,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Work {
    activation: usize,
    control: Control,
    frames: Vec<SuspendedCall>,
    state: CanonicalHandleState,
}

/// Only evaluated numeric metadata widens. The key still includes every word,
/// byte range, reference target, case, binding, original continuation and ledger.
/// A later unknown condition explores both edges rather than reviving a fact.
fn forget_numbers(state: &mut CanonicalHandleState) {
    state.constants = state
        .constants
        .map(|_, value| value.as_ref().map(|_| KnownHandleScalar::Unknown));
    state.stored_constants.clear();
}

fn join_numbers(retained: &mut CanonicalHandleState, incoming: &CanonicalHandleState) -> bool {
    let mut changed = false;
    retained.constants = retained.constants.map(|id, value| {
        value.as_ref().map(|value| {
            if *value != KnownHandleScalar::Unknown
                && incoming.constants.get(id).and_then(Option::as_ref) != Some(value)
            {
                changed = true;
                KnownHandleScalar::Unknown
            } else {
                value.clone()
            }
        })
    });
    retained.stored_constants.retain(|range, value| {
        let keep = incoming.stored_constants.get(range) == Some(value);
        changed |= !keep;
        keep
    });
    changed
}

#[derive(Default)]
struct Pending {
    rows: BTreeMap<Work, Work>,
    queue: std::collections::VecDeque<Work>,
}

impl Pending {
    fn push(&mut self, work: Work) -> Result<(), String> {
        let mut key = work.clone();
        forget_numbers(&mut key.state);
        for suspended in &mut key.frames {
            forget_numbers(&mut suspended.frame.caller);
        }
        if let Some(retained) = self.rows.get_mut(&key) {
            let mut changed = join_numbers(&mut retained.state, &work.state);
            for (retained, incoming) in retained.frames.iter_mut().zip(&work.frames) {
                changed |= join_numbers(&mut retained.frame.caller, &incoming.frame.caller);
            }
            if changed {
                self.queue.push_back(retained.clone());
            }
            return Ok(());
        }
        if self.rows.len() >= 65_536 {
            return Err(
                "handle execution exceeds its bounded distinct control and resource states".into(),
            );
        }
        self.rows.insert(key, work.clone());
        self.queue.push_back(work);
        Ok(())
    }
}

pub(super) struct CheckedHandleExecution {
    pub occurrences: BTreeMap<NativeOccurrence, WindowsHandleContract>,
}

struct Execution<'a, 'module> {
    context: &'a ReviewContext<'module>,
    inputs: &'a WindowsHandleInputs<'a>,
    arguments: BTreeMap<Caller, PreparedHandleArguments>,
    activations: HandleReplayActivations,
    blocks: BTreeMap<(MachineId, BlockId), &'a terminal_psi::Block>,
    pending: Pending,
    occurrences: BTreeMap<NativeOccurrence, WindowsHandleContract>,
    completed: usize,
}

impl<'a, 'module> Execution<'a, 'module> {
    fn body(&self, activation: usize) -> Result<&'a terminal_psi::TerminalMachine, String> {
        let scope = self
            .activations
            .rows
            .get(activation)
            .ok_or("handle execution has no original activation")?;
        self.inputs
            .calls
            .machine(scope.machine)
            .ok_or("handle execution lost its selected body".into())
    }

    fn block(&self, body: MachineId, block: BlockId) -> Result<&'a terminal_psi::Block, String> {
        self.blocks
            .get(&(body, block))
            .copied()
            .ok_or("handle execution lost its exact original block".into())
    }

    fn call(&mut self, work: Work, caller: Caller, continuation: Control) -> Result<(), String> {
        let calls = self
            .inputs
            .calls
            .calls
            .get(&caller)
            .ok_or("handle execution lost the complete selected call alternatives")?;
        if calls.is_empty() {
            return Err("handle execution has an empty selected call roster".into());
        }
        for call in calls {
            match call.target {
                CallTarget::Machine(callee) => {
                    if work.frames.len() >= 256 {
                        return Err(
                            "handle execution exceeds its bounded matched-call depth".into()
                        );
                    }
                    let (frame, state) = work
                        .state
                        .clone()
                        .enter_call(
                            HandleReplayInvocation {
                                parent: work.activation,
                                caller,
                                callee,
                            },
                            self.inputs,
                            &mut self.activations,
                            self.context,
                        )
                        .map_err(|error| {
                            format!("handle call {caller:?} to {callee:?}: {error}")
                        })?;
                    let activation = frame.activation;
                    let mut frames = work.frames.clone();
                    frames.push(SuspendedCall {
                        frame,
                        continuation: continuation.clone(),
                    });
                    let body = self.body(activation)?;
                    self.pending.push(Work {
                        activation,
                        control: Control::Position((body.entry, 0)),
                        frames,
                        state,
                    })?;
                }
                CallTarget::Boundary(_) => {
                    let input = self.inputs.natives.get(&caller).ok_or_else(|| {
                        if self.inputs.opaque_calls.contains_key(&caller) {
                            format!("handle execution has no preserving contract for boundary call {caller:?}")
                        } else {
                            format!("handle execution lost its original boundary call {caller:?}")
                        }
                    })?;
                    let body = self.body(work.activation)?;
                    let block = self.block(body.id, caller.block)?;
                    let prepared = self
                        .inputs
                        .machines
                        .get(body.id)
                        .and_then(Option::as_ref)
                        .ok_or("handle native execution lost its prepared body")?;
                    let bindings = work.state.bindings.clone();
                    let geometry = HandleStorageGeometry::new(
                        self.context.input.module(),
                        body,
                        &self.inputs.storage[prepared.storage_index],
                        target::TargetProfile::WindowsX64,
                        work.activation,
                        &bindings,
                    )?;
                    let machine = HandleMachineReplay {
                        body,
                        block,
                        prepared,
                        graph: &self.inputs.storage[prepared.storage_index],
                        calls: &self.inputs.calls,
                        geometry: &geometry,
                    };
                    let occurrence = self.activations.native(
                        work.activation,
                        caller,
                        self.context,
                        &self.inputs.calls,
                    )?;
                    let arguments = self
                        .arguments
                        .get(&caller)
                        .ok_or("handle native execution lost its checked arguments")?;
                    let outcomes = work
                        .state
                        .clone()
                        .native(&HandleNativeReplay {
                            machine: &machine,
                            input,
                            arguments,
                            occurrence,
                        })
                        .map_err(|error| {
                            format!(
                                "native handle call {caller:?}, boundary {:?}, contract {:?}: {error}",
                                occurrence.boundary, input.reviewed.contract,
                            )
                        })?;
                    if outcomes.states.is_empty() {
                        return Err("handle native execution produced no checked outcome".into());
                    }
                    if self
                        .occurrences
                        .insert(outcomes.occurrence, outcomes.contract)
                        .is_some_and(|previous| previous != outcomes.contract)
                    {
                        return Err(
                            "one native source occurrence changes its reviewed contract".into()
                        );
                    }
                    for state in outcomes.states {
                        self.pending.push(Work {
                            activation: work.activation,
                            control: continuation.clone(),
                            frames: work.frames.clone(),
                            state,
                        })?;
                    }
                }
                CallTarget::Intrinsic(execution) => {
                    if !self.inputs.intrinsic_calls.contains_key(&caller) {
                        return Err(
                            "handle intrinsic lost its prepared original call roster".into()
                        );
                    }
                    self.intrinsic(work.clone(), *call, execution, continuation.clone())?;
                }
            }
        }
        Ok(())
    }

    fn intrinsic(
        &mut self,
        mut work: Work,
        call: RuntimeCall<'_>,
        execution: effects::CompilerIntrinsicExecutionIdentity,
        _continuation: Control,
    ) -> Result<(), String> {
        // External completion never supplies a fabricated ordinary return.
        // The original admitted mechanism, call kind and terminator must agree.
        if execution != effects::CompilerIntrinsicExecutionIdentity::HostedExitProcessI32
            || call.normal_return
        {
            return Err(format!(
                "handle execution has no value/storage contract for intrinsic {execution:?}"
            ));
        }
        let CallSite::Invocation(InvocationSite::ExternalCompletion(edge)) = call.caller.site
        else {
            return Err("process exit has no original external-completion occurrence".into());
        };
        let body = self.body(work.activation)?;
        let block = self.block(body.id, call.caller.block)?;
        let terminal_psi::Terminator::ExternalCompletion {
            edge: actual,
            boundary,
            arguments,
            structural_arguments,
            kind,
            ..
        } = &block.terminator
        else {
            return Err("process exit substitutes another original terminator".into());
        };
        if *actual != edge
            || call.boundary != Some(*boundary)
            || !structural_arguments.is_empty()
            || arguments.len() != 1
            || !matches!(
                kind,
                semantic_vocabulary::ExternalCompletionKind::ProcessExit
            )
        {
            return Err("process exit changes its exact closed completion signature".into());
        }
        let prepared = self
            .inputs
            .machines
            .get(body.id)
            .and_then(Option::as_ref)
            .ok_or("process exit lost its scalar snapshot roster")?;
        let occurrence = self
            .activations
            .scalar(work.activation, ScalarUseSite::Edge(edge))?;
        let value =
            work.state
                .scalar(arguments[0], work.control.position(), &prepared.expressions)?;
        work.state
            .observe(value.word.dependencies().iter().copied(), occurrence)?;
        let i32_type = ScalarType::Integer(
            IntegerType::new(semantic_vocabulary::IntegerSign::Signed, 32).map_err(|reason| {
                format!("process exit has no supported status carrier: {reason:?}")
            })?,
        );
        if value.word.scalar_type() != i32_type || !value.word.dependencies().is_empty() {
            return Err(
                "process exit changes its status carrier or exposes tracked handle bits".into(),
            );
        }
        work.state
            .ledger
            .returning_path(occurrence, &[])
            .map_err(|reason| {
                format!("process exit retains an unproved handle lifetime: {reason:?}")
            })?;
        self.completed += 1;
        Ok(())
    }

    fn cleanup(
        &mut self,
        mut work: Work,
        position: DescriptorPosition,
        edge: EdgeId,
        ordinal: usize,
        completion: Completion,
    ) -> Result<(), String> {
        let body = self.body(work.activation)?;
        let block = self.block(body.id, position.0)?;
        let actions = block
            .terminator
            .cleanup_rosters()
            .find(|(actual, _)| *actual == edge)
            .map(|(_, actions)| actions)
            .ok_or("handle cleanup lost its actual edge roster")?;
        if let Some(action) = actions.get(ordinal) {
            let next = Control::Cleanup {
                position,
                edge,
                ordinal: ordinal + 1,
                completion,
            };
            if matches!(
                action,
                terminal_psi::TerminalAffineCleanupAction::InvokeNominal(_)
            ) {
                return self.call(
                    work,
                    Caller {
                        machine: body.id,
                        block: block.id,
                        operation_index: position.1 as usize,
                        site: CallSite::Cleanup { edge, ordinal },
                    },
                    next,
                );
            }
            // Plain value disposal retires no external HANDLE. Keep its integer
            // aliases in the lifetime world until the complete outgoing census;
            // this cannot turn a missing nominal/native release into a close.
            work.control = next;
            return self.pending.push(work);
        }
        match completion {
            Completion::Arrival(arrival) => {
                let target = self.block(body.id, arrival.target.0)?;
                let prepared = self
                    .inputs
                    .machines
                    .get(body.id)
                    .and_then(Option::as_ref)
                    .ok_or("handle arrival lost its prepared original body")?;
                let bindings = work.state.bindings.clone();
                let geometry = HandleStorageGeometry::new(
                    self.context.input.module(),
                    body,
                    &self.inputs.storage[prepared.storage_index],
                    target::TargetProfile::WindowsX64,
                    work.activation,
                    &bindings,
                )?;
                let machine = HandleMachineReplay {
                    body,
                    block: target,
                    prepared,
                    graph: &self.inputs.storage[prepared.storage_index],
                    calls: &self.inputs.calls,
                    geometry: &geometry,
                };
                work.state = work.state.install_arrival(arrival, &machine)?;
                work.control = Control::Position((target.id, 0));
                self.pending.push(work)
            }
            Completion::Return(returned) => {
                if let Some(suspended) = work.frames.pop() {
                    work.activation = suspended.frame.invocation.parent;
                    work.state = work.state.resume_after_cleanup(
                        suspended.frame,
                        returned,
                        self.inputs,
                        &self.activations,
                        self.context,
                    )?;
                    work.control = suspended.continuation;
                    self.pending.push(work)
                } else {
                    let occurrence = self
                        .activations
                        .scalar(work.activation, ScalarUseSite::Edge(returned.edge))?;
                    // The closed entry returns its process status, not a
                    // resource to an independently authorized external caller.
                    match returned.value {
                        CapturedReturnValue::Scalar(value)
                            if !value.word.dependencies().is_empty() =>
                        {
                            return Err(
                                "entry result exposes tracked handle bits after cleanup".into()
                            );
                        }
                        CapturedReturnValue::Structural { .. } => {
                            return Err("entry structural output has no independently supplied receiver custody".into());
                        }
                        _ => {}
                    }
                    work.state
                        .ledger
                        .returning_path(occurrence, &[])
                        .map_err(|reason| {
                            format!("entry return has an incomplete handle lifetime: {reason:?}")
                        })?;
                    self.completed += 1;
                    Ok(())
                }
            }
        }
    }

    fn step(&mut self, mut work: Work) -> Result<(), String> {
        let Control::Position(position) = work.control.clone() else {
            let Control::Cleanup {
                position,
                edge,
                ordinal,
                completion,
            } = work.control.clone()
            else {
                unreachable!()
            };
            return self.cleanup(work, position, edge, ordinal, completion);
        };
        let body = self.body(work.activation)?;
        let block = self.block(body.id, position.0)?;
        let prepared = self
            .inputs
            .machines
            .get(body.id)
            .and_then(Option::as_ref)
            .ok_or("handle execution lost the prepared body for its actual cut")?;
        let graph = &self.inputs.storage[prepared.storage_index];
        let bindings = work.state.bindings.clone();
        let geometry = HandleStorageGeometry::new(
            self.context.input.module(),
            body,
            graph,
            target::TargetProfile::WindowsX64,
            work.activation,
            &bindings,
        )?;
        let machine = HandleMachineReplay {
            body,
            block,
            prepared,
            graph,
            calls: &self.inputs.calls,
            geometry: &geometry,
        };
        if let Some(operation) = block.operations.get(position.1 as usize) {
            let caller = Caller {
                machine: body.id,
                block: block.id,
                operation_index: position.1 as usize,
                site: CallSite::Invocation(InvocationSite::Operation(operation.id)),
            };
            let next = Control::Position((
                position.0,
                position
                    .1
                    .checked_add(1)
                    .ok_or("handle operation position overflows")?,
            ));
            if self.inputs.calls.calls.contains_key(&caller) {
                return self.call(work, caller, next);
            }
            let occurrence = self
                .activations
                .scalar(work.activation, ScalarUseSite::Operation(operation.id))?;
            work.state = work
                .state
                .operation(position, &machine, occurrence)
                .map_err(|error| {
                    format!(
                        "handle operation {:?}/{position:?}/{:?} {:?}: {error}",
                        body.id, operation.id, operation.kind,
                    )
                })?;
            work.control = next;
            return self.pending.push(work);
        }
        if position.1 as usize != block.operations.len() {
            return Err("handle execution steps outside its original block".into());
        }
        match &block.terminator {
            terminal_psi::Terminator::InvokeNoNormal { edge, .. }
            | terminal_psi::Terminator::ExternalCompletion { edge, .. } => {
                let site = if matches!(
                    block.terminator,
                    terminal_psi::Terminator::InvokeNoNormal { .. }
                ) {
                    InvocationSite::InvokeNoNormal(*edge)
                } else {
                    InvocationSite::ExternalCompletion(*edge)
                };
                self.call(
                    work,
                    Caller {
                        machine: body.id,
                        block: block.id,
                        operation_index: position.1 as usize,
                        site: CallSite::Invocation(site),
                    },
                    Control::Position(position),
                )
            }
            terminal_psi::Terminator::Return { .. }
            | terminal_psi::Terminator::ReturnUnit { .. }
            | terminal_psi::Terminator::ReturnStructural { .. } => {
                let returned = work.state.capture_return(
                    work.activation,
                    position,
                    self.inputs,
                    &self.activations,
                    self.context,
                )?;
                work.control = Control::Cleanup {
                    position,
                    edge: returned.edge,
                    ordinal: 0,
                    completion: Completion::Return(returned),
                };
                self.pending.push(work)
            }
            terminal_psi::Terminator::Crash { edge, .. } => {
                // A crash cannot silently dispose the process's resource debt.
                work.state
                    .ledger
                    .returning_path(
                        self.activations
                            .scalar(work.activation, ScalarUseSite::Edge(*edge))?,
                        &[],
                    )
                    .map_err(|reason| {
                        format!("crash path has an incomplete handle lifetime: {reason:?}")
                    })?;
                self.completed += 1;
                Ok(())
            }
            _ => self.successors(work, &machine),
        }
    }

    fn successors(&mut self, work: Work, machine: &HandleMachineReplay<'_>) -> Result<(), String> {
        let position = work.control.position();
        let mut selected = Vec::new();
        let mut case_subject = None;
        let mut condition_dependencies = BTreeSet::new();
        let mut unknown_case_dependencies = BTreeSet::new();
        match &machine.block.terminator {
            terminal_psi::Terminator::Conditional {
                condition,
                when_true,
                when_false,
            } => {
                let value =
                    work.state
                        .scalar(*condition, position, &machine.prepared.expressions)?;
                // The saved evaluated Boolean selects the edge. Observations
                // are recorded on that actual edge, not its untaken sibling.
                condition_dependencies.extend(value.word.dependencies());
                for (ordinal, (edge, truth)) in [(when_true, true), (when_false, false)]
                    .into_iter()
                    .enumerate()
                {
                    if value.known != KnownHandleScalar::Boolean(!truth) {
                        selected.push((ordinal, edge.edge, edge.target, None));
                    }
                }
            }
            terminal_psi::Terminator::Jump { edge, target, .. } => {
                selected.push((0, *edge, *target, None))
            }
            terminal_psi::Terminator::StructuralCase {
                source,
                path,
                cases,
            } => {
                let location = machine.geometry.location(*source, path)?;
                work.state
                    .require_active_payload(&location, machine.geometry)?;
                let (range, _, _, address_dependencies) = machine
                    .geometry
                    .resolve(&location, &work.state.references)?;
                let carrier = machine
                    .geometry
                    .carrier(&location)?
                    .ok_or("handle case branch has no original sum carrier")?;
                let current = work
                    .state
                    .cases
                    .get(&range)
                    .filter(|tag| tag.carrier == carrier);
                unknown_case_dependencies.extend(address_dependencies);
                if current.is_none() {
                    // Represented writes can change the discriminator. Read
                    // its selected bytes, not neighboring payload aliases.
                    let (tag, dependencies) = machine
                        .geometry
                        .case_tag(&location, &work.state.references)?;
                    unknown_case_dependencies.extend(dependencies);
                    unknown_case_dependencies.extend(work.state.aliases.stored_dependencies(&tag));
                }
                for (ordinal, edge) in cases.iter().enumerate() {
                    if current.is_none_or(|tag| tag.case == edge.case) {
                        selected.push((ordinal, edge.edge, edge.target, Some(edge.case)));
                    }
                }
                case_subject = Some((range, carrier));
            }
            _ => return Err("handle execution has no supported original successor form".into()),
        }
        if selected.is_empty() {
            return Err("handle execution cannot account for any original successor".into());
        }
        for (ordinal, edge, target, case) in selected {
            let target = self.block(machine.body.id, target)?;
            let mut branch = work.clone();
            branch.state.observe(
                condition_dependencies
                    .iter()
                    .chain(&unknown_case_dependencies)
                    .copied(),
                self.activations
                    .scalar(work.activation, ScalarUseSite::Edge(edge))?,
            )?;
            if let Some((range, carrier)) = case_subject {
                let case = case.ok_or("handle case branch lost its selected case")?;
                branch
                    .state
                    .cases
                    .insert(range, CapturedCaseTag { carrier, case });
            }
            let occurrence = self
                .activations
                .scalar(work.activation, ScalarUseSite::Edge(edge))?;
            let arrival = branch
                .state
                .capture_arrival(position, ordinal, target, machine, occurrence)?;
            branch.control = Control::Cleanup {
                position,
                edge,
                ordinal: 0,
                completion: Completion::Arrival(arrival),
            };
            self.pending.push(branch)?;
        }
        Ok(())
    }
}

pub(super) fn derive(
    artifact: [u8; 32],
    context: &ReviewContext<'_>,
    inputs: &WindowsHandleInputs<'_>,
) -> Result<CheckedHandleExecution, String> {
    let activations = HandleReplayActivations::new(artifact, context, &inputs.calls)?;
    let body = inputs
        .calls
        .machine(inputs.calls.entry)
        .ok_or("handle entry body is absent")?;
    if !body.parameters.is_empty() || !body.structural_parameters.is_empty() {
        return Err("handle execution entry has no independently supplied argument custody".into());
    }
    let prepared = inputs
        .machines
        .get(body.id)
        .and_then(Option::as_ref)
        .ok_or("handle entry has no complete prepared scalar roster")?;
    let initial = CanonicalHandleState {
        ledger: HandleLifetimes::default(),
        aliases: HandleAliases::default(),
        references: HandleReferences::default(),
        cases: BTreeMap::new(),
        constants: IdTable::default(),
        stored_constants: BTreeMap::new(),
        bindings: HandleStorageBindings::new(body)?,
    }
    .enter(&prepared.expressions, body)?;
    let mut blocks = BTreeMap::new();
    for (machine, body) in inputs.calls.machines() {
        for block in &body.blocks {
            if blocks.insert((machine, block.id), block).is_some() {
                return Err("handle execution has duplicate original block identities".into());
            }
        }
    }
    let mut execution = Execution {
        context,
        inputs,
        arguments: super::super::windows_handle_arguments::prepare_handle_arguments(inputs)?,
        activations,
        blocks,
        pending: Pending::default(),
        occurrences: BTreeMap::new(),
        completed: 0,
    };
    execution.pending.push(Work {
        activation: 0,
        control: Control::Position((body.entry, 0)),
        frames: Vec::new(),
        state: initial,
    })?;
    let mut steps = 0usize;
    while let Some(work) = execution.pending.queue.pop_front() {
        steps += 1;
        if steps > 1_048_576 {
            return Err("handle execution exceeds its bounded total replay steps".into());
        }
        execution.step(work)?;
    }
    if execution.completed == 0 {
        return Err("handle execution has no checked complete path".into());
    }
    let reached = execution
        .occurrences
        .keys()
        .map(|occurrence| (occurrence.machine, occurrence.site))
        .collect::<BTreeSet<_>>();
    for caller in inputs.close_sites.iter().chain(&inputs.duplicate_sites) {
        let CallSite::Invocation(site) = caller.site else {
            return Err("handle release candidate has no original invocation identity".into());
        };
        if !reached.contains(&(caller.machine, site)) {
            return Err("handle release candidate has no checked reachable occurrence".into());
        }
    }
    Ok(CheckedHandleExecution {
        occurrences: execution.occurrences,
    })
}

//! Follow original control edges and finish cleanup before binding arrivals.
//!
//! Every queued state retains its own resource ledger. Numeric snapshots may
//! lose precision at a repeated cut; resource identities, pointers, callers and
//! retirement histories never join by selecting one convenient predecessor.

use super::super::windows_handle_calls::RuntimeArguments;
use super::*;
use semantic_vocabulary::{BlockId, BoundaryMachineId};
use std::cmp::Ordering;
use std::sync::Arc;
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
    frames: Vec<Arc<SuspendedCall>>,
    state: CanonicalHandleState,
}

/// Compare the same complete resource key without copying a second world.
/// Only evaluated numeric metadata widens. Past-use sets are unioned at an
/// otherwise identical state; generations, retirement, initialized coordinates,
/// pointer targets and every suspended caller keep their exact identities.
fn compare_resources(left: &CanonicalHandleState, right: &CanonicalHandleState) -> Ordering {
    let CanonicalHandleState {
        ledger: left_ledger,
        aliases: left_aliases,
        references: left_references,
        cases: left_cases,
        constants: left_constants,
        stored_constants: _, // The previous key cleared this numeric map.
        bindings: left_bindings,
    } = left;
    let CanonicalHandleState {
        ledger: right_ledger,
        aliases: right_aliases,
        references: right_references,
        cases: right_cases,
        constants: right_constants,
        stored_constants: _,
        bindings: right_bindings,
    } = right;
    left_ledger
        .compare_resource_state(right_ledger)
        .then_with(|| {
            (left_aliases, left_references, left_cases).cmp(&(
                right_aliases,
                right_references,
                right_cases,
            ))
        })
        .then_with(|| {
            left_constants
                .iter()
                .map(|(id, value)| (id, value.is_some()))
                .cmp(
                    right_constants
                        .iter()
                        .map(|(id, value)| (id, value.is_some())),
                )
        })
        .then_with(|| left_bindings.cmp(right_bindings))
}

fn compare_suspended(left: &Arc<SuspendedCall>, right: &Arc<SuspendedCall>) -> Ordering {
    if Arc::ptr_eq(left, right) {
        return Ordering::Equal;
    }
    let SuspendedCall {
        frame: left,
        continuation: left_continuation,
    } = left.as_ref();
    let SuspendedCall {
        frame: right,
        continuation: right_continuation,
    } = right.as_ref();
    let HandleReplayFrame {
        invocation: left_invocation,
        activation: left_activation,
        caller: left_caller,
        mutable_outputs: left_outputs,
        normal_return: left_normal,
        receiving: left_receiving,
    } = left;
    let HandleReplayFrame {
        invocation: right_invocation,
        activation: right_activation,
        caller: right_caller,
        mutable_outputs: right_outputs,
        normal_return: right_normal,
        receiving: right_receiving,
    } = right;
    (left_invocation, left_activation)
        .cmp(&(right_invocation, right_activation))
        .then_with(|| compare_resources(left_caller, right_caller))
        .then_with(|| {
            (left_outputs, left_normal, left_receiving, left_continuation).cmp(&(
                right_outputs,
                right_normal,
                right_receiving,
                right_continuation,
            ))
        })
}

#[derive(Clone)]
struct ResourceWork(Arc<Work>);

impl Ord for ResourceWork {
    fn cmp(&self, other: &Self) -> Ordering {
        if Arc::ptr_eq(&self.0, &other.0) {
            return Ordering::Equal;
        }
        let Work {
            activation: left_activation,
            control: left_control,
            frames: left_frames,
            state: left_state,
        } = self.0.as_ref();
        let Work {
            activation: right_activation,
            control: right_control,
            frames: right_frames,
            state: right_state,
        } = other.0.as_ref();
        (left_activation, left_control)
            .cmp(&(right_activation, right_control))
            .then_with(|| {
                left_frames
                    .iter()
                    .zip(right_frames)
                    .map(|(left, right)| compare_suspended(left, right))
                    .find(|order| *order != Ordering::Equal)
                    .unwrap_or_else(|| left_frames.len().cmp(&right_frames.len()))
            })
            .then_with(|| compare_resources(left_state, right_state))
    }
}

impl PartialOrd for ResourceWork {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for ResourceWork {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for ResourceWork {}

fn join_numbers(retained: &mut CanonicalHandleState, incoming: &CanonicalHandleState) -> bool {
    let mut changed = false;
    let updates = retained
        .constants
        .iter()
        .filter_map(|(id, value)| {
            let value = value.as_ref()?;
            (*value != KnownHandleScalar::Unknown
                && incoming.constants.get(id).and_then(Option::as_ref) != Some(value))
            .then_some(id)
        })
        .collect::<Vec<_>>();
    changed |= !updates.is_empty();
    for id in updates {
        *retained
            .constants
            .get_mut(id)
            .expect("numeric join retains its resource-key namespace") =
            Some(KnownHandleScalar::Unknown);
    }
    retained.stored_constants.retain(|range, value| {
        let keep = incoming.stored_constants.get(range) == Some(value);
        changed |= !keep;
        keep
    });
    changed
}

#[derive(Default)]
struct Pending {
    rows: BTreeMap<ResourceWork, Arc<Work>>,
    queue: std::collections::VecDeque<Arc<Work>>,
}

impl Pending {
    fn push(&mut self, work: Work) -> Result<(), String> {
        let work = Arc::new(work);
        let key = ResourceWork(Arc::clone(&work));
        if let Some(retained) = self.rows.get_mut(&key) {
            let representative = Arc::make_mut(retained);
            let mut changed = representative
                .state
                .ledger
                .merge_use_history(&work.state.ledger)
                .map_err(|reason| {
                    format!("handle state join changes resource custody: {reason:?}")
                })?;
            changed |= join_numbers(&mut representative.state, &work.state);
            for (retained, incoming) in representative.frames.iter_mut().zip(&work.frames) {
                if !Arc::ptr_eq(retained, incoming) {
                    let caller = &mut Arc::make_mut(retained).frame.caller;
                    changed |= caller
                        .ledger
                        .merge_use_history(&incoming.frame.caller.ledger)
                        .map_err(|reason| {
                            format!("handle caller join changes resource custody: {reason:?}")
                        })?;
                    changed |= join_numbers(caller, &incoming.frame.caller);
                }
            }
            if changed {
                self.queue.push_back(Arc::clone(retained));
            }
            return Ok(());
        }
        if self.rows.len() >= 65_536 {
            return Err(
                "handle execution exceeds its bounded distinct control and resource states".into(),
            );
        }
        self.rows.insert(key, Arc::clone(&work));
        self.queue.push_back(work);
        Ok(())
    }
}

pub(super) struct CheckedHandleExecution {
    pub occurrences: BTreeMap<NativeOccurrence, WindowsHandleContract>,
    pub external_completions: BTreeSet<(ScalarOccurrence, BoundaryMachineId)>,
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
    external_completions: BTreeSet<(ScalarOccurrence, BoundaryMachineId)>,
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
                    frames.push(Arc::new(SuspendedCall {
                        frame,
                        continuation: continuation.clone(),
                    }));
                    let body = self.body(activation)?;
                    self.pending.push(Work {
                        activation,
                        control: Control::Position((body.entry, 0)),
                        frames,
                        state,
                    })?;
                }
                CallTarget::Boundary(boundary) => {
                    if let Some(declaration) = self.inputs.external_calls.get(&caller) {
                        if declaration.completion
                            != terminal_psi::BoundaryCompletion::External(
                                semantic_vocabulary::ExternalCompletionKind::ProcessExit,
                            )
                        {
                            return Err("process exit lost its prepared admitted completion".into());
                        }
                        self.process_exit(work.clone(), *call)?;
                        continue;
                    }
                    let input = self.inputs.natives.get(&caller).ok_or_else(|| {
                        if self.inputs.opaque_calls.contains_key(&caller) {
                            format!("handle execution has no preserving contract for boundary call {caller:?}, boundary {boundary:?}, declaration {:?}, mechanism {:?}", self.context.boundaries.get(&boundary), self.context.mechanism(boundary))
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
                    self.intrinsic(work.clone(), *call, execution)?;
                }
            }
        }
        Ok(())
    }

    fn intrinsic(
        &mut self,
        work: Work,
        call: RuntimeCall<'_>,
        execution: effects::CompilerIntrinsicExecutionIdentity,
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
        self.process_exit(work, call)
    }

    fn process_exit(&mut self, mut work: Work, call: RuntimeCall<'_>) -> Result<(), String> {
        // This explicit canonical outcome has no normal successor or release
        // receipt. It remains conditional on the runtime's independently
        // delegated containing-domain authority; this replay grants none.
        // Ordinary returns still check every acquired handle below.
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
        let RuntimeArguments::Explicit { scalar, structural } = call.arguments else {
            return Err("process exit substitutes implicit cleanup operands".into());
        };
        if scalar != arguments.as_slice()
            || structural != structural_arguments.as_slice()
            || call.caller.machine != body.id
            || call.caller.operation_index != block.operations.len()
        {
            return Err("process exit substitutes its original caller/operand occurrence".into());
        }
        let declaration = self
            .context
            .input
            .module()
            .boundary_machine(*boundary)
            .ok_or("process exit lost its original Terminal declaration")?;
        if call.normal_return
            || call.receiving.is_some()
            || declaration.completion != terminal_psi::BoundaryCompletion::External(*kind)
            || *actual != edge
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
        work.state.observe_process_exit_status(&value, occurrence)?;
        // Do not close, erase or mark abandoned handles as released. There is
        // no continuation to receive this state, and earlier/later ordinary
        // returning alternatives keep their separate unchanged ledgers.
        self.external_completions.insert((occurrence, *boundary));
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
                    let suspended = Arc::unwrap_or_clone(suspended);
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

impl CanonicalHandleState {
    /// Reading the explicit exit status still observes its exact live value.
    /// Ending this path does not dispose of any handle in the retained ledger.
    fn observe_process_exit_status(
        &mut self,
        value: &CapturedHandleScalar,
        occurrence: ScalarOccurrence,
    ) -> Result<(), String> {
        self.observe(value.word.dependencies().iter().copied(), occurrence)?;
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
        Ok(())
    }
}

pub(super) fn derive(
    artifact: [u8; 32],
    context: &ReviewContext<'_>,
    inputs: &WindowsHandleInputs<'_>,
    progress: &HandleProofProgress,
) -> Result<CheckedHandleExecution, String> {
    progress.note(format_args!("execution setup started"));
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
        constants: HandleScalarTable::default(),
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
    progress.note(format_args!("closed argument checking started"));
    let arguments = super::super::windows_handle_arguments::prepare_handle_arguments(inputs)?;
    progress.note(format_args!("closed argument checking completed"));
    let mut execution = Execution {
        context,
        inputs,
        arguments,
        activations,
        blocks,
        pending: Pending::default(),
        occurrences: BTreeMap::new(),
        completed: 0,
        external_completions: BTreeSet::new(),
    };
    execution.pending.push(Work {
        activation: 0,
        control: Control::Position((body.entry, 0)),
        frames: Vec::new(),
        state: initial,
    })?;
    progress.note(format_args!("all-path replay started"));
    let mut steps = 0usize;
    while let Some(work) = execution.pending.queue.pop_front() {
        steps += 1;
        if steps > 1_048_576 {
            return Err("handle execution exceeds its bounded total replay steps".into());
        }
        execution.step(Arc::unwrap_or_clone(work))?;
        progress.replay(
            steps,
            execution.pending.rows.len(),
            execution.pending.queue.len(),
        );
    }
    progress.note(format_args!(
        "all-path replay completed: steps={steps}, retained={}, complete_paths={}",
        execution.pending.rows.len(),
        execution.completed,
    ));
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
        external_completions: execution.external_completions,
    })
}

#[cfg(test)]
mod tests {
    use super::super::super::windows_handle_lifetimes::HandleInstance;
    use super::super::super::windows_handle_storage::CapturedHandleReference;
    use super::*;
    use semantic_vocabulary::{BoundaryMachineId, ContractId, IntegerSign, OperationId, PlaceId};

    fn integer(value: u128) -> KnownHandleScalar {
        KnownHandleScalar::Integer(
            IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
            IntegerValue::Unsigned(value),
        )
    }

    fn home(place: u64) -> HandleStorageRange {
        HandleStorageRange::new(
            HandleStorageBacking {
                owner: HandleStorageOwner::Activation(0),
                place: PlaceId::new(place).unwrap(),
            },
            0,
            8,
        )
        .unwrap()
    }

    fn occurrence(operation: u64) -> NativeOccurrence {
        NativeOccurrence {
            machine: MachineId::new(1).unwrap(),
            site: InvocationSite::Operation(OperationId::new(operation).unwrap()),
            context: [1; 32],
            boundary: BoundaryMachineId::new(1).unwrap(),
        }
    }

    fn state() -> CanonicalHandleState {
        let body = terminal_psi::TerminalMachine {
            assembly_target_requirement: terminal_psi::RegisterTargetApplicability::Any,
            id: MachineId::new(1).unwrap(),
            attachment: None,
            nominal_cleanup_subject: None,
            ranked_scc: None,
            structural_places: Vec::new(),
            block_entry_claims: Vec::new(),
            loan_completions: Vec::new(),
            content_identity_reshuffles: Vec::new(),
            content_partition_compositions: Vec::new(),
            entry: BlockId::new(1).unwrap(),
            blocks: Vec::new(),
            contract: terminal_psi::MachineContract {
                declaration: None,
                parameters: Vec::new(),
                parameter_order: Vec::new(),
                structural_parameters: Vec::new(),
                result: terminal_psi::TerminalMachineResult::Unit,
                entry_claims: Vec::new(),
                declared_service_reach: None,
                closed_reach_application: None,
                published_synchronous_entries: Vec::new(),
                published_service_ceiling: Vec::new(),
                content_entry_claims: Vec::new(),
                ..terminal_psi::MachineContract::empty(ContractId::new(1).unwrap())
            },
        };
        let value = ValueId::new(5).unwrap();
        let later = ValueId::new(8).unwrap();
        let mut aliases = HandleAliases::with_scalar_namespace([value, later]).unwrap();
        aliases
            .define_scalar(
                value,
                HandleWordAliases::untracked(ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                )),
            )
            .unwrap();
        CanonicalHandleState {
            ledger: HandleLifetimes::default(),
            aliases,
            references: HandleReferences::default(),
            cases: BTreeMap::new(),
            constants: HandleScalarTable::from_table(
                IdTable::from_rows_with_gaps([(value, Some(integer(1))), (later, None)], 4)
                    .unwrap(),
            ),
            stored_constants: BTreeMap::from([(home(1), integer(1))]),
            bindings: HandleStorageBindings::new(&body).unwrap(),
        }
    }

    fn work() -> Work {
        Work {
            activation: 1,
            control: Control::Position((BlockId::new(2).unwrap(), 0)),
            frames: vec![Arc::new(SuspendedCall {
                frame: HandleReplayFrame {
                    invocation: HandleReplayInvocation {
                        parent: 0,
                        caller: Caller {
                            machine: MachineId::new(1).unwrap(),
                            block: BlockId::new(1).unwrap(),
                            operation_index: 0,
                            site: CallSite::Invocation(InvocationSite::Operation(
                                OperationId::new(1).unwrap(),
                            )),
                        },
                        callee: MachineId::new(2).unwrap(),
                    },
                    activation: 1,
                    caller: state(),
                    mutable_outputs: Vec::new(),
                    normal_return: true,
                    receiving: Some(terminal_psi::OperationResult::Unit),
                },
                continuation: Control::Position((BlockId::new(1).unwrap(), 1)),
            })],
            state: state(),
        }
    }

    #[test]
    fn windows_handle_process_exit_status_creates_no_release_receipt() {
        let mut active = state();
        let instance = HandleInstance(0);
        active
            .ledger
            .acquire_file(instance, occurrence(1), 0)
            .unwrap();
        let site = ScalarOccurrence {
            machine: MachineId::new(1).unwrap(),
            site: ScalarUseSite::Edge(EdgeId::new(2).unwrap()),
            context: [1; 32],
        };
        let status_type = ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap());
        let status = CapturedHandleScalar {
            word: HandleWordAliases::untracked(status_type),
            known: KnownHandleScalar::Unknown,
        };
        let before = active.ledger.clone();
        active.observe_process_exit_status(&status, site).unwrap();
        assert_eq!(active.ledger, before);
        // An ordinary return still owes close, even after that status read.
        assert!(active.ledger.returning_path(site, &[]).is_err());
        active.ledger.release_file(instance, occurrence(3)).unwrap();
        active.ledger.returning_path(site, &[]).unwrap();
        // No authority to read retired handle bits is supplied by exit.
        let retired_status = CapturedHandleScalar {
            word: HandleWordAliases::derived_from_dependencies(status_type, [instance]),
            known: KnownHandleScalar::Unknown,
        };
        assert!(
            active
                .observe_process_exit_status(&retired_status, site)
                .is_err()
        );
    }

    // The former production key is retained only as the equivalence oracle.
    fn old_key(mut work: Work) -> Work {
        fn forget(state: &mut CanonicalHandleState) {
            state.constants = state
                .constants
                .map(|_, value| value.as_ref().map(|_| KnownHandleScalar::Unknown));
            state.stored_constants.clear();
        }
        forget(&mut work.state);
        for suspended in &mut work.frames {
            forget(&mut Arc::make_mut(suspended).frame.caller);
        }
        work
    }

    #[test]
    fn resource_keys_preserve_the_original_numeric_widening_equivalence() {
        let base = work();
        let id = ValueId::new(5).unwrap();
        let mut variants = vec![base.clone()];
        let mut numeric = base.clone();
        *numeric.state.constants.get_mut(id).unwrap() = Some(integer(2));
        numeric.state.stored_constants.clear();
        *Arc::make_mut(&mut numeric.frames[0])
            .frame
            .caller
            .constants
            .get_mut(id)
            .unwrap() = Some(integer(3));
        variants.push(numeric);
        let mut missing = base.clone();
        *missing.state.constants.get_mut(id).unwrap() = None;
        variants.push(missing);
        let mut sibling = base.clone();
        Arc::make_mut(&mut sibling.frames[0])
            .frame
            .invocation
            .caller
            .site = CallSite::Invocation(InvocationSite::Operation(OperationId::new(2).unwrap()));
        variants.push(sibling);
        let mut continuation = base.clone();
        Arc::make_mut(&mut continuation.frames[0]).continuation =
            Control::Position((BlockId::new(3).unwrap(), 0));
        variants.push(continuation);
        let mut acquired = base.clone();
        acquired
            .state
            .ledger
            .acquire_file(HandleInstance(0), occurrence(1), 0)
            .unwrap();
        variants.push(acquired.clone());
        acquired
            .state
            .aliases
            .define_scalar(
                id,
                HandleWordAliases::acquired(
                    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap()),
                    HandleInstance(0),
                )
                .unwrap(),
            )
            .unwrap();
        variants.push(acquired.clone());
        acquired
            .state
            .ledger
            .release_file(HandleInstance(0), occurrence(2))
            .unwrap();
        variants.push(acquired);
        for target in [2, 3] {
            let mut pointer = base.clone();
            pointer.state.references.replace(
                home(1),
                CapturedHandleReference {
                    referent: StructuralTypeId::new(1).unwrap(),
                    range: home(target),
                    stored_scalar: None,
                    dependencies: BTreeSet::new(),
                },
            );
            variants.push(pointer);
        }
        for left in &variants {
            for right in &variants {
                let actual =
                    ResourceWork(Arc::new(left.clone())) == ResourceWork(Arc::new(right.clone()));
                assert_eq!(actual, old_key(left.clone()) == old_key(right.clone()));
            }
        }
        assert_eq!(
            compare_resources(&variants[0].state, &variants[1].state),
            Ordering::Equal
        );
        for changed in &variants[2..] {
            assert!(
                ResourceWork(Arc::new(base.clone())) != ResourceWork(Arc::new(changed.clone()))
            );
        }
    }

    #[test]
    fn windows_handle_history_join_propagates_current_and_suspended_uses_without_merging_other_state()
     {
        let instance = HandleInstance(0);
        let mut original = work();
        original
            .state
            .ledger
            .acquire_file(instance, occurrence(1), 0)
            .unwrap();
        Arc::make_mut(&mut original.frames[0]).frame.caller.ledger = original.state.ledger.clone();
        let mut incoming = original.clone();
        incoming
            .state
            .ledger
            .preserve(instance, occurrence(2))
            .unwrap();
        Arc::make_mut(&mut incoming.frames[0])
            .frame
            .caller
            .ledger
            .preserve(instance, occurrence(3))
            .unwrap();
        assert!(
            ResourceWork(Arc::new(original.clone())) == ResourceWork(Arc::new(incoming.clone()))
        );
        let mut pending = Pending::default();
        pending.push(original.clone()).unwrap();
        pending.push(incoming.clone()).unwrap();
        assert_eq!(pending.rows.len(), 1);
        assert_eq!(
            pending.queue.len(),
            2,
            "new history must reach every later successor"
        );
        let (key, representative) = pending.rows.first_key_value().unwrap();
        assert_eq!(key.0.state.ledger, original.state.ledger);
        assert_eq!(
            pending.queue.front().unwrap().state.ledger,
            original.state.ledger
        );
        assert_eq!(representative.state.ledger, incoming.state.ledger);
        assert_eq!(
            representative.frames[0].frame.caller.ledger,
            incoming.frames[0].frame.caller.ledger
        );
        assert_eq!(
            key.0.frames[0].frame.caller.ledger,
            original.frames[0].frame.caller.ledger
        );
        pending.push(incoming.clone()).unwrap();
        assert_eq!(
            pending.queue.len(),
            2,
            "an identical history cannot cause another replay"
        );
        let mut next = incoming.clone();
        next.state.ledger.preserve(instance, occurrence(4)).unwrap();
        pending.push(next.clone()).unwrap();
        assert_eq!(pending.rows.len(), 1);
        assert_eq!(pending.queue.len(), 3);
        assert_eq!(
            pending.queue.back().unwrap().state.ledger,
            next.state.ledger
        );
        let mut retired = original.clone();
        retired
            .state
            .ledger
            .release_file(instance, occurrence(5))
            .unwrap();
        assert!(ResourceWork(Arc::new(original.clone())) != ResourceWork(Arc::new(retired)));
        let mut sibling = original.clone();
        Arc::make_mut(&mut sibling.frames[0])
            .frame
            .invocation
            .caller
            .site = CallSite::Invocation(InvocationSite::Operation(OperationId::new(9).unwrap()));
        assert!(ResourceWork(Arc::new(original.clone())) != ResourceWork(Arc::new(sibling)));
        let mut case = original.clone();
        case.state.cases.insert(
            home(1),
            CapturedCaseTag {
                carrier: StructuralTypeId::new(1).unwrap(),
                case: semantic_vocabulary::StructuralCaseId::new(1).unwrap(),
            },
        );
        assert!(ResourceWork(Arc::new(original.clone())) != ResourceWork(Arc::new(case)));
        let mut unevaluated = original.clone();
        *unevaluated
            .state
            .constants
            .get_mut(ValueId::new(5).unwrap())
            .unwrap() = None;
        assert!(ResourceWork(Arc::new(original)) != ResourceWork(Arc::new(unevaluated)));
    }

    #[test]
    fn pending_shares_saved_worlds_and_widens_a_separate_representative() {
        let original = work();
        let caller = Arc::clone(&original.frames[0]);
        let mut pending = Pending::default();
        pending.push(original.clone()).unwrap();
        let (key, representative) = pending.rows.first_key_value().unwrap();
        assert!(Arc::ptr_eq(&key.0, representative));
        assert!(Arc::ptr_eq(representative, pending.queue.front().unwrap()));
        assert!(Arc::ptr_eq(&representative.frames[0], &caller));
        let id = ValueId::new(5).unwrap();
        let mut incoming = original;
        *incoming.state.constants.get_mut(id).unwrap() = Some(integer(2));
        *Arc::make_mut(&mut incoming.frames[0])
            .frame
            .caller
            .constants
            .get_mut(id)
            .unwrap() = Some(integer(3));
        pending.push(incoming).unwrap();
        assert_eq!(pending.rows.len(), 1);
        assert_eq!(pending.queue.len(), 2);
        let (key, representative) = pending.rows.first_key_value().unwrap();
        assert!(!Arc::ptr_eq(&key.0, representative));
        assert_eq!(key.0.state.constants.get(id), Some(&Some(integer(1))));
        assert_eq!(
            representative.state.constants.get(id),
            Some(&Some(KnownHandleScalar::Unknown))
        );
        assert_eq!(
            representative.frames[0].frame.caller.constants.get(id),
            Some(&Some(KnownHandleScalar::Unknown))
        );
        assert_eq!(
            caller.frame.caller.constants.get(id),
            Some(&Some(integer(1)))
        );
        assert!(Arc::ptr_eq(representative, pending.queue.back().unwrap()));
    }
}

//! Transfer a single execution claim between a caller and its prepared frames.
//! Only step enters the worker. Answering a parked call merely deposits its input.
use super::{StepControl, publication_slot, results::RetainedBacking};
use crate::TerminalInterpretError;
use crate::boundary::errors::malformed;
use crate::engine::execution::{
    CompletedReturn, OperationFlow, SuspendedCallResult, TerminalExecution, TerminatorFlow,
};
use crate::frame_store::ParkedFrames;
use crate::storage::runtime_rows::RuntimeRows;
use crate::storage::structural_storage::RetainedStorage;
use build_model::accounting::EvaluationInvocationAccount;
use terminal_psi::{
    BoundaryMachineDeclaration, ClaimTransfer, Operation, OperationKind,
    TerminalPreparedActivationEntry,
};
use terminal_semantics::calls::owned_activation::{self, StepShape};

type ResultBackings = Vec<Vec<crate::storage::reference::ReferenceBacking>>;

struct Parked {
    selected: TerminalPreparedActivationEntry,
    backing: RetainedBacking,
    result_backings: ResultBackings,
    frames: ParkedFrames,
    // Bind each byte-backed return address to its original checked block site.
    resume_sites: Vec<Option<semantic_vocabulary::OperationId>>,
    spills: super::spills::ScalarSpills,
    control: StepControl,
}

struct Running {
    shape: StepShape,
    operation: Operation,
    root: usize,
    backing: RetainedBacking,
    result_backings: ResultBackings,
    caller_control: Option<StepControl>,
}

pub(crate) struct OwnedActivations {
    parked: RuntimeRows<u64, Parked>,
    running: Vec<Running>,
    next_token: u64,
}

impl OwnedActivations {
    pub(crate) fn new(account: Option<&EvaluationInvocationAccount>) -> Self {
        Self {
            parked: RuntimeRows::new(account),
            running: Vec::new(),
            next_token: 0,
        }
    }

    pub(crate) fn has_retained_backing(&self) -> bool {
        !self.parked.is_empty() || !self.running.is_empty()
    }

    fn insert(&mut self, parked: Parked) -> Result<u64, TerminalInterpretError> {
        self.next_token = self.next_token.checked_add(1).ok_or(malformed())?;
        self.parked.insert_vacant(self.next_token, parked)?;
        Ok(self.next_token)
    }

    pub(crate) fn result_backings(
        &self,
        home: usize,
    ) -> Option<&[Vec<crate::storage::reference::ReferenceBacking>]> {
        let running = self.running.last()?;
        (running.root == home).then_some(running.result_backings.as_slice())
    }

    pub(crate) fn holds_byte_loan(
        &self,
        matches: impl Fn(&crate::storage::byte_sequences::binding::ByteSequenceBinding) -> bool,
    ) -> bool {
        let backings = self
            .parked
            .values()
            .map(|parked| &parked.backing)
            .chain(self.running.iter().map(|running| &running.backing));
        backings.into_iter().any(|backing| {
            matches!(backing.binding(), super::results::RetainedDescriptor::Bytes(binding) if matches(binding))
        }) || self.parked.values().any(|parked| {
            matches!(&parked.control, StepControl::Answered(answer)
                if answer.1.completed.byte_sequence.as_ref().is_some_and(&matches))
        })
    }

    pub(crate) fn retain_storage(&self, kept: &mut RetainedStorage) {
        for parked in self.parked.values() {
            parked.backing.retain_storage(kept);
            for source in parked.result_backings.iter().flatten() {
                source.retain_storage(kept);
            }
            parked.control.retain_storage(kept);
        }
        for running in &self.running {
            running.backing.retain_storage(kept);
            for source in running.result_backings.iter().flatten() {
                source.retain_storage(kept);
            }
            if let Some(control) = &running.caller_control {
                control.retain_storage(kept);
            }
        }
    }
}

impl StepControl {
    pub(crate) fn retain_storage(&self, kept: &mut RetainedStorage) {
        if let Self::Answered(answer) = self {
            let reply = &answer.1.completed;
            if let crate::TerminalExecutionResult::Structural(value) = &reply.value {
                kept.value(&value.value);
            }
            if let Some(binding) = &reply.byte_sequence {
                kept.byte_binding(binding);
            }
            if let Some(view) = &reply.element_view {
                view.retain_storage(kept);
            }
        }
    }
}

impl TerminalExecution<'_> {
    fn save_activation_frames(
        &mut self,
        root: usize,
        backing: &RetainedBacking,
    ) -> Result<(ParkedFrames, Vec<Option<semantic_vocabulary::OperationId>>), TerminalInterpretError>
    {
        let worker = self.take_frame(SuspendedCallResult::Unit, Vec::new())?;
        let (caller, frames) = self.call_stack.park(root, worker).ok_or(malformed())?;
        self.restore_return_caller(caller);
        let homes: Vec<_> = self.call_stack.parked_homes(&frames).collect();
        let mut sites = Vec::with_capacity(homes.len());
        for (index, home) in homes.into_iter().enumerate() {
            let saved = self.call_stack.get_mut(home).ok_or(malformed())?;
            let position = std::mem::take(&mut saved.next_operation);
            let block = self
                .program
                .verified_block(saved.current_machine, saved.current)?;
            if position > block.operations.len() {
                return Err(malformed());
            }
            sites.push(block.operations.get(position).map(|op| op.id));
            backing.put_position(self, index, position)?;
        }
        Ok((frames, sites))
    }

    fn restore_activation_positions(
        &mut self,
        parked: &Parked,
    ) -> Result<(), TerminalInterpretError> {
        let homes: Vec<_> = self.call_stack.parked_homes(&parked.frames).collect();
        if homes.len() != parked.resume_sites.len() {
            return Err(malformed());
        }
        for (index, (home, site)) in homes.into_iter().zip(&parked.resume_sites).enumerate() {
            let position = parked.backing.position(self, index)?;
            let saved = self.call_stack.get_mut(home).ok_or(malformed())?;
            let block = self
                .program
                .verified_block(saved.current_machine, saved.current)?;
            if position > block.operations.len()
                || block.operations.get(position).map(|op| op.id) != *site
            {
                return Err(malformed());
            }
            saved.next_operation = position;
        }
        Ok(())
    }

    pub(super) fn prepare_activation(
        &mut self,
        operation: &Operation,
        boundary: &BoundaryMachineDeclaration,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::BoundaryCall {
            arguments,
            structural_arguments,
            completion_receipts,
            ..
        } = &operation.kind
        else {
            return Err(malformed());
        };
        let program = std::sync::Arc::clone(&self.program);
        let shape = owned_activation::prepare_shape(&program.catalogs, boundary)
            .map_err(|_| malformed())?;
        let (retained, inputs) = structural_arguments.split_last().ok_or(malformed())?;
        let backing = RetainedBacking::capture(self, retained)?;
        if !super::funding::supports(self, shape.selected, &backing)? {
            self.return_preparation_inputs(operation, &shape)?;
            return Ok(OperationFlow::Advance);
        }
        let worker = shape.selected.entry_machine;
        let callee = program.verified_machine(worker)?;
        let scalars = self.scalar_values_at(arguments)?;
        let prepared = self.prepare_structural_call_arguments(worker, inputs)?;
        let result_backings = match callee.result.structural() {
            Some(result) if !result.reference_sources.is_empty() => {
                let views = self.bind_call_access_views(worker, inputs, &prepared.values)?;
                self.bind_reference_return_backings(
                    &callee.structural_parameters,
                    result,
                    &prepared.values,
                    &prepared.aggregate_backings,
                    &prepared.primitive_loans,
                    &prepared.element_views,
                    &prepared.byte_sequences,
                    &views,
                )?
            }
            _ => Vec::new(),
        };

        // Both declarations use the same first source argument. Receipts for
        // the retained descriptor are settled in the caller, not given to Entry.
        let transfers: Vec<_> = completion_receipts
            .iter()
            .filter(|receipt| (receipt.argument_index as usize) < inputs.len())
            .map(|receipt| ClaimTransfer {
                claim: receipt.claim,
                argument_index: receipt.argument_index,
                path: receipt.path.clone(),
            })
            .collect();
        self.enter_call(
            worker,
            callee,
            &scalars,
            inputs,
            prepared,
            &transfers,
            SuspendedCallResult::Unit,
        )?;
        let root = self.call_stack.current();
        // enter_call advanced the caller's prepare site but did not execute Entry.
        let (frames, resume_sites) = self.save_activation_frames(root, &backing)?;
        let spills = super::spills::ScalarSpills::save(self, &backing, &frames, true)?;
        self.consume_activation_input(operation, retained)?;
        let token = self.owned_activations.insert(Parked {
            control: StepControl::Running,
            selected: shape.selected,
            result_backings,
            backing,
            frames,
            resume_sites,
            spills,
        })?;
        self.issue_activation_owner(operation, shape.ready, shape.owner, token)?;
        Ok(OperationFlow::Redispatch)
    }

    pub(super) fn step_activation(
        &mut self,
        operation: &Operation,
        boundary: &BoundaryMachineDeclaration,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::BoundaryCall {
            structural_arguments,
            ..
        } = &operation.kind
        else {
            return Err(malformed());
        };
        let [owner] = structural_arguments.as_slice() else {
            return Err(malformed());
        };
        let token = self.activation_token(owner)?;
        let parked = self
            .owned_activations
            .parked
            .get(&token)
            .ok_or(malformed())?;
        if matches!(parked.control, StepControl::Waiting(_)) {
            return Err(malformed());
        }
        let shape = owned_activation::step_shape(
            &self.program.catalogs,
            boundary,
            parked.selected.preparation,
        )
        .map_err(|_| malformed())?;
        self.consume_activation_input(operation, owner)?;
        let parked = self
            .owned_activations
            .parked
            .remove(&token)
            .ok_or(malformed())?;
        self.restore_activation_positions(&parked)?;
        parked.spills.restore(self, &parked.backing)?;
        let caller = self.take_frame(SuspendedCallResult::Unit, Vec::new())?;
        let root = parked.frames.root();
        let worker = self
            .call_stack
            .enter_parked(caller, parked.frames)
            .ok_or(malformed())?;
        self.restore_return_caller(worker);
        let position = self.next_operation;
        match &parked.control {
            StepControl::Running if position == 0 => {}
            StepControl::Answered(answer)
                if self
                    .program
                    .verified_block(self.current_machine, self.current)?
                    .operations
                    .get(position)
                    .is_some_and(|op| op.id == answer.0.site.operation) => {}
            _ => return Err(malformed()),
        }
        let caller_control = self.owned_step.replace(parked.control);
        self.owned_activations.running.push(Running {
            root,
            shape,
            backing: parked.backing,
            result_backings: parked.result_backings,
            operation: operation.clone(),
            caller_control,
        });
        Ok(OperationFlow::Redispatch)
    }

    pub(super) fn publish_parked_activation(
        &mut self,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let running = self.owned_activations.running.pop().ok_or(malformed())?;
        let control =
            std::mem::replace(&mut self.owned_step, running.caller_control).ok_or(malformed())?;
        let (frames, resume_sites) = self.save_activation_frames(running.root, &running.backing)?;
        let spills = super::spills::ScalarSpills::save(self, &running.backing, &frames, false)?;
        let token = self.owned_activations.insert(Parked {
            selected: running.shape.selected,
            backing: running.backing,
            result_backings: running.result_backings,
            frames,
            resume_sites,
            spills,
            control,
        })?;
        self.issue_activation_owner(
            &running.operation,
            running.shape.parked,
            running.shape.owner,
            token,
        )?;
        self.next_operation += 1;
        Ok(OperationFlow::Redispatch)
    }

    pub(crate) fn returning_owned_activation(&self) -> bool {
        self.owned_activations
            .running
            .last()
            .is_some_and(|running| running.root == self.call_stack.current())
    }

    pub(crate) fn finish_owned_activation(
        &mut self,
        completed: CompletedReturn,
    ) -> Result<TerminatorFlow, TerminalInterpretError> {
        let running = self.owned_activations.running.pop().ok_or(malformed())?;
        self.owned_step = running.caller_control;
        let caller = self.call_stack.pop().ok_or(malformed())?;
        self.restore_return_caller(caller);
        self.complete_activation_output(
            &running.operation,
            &running.shape,
            running.backing,
            completed,
        )?;
        self.next_operation += 1;
        Ok(TerminatorFlow::Continue)
    }

    pub(super) fn answer_activation(
        &mut self,
        operation: &Operation,
        boundary: &BoundaryMachineDeclaration,
    ) -> Result<OperationFlow, TerminalInterpretError> {
        let OperationKind::BoundaryCall {
            structural_arguments,
            ..
        } = &operation.kind
        else {
            return Err(malformed());
        };
        let [owner, permit, ..] = structural_arguments.as_slice() else {
            return Err(malformed());
        };
        let token = self.activation_token(owner)?;
        let parked = self
            .owned_activations
            .parked
            .get(&token)
            .ok_or(malformed())?;
        let StepControl::Waiting(waiting) = &parked.control else {
            return Err(malformed());
        };
        let occurrence = waiting.suspension.clone();
        let program = std::sync::Arc::clone(&self.program);
        let shape = owned_activation::resume_shape(
            &program.catalogs,
            boundary,
            parked.selected.preparation,
        )
        .map_err(|_| malformed())?;
        let slot = publication_slot(self, permit, shape.permit.anchor_identity)?;
        let reply = program
            .catalogs
            .identities
            .text(shape.permit.reply)
            .ok_or(malformed())?;
        let mut parked = self
            .owned_activations
            .parked
            .remove(&token)
            .ok_or(malformed())?;
        if parked
            .control
            .answer_from(&occurrence, &slot, reply, self, |sender| {
                sender.take_activation_answer(operation, permit, &shape.permit)
            })?
        {
            self.consume_activation_input(operation, owner)?;
            let token = self.owned_activations.insert(parked)?;
            self.issue_activation_owner(operation, shape.ready, shape.ready_field, token)?;
        } else {
            self.owned_activations.parked.insert_vacant(token, parked)?;
            self.return_mismatched_reply(operation, &shape)?;
        }
        Ok(OperationFlow::Advance)
    }
}

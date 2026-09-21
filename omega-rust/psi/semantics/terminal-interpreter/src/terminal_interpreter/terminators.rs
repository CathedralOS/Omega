//! Block terminators of the interpreter loop: the Unit, structural and
//! scalar returns with their cleanup custody, jumps and conditionals, case
//! dispatch, and crashes.

use crate::terminal_interpreter::custody::{
    commit_cleanup_actions, consume_affine_projection, has_live_linear_claims,
    rebind_structural_result_claims, remove_affine_root, resolve_structural_path_type,
};
use crate::terminal_interpreter::execution::{
    LiveClaim, SuspendedCall, SuspendedCallResult, TerminalExecution, TerminatorFlow,
};
use crate::terminal_interpreter::reference;
use crate::terminal_interpreter::results::meter_status;
use crate::terminal_interpreter::{
    TerminalCrash, TerminalCrashSite, TerminalExecutionResult, TerminalExecutionStatus,
    TerminalInterpretError, TerminalScalarCaseResult, TerminalScalarValue,
    TerminalStructuralResult,
};
use semantic_vocabulary::{ClaimId, PlaceId, StructuralTypeId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_fuel::TerminalFuelMeter;
use terminal_psi::{
    StructuralAccess, StructuralAffineDiscard, StructuralMultiplicity, StructuralOperationResult,
    StructuralPathSegment, StructuralTypeShape, TerminalMachineResult, Terminator,
};

impl TerminalExecution {
    pub(super) fn settle_return_unit_nominal_affine(
        &mut self,
        terminator: &Terminator,
        meter: &mut TerminalFuelMeter,
    ) -> Result<TerminatorFlow, TerminalInterpretError> {
        let Terminator::ReturnUnitNominalAffine { cleanups, .. } = terminator else {
            unreachable!("dispatched settle_return_unit_nominal_affine")
        };
        let machine = self.machines.get(&self.current_machine).ok_or(
            TerminalInterpretError::VerifiedCallTargetMissing(self.current_machine),
        )?;
        if machine.result != TerminalMachineResult::Unit
            || has_live_linear_claims(&self.live_claims)
            || !self.live_claims.is_empty()
            || cleanups.is_empty()
        {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        let mut expected_frontier = BTreeSet::new();
        let mut cleanup_values = Vec::with_capacity(cleanups.len());
        for cleanup in cleanups {
            let value = self.structural_values.get(&cleanup.place).cloned().ok_or(
                TerminalInterpretError::VerifiedStructuralPlaceMissing(cleanup.place),
            )?;
            if value.structural_type != cleanup.structural_type
                || !expected_frontier.insert(StructuralAffineDiscard {
                    place: cleanup.place,
                    path: Vec::new(),
                    structural_type: cleanup.structural_type,
                })
            {
                return Err(TerminalInterpretError::AffineFrontierMismatch);
            }
            self.machines.get(&cleanup.cleanup_machine).ok_or(
                TerminalInterpretError::VerifiedCallTargetMissing(cleanup.cleanup_machine),
            )?;
            cleanup_values.push((cleanup.clone(), value));
        }
        if self.structural_values.len() != cleanup_values.len() + self.primitive_local_count()
            || self.live_affine_frontier != expected_frontier
        {
            return Err(TerminalInterpretError::AffineFrontierMismatch);
        }
        if let Err(error) = meter.charge_terminator(terminator) {
            return meter_status(error).map(TerminatorFlow::Yield);
        }
        for (cleanup, _) in &cleanup_values {
            self.structural_values
                .remove(&cleanup.place)
                .expect("validated nominal cleanup roots remain live through edge charge");
        }
        self.retire_plain_locals();
        if !self.structural_values.is_empty() {
            return Err(TerminalInterpretError::AffineFrontierMismatch);
        }
        let (completed, remaining) = cleanup_values
            .split_first()
            .expect("non-empty nominal cleanup list was validated");
        let completed = completed.clone();
        let remaining = remaining.to_vec();
        let machines = std::sync::Arc::clone(&self.machines);
        let callee = machines
            .get(&completed.0.cleanup_machine)
            .expect("all nominal cleanup targets were validated before edge charge");
        // The edge lends the consumed value to the hook's borrowed `self`
        // receiver: the callee frame starts holding it at the declared
        // receiver place, and the hook's ordinary body runs against it.
        let receiver_binding = completed
            .0
            .cleanup_receiver
            .map(|receiver| (receiver, completed.1.clone()));
        self.call_stack.push(SuspendedCall {
            values: std::mem::take(&mut self.values),
            structural_values: std::mem::take(&mut self.structural_values),
            byte_sequence_values: std::mem::take(&mut self.byte_sequence_values),
            scalar_case_values: std::mem::take(&mut self.scalar_case_values),
            scalar_array_values: std::mem::take(&mut self.scalar_array_values),
            live_affine_frontier: std::mem::take(&mut self.live_affine_frontier),
            live_claims: std::mem::take(&mut self.live_claims),
            dynamic_parameters: std::mem::take(&mut self.dynamic_parameters),
            current_machine: self.current_machine,
            current: self.current,
            next_operation: self.next_operation,
            result: SuspendedCallResult::NominalCleanups {
                completed,
                remaining,
                final_result: None,
            },
        });
        self.values = BTreeMap::new();
        self.structural_values = BTreeMap::new();
        if let Some((receiver, value)) = receiver_binding {
            self.structural_values.insert(receiver, value);
        }
        self.live_affine_frontier = BTreeSet::new();
        self.live_claims = BTreeMap::new();
        self.dynamic_parameters = BTreeMap::new();
        self.current_machine = cleanups[0].cleanup_machine;
        self.current = callee.entry;
        self.next_operation = 0;
        Ok(TerminatorFlow::Continue)
    }

    pub(super) fn settle_return_unit_partial_affine(
        &mut self,
        terminator: &Terminator,
        meter: &mut TerminalFuelMeter,
    ) -> Result<TerminatorFlow, TerminalInterpretError> {
        let Terminator::ReturnUnitPartialAffine {
            trivial_affine_discards,
            residual_affine_discards,
            ..
        } = terminator
        else {
            unreachable!("dispatched settle_return_unit_partial_affine")
        };
        let machine = self.machines.get(&self.current_machine).ok_or(
            TerminalInterpretError::VerifiedCallTargetMissing(self.current_machine),
        )?;
        if machine.result != TerminalMachineResult::Unit
            || has_live_linear_claims(&self.live_claims)
        {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }

        // Validate the entire cleanup transaction before charging or
        // mutating state. In particular, a projected cleanup may not
        // be approximated by deleting its root-addressed carrier.
        let mut expected_frontier = BTreeSet::new();
        for place in trivial_affine_discards {
            let structural_type = self
                .structural_values
                .get(place)
                .map(|value| value.structural_type)
                .or_else(|| {
                    self.scalar_case_values
                        .get(place)
                        .map(|value| value.structural_type)
                })
                .ok_or(TerminalInterpretError::VerifiedStructuralPlaceMissing(
                    *place,
                ))?;
            if !expected_frontier.insert(StructuralAffineDiscard {
                place: *place,
                path: Vec::new(),
                structural_type,
            }) {
                return Err(TerminalInterpretError::AffineFrontierMismatch);
            }
        }
        for discard in residual_affine_discards {
            let root = self.structural_values.get(&discard.place).ok_or(
                TerminalInterpretError::VerifiedStructuralPlaceMissing(discard.place),
            )?;
            let actual_type = resolve_structural_path_type(
                &self.structural_types,
                root.structural_type,
                &discard.path,
            )?;
            if actual_type != discard.structural_type
                || discard.path.is_empty()
                || !expected_frontier.insert(discard.clone())
            {
                return Err(TerminalInterpretError::AffineFrontierMismatch);
            }
        }
        if expected_frontier != self.live_affine_frontier {
            return Err(TerminalInterpretError::AffineFrontierMismatch);
        }
        if let Err(error) = meter.charge_terminator(terminator) {
            return meter_status(error).map(TerminatorFlow::Yield);
        }

        // The edge is now committed. Root cleanup may release its
        // root-addressed carrier; projected cleanup removes only the
        // exact semantic paths and leaves the opaque root untouched.
        for place in trivial_affine_discards {
            reference::discard_structural_value(
                &mut self.structural_values,
                &mut self.reference_referents,
                *place,
            );
            self.scalar_case_values.remove(place);
            self.live_affine_frontier.remove(&StructuralAffineDiscard {
                place: *place,
                path: Vec::new(),
                structural_type: expected_frontier
                    .iter()
                    .find(|entry| entry.place == *place && entry.path.is_empty())
                    .expect("validated root affine cleanup")
                    .structural_type,
            });
        }
        for discard in residual_affine_discards {
            self.live_affine_frontier.remove(discard);
        }
        debug_assert!(self.live_affine_frontier.is_empty());

        if let Some(caller) = self.call_stack.pop() {
            if !matches!(caller.result, SuspendedCallResult::Unit) {
                return Err(TerminalInterpretError::VerifiedOperationMalformed);
            }
            self.values = caller.values;
            self.retire_plain_locals();
            self.structural_values = caller.structural_values;
            self.scalar_case_values = caller.scalar_case_values;
            self.scalar_array_values = caller.scalar_array_values;
            self.byte_sequence_values = caller.byte_sequence_values;
            self.live_affine_frontier = caller.live_affine_frontier;
            self.live_claims = caller.live_claims;
            self.dynamic_parameters = caller.dynamic_parameters;
            self.current_machine = caller.current_machine;
            self.current = caller.current;
            self.next_operation = caller.next_operation;
            return Ok(TerminatorFlow::Continue);
        }
        let result = TerminalExecutionResult::Unit;
        self.retire_plain_locals();
        self.result = Some(result.clone());
        Ok(TerminatorFlow::Yield(TerminalExecutionStatus::Complete(
            result,
        )))
    }

    pub(super) fn settle_jump(
        &mut self,
        terminator: &Terminator,
        meter: &mut TerminalFuelMeter,
    ) -> Result<TerminatorFlow, TerminalInterpretError> {
        let Terminator::Jump {
            target,
            arguments,
            structural_arguments,
            trivial_affine_discards,
            residual_affine_discards,
            ..
        } = terminator
        else {
            unreachable!("dispatched settle_jump")
        };
        // Projected owned arguments open partial custody on their
        // roots: each moves an exact affine child while the
        // residual list closes every dying root's untouched
        // complement. Replay the splits on a scratch frontier so
        // the whole edge validates before charging or mutating
        // state.
        let mut projected_frontier = None;
        let mut residual_roots = BTreeSet::new();
        for argument in structural_arguments.iter().filter(|argument| {
            argument.access == StructuralAccess::Owned && !argument.path.is_empty()
        }) {
            residual_roots.insert(argument.place);
            consume_affine_projection(
                &self.structural_types,
                &self.structural_values,
                projected_frontier.get_or_insert_with(|| self.live_affine_frontier.clone()),
                argument,
            )?;
        }
        let mut expected = BTreeSet::new();
        for discard in residual_affine_discards {
            let root = self.structural_values.get(&discard.place).ok_or(
                TerminalInterpretError::VerifiedStructuralPlaceMissing(discard.place),
            )?;
            if discard.path.is_empty()
                || resolve_structural_path_type(
                    &self.structural_types,
                    root.structural_type,
                    &discard.path,
                )? != discard.structural_type
                || !expected.insert(discard.clone())
            {
                return Err(TerminalInterpretError::AffineFrontierMismatch);
            }
            residual_roots.insert(discard.place);
        }
        let replayed = projected_frontier
            .as_ref()
            .unwrap_or(&self.live_affine_frontier);
        // Every root this edge touches must be left with exactly its
        // declared residual rows: projected transfers opened the
        // holes, and the listed complement is the only remainder.
        for place in &residual_roots {
            if replayed
                .iter()
                .filter(|entry| entry.place == *place)
                .cloned()
                .collect::<BTreeSet<_>>()
                != expected
                    .iter()
                    .filter(|entry| entry.place == *place)
                    .cloned()
                    .collect::<BTreeSet<_>>()
            {
                return Err(TerminalInterpretError::AffineFrontierMismatch);
            }
        }
        if let Err(error) = meter.charge_terminator(terminator) {
            return meter_status(error).map(TerminatorFlow::Yield);
        }
        let bindings =
            self.prepare_block_bindings(*target, arguments, structural_arguments, true)?;
        bindings.validate_discards(self, trivial_affine_discards, residual_affine_discards)?;
        // The edge committed: each projected argument retires the
        // containing root entry and leaves the residual sibling
        // subtrees live; the residual list then closes exactly
        // that remainder.
        for argument in structural_arguments.iter().filter(|argument| {
            argument.access == StructuralAccess::Owned && !argument.path.is_empty()
        }) {
            consume_affine_projection(
                &self.structural_types,
                &self.structural_values,
                &mut self.live_affine_frontier,
                argument,
            )?;
        }
        for discard in residual_affine_discards {
            self.live_affine_frontier.remove(discard);
        }
        for place in &residual_roots {
            // Every remaining semantic path was validated and
            // disposed. Only now may the dead root's opaque backing
            // leave storage. Referent descriptors under a moved
            // path belong to the destination's retained identity;
            // every other captured subtree dies with the root.
            let moved_paths: Vec<&[StructuralPathSegment]> = structural_arguments
                .iter()
                .filter(|argument| {
                    argument.access == StructuralAccess::Owned
                        && !argument.path.is_empty()
                        && argument.place == *place
                })
                .map(|argument| argument.path.as_slice())
                .collect();
            if let Some(value) = self.structural_values.remove(place) {
                self.reference_referents.retain(|carrier, _| {
                    if carrier.opaque_identity != value.opaque_identity
                        || !carrier.path.starts_with(&value.path)
                    {
                        return true;
                    }
                    moved_paths
                        .iter()
                        .any(|moved| carrier.path[value.path.len()..].starts_with(moved))
                });
            }
        }
        for place in trivial_affine_discards {
            if reference::discard_structural_value(
                &mut self.structural_values,
                &mut self.reference_referents,
                *place,
            )
            .is_none()
                && self.scalar_case_values.remove(place).is_none()
            {
                return Err(TerminalInterpretError::VerifiedStructuralPlaceMissing(
                    *place,
                ));
            }
            remove_affine_root(&mut self.live_affine_frontier, *place);
        }
        bindings.commit(self);
        self.current = *target;
        self.next_operation = 0;
        Ok(TerminatorFlow::Continue)
    }

    pub(super) fn settle_conditional(
        &mut self,
        terminator: &Terminator,
        meter: &mut TerminalFuelMeter,
    ) -> Result<TerminatorFlow, TerminalInterpretError> {
        let Terminator::Conditional {
            condition,
            when_true,
            when_false,
        } = terminator
        else {
            unreachable!("dispatched settle_conditional")
        };
        let condition = self
            .values
            .get(condition)
            .copied()
            .ok_or(TerminalInterpretError::VerifiedValueMissing(*condition))?;
        let TerminalScalarValue::Boolean(condition) = condition else {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        };
        let successor = if condition { when_true } else { when_false };
        if let Err(error) = meter.charge_edge(successor.edge, terminator) {
            return meter_status(error).map(TerminatorFlow::Yield);
        }
        let bindings = self.prepare_block_bindings(
            successor.target,
            &successor.arguments,
            &successor.structural_arguments,
            false,
        )?;
        bindings.validate_discards(self, &successor.trivial_affine_discards, &[])?;
        for place in &successor.trivial_affine_discards {
            if reference::discard_structural_value(
                &mut self.structural_values,
                &mut self.reference_referents,
                *place,
            )
            .is_none()
                && self.scalar_case_values.remove(place).is_none()
            {
                return Err(TerminalInterpretError::VerifiedStructuralPlaceMissing(
                    *place,
                ));
            }
            remove_affine_root(&mut self.live_affine_frontier, *place);
        }
        bindings.commit(self);
        self.current = successor.target;
        self.next_operation = 0;
        Ok(TerminatorFlow::Continue)
    }

    pub(super) fn settle_structural_case(
        &mut self,
        terminator: &Terminator,
        meter: &mut TerminalFuelMeter,
    ) -> Result<TerminatorFlow, TerminalInterpretError> {
        let Terminator::StructuralCase { source, cases } = terminator else {
            unreachable!("dispatched settle_structural_case")
        };
        // Only internally established values carry a discriminator
        // and scalar payload. Opaque host roots remain unsupported.
        let value = self.scalar_case_values.get(source).ok_or(
            TerminalInterpretError::VerifiedStructuralPlaceMissing(*source),
        )?;
        let successor = cases
            .iter()
            .find(|successor| successor.case == value.result_case)
            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
        let target = self
            .machines
            .get(&self.current_machine)
            .ok_or(TerminalInterpretError::VerifiedCallTargetMissing(
                self.current_machine,
            ))?
            .blocks
            .get(&successor.target)
            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
        if target.parameters.len() != successor.payload_fields.len() {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        let mut bindings = Vec::with_capacity(target.parameters.len());
        for (parameter, field) in target.parameters.iter().zip(&successor.payload_fields) {
            let scalar = value
                .fields
                .iter()
                .find(|(identity, _)| identity == field)
                .map(|(_, scalar)| *scalar)
                .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
            if scalar.scalar_type() != parameter.scalar_type {
                return Err(TerminalInterpretError::VerifiedOperationMalformed);
            }
            bindings.push((parameter.id, scalar));
        }
        for place in &successor.trivial_affine_discards {
            if !self.structural_values.contains_key(place)
                && !self.scalar_case_values.contains_key(place)
            {
                return Err(TerminalInterpretError::VerifiedStructuralPlaceMissing(
                    *place,
                ));
            }
        }
        if let Err(error) = meter.charge_edge(successor.edge, terminator) {
            return meter_status(error).map(TerminatorFlow::Yield);
        }
        for place in &successor.trivial_affine_discards {
            reference::discard_structural_value(
                &mut self.structural_values,
                &mut self.reference_referents,
                *place,
            );
            self.scalar_case_values.remove(place);
            remove_affine_root(&mut self.live_affine_frontier, *place);
        }
        self.values.extend(bindings);
        self.current = successor.target;
        self.next_operation = 0;
        Ok(TerminatorFlow::Continue)
    }

    pub(super) fn settle_return(
        &mut self,
        terminator: &Terminator,
        meter: &mut TerminalFuelMeter,
    ) -> Result<TerminatorFlow, TerminalInterpretError> {
        let Terminator::Return {
            value,
            cleanup_actions,
            ..
        } = terminator
        else {
            unreachable!("dispatched settle_return")
        };
        let machine = self.machines.get(&self.current_machine).ok_or(
            TerminalInterpretError::VerifiedCallTargetMissing(self.current_machine),
        )?;
        if !matches!(machine.result, TerminalMachineResult::Scalar(_))
            || has_live_linear_claims(&self.live_claims)
        {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        if let Err(error) = meter.charge_terminator(terminator) {
            return meter_status(error).map(TerminatorFlow::Yield);
        }
        let result = self
            .values
            .get(value)
            .copied()
            .ok_or(TerminalInterpretError::VerifiedValueMissing(*value))?;
        for parameter in machine
            .structural_parameters
            .iter()
            .chain(
                machine
                    .blocks
                    .values()
                    .flat_map(|block| &block.structural_parameters),
            )
            .filter(|parameter| {
                parameter.multiplicity == StructuralMultiplicity::Unrestricted
                    || (parameter.is_self && parameter.access != StructuralAccess::Owned)
            })
        {
            self.structural_values.remove(&parameter.place);
        }
        // Frame-local byte views and loans end here and carry
        // no affine cleanup, including locally established literals.
        for place in self.byte_sequence_values.keys() {
            reference::discard_structural_value(
                &mut self.structural_values,
                &mut self.reference_referents,
                *place,
            );
        }
        self.byte_sequence_values.clear();
        self.retire_plain_locals();
        let cleanups = commit_cleanup_actions(
            &self.structural_types,
            &self.machines,
            &mut self.structural_values,
            &mut self.reference_referents,
            &mut self.scalar_case_values,
            &mut self.live_affine_frontier,
            &mut self.live_claims,
            cleanup_actions,
        )?;
        if let Some((completed, remaining)) = cleanups.split_first() {
            let completed = completed.clone();
            let machines = std::sync::Arc::clone(&self.machines);
            let callee = machines
                .get(&completed.0.cleanup_machine)
                .expect("verified cleanup target remains installed");
            self.call_stack.push(SuspendedCall {
                values: std::mem::take(&mut self.values),
                structural_values: std::mem::take(&mut self.structural_values),
                byte_sequence_values: std::mem::take(&mut self.byte_sequence_values),
                scalar_case_values: std::mem::take(&mut self.scalar_case_values),
                scalar_array_values: std::mem::take(&mut self.scalar_array_values),
                live_affine_frontier: std::mem::take(&mut self.live_affine_frontier),
                live_claims: std::mem::take(&mut self.live_claims),
                dynamic_parameters: std::mem::take(&mut self.dynamic_parameters),
                current_machine: self.current_machine,
                current: self.current,
                next_operation: self.next_operation,
                result: SuspendedCallResult::NominalCleanups {
                    completed: completed.clone(),
                    remaining: remaining.to_vec(),
                    final_result: Some(result),
                },
            });
            self.values = BTreeMap::new();
            self.structural_values = BTreeMap::new();
            if let Some(receiver) = completed.0.cleanup_receiver {
                self.structural_values.insert(receiver, completed.1.clone());
            }
            self.live_affine_frontier = BTreeSet::new();
            self.live_claims = BTreeMap::new();
            self.dynamic_parameters = BTreeMap::new();
            self.current_machine = completed.0.cleanup_machine;
            self.current = callee.entry;
            self.next_operation = 0;
            return Ok(TerminatorFlow::Continue);
        }
        if let Some(caller) = self.call_stack.pop() {
            let SuspendedCallResult::Scalar(result_value) = caller.result else {
                return Err(TerminalInterpretError::VerifiedOperationMalformed);
            };
            self.values = caller.values;
            self.values.insert(result_value, result);
            self.retire_plain_locals();
            self.structural_values = caller.structural_values;
            self.scalar_case_values = caller.scalar_case_values;
            self.scalar_array_values = caller.scalar_array_values;
            self.byte_sequence_values = caller.byte_sequence_values;
            self.live_affine_frontier = caller.live_affine_frontier;
            self.live_claims = caller.live_claims;
            self.dynamic_parameters = caller.dynamic_parameters;
            self.current_machine = caller.current_machine;
            self.current = caller.current;
            self.next_operation = caller.next_operation;
            return Ok(TerminatorFlow::Continue);
        }
        let result = TerminalExecutionResult::Scalar(result);
        self.retire_plain_locals();
        self.result = Some(result.clone());
        Ok(TerminatorFlow::Yield(TerminalExecutionStatus::Complete(
            result,
        )))
    }

    pub(super) fn settle_return_unit(
        &mut self,
        terminator: &Terminator,
        meter: &mut TerminalFuelMeter,
    ) -> Result<TerminatorFlow, TerminalInterpretError> {
        let Terminator::ReturnUnit {
            trivial_affine_discards,
            ..
        } = terminator
        else {
            unreachable!("dispatched settle_return_unit")
        };
        let machine = self.machines.get(&self.current_machine).ok_or(
            TerminalInterpretError::VerifiedCallTargetMissing(self.current_machine),
        )?;
        if machine.result != TerminalMachineResult::Unit
            || has_live_linear_claims(&self.live_claims)
        {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        if let Err(error) = meter.charge_terminator(terminator) {
            return meter_status(error).map(TerminatorFlow::Yield);
        }
        for place in trivial_affine_discards {
            if reference::discard_structural_value(
                &mut self.structural_values,
                &mut self.reference_referents,
                *place,
            )
            .is_none()
                && self.scalar_case_values.remove(place).is_none()
            {
                return Err(TerminalInterpretError::VerifiedStructuralPlaceMissing(
                    *place,
                ));
            }
            remove_affine_root(&mut self.live_affine_frontier, *place);
        }
        if let Some(caller) = self.call_stack.pop() {
            let result = caller.result;
            self.values = caller.values;
            self.retire_plain_locals();
            self.structural_values = caller.structural_values;
            self.scalar_case_values = caller.scalar_case_values;
            self.scalar_array_values = caller.scalar_array_values;
            self.byte_sequence_values = caller.byte_sequence_values;
            self.live_affine_frontier = caller.live_affine_frontier;
            self.live_claims = caller.live_claims;
            self.dynamic_parameters = caller.dynamic_parameters;
            self.current_machine = caller.current_machine;
            self.current = caller.current;
            self.next_operation = caller.next_operation;
            match result {
                SuspendedCallResult::Unit => {}
                SuspendedCallResult::NominalCleanups {
                    completed,
                    mut remaining,
                    final_result,
                } => {
                    if !self.structural_values.is_empty()
                        || !self.live_affine_frontier.remove(&StructuralAffineDiscard {
                            place: completed.0.place,
                            path: Vec::new(),
                            structural_type: completed.1.structural_type,
                        })
                    {
                        return Err(TerminalInterpretError::AffineFrontierMismatch);
                    }
                    if !remaining.is_empty() {
                        let completed = remaining.remove(0);
                        let machines = std::sync::Arc::clone(&self.machines);
                        let callee = machines.get(&completed.0.cleanup_machine).ok_or(
                            TerminalInterpretError::VerifiedCallTargetMissing(
                                completed.0.cleanup_machine,
                            ),
                        )?;
                        self.call_stack.push(SuspendedCall {
                            values: std::mem::take(&mut self.values),
                            structural_values: std::mem::take(&mut self.structural_values),
                            byte_sequence_values: std::mem::take(&mut self.byte_sequence_values),
                            scalar_case_values: std::mem::take(&mut self.scalar_case_values),
                            scalar_array_values: std::mem::take(&mut self.scalar_array_values),
                            live_affine_frontier: std::mem::take(&mut self.live_affine_frontier),
                            live_claims: std::mem::take(&mut self.live_claims),
                            dynamic_parameters: std::mem::take(&mut self.dynamic_parameters),
                            current_machine: self.current_machine,
                            current: self.current,
                            next_operation: self.next_operation,
                            result: SuspendedCallResult::NominalCleanups {
                                completed: completed.clone(),
                                remaining,
                                final_result,
                            },
                        });
                        self.values = BTreeMap::new();
                        self.structural_values = BTreeMap::new();
                        if let Some(receiver) = completed.0.cleanup_receiver {
                            self.structural_values.insert(receiver, completed.1.clone());
                        }
                        self.live_affine_frontier = BTreeSet::new();
                        self.live_claims = BTreeMap::new();
                        self.dynamic_parameters = BTreeMap::new();
                        self.current_machine = completed.0.cleanup_machine;
                        self.current = callee.entry;
                        self.next_operation = 0;
                        return Ok(TerminatorFlow::Continue);
                    }
                    if !self.live_affine_frontier.is_empty() {
                        return Err(TerminalInterpretError::AffineFrontierMismatch);
                    }
                    if let Some(returned) = final_result
                        && let Some(caller) = self.call_stack.pop()
                    {
                        let SuspendedCallResult::Scalar(result_value) = caller.result else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        self.values = caller.values;
                        self.values.insert(result_value, returned);
                        self.retire_plain_locals();
                        self.structural_values = caller.structural_values;
                        self.scalar_case_values = caller.scalar_case_values;
                        self.scalar_array_values = caller.scalar_array_values;
                        self.byte_sequence_values = caller.byte_sequence_values;
                        self.live_affine_frontier = caller.live_affine_frontier;
                        self.live_claims = caller.live_claims;
                        self.dynamic_parameters = caller.dynamic_parameters;
                        self.current_machine = caller.current_machine;
                        self.current = caller.current;
                        self.next_operation = caller.next_operation;
                        return Ok(TerminatorFlow::Continue);
                    }
                    if final_result.is_none()
                        && let Some(caller) = self.call_stack.pop()
                    {
                        let SuspendedCallResult::Unit = caller.result else {
                            return Err(TerminalInterpretError::VerifiedOperationMalformed);
                        };
                        self.values = caller.values;
                        self.retire_plain_locals();
                        self.structural_values = caller.structural_values;
                        self.scalar_case_values = caller.scalar_case_values;
                        self.scalar_array_values = caller.scalar_array_values;
                        self.byte_sequence_values = caller.byte_sequence_values;
                        self.live_affine_frontier = caller.live_affine_frontier;
                        self.live_claims = caller.live_claims;
                        self.dynamic_parameters = caller.dynamic_parameters;
                        self.current_machine = caller.current_machine;
                        self.current = caller.current;
                        self.next_operation = caller.next_operation;
                        return Ok(TerminatorFlow::Continue);
                    }
                    let result = final_result.map_or(
                        TerminalExecutionResult::Unit,
                        TerminalExecutionResult::Scalar,
                    );
                    self.retire_plain_locals();
                    self.result = Some(result.clone());
                    return Ok(TerminatorFlow::Yield(TerminalExecutionStatus::Complete(
                        result,
                    )));
                }
                SuspendedCallResult::Scalar(_) | SuspendedCallResult::Structural { .. } => {
                    return Err(TerminalInterpretError::VerifiedOperationMalformed);
                }
            }
            return Ok(TerminatorFlow::Continue);
        }
        let result = TerminalExecutionResult::Unit;
        self.retire_plain_locals();
        self.result = Some(result.clone());
        Ok(TerminatorFlow::Yield(TerminalExecutionStatus::Complete(
            result,
        )))
    }

    pub(super) fn settle_return_structural(
        &mut self,
        terminator: &Terminator,
        meter: &mut TerminalFuelMeter,
    ) -> Result<TerminatorFlow, TerminalInterpretError> {
        let Terminator::ReturnStructural {
            source,
            returned_claims,
            trivial_affine_discards,
            ..
        } = terminator
        else {
            unreachable!("dispatched settle_return_structural")
        };
        let machine = self.machines.get(&self.current_machine).ok_or(
            TerminalInterpretError::VerifiedCallTargetMissing(self.current_machine),
        )?;
        let Some(signature) = machine.result.structural() else {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        };
        if terminal_semantics::scalar_array_leaf_shape(
            self.structural_types.values(),
            signature.structural_type,
        )
        .is_some()
        {
            if let Some(status) = self.return_scalar_array(terminator, meter)? {
                return Ok(TerminatorFlow::Yield(status));
            }
            return Ok(TerminatorFlow::Continue);
        }
        if let Some(value) = self.scalar_case_values.get(source).cloned() {
            let internal_result = match self.call_stack.last() {
                Some(SuspendedCall {
                    structural_values,
                    scalar_case_values,
                    live_affine_frontier,
                    result:
                        SuspendedCallResult::Structural {
                            result,
                            returned_claim_transfers,
                            ..
                        },
                    ..
                }) if result.multiplicity == signature.multiplicity
                    && result.qualifications.is_empty()
                    && result.claims.is_empty()
                    && returned_claim_transfers.is_empty()
                    && !structural_values.contains_key(&result.place)
                    && !scalar_case_values.contains_key(&result.place)
                    && live_affine_frontier
                        .iter()
                        .all(|entry| entry.place != result.place) =>
                {
                    Some(result.clone())
                }
                Some(_) => {
                    return Err(TerminalInterpretError::VerifiedOperationMalformed);
                }
                None => None,
            };
            if value.structural_type != signature.structural_type
                || !signature.qualifications.is_empty()
                || !returned_claims.is_empty()
                || !self.live_claims.is_empty()
                || trivial_affine_discards.iter().any(|place| {
                    *place == *source
                        || (!self.structural_values.contains_key(place)
                            && !self.scalar_case_values.contains_key(place))
                })
            {
                return Err(TerminalInterpretError::VerifiedOperationMalformed);
            }
            if let Err(error) = meter.charge_terminator(terminator) {
                return meter_status(error).map(TerminatorFlow::Yield);
            }
            self.scalar_case_values.remove(source);
            remove_affine_root(&mut self.live_affine_frontier, *source);
            for place in trivial_affine_discards {
                reference::discard_structural_value(
                    &mut self.structural_values,
                    &mut self.reference_referents,
                    *place,
                );
                self.scalar_case_values.remove(place);
                remove_affine_root(&mut self.live_affine_frontier, *place);
            }
            if let Some(result) = internal_result {
                let caller = self
                    .call_stack
                    .pop()
                    .expect("an internal scalar-case return has a caller frame");
                let SuspendedCallResult::Structural { .. } = caller.result else {
                    unreachable!("scalar-case return preflight matched its caller")
                };
                self.values = caller.values;
                self.retire_plain_locals();
                self.structural_values = caller.structural_values;
                self.scalar_case_values = caller.scalar_case_values;
                self.scalar_array_values = caller.scalar_array_values;
                self.byte_sequence_values = caller.byte_sequence_values;
                if self
                    .scalar_case_values
                    .insert(result.place, value)
                    .is_some()
                {
                    return Err(TerminalInterpretError::VerifiedOperationMalformed);
                }
                self.live_affine_frontier = caller.live_affine_frontier;
                if result.multiplicity == StructuralMultiplicity::Affine {
                    self.live_affine_frontier.insert(StructuralAffineDiscard {
                        place: result.place,
                        path: Vec::new(),
                        structural_type: result.structural_type,
                    });
                }
                self.live_claims = caller.live_claims;
                self.dynamic_parameters = caller.dynamic_parameters;
                self.current_machine = caller.current_machine;
                self.current = caller.current;
                self.next_operation = caller.next_operation;
                return Ok(TerminatorFlow::Continue);
            }
            let result = TerminalExecutionResult::ScalarCase(TerminalScalarCaseResult { value });
            self.retire_plain_locals();
            self.result = Some(result.clone());
            return Ok(TerminatorFlow::Yield(TerminalExecutionStatus::Complete(
                result,
            )));
        }
        let mut value = self.structural_values.get(source).cloned().ok_or(
            TerminalInterpretError::VerifiedStructuralPlaceMissing(*source),
        )?;
        // A machine's declared result qualifications are introduced at
        // return: the signature's `established by` authority mints its
        // domains onto the handed-back value, and a forwarded value may
        // already carry them.
        for domain in &signature.qualifications {
            if !value.qualifications.contains(domain) {
                value.qualifications.push(*domain);
            }
        }
        value.qualifications.sort();
        self.validate_reference_return(signature, &value)?;
        if value.structural_type != signature.structural_type
            || signature
                .qualifications
                .iter()
                .any(|domain| !value.qualifications.contains(domain))
        {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        let actual_claims = self
            .live_claims
            .iter()
            .filter_map(|(claim, live)| (live.place == Some(*source)).then_some(*claim))
            .collect::<Vec<_>>();
        if actual_claims != *returned_claims
            || self
                .live_claims
                .keys()
                .any(|claim| !returned_claims.contains(claim))
            || trivial_affine_discards
                .iter()
                .any(|place| *place == *source || !self.structural_values.contains_key(place))
        {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        let internal_return = match self.call_stack.last() {
            Some(SuspendedCall {
                structural_values,
                live_affine_frontier,
                live_claims,
                result:
                    SuspendedCallResult::Structural {
                        result,
                        returned_claim_transfers,
                        ..
                    },
                ..
            }) => {
                if result.structural_type != signature.structural_type
                    || result.multiplicity != signature.multiplicity
                    || result.qualifications != signature.qualifications
                    || structural_values.contains_key(&result.place)
                    || live_affine_frontier
                        .iter()
                        .any(|entry| entry.place == result.place)
                    || result
                        .qualifications
                        .iter()
                        .any(|domain| !value.qualifications.contains(domain))
                {
                    return Err(TerminalInterpretError::VerifiedOperationMalformed);
                }
                Some((
                    result.clone(),
                    if !result.claims.is_empty() && returned_claim_transfers.is_empty() {
                        // A boundary call carries no returned-claim transfer
                        // slots, so non-empty `result.claims` without
                        // transfers identifies a boundary frame: the route
                        // mints each binding on the caller by its own
                        // establishment authority — conditioned on the
                        // returned value inhabiting the claimed path — rather
                        // than rebinding a claim the callee returned.
                        self.mint_boundary_result_claims(live_claims, *source, result)?
                    } else {
                        rebind_structural_result_claims(
                            live_claims,
                            &self.live_claims,
                            *source,
                            result,
                            returned_claim_transfers,
                            returned_claims,
                        )?
                    },
                ))
            }
            Some(_) => {
                return Err(TerminalInterpretError::VerifiedOperationMalformed);
            }
            None => None,
        };
        if let Err(error) = meter.charge_terminator(terminator) {
            return meter_status(error).map(TerminatorFlow::Yield);
        }
        // Commit only after fuel and every structural/claim check succeeds.
        self.structural_values.remove(source);
        remove_affine_root(&mut self.live_affine_frontier, *source);
        for claim in returned_claims {
            self.live_claims.remove(claim);
        }
        for place in trivial_affine_discards {
            reference::discard_structural_value(
                &mut self.structural_values,
                &mut self.reference_referents,
                *place,
            );
            self.scalar_case_values.remove(place);
            remove_affine_root(&mut self.live_affine_frontier, *place);
        }
        if let Some((result, rebound_claims)) = internal_return {
            let caller = self
                .call_stack
                .pop()
                .expect("an internal structural return has a caller frame");
            let SuspendedCallResult::Structural { .. } = caller.result else {
                unreachable!("preflight matched the structural caller frame")
            };
            self.values = caller.values;
            self.retire_plain_locals();
            self.structural_values = caller.structural_values;
            self.scalar_case_values = caller.scalar_case_values;
            self.scalar_array_values = caller.scalar_array_values;
            self.byte_sequence_values = caller.byte_sequence_values;
            if self.structural_values.insert(result.place, value).is_some() {
                return Err(TerminalInterpretError::VerifiedOperationMalformed);
            }
            self.live_affine_frontier = caller.live_affine_frontier;
            if result.multiplicity == StructuralMultiplicity::Affine
                && !self.live_affine_frontier.insert(StructuralAffineDiscard {
                    place: result.place,
                    path: Vec::new(),
                    structural_type: result.structural_type,
                })
            {
                return Err(TerminalInterpretError::AffineFrontierMismatch);
            }
            self.live_claims = rebound_claims;
            self.current_machine = caller.current_machine;
            self.current = caller.current;
            self.next_operation = caller.next_operation;
            return Ok(TerminatorFlow::Continue);
        }
        let result = TerminalExecutionResult::Structural(TerminalStructuralResult {
            value,
            claims: returned_claims.clone(),
        });
        self.retire_plain_locals();
        self.result = Some(result.clone());
        Ok(TerminatorFlow::Yield(TerminalExecutionStatus::Complete(
            result,
        )))
    }

    /// Installs the claims a boundary result declaration mints on the
    /// caller. Each binding lands only when the returned value inhabits the
    /// claimed path — a claim under a case payload mints only for a return
    /// observing that case, so e.g. a `Rejected` return never mints the
    /// `Registered` payload's claim.
    fn mint_boundary_result_claims(
        &self,
        caller_claims: &BTreeMap<ClaimId, LiveClaim>,
        source: PlaceId,
        result: &StructuralOperationResult,
    ) -> Result<BTreeMap<ClaimId, LiveClaim>, TerminalInterpretError> {
        let mut claims = caller_claims.clone();
        for binding in &result.claims {
            if !self.boundary_claim_path_inhabited(result.structural_type, source, &binding.path)? {
                continue;
            }
            if claims
                .insert(
                    binding.claim,
                    LiveClaim {
                        place: Some(result.place),
                        path: binding.path.clone(),
                        multiplicity: Some(if binding.path.is_empty() {
                            result.multiplicity
                        } else {
                            StructuralMultiplicity::Linear
                        }),
                    },
                )
                .is_some()
            {
                return Err(TerminalInterpretError::VerifiedOperationMalformed);
            }
        }
        Ok(claims)
    }

    /// Whether the returned value at `source` inhabits a minted claim's
    /// `path`. Non-case segments resolve against the declared shape; a field
    /// addressed through a sum names one case's payload member and is
    /// inhabited only when the returned value's observed case carries a
    /// field of that identity.
    fn boundary_claim_path_inhabited(
        &self,
        root: StructuralTypeId,
        source: PlaceId,
        path: &[StructuralPathSegment],
    ) -> Result<bool, TerminalInterpretError> {
        let mut structural_type = root;
        let mut prefix = Vec::new();
        for segment in path {
            let declaration = self
                .structural_types
                .get(&structural_type)
                .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
            if let (StructuralPathSegment::Field(identity), StructuralTypeShape::Sum { cases }) =
                (segment, &declaration.shape)
            {
                let Ok(inhabited) = self.observe_structural_case(source, &prefix) else {
                    return Ok(false);
                };
                let Some(case) = cases.iter().find(|case| case.id == inhabited) else {
                    return Ok(false);
                };
                let Some(field) = case.fields.iter().find(|field| field.identity == *identity)
                else {
                    return Ok(false);
                };
                let terminal_psi::StructuralFieldType::Structural(next) = field.field_type else {
                    return Err(TerminalInterpretError::VerifiedOperationMalformed);
                };
                structural_type = next;
            } else {
                structural_type = resolve_structural_path_type(
                    &self.structural_types,
                    structural_type,
                    std::slice::from_ref(segment),
                )?;
            }
            prefix.push(segment.clone());
        }
        Ok(true)
    }

    pub(super) fn settle_crash(
        &mut self,
        terminator: &Terminator,
        meter: &mut TerminalFuelMeter,
    ) -> Result<TerminatorFlow, TerminalInterpretError> {
        let Terminator::Crash {
            edge,
            cause,
            site_guard,
            frontier_lower_bound,
            ..
        } = terminator
        else {
            unreachable!("dispatched settle_crash")
        };
        if frontier_lower_bound != &self.live_claims.keys().copied().collect::<Vec<_>>() {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        if let Err(error) = meter.charge_terminator(terminator) {
            return meter_status(error).map(TerminatorFlow::Yield);
        }
        let crash = TerminalCrash {
            site: TerminalCrashSite::Edge(*edge),
            cause: *cause,
            site_guard: site_guard.clone(),
            frontier_lower_bound: frontier_lower_bound.clone(),
        };
        self.crash = Some(crash.clone());
        Ok(TerminatorFlow::Yield(TerminalExecutionStatus::Crashed(
            crash,
        )))
    }
}

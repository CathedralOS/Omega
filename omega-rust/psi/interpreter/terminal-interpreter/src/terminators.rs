//! Block terminators of the interpreter loop: the Unit, structural and
//! scalar returns with their cleanup custody, jumps and conditionals, case
//! dispatch, and crashes.
//!
//! `block_bindings.rs` snapshots the selected successor operands before a
//! jump's cleanup and parameter replacement.

pub(crate) mod block_bindings;
mod completion;
mod structural_return;

use crate::custody::{
    commit_cleanup_action, consume_affine_projection, has_live_linear_claims,
    rebind_structural_result_claims, remove_affine_root, resolve_structural_path_type,
    validate_nominal_cleanup_action,
};
use crate::errors::TerminalInterpretError;
use crate::execution::{
    CompletedReturn, LiveClaim, SuspendedCall, SuspendedCallResult, TerminalExecution,
    TerminatorFlow,
};
use crate::reference;
use crate::results::meter_status;
use crate::results::{
    TerminalCrash, TerminalCrashSite, TerminalExecutionResult, TerminalExecutionStatus,
    TerminalScalarCaseResult, TerminalStructuralResult,
};
use crate::values::TerminalScalarValue;
use semantic_vocabulary::{BlockId, ClaimId, EdgeId, PlaceId, StructuralTypeId, ValueId};
use std::collections::{BTreeMap, BTreeSet};
use terminal_fuel::TerminalFuelMeter;
use terminal_psi::{
    StructuralAccess, StructuralAffineDiscard, StructuralArgument, StructuralMultiplicity,
    StructuralOperationResult, StructuralPathSegment, StructuralTypeShape, TerminalMachineResult,
    Terminator,
};

/// Bindings and cleanup selected by either an unconditional or conditional edge.
struct SelectedSuccessor<'edge> {
    edge: EdgeId,
    target: BlockId,
    arguments: &'edge [ValueId],
    structural_arguments: &'edge [StructuralArgument],
    trivial_affine_discards: &'edge [PlaceId],
    residual_affine_discards: &'edge [StructuralAffineDiscard],
}

impl TerminalExecution {
    pub(crate) fn settle_return_unit_nominal_affine(
        &mut self,
        terminator: &Terminator,
        meter: &mut TerminalFuelMeter,
    ) -> Result<TerminatorFlow, TerminalInterpretError> {
        let Terminator::ReturnUnitNominalAffine { cleanups, .. } = terminator else {
            unreachable!("dispatched settle_return_unit_nominal_affine")
        };
        let machine = self.program.machines.get(&self.current_machine).ok_or(
            TerminalInterpretError::VerifiedCallTargetMissing(self.current_machine),
        )?;
        if machine.result != TerminalMachineResult::Unit || cleanups.is_empty() {
            return Err(TerminalInterpretError::VerifiedOperationMalformed);
        }
        let mut expected_frontier = BTreeSet::new();
        let mut consumed_claims = BTreeSet::new();
        let mut consumed_places = BTreeSet::new();
        let mut cleanup_values = Vec::with_capacity(cleanups.len());
        for cleanup in cleanups {
            let value = self.structural_values.get(&cleanup.place).cloned().ok_or(
                TerminalInterpretError::VerifiedStructuralPlaceMissing(cleanup.place),
            )?;
            validate_nominal_cleanup_action(
                &self.program.structural_types,
                &self.program.machines,
                &self.structural_values,
                &self.live_affine_frontier,
                &self.live_claims,
                cleanup,
            )?;
            if !consumed_places.insert(cleanup.place)
                || cleanup
                    .consumed_claims
                    .iter()
                    .any(|claim| !consumed_claims.insert(*claim))
            {
                return Err(TerminalInterpretError::AffineFrontierMismatch);
            }
            if cleanup.consumed_claims.is_empty() {
                expected_frontier.insert(StructuralAffineDiscard {
                    place: cleanup.place,
                    path: Vec::new(),
                    structural_type: cleanup.structural_type,
                });
            }
            cleanup_values.push((cleanup.clone(), value));
        }
        if self.structural_values.len() != cleanup_values.len() + self.primitive_local_count()
            || self.live_affine_frontier != expected_frontier
            || consumed_claims != self.live_claims.keys().copied().collect::<BTreeSet<_>>()
        {
            return Err(TerminalInterpretError::AffineFrontierMismatch);
        }
        if let Err(error) = meter.charge_terminator(terminator) {
            return meter_status(error).map(TerminatorFlow::Yield);
        }
        self.retire_plain_locals();
        self.continue_cleanup_actions(
            cleanups
                .iter()
                .cloned()
                .map(terminal_psi::TerminalAffineCleanupAction::InvokeNominal)
                .collect::<Vec<_>>()
                .into_iter(),
            CompletedReturn::plain(TerminalExecutionResult::Unit),
        )
    }

    pub(crate) fn settle_return_unit_partial_affine(
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
        let machine = self.program.machines.get(&self.current_machine).ok_or(
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
                &self.program.structural_types,
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
                &mut self.structural_element_view_fields,
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
            self.element_view_values = caller.element_view_values;
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

    pub(crate) fn settle_jump(
        &mut self,
        terminator: &Terminator,
        meter: &mut TerminalFuelMeter,
    ) -> Result<TerminatorFlow, TerminalInterpretError> {
        let Terminator::Jump {
            edge,
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
        self.settle_successor(
            SelectedSuccessor {
                edge: *edge,
                target: *target,
                arguments,
                structural_arguments,
                trivial_affine_discards,
                residual_affine_discards,
            },
            terminator,
            meter,
        )
    }

    fn prepare_residual_disposal(
        &self,
        structural_arguments: &[StructuralArgument],
        residual_affine_discards: &[StructuralAffineDiscard],
    ) -> Result<BTreeSet<PlaceId>, TerminalInterpretError> {
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
            consume_affine_projection(
                &self.program.structural_types,
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
                    &self.program.structural_types,
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
        // Every root this edge disposes must be left with exactly its
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
        Ok(residual_roots)
    }

    fn commit_residual_disposal(
        &mut self,
        structural_arguments: &[StructuralArgument],
        residual_affine_discards: &[StructuralAffineDiscard],
        residual_roots: &BTreeSet<PlaceId>,
    ) -> Result<(), TerminalInterpretError> {
        // The edge committed: each projected argument retires the
        // containing root entry and leaves the residual sibling
        // subtrees live; the residual list then closes exactly
        // that remainder.
        for argument in structural_arguments.iter().filter(|argument| {
            argument.access == StructuralAccess::Owned && !argument.path.is_empty()
        }) {
            consume_affine_projection(
                &self.program.structural_types,
                &self.structural_values,
                &mut self.live_affine_frontier,
                argument,
            )?;
        }
        for discard in residual_affine_discards {
            self.live_affine_frontier.remove(discard);
            let root = self.structural_values.get(&discard.place).ok_or(
                TerminalInterpretError::VerifiedStructuralPlaceMissing(discard.place),
            )?;
            let mut path = root.path.clone();
            path.extend_from_slice(&discard.path);
            // A transferred child retains its captured identity, including
            // transfers on earlier edges. Only the declared residual subtree
            // dies here; deleting all descriptors under the old root would
            // also erase those surviving children.
            self.reference_referents.retain(|carrier, _| {
                carrier.opaque_identity != root.opaque_identity || !carrier.path.starts_with(&path)
            });
            self.structural_element_view_fields.retain(|carrier, _| {
                carrier.opaque_identity != root.opaque_identity || !carrier.path.starts_with(&path)
            });
        }
        for place in residual_roots {
            self.structural_values.remove(place);
        }
        Ok(())
    }

    fn settle_successor(
        &mut self,
        successor: SelectedSuccessor<'_>,
        terminator: &Terminator,
        meter: &mut TerminalFuelMeter,
    ) -> Result<TerminatorFlow, TerminalInterpretError> {
        let SelectedSuccessor {
            edge,
            target,
            arguments,
            structural_arguments,
            trivial_affine_discards,
            residual_affine_discards,
        } = successor;
        let residual_roots =
            self.prepare_residual_disposal(structural_arguments, residual_affine_discards)?;
        if let Err(error) = meter.charge_edge(edge, terminator) {
            return meter_status(error).map(TerminatorFlow::Yield);
        }
        let bindings = self.prepare_block_bindings(target, arguments, structural_arguments)?;
        bindings.validate_discards(self, trivial_affine_discards, residual_affine_discards)?;
        for place in trivial_affine_discards {
            if reference::discard_structural_value(
                &mut self.structural_values,
                &mut self.reference_referents,
                &mut self.structural_element_view_fields,
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
        self.commit_residual_disposal(
            structural_arguments,
            residual_affine_discards,
            &residual_roots,
        )?;
        bindings.commit(self);
        self.current = target;
        self.next_operation = 0;
        Ok(TerminatorFlow::Continue)
    }

    pub(crate) fn settle_conditional(
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
        self.settle_successor(
            SelectedSuccessor {
                edge: successor.edge,
                target: successor.target,
                arguments: &successor.arguments,
                structural_arguments: &successor.structural_arguments,
                trivial_affine_discards: &successor.trivial_affine_discards,
                residual_affine_discards: &successor.residual_affine_discards,
            },
            terminator,
            meter,
        )
    }

    pub(crate) fn settle_structural_case(
        &mut self,
        terminator: &Terminator,
        meter: &mut TerminalFuelMeter,
    ) -> Result<TerminatorFlow, TerminalInterpretError> {
        let Terminator::StructuralCase { source, cases } = terminator else {
            unreachable!("dispatched settle_structural_case")
        };
        // Only internally established values carry a discriminator and
        // payload; opaque host roots remain unsupported. A scalar case holds
        // its tag and payload as one value. A structurally established case
        // (a mixed shape's, or one with a non-scalar member) keeps its tag
        // in `structural_cases` at its root and each scalar payload member
        // in that root's payload cells, apart from any common field.
        let scalar_case = self.scalar_case_values.get(source);
        let active_case = match scalar_case {
            Some(value) => value.result_case,
            None => self.observe_structural_case(*source, &[])?,
        };
        let payload_root = match scalar_case {
            Some(_) => None,
            None => self
                .structural_values
                .get(source)
                .map(crate::values::StructuralRuntimePlace::from),
        };
        let successor = cases
            .iter()
            .find(|successor| successor.case == active_case)
            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
        let target = self
            .program
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
            let scalar = match (scalar_case, &payload_root) {
                (Some(value), _) => value
                    .fields
                    .iter()
                    .find(|(identity, _)| identity == field)
                    .map(|(_, scalar)| *scalar),
                (None, Some(root)) => self
                    .structural_scalar_fields
                    .get(&crate::values::StructuralScalarRuntimeField::case_payload(
                        root.clone(),
                        *field,
                    ))
                    .copied(),
                (None, None) => None,
            }
            .ok_or(TerminalInterpretError::VerifiedOperationMalformed)?;
            if scalar.scalar_type() != parameter.scalar_type {
                return Err(TerminalInterpretError::VerifiedOperationMalformed);
            }
            bindings.push((parameter.id, scalar));
        }
        let residual_roots =
            self.prepare_residual_disposal(&[], &successor.residual_affine_discards)?;
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
                &mut self.structural_element_view_fields,
                *place,
            );
            self.scalar_case_values.remove(place);
            remove_affine_root(&mut self.live_affine_frontier, *place);
        }
        self.commit_residual_disposal(&[], &successor.residual_affine_discards, &residual_roots)?;
        self.values.extend(bindings);
        self.current = successor.target;
        self.next_operation = 0;
        Ok(TerminatorFlow::Continue)
    }

    pub(crate) fn settle_return(
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
        let machine = self.program.machines.get(&self.current_machine).ok_or(
            TerminalInterpretError::VerifiedCallTargetMissing(self.current_machine),
        )?;
        if !matches!(machine.result, TerminalMachineResult::Scalar(_)) {
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
        self.retire_return_inputs();
        self.continue_cleanup_actions(
            cleanup_actions.clone().into_iter(),
            CompletedReturn::plain(TerminalExecutionResult::Scalar(result)),
        )
    }

    pub(crate) fn settle_return_unit(
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
        let machine = self.program.machines.get(&self.current_machine).ok_or(
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
                &mut self.structural_element_view_fields,
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
        self.finish_return(CompletedReturn::plain(TerminalExecutionResult::Unit))
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
                .program
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
                    &self.program.structural_types,
                    structural_type,
                    std::slice::from_ref(segment),
                )?;
            }
            prefix.push(segment.clone());
        }
        Ok(true)
    }

    pub(crate) fn settle_crash(
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

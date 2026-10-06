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
    commit_cleanup_action, consume_affine_projection, rebind_structural_result_claims,
    remove_affine_root, resolve_structural_path_type,
};
use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::{
    CleanupContinuation, CompletedReturn, LiveClaim, SuspendedCall, SuspendedCallResult,
    TerminalExecution, TerminatorFlow,
};
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
    cleanup_actions: &'edge [terminal_psi::TerminalAffineCleanupAction],
}

impl TerminalExecution {
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
            cleanup_actions,
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
                cleanup_actions,
            },
            terminator,
            meter,
        )
    }

    fn prepare_cleanup_partition(
        &self,
        structural_arguments: &[StructuralArgument],
        actions: &[terminal_psi::TerminalAffineCleanupAction],
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
        for (place, path, structural_type) in actions.iter().filter_map(|action| match action {
            terminal_psi::TerminalAffineCleanupAction::DiscardResidual(discard) => Some((
                discard.place,
                discard.path.as_slice(),
                discard.structural_type,
            )),
            terminal_psi::TerminalAffineCleanupAction::InvokeNominal(cleanup)
                if !cleanup.path.is_empty() =>
            {
                Some((
                    cleanup.place,
                    cleanup.path.as_slice(),
                    cleanup.structural_type,
                ))
            }
            _ => None,
        }) {
            let root = self.structural_values.get(&place).ok_or(
                TerminalInterpretError::VerifiedStructuralPlaceMissing(place),
            )?;
            if path.is_empty()
                || resolve_structural_path_type(
                    &self.program.structural_types,
                    root.structural_type,
                    path,
                )? != structural_type
            {
                return Err(TerminalInterpretError::AffineFrontierMismatch);
            }
            consume_affine_projection(
                &self.program.structural_types,
                &self.structural_values,
                projected_frontier.get_or_insert_with(|| self.live_affine_frontier.clone()),
                &StructuralArgument {
                    place,
                    path: path.to_vec(),
                    access: StructuralAccess::Owned,
                },
            )?;
            residual_roots.insert(place);
        }
        let replayed = projected_frontier
            .as_ref()
            .unwrap_or(&self.live_affine_frontier);
        // Every root this edge disposes must be left with exactly its
        // declared residual rows: projected transfers opened the
        // holes, and the listed complement is the only remainder.
        for place in &residual_roots {
            if replayed.iter().any(|entry| entry.place == *place) {
                return Err(TerminalInterpretError::AffineFrontierMismatch);
            }
        }
        Ok(residual_roots)
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
            cleanup_actions,
        } = successor;
        self.prepare_cleanup_partition(structural_arguments, cleanup_actions)?;
        let bindings = self.prepare_block_bindings(target, arguments, structural_arguments)?;
        bindings.validate_cleanup_actions(self, cleanup_actions)?;
        if let Err(error) = meter.charge_edge(edge, terminator) {
            return meter_status(error).map(TerminatorFlow::Yield);
        }
        bindings.reserve_identities(self);
        // Snapshot first: hooks may change storage, and a target may reuse a
        // source name. Projected ownership splits commit exactly once here.
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
        // The suspended frame owns the remaining cursor across a hook call.
        let remaining = cleanup_actions.to_vec();
        self.continue_cleanup_actions(
            remaining.into_iter(),
            CleanupContinuation::Successor { target, bindings },
        )
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
            return Err(malformed());
        };
        let successor = if condition { when_true } else { when_false };
        self.settle_successor(
            SelectedSuccessor {
                edge: successor.edge,
                target: successor.target,
                arguments: &successor.arguments,
                structural_arguments: &successor.structural_arguments,
                cleanup_actions: &successor.cleanup_actions,
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
            .ok_or(malformed())?;
        let target = self
            .program
            .machines
            .get(&self.current_machine)
            .ok_or(TerminalInterpretError::VerifiedCallTargetMissing(
                self.current_machine,
            ))?
            .blocks
            .get(&successor.target)
            .ok_or(malformed())?;
        if target.parameters.len() != successor.payload_fields.len() {
            return Err(malformed());
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
            .ok_or(malformed())?;
            if scalar.scalar_type() != parameter.scalar_type {
                return Err(malformed());
            }
            bindings.push((parameter.id, scalar));
        }
        let bindings = block_bindings::BlockBindings::from_scalars(bindings);
        self.prepare_cleanup_partition(&[], &successor.cleanup_actions)?;
        bindings.validate_cleanup_actions(self, &successor.cleanup_actions)?;
        if let Err(error) = meter.charge_edge(successor.edge, terminator) {
            return meter_status(error).map(TerminatorFlow::Yield);
        }
        self.continue_cleanup_actions(
            successor.cleanup_actions.clone().into_iter(),
            CleanupContinuation::Successor {
                target: successor.target,
                bindings,
            },
        )
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
            return Err(malformed());
        }
        self.prepare_cleanup_partition(&[], cleanup_actions)?;
        block_bindings::BlockBindings::default().validate_cleanup_actions(self, cleanup_actions)?;
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
            CleanupContinuation::Return(CompletedReturn::plain(TerminalExecutionResult::Scalar(
                result,
            ))),
        )
    }

    pub(crate) fn settle_return_unit(
        &mut self,
        terminator: &Terminator,
        meter: &mut TerminalFuelMeter,
    ) -> Result<TerminatorFlow, TerminalInterpretError> {
        let Terminator::ReturnUnit {
            cleanup_actions, ..
        } = terminator
        else {
            unreachable!("dispatched settle_return_unit")
        };
        let machine = self.program.machines.get(&self.current_machine).ok_or(
            TerminalInterpretError::VerifiedCallTargetMissing(self.current_machine),
        )?;
        if machine.result != TerminalMachineResult::Unit {
            return Err(malformed());
        }
        self.prepare_cleanup_partition(&[], cleanup_actions)?;
        block_bindings::BlockBindings::default().validate_cleanup_actions(self, cleanup_actions)?;
        if let Err(error) = meter.charge_terminator(terminator) {
            return meter_status(error).map(TerminatorFlow::Yield);
        }
        self.retire_return_inputs();
        self.continue_cleanup_actions(
            cleanup_actions.clone().into_iter(),
            CleanupContinuation::Return(CompletedReturn::plain(TerminalExecutionResult::Unit)),
        )
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
                return Err(malformed());
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
                .ok_or(malformed())?;
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
                    return Err(malformed());
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
            return Err(malformed());
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

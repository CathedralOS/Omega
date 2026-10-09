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
    commit_cleanup_action, consume_affine_projection, consume_projected_ownership,
    rebind_structural_result_claims, remove_affine_root, resolve_structural_path_type,
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
use std::collections::BTreeMap;
use terminal_fuel::TerminalFuelMeter;
use terminal_psi::{
    StructuralAccess, StructuralAffineDiscard, StructuralArgument, StructuralMultiplicity,
    StructuralOperationResult, StructuralPathSegment, StructuralTypeShape, TerminalMachineResult,
    Terminator, find_structural_case,
};

/// Bindings and cleanup selected by either an unconditional or conditional edge.
struct SelectedSuccessor<'edge> {
    edge: EdgeId,
    target: BlockId,
    arguments: &'edge [ValueId],
    structural_arguments: &'edge [StructuralArgument],
    cleanup_actions: &'edge [terminal_psi::TerminalAffineCleanupAction],
}

impl TerminalExecution<'_> {
    /// Consumes a projected structural argument against this execution's
    /// program catalogs and live frontier — the shape every terminator-side
    /// affine consumption shares.
    pub(crate) fn consume_affine_projection(
        &self,
        frontier: &mut std::collections::BTreeSet<StructuralAffineDiscard>,
        argument: &StructuralArgument,
    ) -> Result<(), TerminalInterpretError> {
        consume_affine_projection(
            &self.program.structural_types(),
            &self.structural_values,
            &self.structural_cases,
            frontier,
            argument,
        )
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
        let bindings = self.prepare_block_bindings(target, arguments, structural_arguments)?;
        bindings.validate_cleanup_actions(self, structural_arguments, cleanup_actions)?;
        if let Err(error) = meter.charge_edge(edge, terminator) {
            return meter_status(error).map(TerminatorFlow::Yield);
        }
        bindings.reserve_identities(self);
        // Snapshot first: hooks may change storage, and a target may reuse a
        // source name. Projected ownership splits commit exactly once here.
        for argument in structural_arguments.iter().filter(|argument| {
            argument.access == StructuralAccess::Owned && !argument.path.is_empty()
        }) {
            consume_projected_ownership(
                &self.program.structural_types(),
                &self.structural_values,
                &self.structural_cases,
                &mut self.live_affine_frontier,
                &mut self.live_claims,
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
        let condition = self.scalar_value(*condition)?;
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
        let Terminator::StructuralCase {
            source,
            path,
            cases,
        } = terminator
        else {
            unreachable!("dispatched settle_structural_case")
        };
        // A case step selects an overlaid payload namespace, so authenticate
        // each enclosing discriminator before resolving storage beneath it.
        for (position, segment) in path.iter().enumerate() {
            if let StructuralPathSegment::Case(case) = segment
                && self.observe_structural_case(*source, &path[..position])? != *case
            {
                return Err(malformed());
            }
        }
        // Every established case owns one placed discriminator and payload.
        // An opaque host identity alone cannot establish either contents.
        let active_case = self.observe_structural_case(*source, path)?;
        let projected =
            self.resolve_live_projection(*source, path, StructuralAccess::SharedBorrow)?;
        let payload_root = crate::values::StructuralRuntimePlace::from(&projected);
        let successor = cases
            .iter()
            .find(|successor| successor.case == active_case)
            .ok_or(malformed())?;
        let target = self
            .program
            .machines()
            .get(&self.current_machine)
            .ok_or(TerminalInterpretError::VerifiedCallTargetMissing(
                self.current_machine,
            ))?
            .block(&successor.target)
            .ok_or(malformed())?;
        if target.parameters.len() != successor.payload_fields.len() {
            return Err(malformed());
        }
        let mut bindings = Vec::with_capacity(target.parameters.len());
        for (parameter, field) in target.parameters.iter().zip(&successor.payload_fields) {
            let scalar = self
                .structural_scalar_fields
                .get(&crate::values::StructuralScalarRuntimeField::case_payload(
                    payload_root.clone(),
                    *field,
                ))
                .copied()
                .ok_or(malformed())?;
            if scalar.scalar_type() != parameter.scalar_type {
                return Err(malformed());
            }
            bindings.push((parameter.id, scalar));
        }
        let bindings = block_bindings::BlockBindings::from_scalars(bindings);
        bindings.validate_cleanup_actions(self, &[], &successor.cleanup_actions)?;
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
        let machine = self.program.machines().get(&self.current_machine).ok_or(
            TerminalInterpretError::VerifiedCallTargetMissing(self.current_machine),
        )?;
        if !matches!(machine.result, TerminalMachineResult::Scalar(_)) {
            return Err(malformed());
        }
        block_bindings::BlockBindings::default().validate_cleanup_actions(
            self,
            &[],
            cleanup_actions,
        )?;
        if let Err(error) = meter.charge_terminator(terminator) {
            return meter_status(error).map(TerminatorFlow::Yield);
        }
        let result = self.scalar_value(*value)?;
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
        let machine = self.program.machines().get(&self.current_machine).ok_or(
            TerminalInterpretError::VerifiedCallTargetMissing(self.current_machine),
        )?;
        if machine.result != TerminalMachineResult::Unit {
            return Err(malformed());
        }
        block_bindings::BlockBindings::default().validate_cleanup_actions(
            self,
            &[],
            cleanup_actions,
        )?;
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
            crate::custody::bind_result_claim(
                &mut claims,
                binding,
                result.place,
                result.multiplicity,
            )?;
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
                .structural_types()
                .get(&structural_type)
                .ok_or(malformed())?;
            if let (StructuralPathSegment::Field(identity), StructuralTypeShape::Sum { cases }) =
                (segment, &declaration.shape)
            {
                let Ok(inhabited) = self.observe_structural_case(source, &prefix) else {
                    return Ok(false);
                };
                let Some(case) = find_structural_case(cases, inhabited) else {
                    return Ok(false);
                };
                let Some(field) = terminal_psi::field_by_identity(&case.fields, identity) else {
                    return Ok(false);
                };
                let Some(next) = field.field_type.as_structural() else {
                    return Err(malformed());
                };
                structural_type = next;
            } else {
                structural_type = resolve_structural_path_type(
                    &self.program.structural_types(),
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

    /// Commit an authorized domain-ending edge without running any cleanup,
    /// returning caller frames, retiring host loans, or manufacturing receipts.
    pub(crate) fn settle_external_completion(
        &mut self,
        terminator: &Terminator,
        meter: &mut TerminalFuelMeter,
    ) -> Result<TerminatorFlow, TerminalInterpretError> {
        let Terminator::ExternalCompletion {
            edge,
            boundary,
            kind,
            arguments,
            structural_arguments,
            frontier_lower_bound,
            ..
        } = terminator
        else {
            return Err(malformed());
        };
        let demand = crate::capabilities::TerminalExternalCompletionDemand {
            boundary: *boundary,
            kind: *kind,
        };
        if !self
            .external_bindings
            .get(boundary)
            .is_some_and(|admission| {
                admission.answers(self.program.identity, self.process_domain.as_ref(), demand)
            })
        {
            return Err(TerminalInterpretError::UnboundExternalCompletions {
                missing: vec![demand],
            });
        }
        let declaration = self.program.boundary_machines().get(boundary).ok_or(
            TerminalInterpretError::VerifiedBoundaryMachineMissing(*boundary),
        )?;
        if declaration.completion != terminal_psi::BoundaryCompletion::External(*kind)
            || arguments.len() != 1
            || !structural_arguments.is_empty()
            || frontier_lower_bound
                .windows(2)
                .any(|claims| claims[0] >= claims[1])
            || frontier_lower_bound
                .iter()
                .any(|claim| !self.live_claims.contains_key(claim))
        {
            return Err(malformed());
        }
        let actual = self.scalar_value(arguments[0])?;
        crate::custody::bind_boundary_arguments(&declaration.scalar_parameters, &[actual])?;
        crate::custody::validate_boundary_requirements(declaration, |_| None)?;
        let TerminalScalarValue::Integer {
            scalar_type,
            value: semantic_vocabulary::IntegerValue::Signed(value),
        } = actual
        else {
            return Err(malformed());
        };
        if scalar_type
            != semantic_vocabulary::IntegerType::new(semantic_vocabulary::IntegerSign::Signed, 32)
                .map_err(|_| malformed())?
        {
            return Err(malformed());
        }
        let return_code = i32::try_from(value).map_err(|_| malformed())?;
        match kind {
            semantic_vocabulary::ExternalCompletionKind::ProcessExit => {}
        }
        if let Err(error) = meter.charge_terminator(terminator) {
            return meter_status(error).map(TerminatorFlow::Yield);
        }
        let completion = crate::results::TerminalExternalCompletion {
            machine: self.current_machine,
            block: self.current,
            edge: *edge,
            boundary: *boundary,
            kind: *kind,
            return_code,
            frontier_lower_bound: frontier_lower_bound.clone(),
        };
        self.external_completion = Some(completion.clone());
        Ok(TerminatorFlow::Yield(
            TerminalExecutionStatus::ExternalCompletion(completion),
        ))
    }
}

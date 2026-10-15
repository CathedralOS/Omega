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
    rebind_structural_result_claims,
};
use crate::errors::{TerminalInterpretError, malformed};
use crate::execution::{
    CleanupContinuation, CompletedReturn, SuspendedCall, SuspendedCallResult, TerminalExecution,
    TerminatorFlow,
};
use crate::results::meter_status;
use crate::results::{
    TerminalCrash, TerminalCrashSite, TerminalExecutionResult, TerminalExecutionStatus,
    TerminalScalarCaseResult, TerminalStructuralResult,
};
use crate::values::TerminalScalarValue;
use semantic_vocabulary::{BlockId, EdgeId, PlaceId, ValueId};
use std::collections::BTreeMap;
use terminal_fuel::TerminalFuelMeter;
use terminal_psi::{
    StructuralAccess, StructuralAffineDiscard, StructuralArgument, StructuralMultiplicity,
    StructuralPathSegment, TerminalMachineResult, Terminator,
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
    /// Claim paths under one live place — every terminator-side frontier
    /// walk reads the same projection.
    fn live_claim_paths_at(
        &self,
        place: PlaceId,
    ) -> impl Iterator<Item = &[StructuralPathSegment]> + '_ {
        self.live_claims
            .values()
            .filter(move |claim| claim.place == Some(place))
            .map(|claim| claim.path.as_slice())
    }

    /// The stored case at one discard's projection — the membership lookup
    /// every terminator-side frontier and exclusion call passes in.
    fn stored_case_at_discard(
        &self,
        structural_values: &BTreeMap<PlaceId, crate::values::TerminalStructuralValue>,
        discard: &StructuralAffineDiscard,
    ) -> Option<semantic_vocabulary::StructuralCaseId> {
        crate::structural_inputs::case_membership::stored_case_at_projection(
            &self.program.structural_types(),
            structural_values.get(&discard.place)?,
            &self.structural_cases,
            &discard.path,
        )
    }

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
        // Only the selected edge evaluates its retained coordinates. Freeze
        // every path before snapshotting operands, cleanup or home replacement,
        // exactly as operation projections resolve before operation dispatch.
        let mut selected_arguments = structural_arguments.to_vec();
        for (declared, selected) in structural_arguments.iter().zip(&mut selected_arguments) {
            if declared
                .path
                .iter()
                .any(|segment| segment.runtime_index().is_some())
            {
                self.select_runtime_projection(
                    declared.place,
                    self.live_structural_type(declared.place)?,
                    &declared.path,
                    &mut selected.path,
                )?;
            }
        }
        let structural_arguments = selected_arguments.as_slice();
        let bindings = self
            .prepare_block_bindings(target, arguments, structural_arguments)
            .map_err(|error| {
                error.in_argument_preparation("state arrival storage and memberships")
            })?;
        let cleanup_actions =
            bindings.prepare_cleanup_actions(self, structural_arguments, cleanup_actions)?;
        if let Err(error) = meter.charge_edge(edge, terminator) {
            return meter_status(error).map(TerminatorFlow::Yield);
        }
        bindings.reserve_identities(self);
        // Snapshot first: hooks may change storage, and a target may reuse a
        // source name. Projected ownership splits commit exactly once here.
        for argument in structural_arguments.iter().filter(|argument| {
            argument.access == StructuralAccess::Owned
                && !argument.path.is_empty()
                && bindings.projected_sources.contains(&argument.place)
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
        let remaining = cleanup_actions;
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
            .verified_machine(self.current_machine)?
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
        let cleanup_actions =
            bindings.prepare_cleanup_actions(self, &[], &successor.cleanup_actions)?;
        if let Err(error) = meter.charge_edge(successor.edge, terminator) {
            return meter_status(error).map(TerminatorFlow::Yield);
        }
        self.continue_cleanup_actions(
            cleanup_actions.into_iter(),
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
        let machine = self.program.verified_machine(self.current_machine)?;
        if !matches!(machine.result, TerminalMachineResult::Scalar(_)) {
            return Err(malformed());
        }
        let cleanup_actions = block_bindings::BlockBindings::default().prepare_cleanup_actions(
            self,
            &[],
            cleanup_actions,
        )?;
        if let Err(error) = meter.charge_terminator(terminator) {
            return meter_status(error).map(TerminatorFlow::Yield);
        }
        let result = self.scalar_value(*value)?;
        self.continue_cleanup_actions(
            cleanup_actions.into_iter(),
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
        let machine = self.program.verified_machine(self.current_machine)?;
        if machine.result != TerminalMachineResult::Unit {
            return Err(malformed());
        }
        let cleanup_actions = block_bindings::BlockBindings::default().prepare_cleanup_actions(
            self,
            &[],
            cleanup_actions,
        )?;
        if let Err(error) = meter.charge_terminator(terminator) {
            return meter_status(error).map(TerminatorFlow::Yield);
        }
        self.continue_cleanup_actions(
            cleanup_actions.into_iter(),
            CleanupContinuation::Return(CompletedReturn::plain(TerminalExecutionResult::Unit)),
        )
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
        let declaration = self.program.verified_boundary_machine(boundary)?;
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

//! Lower the selected normal edge's owned dispositions in the live value namespace.
//!
//! Source ownership has already separated target and continuation exits. This
//! join keeps that coordinate through value materialization; a state's union of
//! dead roots cannot choose actions for one return. Hook requirements become
//! ordinary edge obligations, proved from the caller contract and selected path.
use super::super::{CheckedTrees, LoweringError, catalogs};
use crate::emission::operation_emission::buffer::OperationBuffer;
use crate::unit::attached_unit::argument_evaluation::{Evaluation, ScalarReturnCleanup};
use crate::unit::attached_unit::completion_cleanup::{
    self, CompletionCleanupCatalog, NormalExitSource,
};
use crate::unit::attached_unit::composed_control::emission::with_call_namespace;
use crate::unit::unit_cleanup::CleanupRegion;
use checked_trees::{
    CheckedComposedUnitControlStatePlan, CheckedScalarBranchDestination, FlowClaimJoinExit,
};
use terminal_psi::StructuralParameterDeclaration;

mod reference_reads;
pub(super) use reference_reads::{observed_reference_retirements, retire_observed_references};

fn source<'a>(
    state: &'a CheckedComposedUnitControlStatePlan,
    control: &'a checked_trees::CheckedCleanupControlPlans,
) -> NormalExitSource<'a> {
    NormalExitSource {
        cleanups: control.normal_exits(state.normal_exit_cleanups),
        trivial_affine_discards: &[],
        operations: &state.operations,
        parameters: &state.structural_parameters,
    }
}

pub(super) fn destination(
    control: &checked_trees::CheckedCleanupControlPlans,
    state: &CheckedComposedUnitControlStatePlan,
    destination: &CheckedScalarBranchDestination,
) -> Result<FlowClaimJoinExit, LoweringError> {
    completion_cleanup::destination(source(state, control), destination)
}
pub(super) fn coordinate(
    control: &checked_trees::CheckedCleanupControlPlans,
    state: &CheckedComposedUnitControlStatePlan,
    statement_index: usize,
    continuation: bool,
) -> Result<FlowClaimJoinExit, LoweringError> {
    completion_cleanup::coordinate(source(state, control), statement_index, continuation)
}
pub(super) use completion_cleanup::selection;

/// A fused pending invocation has an internal arrival but no source normal
/// completion. Retain its exact source call association; absence of a normal
/// cleanup row alone cannot authorize this selection or retire any owner.
#[allow(clippy::too_many_arguments)]
pub(super) fn for_successor_regions(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    claims: crate::emission::claim_bindings::PublishedClaims<'_>,
    checked: &CheckedTrees,
    owner: symbols::SymbolHandle,
    state: &CheckedComposedUnitControlStatePlan,
    edge: &checked_trees::CheckedStructuralControlSuccessorPlan,
    catalogs: &mut catalogs::ComposedCatalogs<'_>,
    parameters: &[StructuralParameterDeclaration],
    evaluation: &Evaluation,
    operations: &OperationBuffer,
) -> Result<Vec<CleanupRegion>, LoweringError> {
    if edge.pending_source_invocation.is_valid() {
        let flow = checked.facts.flow.control.state(owner, state.state).ok_or(
            LoweringError::Unsupported("pending fused invocation has no source state"),
        )?;
        if !flow
            .calls
            .handles()
            .any(|handle| handle == edge.pending_source_invocation)
        {
            return Err(LoweringError::Unsupported(
                "pending fused invocation is outside its source call roster",
            ));
        }
        let call = checked
            .facts
            .flow
            .control
            .calls
            .get(edge.pending_source_invocation);
        if edge.is_continuation
            || call.authored_site != checked_trees::FlowCallSite::NamedTransition
            || !call.authored_source_custody_valid
            || call.statement_index != edge.statement_ordinal as usize
        {
            return Err(LoweringError::Unsupported(
                "pending fused invocation disagrees with its selected source occurrence",
            ));
        }
        let source_owner =
            checked
                .typed
                .machine_by_symbol(owner)
                .ok_or(LoweringError::Unsupported(
                    "pending fused invocation has no source machine",
                ))?;
        if !validation::terminal_callable_target(&checked.typed, source_owner, call.target_symbol)
            .is_some_and(|(entry, _)| entry == edge.target_state)
        {
            return Err(LoweringError::Unsupported(
                "pending fused invocation has a different executable arrival",
            ));
        }
        // No cleanup, release, or return is manufactured here. Whole no-code
        // carrier custody remains with the emitted affine frontier solver;
        // all other live owners and loans retain their existing obligations.
        return Ok(Vec::new());
    }
    let exit = coordinate(
        catalogs.cleanup_control,
        state,
        edge.statement_ordinal as usize,
        edge.is_continuation,
    )?;
    let mut regions = for_exit_regions(
        identities, claims, checked, state, exit, catalogs, parameters, evaluation, operations,
    )?;
    selection(
        state.state,
        &state.operations,
        evaluation,
        operations,
        &mut regions,
    )?;
    Ok(regions)
}

pub(super) fn scalar_returns(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    claims: crate::emission::claim_bindings::PublishedClaims<'_>,
    checked: &CheckedTrees,
    state: &CheckedComposedUnitControlStatePlan,
    control: &checked_trees::CheckedUnitScalarControlPlan,
    catalogs: &mut catalogs::ComposedCatalogs<'_>,
    parameters: &[StructuralParameterDeclaration],
    evaluation: &Evaluation,
    operations: &OperationBuffer,
) -> Result<Vec<ScalarReturnCleanup>, LoweringError> {
    with_call_namespace(&mut catalogs.scalar_calls, |calls| {
        let mut catalog = CompletionCleanupCatalog {
            control: catalogs.cleanup_control,
            types: &catalogs.structural_types,
            type_ids: &catalogs.type_ids,
            calls,
        };
        completion_cleanup::scalar_returns(
            identities,
            claims.bindings,
            checked,
            source(state, catalogs.cleanup_control),
            control,
            &mut catalog,
            parameters,
            evaluation,
            operations,
        )
    })
}
pub(super) fn for_exit_regions(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    claims: crate::emission::claim_bindings::PublishedClaims<'_>,
    checked: &CheckedTrees,
    state: &CheckedComposedUnitControlStatePlan,
    exit: FlowClaimJoinExit,
    catalogs: &mut catalogs::ComposedCatalogs<'_>,
    parameters: &[StructuralParameterDeclaration],
    evaluation: &Evaluation,
    operations: &OperationBuffer,
) -> Result<Vec<CleanupRegion>, LoweringError> {
    with_call_namespace(&mut catalogs.scalar_calls, |calls| {
        let mut catalog = CompletionCleanupCatalog {
            control: catalogs.cleanup_control,
            types: &catalogs.structural_types,
            type_ids: &catalogs.type_ids,
            calls,
        };
        completion_cleanup::for_exit_regions(
            identities,
            claims.bindings,
            checked,
            source(state, catalogs.cleanup_control),
            exit,
            &mut catalog,
            parameters,
            evaluation,
            operations,
        )
    })
}

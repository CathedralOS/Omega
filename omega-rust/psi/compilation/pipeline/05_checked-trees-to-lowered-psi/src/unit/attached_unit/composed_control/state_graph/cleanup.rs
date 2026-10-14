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
use crate::unit::unit_cleanup::CleanupRegion;
use checked_trees::{
    CheckedComposedUnitControlStatePlan, CheckedScalarBranchDestination, FlowClaimJoinExit,
};
use terminal_psi::StructuralParameterDeclaration;

mod case_capture;
pub(super) use case_capture::retire_captured_subjects;

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
    let mut calls = catalogs.scalar_calls.emission_context();
    let mut catalog = CompletionCleanupCatalog {
        control: catalogs.cleanup_control,
        types: &catalogs.structural_types,
        type_ids: &catalogs.type_ids,
        calls: &mut calls,
    };
    let result = completion_cleanup::scalar_returns(
        identities,
        claims.bindings,
        checked,
        source(state, catalogs.cleanup_control),
        control,
        &mut catalog,
        parameters,
        evaluation,
        operations,
    );
    catalogs.scalar_calls.next_call_obligation = calls.next_obligation_identity;
    result
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
    let mut calls = catalogs.scalar_calls.emission_context();
    let mut catalog = CompletionCleanupCatalog {
        control: catalogs.cleanup_control,
        types: &catalogs.structural_types,
        type_ids: &catalogs.type_ids,
        calls: &mut calls,
    };
    let result = completion_cleanup::for_exit_regions(
        identities,
        claims.bindings,
        checked,
        source(state, catalogs.cleanup_control),
        exit,
        &mut catalog,
        parameters,
        evaluation,
        operations,
    );
    catalogs.scalar_calls.next_call_obligation = calls.next_obligation_identity;
    result
}

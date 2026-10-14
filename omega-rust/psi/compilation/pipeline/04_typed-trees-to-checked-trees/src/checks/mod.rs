//! The checks over checked facts. Each check reads the typed program and the
//! fact tables that `facts` built, and reports a diagnostic where a call,
//! borrow, ownership transfer, crash route, operator use, indexed access or
//! ranked cycle fails its obligation.
//!
//! `checking::lower_typed_trees` prepares the lowering-consumed borrow
//! resources, then enters the checks once over the constructed fact tables.
//!
//! `check_checked_facts_recording` runs every check and
//! collects all diagnostics instead of stopping at the first failure. It
//! first builds the call frame resolver and the incoming guard index
//! (`ranges::incoming_guards`) that several checks share, then runs:
//!
//! 1. `contracts::bind_call_evidence_arguments` binds the evidence arguments
//!    of contract expressions and calls;
//! 2. `borrows` checks call and statement borrows, then `case_access` checks
//!    every evaluated payload access against its live selected tag;
//! 3. `contracts::check_flow_call_contracts` checks call requirements, exit
//!    guarantees, arrival requirements and domain field writes;
//! 4. `multiplicity` checks linear obligations;
//! 5. `crashes` checks crash exit edge isolation and operator invocation
//!    custody, infers path-conditioned guard coverage, and checks published
//!    ceiling coverage;
//! 6. `content` infers identity-preserving reshuffles and composes partition
//!    wrappers, then checks retained content custody;
//! 7. `carry` rejects a call that may suspend while a suspension-forbidden
//!    value is live;
//! 8. `operators` checks operator resolution;
//! 9. `ranges` checks indexed accesses;
//! 10. `termination` checks each terminating machine's ranking.
//!
//! `contracts` and `termination` are also used outside this module (by proof,
//! fact construction, package review and execution planning, among others),
//! and `multiplicity` and `operators` export queries used elsewhere in the
//! crate.

// Call obligations: contract requirements and guarantees, and borrows.
mod borrows;
pub(crate) use borrows::{StorageSelectorFacts, storage_path_compatibility};
pub(crate) mod atomic_fences;
mod case_access;
pub(crate) mod collection_extents;
pub(crate) mod contracts;
pub(crate) mod current_scalar_values;
pub(crate) mod float_implications;
mod runtime_indices;
mod stored_integer_calls;

// Ownership and custody: linear obligations, retained content, and values
// live across a suspending call.
mod carry;
mod content;
mod multiplicity;
mod ownership_occurrences;

// Crash routes.
mod cleanup_admission;
mod crashes;
pub(crate) use cleanup_admission::validate_selected_cleanup_guarantees;

// Operator resolution, indexed access bounds, and machine termination.
mod indexed_writes;
mod operators;
mod ranges;
pub(crate) mod termination;

use diagnostics::Diagnostic;
use language_core::OperatorSpelling;
pub(crate) use operators::{named_operator_route_is_false, operator_route_is_false};
use symbols::SymbolHandle;
use typed_trees::expression::BinaryOperator;
use typed_trees::types::PrimitiveType;

pub(crate) use multiplicity::prepare_permission_events;
pub(crate) use multiplicity::{
    nominal_drop_machine_symbol, owned_temporary_type, type_carries_linear_obligation,
    type_multiplicity, type_reference_is_reference,
};
pub(crate) use ranges::incoming_guards::IncomingGuardIndexCache;

pub(crate) fn initialize_checked_direct_borrow_resources(
    program: &dyn ::validation::CheckingInput,
    facts: &mut checked_trees::CheckFacts,
    mutation_summaries: &crate::flow::StateMutationSummaryCache,
) -> Result<(), Vec<Diagnostic>> {
    borrows::initialize_checked_direct_borrow_resources(program, facts, mutation_summaries)
}

/// Check-stage entry for callers that already retain the pass's shared
/// mutation summary table. The summaries depend only on the immutable typed
/// program and the borrow facts, so the same table answers fact construction,
/// resource construction, borrow statements, and range indexing in one check pass.
pub(crate) fn check_checked_facts_recording(
    program: &dyn ::validation::CheckingInput,
    facts: &mut checked_trees::CheckFacts,
    mutation_summaries: &crate::flow::StateMutationSummaryCache,
    guard_index: &IncomingGuardIndexCache,
    proven_machine_contracts: &[(SymbolHandle, Vec<typed_trees::expression::ExpressionHandle>)],
    expression_intervals: &[validation::ExpressionInterval],
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let call_frames = validation::CallFrameResolver::new(program);
    let incoming_guards = guard_index.index(program, call_frames.as_ref());

    if let Err(mut evidence_diagnostics) = contracts::bind_call_evidence_arguments(program, facts) {
        diagnostics.append(&mut evidence_diagnostics);
    }

    if let Err(mut borrow_diagnostics) = borrows::check_flow_call_borrows(
        program,
        facts,
        mutation_summaries,
        call_frames.as_ref(),
        incoming_guards,
    ) {
        diagnostics.append(&mut borrow_diagnostics);
    }

    runtime_indices::check(program, facts, call_frames.as_ref(), &mut diagnostics);
    case_access::check(program, facts, &mut diagnostics);
    atomic_fences::check(program, facts, &mut diagnostics);

    match contracts::check_flow_call_contracts(
        program,
        facts,
        incoming_guards,
        call_frames.as_ref(),
        proven_machine_contracts,
        expression_intervals,
    ) {
        Ok(decided) => {
            facts.constant_predicate_obligations = decided.constant_obligations;
            facts.decided_call_requires = decided.call_requires;
            facts.qualifications.value_formations = decided.value_qualifications;
        }
        Err(mut contract_diagnostics) => diagnostics.append(&mut contract_diagnostics),
    }

    if let Err(mut store_diagnostics) =
        stored_integer_calls::check(program, facts, call_frames.as_ref())
    {
        diagnostics.append(&mut store_diagnostics);
    }

    if let Err(mut multiplicity_diagnostics) =
        multiplicity::check_linear_obligations(program, facts, incoming_guards)
    {
        diagnostics.append(&mut multiplicity_diagnostics);
    }

    if let Err(mut crash_edge_diagnostics) = crashes::check_crash_exit_edge_isolation(program) {
        diagnostics.append(&mut crash_edge_diagnostics);
    }

    if let Err(mut operator_crash_diagnostics) =
        crashes::check_operator_invocation_custody(program, facts)
    {
        diagnostics.append(&mut operator_crash_diagnostics);
    }
    diagnostics.extend(crashes::infer_path_conditioned_guard_coverage(
        program,
        facts,
        incoming_guards,
    ));
    if let Err(mut crash_diagnostics) = crashes::check_published_ceiling_coverage(program, facts) {
        diagnostics.append(&mut crash_diagnostics);
    }

    content::infer_identity_preserving_reshuffles(program, facts);
    content::compose_partition_wrappers(program, facts);

    if let Err(mut content_diagnostics) = content::check_retained_content_custody(program, facts) {
        diagnostics.append(&mut content_diagnostics);
    }

    if let Err(mut carry_diagnostics) = carry::check_suspension_carry(program, facts) {
        diagnostics.append(&mut carry_diagnostics);
    }

    if let Err(mut operator_diagnostics) = operators::check_operator_resolution(program, facts) {
        diagnostics.append(&mut operator_diagnostics);
    }

    match ranges::check_indexed_accesses(
        program,
        &facts.operators,
        &facts.borrow,
        &facts.flow,
        expression_intervals,
        call_frames.as_ref(),
        incoming_guards,
        mutation_summaries,
    ) {
        Ok((proven, obligations)) => {
            facts.proven_scalar_indexes = proven;
            facts.constant_recipe_obligations.extend(obligations);
        }
        Err(mut range_diagnostics) => diagnostics.append(&mut range_diagnostics),
    }
    indexed_writes::record(program, facts);
    indexed_writes::record_assignment_indexes(program, facts);

    if let Err(mut termination_diagnostics) =
        termination::check_machine_termination(&facts.termination.rank_arrivals)
    {
        diagnostics.append(&mut termination_diagnostics);
    }

    if diagnostics.is_empty() {
        facts
            .qualifications
            .link_value_formations(&mut facts.values.structural_values);
        Ok(())
    } else {
        Err(diagnostics)
    }
}

/// Whether `primitive` is a fixed-width unsigned integer type.
pub(crate) fn unsigned(primitive: PrimitiveType) -> bool {
    matches!(
        primitive,
        typed_trees::unsigned_integer_primitive_pattern!()
    )
}

/// The spelling of the arithmetic binary operators shared by meaning and
/// term-reference normalization; `None` for any other operator.
pub(crate) fn arithmetic_spelling(operator: BinaryOperator) -> Option<OperatorSpelling> {
    match operator {
        BinaryOperator::Add => Some(OperatorSpelling::Add),
        BinaryOperator::Subtract => Some(OperatorSpelling::Subtract),
        BinaryOperator::Multiply => Some(OperatorSpelling::Multiply),
        _ => None,
    }
}

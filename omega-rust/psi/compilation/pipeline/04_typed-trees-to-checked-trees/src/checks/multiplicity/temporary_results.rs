//! Ownership transfers from temporary call results.
//!
//! A partial move from a temporary has no remaining local owner in which to
//! retain unselected linear claims. The selected path must carry every live
//! obligation; ordinary affine siblings may be discarded.
use crate::checks::multiplicity::claim_outcomes::claim_paths_are_case_alternatives;
use crate::checks::multiplicity::linear_claim_frontier;
use crate::checks::multiplicity::linear_validation::event_statement_index;
use crate::checks::multiplicity::linear_validation::permission_source;
use crate::checks::type_carries_linear_obligation;
use crate::checks::type_multiplicity;
use crate::flow::FlowOwnershipEventSource;
use arena::HandleSpan;
use checked_trees::CheckFacts;
use checked_trees::FlowPermissionEventFact;
use diagnostics::Diagnostic;
use language_semantics::Multiplicity;
use language_semantics::PermissionAccess;
use language_semantics::PermissionClaimIdentity;
use language_semantics::PermissionEventKind;
use language_semantics::PermissionProvenance;
use symbols::SymbolHandle;
use typed_trees::types::TypeReferenceNode;

mod borrowed_owners;
mod owned_transfers;
mod owners;
mod projected;

pub(super) use owned_transfers::append_owned_transfer;
pub(crate) use owners::owned_temporary_type;
pub(super) use owners::{TemporaryOwner, projectable_owner, temporary_owners};
pub(super) use projected::{ProjectedConsumer, retire_owner};

/// One call completion disposes temporary owners in reverse establishment
/// order. Usage selects the exact whole or residual plan, not a separate
/// cleanup order for borrowed versus transferred arguments.
#[allow(clippy::too_many_arguments)]
pub(super) fn append_continuation_cleanup(
    program: &dyn validation::CheckingInput,
    facts: &mut CheckFacts,
    machine: SymbolHandle,
    state: SymbolHandle,
    consumer: language_semantics::PermissionEventSource,
    evaluation: checked_trees::FlowPermissionEvaluation,
    owners: &[TemporaryOwner],
    places: &mut [super::linear_obligations::LinearPlace],
    remaining: &super::cleanup_dispositions::RemainingCustody,
    permissions: &mut Vec<FlowPermissionEventFact>,
    dispositions: &mut checked_trees::ContextualCleanupFacts,
) -> Result<Vec<facts::PlaceRoot>, Diagnostic> {
    let mut retired = Vec::new();
    for owner in owners
        .iter()
        .rev()
        .filter(|owner| owner.consumer == consumer && owner.evaluation == evaluation)
    {
        let mut diagnostics = Vec::new();
        match owner.consumption {
            owners::TemporaryConsumption::Borrow { .. } => {
                retired.extend(borrowed_owners::append_borrow(
                    program,
                    facts,
                    machine,
                    state,
                    consumer,
                    evaluation,
                    std::slice::from_ref(owner),
                    places,
                    permissions,
                    &mut diagnostics,
                    dispositions,
                ))
            }
            owners::TemporaryConsumption::OwnedProjection { .. } => {
                retired.extend(projected::append_cleanup(
                    program,
                    facts,
                    machine,
                    state,
                    consumer,
                    evaluation,
                    std::slice::from_ref(owner),
                    places,
                    remaining,
                    permissions,
                )?)
            }
            owners::TemporaryConsumption::OwnedTransfer => {}
        }
        if let Some(diagnostic) = diagnostics.into_iter().next() {
            return Err(diagnostic);
        }
    }
    Ok(retired)
}

pub(super) fn append_whole_affine_transfer(
    program: &dyn ::validation::CheckingInput,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    calls: &[checked_trees::FlowCallFact],
    event: &crate::flow::DiscoveredMoveEvent,
    permission_events: &mut Vec<FlowPermissionEventFact>,
) {
    if !event.segments.is_empty() {
        // Projected owners use the same established source custody as named
        // owners; their consumer continuation publishes residual cleanup.
        return;
    }
    let Some(expression) = event.root.as_expression() else {
        return;
    };
    let FlowOwnershipEventSource::Call {
        statement_index, ..
    } = event.source
    else {
        return;
    };
    // Only the whole result of an actually discovered call
    // participates here. Projected results and reference/qualified or
    // claim-carrying roots retain their existing permission rules.
    let affine_result = event.segments.is_empty()
        && calls.iter().any(|call| {
            call.statement_index == statement_index
                && call.authored_expression == expression
                && validation::call_target_return_type(program, call.target_symbol).is_some_and(
                    |result| {
                        matches!(
                            program.type_reference_table.type_reference(result),
                            TypeReferenceNode::Named { .. }
                                | TypeReferenceNode::Generic { .. }
                                | TypeReferenceNode::FixedArray { .. }
                        ) && type_multiplicity(program, result) == Multiplicity::Affine
                            && !type_carries_linear_obligation(program, result)
                    },
                )
        });
    if affine_result {
        permission_events.push(FlowPermissionEventFact {
            expression: event.expression,
            evaluation: checked_trees::FlowPermissionEvaluation::Statement,
            machine_symbol,
            state_symbol,
            source: permission_source(event.source),
            kind: PermissionEventKind::Transfer,
            multiplicity: Multiplicity::Affine,
            access: PermissionAccess::Owned,
            claim_identity: PermissionClaimIdentity::Unknown,
            provenance: PermissionProvenance::Unknown,
            root: event.root,
            segments: HandleSpan::empty(),
            obligation_live: false,
        });
    }
}

/// An affine record or case construction passed by value is a fresh owned
/// temporary the call consumes whole, like an anonymous affine call result.
/// Move discovery records no place for it (it reads no existing storage), so
/// its transfer into the owned formal is produced here from the call's own
/// arguments, unless an earlier producer already recorded the same transfer.
/// An unrestricted construction is copied, not moved, and needs no transfer;
/// a claim-bearing construction is a temporary owner whose claim frontier
/// transfers instead (`owned_transfers`).
pub(super) fn append_constructed_argument_transfers(
    program: &dyn ::validation::CheckingInput,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    calls: &[checked_trees::FlowCallFact],
    permission_events: &mut Vec<FlowPermissionEventFact>,
) {
    for call in calls
        .iter()
        .filter(|call| call.statement_index == statement_index)
    {
        let Some(site) = validation::find_call_site(
            program,
            machine_symbol,
            state_symbol,
            statement_index,
            call.call_ordinal,
        ) else {
            continue;
        };
        let Some(parameters) = validation::call_target_parameters(program, call.target_symbol)
        else {
            continue;
        };
        let arguments = validation::call_site_argument_expressions(program, &site);
        let explicit_parameters = parameters.iter();
        for (parameter, argument) in explicit_parameters.zip(arguments.iter()) {
            let typed_trees::expression::ExpressionNode::StructLiteral(literal) =
                program.expression_table.expression(*argument)
            else {
                continue;
            };
            let Some(data) = program.data_definition_by_symbol(literal.type_symbol) else {
                continue;
            };
            let multiplicity = data.properties.multiplicity;
            let source = language_semantics::PermissionEventSource::Call {
                statement_index,
                call_ordinal: call.call_ordinal,
                target_symbol: call.target_symbol,
            };
            let root = facts::PlaceRoot::Expression(*argument);
            if (permission_events
                .iter()
                .any(|event| event.source == source && event.root == root))
                || parameter.is_const
                || parameter.relevance.is_erased()
                || multiplicity != Multiplicity::Affine
                || type_multiplicity(program, parameter.type_reference) != multiplicity
                || owners::owned_temporary_type(
                    program,
                    state_symbol,
                    statement_index,
                    parameter,
                    *argument,
                )
                .is_some()
                || matches!(
                    program
                        .type_reference_table
                        .type_reference(parameter.type_reference),
                    TypeReferenceNode::Reference { .. }
                )
            {
                continue;
            }
            permission_events.push(FlowPermissionEventFact {
                expression: *argument,
                evaluation: checked_trees::FlowPermissionEvaluation::Statement,
                machine_symbol,
                state_symbol,
                source,
                kind: PermissionEventKind::Transfer,
                multiplicity,
                access: PermissionAccess::Owned,
                claim_identity: PermissionClaimIdentity::Unknown,
                provenance: PermissionProvenance::Unknown,
                root,
                segments: HandleSpan::empty(),
                obligation_live: false,
            });
        }
    }
}

pub(super) fn check_unselected_claims(
    program: &dyn ::validation::CheckingInput,
    state_symbol: SymbolHandle,
    event: &crate::flow::DiscoveredMoveEvent,
    path: &[facts::PlaceSegment],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(_) = event.root.as_expression() else {
        return;
    };
    let root = validation::CanonicalPlace {
        root: event.root,
        segments: Vec::new(),
    };
    let Some(reference) = validation::canonical_place_type_reference(
        program,
        state_symbol,
        event_statement_index(event.source).unwrap_or(0),
        &root,
    ) else {
        return;
    };
    for claim in linear_claim_frontier(program, reference) {
        if claim.multiplicity != Multiplicity::Linear
            || claim.path.starts_with(path)
            || claim_paths_are_case_alternatives(&claim.path, path)
        {
            continue;
        }
        diagnostics.push(Diagnostic::error(
            "cannot partially move a temporary call result while an unselected linear claim remains; bind the result to a local and transfer or consume every claim",
        ));
        break;
    }
}

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

mod projected;
mod shared_borrows;

pub(super) use shared_borrows::append_shared_borrow;

pub(super) fn append_whole_affine_transfer(
    program: &typed_trees::TypedTrees,
    facts: &mut CheckFacts,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    calls: &[checked_trees::FlowCallFact],
    event: &crate::flow::DiscoveredMoveEvent,
    permission_events: &mut Vec<FlowPermissionEventFact>,
) {
    if !event.segments.is_empty() {
        projected::append(
            program,
            facts,
            machine_symbol,
            state_symbol,
            calls,
            event,
            permission_events,
        );
        return;
    }
    let facts::PlaceRoot::Expression(expression) = event.root else {
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
                && crate::flow::call_target_return_type(program, call.target_symbol).is_some_and(
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
/// linear constructions keep their claim rules.
pub(super) fn append_constructed_argument_transfers(
    program: &typed_trees::TypedTrees,
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
        let Some(site) = crate::semantic::calls::find_call_site(
            program,
            machine_symbol,
            state_symbol,
            statement_index,
            call.call_ordinal,
        ) else {
            continue;
        };
        let Some(parameters) =
            crate::semantic::calls::call_target_parameters(program, call.target_symbol)
        else {
            continue;
        };
        let arguments = crate::semantic::calls::call_site_argument_expressions(program, &site);
        let explicit_self = arguments.len()
            > parameters
                .iter()
                .filter(|parameter| !parameter.is_self)
                .count();
        let explicit_parameters = parameters
            .iter()
            .filter(|parameter| !parameter.is_self || explicit_self);
        for (parameter, argument) in explicit_parameters.zip(arguments.iter()) {
            let typed_trees::expression::ExpressionNode::StructLiteral(literal) =
                program.expression_table.expression(*argument)
            else {
                continue;
            };
            let Some(data) = program
                .data_definitions()
                .iter()
                .find(|data| data.symbol == literal.type_symbol)
            else {
                continue;
            };
            let multiplicity = data.properties.multiplicity;
            let source = language_semantics::PermissionEventSource::Call {
                statement_index,
                call_ordinal: call.call_ordinal,
                target_symbol: call.target_symbol,
            };
            let root = facts::PlaceRoot::Expression(*argument);
            if permission_events
                .iter()
                .any(|event| event.source == source && event.root == root)
                || parameter.is_self
                || parameter.is_const
                || parameter.relevance.is_erased()
                || multiplicity != Multiplicity::Affine
                || type_multiplicity(program, parameter.type_reference) != multiplicity
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
    program: &typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    event: &crate::flow::DiscoveredMoveEvent,
    path: &[facts::PlaceSegment],
    diagnostics: &mut Vec<Diagnostic>,
) {
    let facts::PlaceRoot::Expression(_) = event.root else {
        return;
    };
    let root = crate::flow::CanonicalPlace {
        root: event.root,
        segments: Vec::new(),
    };
    let Some(reference) = crate::flow::canonical_place_type_reference(
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

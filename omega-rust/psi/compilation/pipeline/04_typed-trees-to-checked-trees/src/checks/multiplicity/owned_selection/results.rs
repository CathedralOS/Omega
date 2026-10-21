//! Bind selected obligations by their receiving paths. These names denote
//! arrivals, not fresh resources. The transfer roster retains each original
//! source and exact arm; differing sources never become common lineage.

use crate::checks::ownership_occurrences::EvaluationBranch;
use arena::{Handle, HandleSpan};
use checked_trees::{FlowOwnedSelectionReceipt, FlowOwnedSelectionResultClaim, FlowOwnershipFacts};
use diagnostics::Diagnostic;
use language_semantics::{PermissionClaimIdentity, PermissionEventSource, PermissionProvenance};
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableMatchArm};

/// The same first-matching-arm schedule as ownership occurrence evaluation.
/// The final reachable arm is the remaining edge, not another binary choice.
pub(in crate::checks::multiplicity) fn terminal_arm_route(
    program: &dyn validation::CheckingInput,
    expression: ExpressionHandle,
    terminal: Handle<TableMatchArm>,
) -> Option<Vec<EvaluationBranch>> {
    let ExpressionNode::Match(dispatch) = program.expression_table.expression(expression) else {
        return None;
    };
    let arms = super::operands::reachable_arms(program, dispatch.arms);
    let mut prefix = Vec::new();
    for (decision, (handle, arm)) in arms.iter().enumerate() {
        let mut route = prefix.clone();
        if decision + 1 < arms.len() {
            route.push(EvaluationBranch::new(expression, decision, true));
        }
        if *handle == terminal {
            return Some(route);
        }
        if let Some(nested) = terminal_arm_route(program, arm.value, terminal) {
            route.extend(nested);
            return Some(route);
        }
        prefix.push(EvaluationBranch::new(expression, decision, false));
    }
    None
}

pub(in crate::checks::multiplicity) fn terminal_arms(
    program: &dyn validation::CheckingInput,
    expression: ExpressionHandle,
) -> Vec<(Handle<TableMatchArm>, &TableMatchArm)> {
    let ExpressionNode::Match(dispatch) = program.expression_table.expression(expression) else {
        return Vec::new();
    };
    super::operands::reachable_arms(program, dispatch.arms)
        .into_iter()
        .flat_map(|(handle, arm)| {
            if matches!(
                program.expression_table.expression(arm.value),
                ExpressionNode::Match(_)
            ) {
                terminal_arms(program, arm.value)
            } else {
                vec![(handle, arm)]
            }
        })
        .collect()
}

pub(super) fn operand_result_path(
    program: &dyn validation::CheckingInput,
    selection: ExpressionHandle,
    arm: Handle<TableMatchArm>,
    operand: ExpressionHandle,
    result_type: typed_trees::types::TypeReferenceHandle,
) -> Result<Vec<facts::PlaceSegment>, Diagnostic> {
    let arm = terminal_arms(program, selection)
        .into_iter()
        .find(|(handle, _)| *handle == arm)
        .map(|(_, arm)| arm)
        .ok_or_else(|| Diagnostic::error("selected operand has no reachable source arm"))?;
    if arm.value == operand {
        return Ok(Vec::new());
    }
    let projections =
        crate::flow::literal_expression_projections(program, arm.value, result_type, &[], true)
            .ok_or_else(|| {
                Diagnostic::error("selected construction has no exact result projection")
            })?;
    let mut matching = projections
        .iter()
        .filter(|projection| projection.expression == operand);
    let projection = matching.next().ok_or_else(|| {
        Diagnostic::error("selected operand is absent from its constructed result")
    })?;
    if matching.next().is_some() || !projection.remaining.is_empty() {
        return Err(Diagnostic::error(
            "selected operand has ambiguous result storage",
        ));
    }
    Ok(projection.destination.clone())
}

pub(super) fn bind_results(
    program: &dyn validation::CheckingInput,
    ownership: &mut FlowOwnershipFacts,
    receipt: &FlowOwnedSelectionReceipt,
    identities: &mut super::super::linear_obligations::ClaimIdentityAllocator,
) -> Result<HandleSpan<FlowOwnedSelectionResultClaim>, Diagnostic> {
    let arms = terminal_arms(program, receipt.expression);
    let mut results = Vec::new();
    for template in
        super::super::linear_validation::linear_claim_frontier(program, receipt.type_reference)
    {
        let mut incoming = Vec::new();
        for (arm, _) in &arms {
            let mut matching = Vec::new();
            for transfer in ownership
                .selection_transfers
                .span_or_empty(receipt.transfers)
                .iter()
                .filter(|transfer| transfer.source_arm == *arm)
            {
                let prefix = ownership.segments.span_or_empty(transfer.path);
                for claim in ownership
                    .selection_transfer_claims
                    .span_or_empty(transfer.claims)
                {
                    let Some(suffix) = ownership
                        .segments
                        .span_or_empty(claim.path)
                        .strip_prefix(prefix)
                    else {
                        return Err(Diagnostic::error(
                            "selected claim is outside its moved source",
                        ));
                    };
                    let mut destination = ownership
                        .segments
                        .span_or_empty(transfer.result_path)
                        .to_vec();
                    destination.extend_from_slice(suffix);
                    if destination == template.path {
                        matching.push((claim.claim_identity, claim.provenance));
                    }
                }
            }
            for produced in ownership
                .selection_produced_claims
                .span_or_empty(receipt.produced_claims)
                .iter()
                .filter(|produced| {
                    produced.source_arm == *arm
                        && ownership.segments.span_or_empty(produced.result_path) == template.path
                })
            {
                matching.push((produced.claim_identity, produced.provenance));
            }
            // A binding never fills a missing obligation: each arm supplies
            // either a moved claim or an authenticated literal production.
            let [claim] = matching.as_slice() else {
                return Err(Diagnostic::error(
                    "selected result claim requires one exact incoming claim on every arm",
                ));
            };
            incoming.push(*claim);
        }
        let Some(first) = incoming.first().copied() else {
            return Err(Diagnostic::error("selected result has no returning arm"));
        };
        let (claim_identity, provenance) = if incoming.iter().all(|claim| *claim == first) {
            first
        } else {
            let source = PermissionEventSource::Statement {
                statement_index: receipt.statement_ordinal as usize,
            };
            let identity = identities.mint(receipt.machine, receipt.state, source);
            let PermissionClaimIdentity::Established { ordinal, .. } = identity else {
                return Err(Diagnostic::error(
                    "selected result binding has no allocated identity",
                ));
            };
            (
                identity,
                PermissionProvenance::Joined {
                    machine_symbol: receipt.machine,
                    state_symbol: receipt.state,
                    source,
                    ordinal,
                },
            )
        };
        results.push(FlowOwnedSelectionResultClaim {
            path: ownership.segments.insert_many(template.path),
            claim_identity,
            provenance,
        });
    }
    Ok(ownership.selection_result_claims.insert_many(results))
}

/// Retain actual literal construction separately from source transfers. A
/// missing transfer is not evidence of issuance: the exact typed projection
/// must reach a constructor which establishes this root obligation.
pub(super) fn record_productions(
    program: &dyn validation::CheckingInput,
    ownership: &mut FlowOwnershipFacts,
    receipt: &FlowOwnedSelectionReceipt,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    places: &[super::super::linear_obligations::LinearPlace],
    identities: &mut super::super::linear_obligations::ClaimIdentityAllocator,
) -> Result<HandleSpan<checked_trees::FlowOwnedSelectionProducedClaim>, Diagnostic> {
    let mut produced = Vec::new();
    for (source_arm, arm) in terminal_arms(program, receipt.expression) {
        for template in
            super::super::linear_validation::linear_claim_frontier(program, receipt.type_reference)
        {
            let Some(projections) = crate::flow::literal_expression_projections(
                program,
                arm.value,
                receipt.type_reference,
                &template.path,
                false,
            ) else {
                continue;
            };
            let [projection] = projections.as_slice() else {
                continue;
            };
            if !projection.remaining.is_empty()
                || !(matches!(
                    program.expression_table.expression(projection.expression),
                    ExpressionNode::StructLiteral(_)
                ) || validation::scalar_case_constructor(program, projection.expression)
                    .is_some())
            {
                continue;
            }
            let reference = validation::expression_result_type_reference(
                program,
                machine,
                state,
                projection.expression,
            )
            .ok_or_else(|| Diagnostic::error("selected claim producer has no exact type"))?;
            super::leaves::fresh_leaf_shape(
                program,
                machine,
                state,
                projection.expression,
                reference,
            )?;
            if !super::super::type_multiplicity::expression_establishes_obligation(
                program,
                state.symbol,
                receipt.statement_ordinal as usize,
                arm.value,
                &template.path,
                places,
            ) {
                continue;
            }
            let source = PermissionEventSource::Statement {
                statement_index: receipt.statement_ordinal as usize,
            };
            produced.push(checked_trees::FlowOwnedSelectionProducedClaim {
                source_arm,
                expression: projection.expression,
                result_path: ownership.segments.insert_many(template.path),
                claim_identity: identities.mint(machine.symbol, state.symbol, source),
                provenance: PermissionProvenance::Established {
                    machine_symbol: machine.symbol,
                    state_symbol: state.symbol,
                    source,
                },
            });
        }
    }
    Ok(ownership.selection_produced_claims.insert_many(produced))
}

/// Authenticate a joined namespace against the selection that owns it. Matching
/// a statement number or a non-Unknown provenance alone is not authority.
pub(in crate::checks::multiplicity) fn selection_claim_origin(
    ownership: &FlowOwnershipFacts,
    identity: PermissionClaimIdentity,
    provenance: PermissionProvenance,
) -> bool {
    let PermissionProvenance::Joined {
        machine_symbol,
        state_symbol,
        source: PermissionEventSource::Statement { statement_index },
        ordinal,
    } = provenance
    else {
        return false;
    };
    identity
        == PermissionClaimIdentity::Established {
            machine_symbol,
            state_symbol,
            source: PermissionEventSource::Statement { statement_index },
            ordinal,
        }
        && ownership
            .owned_selection_at(state_symbol, statement_index as u32)
            .is_some_and(|(_, receipt)| {
                receipt.machine == machine_symbol
                    && ownership
                        .selection_result_claims
                        .span_or_empty(receipt.result_claims)
                        .iter()
                        .any(|claim| {
                            claim.claim_identity == identity && claim.provenance == provenance
                        })
            })
}

//! Keep every safety check attached to the call that needs it.
//! Translate source call coordinates into emitted operation identities; the
//! selected implementation must still prove that it fulfills each request.

use checked_trees::CheckedTrees;
use lowered_psi::LoweredPsi;
use terminal_psi::{BoundaryLenderPreservationDemand, StructuralDomainRequirement};

use super::call_requests::{EmittedCallCatalog, EmittedCallLookupError, StatementCallKey};
use crate::shared::lowering_error::LoweringError;
use crate::shared::terminal_identities::StructuralDomainBinding;

pub(super) fn retain(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    lowered: &mut LoweredPsi,
    emitted_calls: &EmittedCallCatalog,
) -> Result<(), LoweringError> {
    if !lowered
        .semantic_module
        .boundary_lender_preservation
        .is_empty()
    {
        return invalid_plan("lender preservation requests were already lowered");
    }
    let mut rows = Vec::new();
    let requested = checked
        .facts
        .flow
        .control
        .states
        .iter()
        .flat_map(|(_, state)| {
            checked
                .facts
                .flow
                .control
                .calls
                .span_or_empty(state.calls)
                .iter()
                .filter(|call| !call.lender_preservation.is_empty())
                .map(move |call| (state, call))
        })
        .collect::<Vec<_>>();
    if requested.is_empty() {
        return Ok(());
    }
    let mut requested_calls = requested
        .iter()
        .enumerate()
        .filter_map(|(request_index, (state, call))| {
            StatementCallKey::from_checked(
                checked,
                checked_trees::CheckedValueOrigin::StateStatement {
                    machine_symbol: state.machine_symbol,
                    state_symbol: state.state_symbol,
                    statement_index: call.statement_index,
                    role: checked_trees::CheckedValueStatementRole::Expression,
                },
                call.call_ordinal,
            )
            .map(|key| (key, request_index))
        })
        .collect::<Vec<_>>();
    requested_calls.sort_unstable();
    let mut visited = vec![false; requested.len()];
    let mut domain_catalog = None;
    for occurrence in &lowered.source_call_occurrences {
        let (Some(coordinate), Some(operation_id)) = (
            occurrence.origin.statement(),
            occurrence.terminal_site.operation(),
        ) else {
            continue;
        };
        let key = StatementCallKey::new(
            coordinate.state,
            coordinate.statement_index,
            coordinate.call_ordinal,
        );
        let request_position = requested_calls.partition_point(|(found, _)| *found < key);
        let Some(&(_, request_index)) = requested_calls
            .get(request_position)
            .filter(|(found, _)| *found == key)
        else {
            continue;
        };
        if requested_calls
            .get(request_position + 1)
            .is_some_and(|(found, _)| *found == key)
        {
            return invalid_plan("a lowered lender call has ambiguous checked request owners");
        }
        let (_, call) = requested[request_index];
        if occurrence.source_target != call.target_symbol
            && occurrence.source_target != call.authored_target
        {
            return invalid_plan("the lender request joins another checked call target");
        }
        let (machine, boundary) = match emitted_calls.boundary_call(operation_id) {
            Ok(call) => call,
            Err(EmittedCallLookupError::MissingOrAmbiguousOperation) => {
                return invalid_plan("the lender call has no unique emitted operation");
            }
            Err(EmittedCallLookupError::NotBoundaryCall) => {
                return invalid_plan("the lender request did not lower to a boundary call");
            }
        };
        let declaration = lowered.semantic_module.boundary_machine(boundary).ok_or(
            LoweringError::InvalidLenderPreservationPlan(
                "the lender call has no public declaration",
            ),
        )?;
        if domain_catalog.is_none() {
            domain_catalog = Some(
                lowered
                    .semantic_module
                    .structural_domains
                    .iter()
                    .map(|domain| {
                        Ok(StructuralDomainBinding {
                            // This catalog field explicitly retains Psi's semantic domain
                            // identity; the concrete carrier-specific domain id is separate.
                            semantic_domain: language_semantics::SemanticDomainId(
                                u32::try_from(domain.semantic_domain.get()).map_err(|_| {
                                    LoweringError::InvalidLenderPreservationPlan(
                                        "the semantic domain exceeds Psi's identity range",
                                    )
                                })?,
                            ),
                            carrier: domain.carrier,
                            domain: domain.id,
                            byte_predicate: domain.byte_predicate.clone(),
                        })
                    })
                    .collect::<Result<Vec<_>, LoweringError>>()?,
            );
        }
        let domain_bindings =
            domain_catalog
                .as_deref()
                .ok_or(LoweringError::InvalidLenderPreservationPlan(
                    "the lender domain catalog was not prepared",
                ))?;
        for request in &call.lender_preservation {
            let mut parameters = declaration
                .structural_parameters
                .iter()
                .enumerate()
                .filter(|(_, formal)| formal.position == request.argument_position);
            let (Some((index, formal)), None) = (parameters.next(), parameters.next()) else {
                return invalid_plan("the lender formal has no exact packed structural position");
            };
            let mut selected =
                crate::unit::attached_unit::parameters::lower_projected_qualifications(
                    identities,
                    &[checked_trees::CheckedStructuralPathQualification {
                        path: request.path.clone(),
                        domain: request.domain,
                    }],
                    domain_bindings,
                    formal.structural_type,
                    &lowered.semantic_module.structural_types,
                )?;
            let qualification =
                selected
                    .pop()
                    .ok_or(LoweringError::InvalidLenderPreservationPlan(
                        "the lender request lost its exact projected domain",
                    ))?;
            if !selected.is_empty() {
                return invalid_plan("the lender request expanded into multiple domains");
            }
            let requirement = StructuralDomainRequirement {
                argument_index: u32::try_from(index).map_err(|_| {
                    LoweringError::InvalidLenderPreservationPlan(
                        "the lender structural position exceeds u32",
                    )
                })?,
                path: qualification.path,
                domain: qualification.domain,
            };
            if !declaration.normal_qualifications.contains(&requirement) {
                return invalid_plan(
                    "the lender request is not the exact public normal contract row",
                );
            }
            rows.push(BoundaryLenderPreservationDemand {
                machine,
                operation: operation_id,
                requirement,
            });
        }
        visited[request_index] = true;
    }
    for ((state, call), visited) in requested.into_iter().zip(visited) {
        let key = StatementCallKey::from_checked(
            checked,
            checked_trees::CheckedValueOrigin::StateStatement {
                machine_symbol: state.machine_symbol,
                state_symbol: state.state_symbol,
                statement_index: call.statement_index,
                role: checked_trees::CheckedValueStatementRole::Expression,
            },
            call.call_ordinal,
        );
        if key.is_some_and(|key| emitted_calls.state_was_emitted(key)) && !visited {
            return invalid_plan("an emitted checked lender request has no call occurrence");
        }
    }
    rows.sort();
    if rows.windows(2).any(|pair| pair[0] == pair[1]) {
        return invalid_plan("multiple lender requests name the same emitted duty");
    }
    lowered.semantic_module.boundary_lender_preservation = rows;
    Ok(())
}

fn invalid_plan<T>(reason: &'static str) -> Result<T, LoweringError> {
    Err(LoweringError::InvalidLenderPreservationPlan(reason))
}

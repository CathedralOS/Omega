//! Carry the caller's requested lender duties onto the exact emitted call.
//! Source checking selects the duties; this join changes only their namespace.
//! Provider selection must still fulfill them independently.

use checked_trees::CheckedTrees;
use lowered_psi::LoweredPsi;
use terminal_psi::{BoundaryLenderPreservationDemand, OperationKind, StructuralDomainRequirement};

use crate::shared::lowering_error::{LoweringError, unsupported};
use crate::shared::terminal_identities::StructuralDomainBinding;

pub(super) fn retain(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    lowered: &mut LoweredPsi,
) -> Result<(), LoweringError> {
    if !lowered
        .semantic_module
        .boundary_lender_preservation
        .is_empty()
    {
        return unsupported("lender preservation requests were already lowered");
    }
    let mut rows = Vec::new();
    let mut visited = std::collections::HashSet::new();
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
    for occurrence in &lowered.source_call_occurrences {
        let (Some(coordinate), Some(operation_id)) = (
            occurrence.origin.statement(),
            occurrence.terminal_site.operation(),
        ) else {
            continue;
        };
        let mut matching = requested.iter().filter(|(state, call)| {
            call.call_ordinal == coordinate.call_ordinal
                && occurrence.matches_checked_origin(
                    checked,
                    checked_trees::CheckedValueOrigin::StateStatement {
                        machine_symbol: state.machine_symbol,
                        state_symbol: state.state_symbol,
                        statement_index: call.statement_index,
                        role: checked_trees::CheckedValueStatementRole::Expression,
                    },
                )
        });
        let Some(&(state, call)) = matching.next() else {
            continue;
        };
        if matching.next().is_some() {
            return unsupported("a lowered lender call has ambiguous checked request owners");
        }
        if occurrence.source_target != call.target_symbol
            && occurrence.source_target != call.authored_target
        {
            return unsupported("the lender request joins another checked call target");
        }
        let mut operations = lowered
            .semantic_module
            .machines
            .iter()
            .flat_map(|machine| {
                machine
                    .operations()
                    .map(move |operation| (machine.id, operation))
            })
            .filter(|(_, operation)| operation.id == operation_id);
        let (Some((machine, operation)), None) = (operations.next(), operations.next()) else {
            return unsupported("the lender call has no unique emitted operation");
        };
        let OperationKind::BoundaryCall { boundary, .. } = operation.kind else {
            return unsupported("the lender request did not lower to a boundary call");
        };
        let declaration = lowered.semantic_module.boundary_machine(boundary).ok_or(
            LoweringError::Unsupported("the lender call has no public declaration"),
        )?;
        let domain_ids = lowered
            .semantic_module
            .structural_domains
            .iter()
            .map(|domain| {
                Ok(StructuralDomainBinding {
                    // This catalog field explicitly retains Psi's semantic domain
                    // identity; the concrete carrier-specific domain id is separate.
                    semantic_domain: language_semantics::SemanticDomainId(
                        u32::try_from(domain.semantic_domain.get()).map_err(|_| {
                            LoweringError::Unsupported(
                                "the semantic domain exceeds Psi's identity range",
                            )
                        })?,
                    ),
                    carrier: domain.carrier,
                    domain: domain.id,
                    byte_predicate: domain.byte_predicate.clone(),
                })
            })
            .collect::<Result<Vec<_>, LoweringError>>()?;
        for request in &call.lender_preservation {
            let mut parameters = declaration
                .structural_parameters
                .iter()
                .enumerate()
                .filter(|(_, formal)| formal.position == request.argument_position);
            let (Some((index, formal)), None) = (parameters.next(), parameters.next()) else {
                return unsupported("the lender formal has no exact packed structural position");
            };
            let mut selected =
                crate::unit::attached_unit::parameters::lower_projected_qualifications(
                    identities,
                    &[checked_trees::CheckedStructuralPathQualification {
                        path: request.path.clone(),
                        domain: request.domain,
                    }],
                    &domain_ids,
                    formal.structural_type,
                    &lowered.semantic_module.structural_types,
                )?;
            let qualification = selected.pop().ok_or(LoweringError::Unsupported(
                "the lender request lost its exact projected domain",
            ))?;
            if !selected.is_empty() {
                return unsupported("the lender request expanded into multiple domains");
            }
            let requirement = StructuralDomainRequirement {
                argument_index: u32::try_from(index).map_err(|_| {
                    LoweringError::Unsupported("the lender structural position exceeds u32")
                })?,
                path: qualification.path,
                domain: qualification.domain,
            };
            if !declaration.normal_qualifications.contains(&requirement) {
                return unsupported(
                    "the lender request is not the exact public normal contract row",
                );
            }
            rows.push(BoundaryLenderPreservationDemand {
                machine,
                operation: operation_id,
                requirement,
            });
        }
        visited.insert((
            state.machine_symbol,
            state.state_symbol,
            coordinate.statement_index,
            coordinate.call_ordinal,
        ));
    }
    for (state, call) in requested {
        let origin =
            checked.consistent_terminal_origin(checked_trees::CheckedValueOrigin::StateStatement {
                machine_symbol: state.machine_symbol,
                state_symbol: state.state_symbol,
                statement_index: call.statement_index,
                role: checked_trees::CheckedValueStatementRole::Expression,
            });
        let emitted = match origin {
            Some(checked_trees::CheckedValueOrigin::StateStatement { state_symbol, .. }) => lowered
                .state_blocks
                .iter()
                .any(|block| block.state == state_symbol),
            _ => false,
        };
        if emitted
            && !visited.contains(&(
                state.machine_symbol,
                state.state_symbol,
                call.statement_index,
                call.call_ordinal,
            ))
        {
            return unsupported("an emitted checked lender request has no call occurrence");
        }
    }
    rows.sort();
    if rows.windows(2).any(|pair| pair[0] == pair[1]) {
        return unsupported("multiple lender requests name the same emitted duty");
    }
    lowered.semantic_module.boundary_lender_preservation = rows;
    Ok(())
}

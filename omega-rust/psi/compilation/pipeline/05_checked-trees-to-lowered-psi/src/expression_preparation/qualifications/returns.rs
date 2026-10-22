//! Close source-checked result memberships over emitted normal return edges.
//! The checker proved every reachable source exit before this owner runs.
//! Lowering may join several such exits into one return block; retain their
//! origin alternatives rather than inventing an issuer for the joined value.
//! Numeric predicates remain ordinary normal-return contracts. These rows
//! preserve the separate provenance decision and exact returned SSA occurrence.

use super::{CheckedTrees, LoweringError, ScalarQualificationCatalog, SymbolHandle, unsupported};
use checked_trees::CheckedScalarReturnOrigin;
use semantic_vocabulary::MachineId;
use terminal_psi::{
    ScalarReturnOrigin, ScalarReturnQualification, ScalarReturnSite, TerminalMachine, Terminator,
};

/// Admission belongs to one fresh return-completion binding, never to its
/// operand. Only this owner can construct it from exact source exit judgments.
pub(crate) struct CheckedReturnCompletion {
    source: semantic_vocabulary::QualifiedScalarType,
    result: semantic_vocabulary::QualifiedScalarType,
}

impl CheckedReturnCompletion {
    pub(crate) fn destination(
        self,
        source: semantic_vocabulary::QualifiedScalarType,
    ) -> Result<semantic_vocabulary::QualifiedScalarType, LoweringError> {
        if source != self.source {
            return unsupported("checked return completion changed its operand type");
        }
        Ok(self.result)
    }
}

impl super::PreparedScalarQualifications {
    pub(crate) fn return_completion(
        &self,
        checked: &CheckedTrees,
        machine: SymbolHandle,
        exit: &checked_trees::FlowClaimJoinExit,
        source: semantic_vocabulary::QualifiedScalarType,
        result: semantic_vocabulary::QualifiedScalarType,
    ) -> Result<CheckedReturnCompletion, LoweringError> {
        if source.scalar_type != result.scalar_type {
            return unsupported("checked scalar return completion changes its carrier");
        }
        let domains = |qualification| {
            if qualification == semantic_vocabulary::ScalarQualificationSetId::default() {
                return Ok(&[][..]);
            }
            self.catalog
                .sets
                .iter()
                .find(|set| set.id == qualification)
                .map(|set| set.domains.as_slice())
                .ok_or(LoweringError::Unsupported(
                    "scalar return completion lost its qualification set",
                ))
        };
        let source_domains = domains(source.qualifications)?;
        let result_domains = domains(result.qualifications)?;
        if source_domains
            .iter()
            .any(|domain| !result_domains.contains(domain))
        {
            return unsupported(
                "checked scalar return completion cannot erase unrelated membership",
            );
        }
        for domain_id in result_domains
            .iter()
            .filter(|domain| !source_domains.contains(domain))
        {
            let domain = self
                .catalog
                .domains
                .iter()
                .find(|domain| domain.id == *domain_id)
                .ok_or(LoweringError::Unsupported(
                    "scalar return completion lost its domain",
                ))?;
            if domain.predicate.is_none() && domain.establishment_routes.is_empty() {
                continue;
            }
            checked_exit_decision(checked, machine, exit, domain.semantic_domain)?;
        }
        Ok(CheckedReturnCompletion { source, result })
    }
}

fn checked_exit_decision<'checked>(
    checked: &'checked CheckedTrees,
    machine: SymbolHandle,
    exit: &checked_trees::FlowClaimJoinExit,
    domain: semantic_vocabulary::DomainSemanticId,
) -> Result<&'checked checked_trees::CheckedScalarReturnQualification, LoweringError> {
    let mut decisions = checked
        .facts
        .qualifications
        .scalar_returns
        .iter()
        .filter(|decision| {
            decision.machine == machine
                && u64::from(decision.semantic_domain.0) == domain.get()
                && matches_exit(checked, exit, decision)
        });
    let decision = decisions.next().ok_or(LoweringError::Unsupported(
        "scalar return has an exit without its checked membership decision",
    ))?;
    if decisions.next().is_some() {
        return unsupported("scalar return has ambiguous checked exit membership");
    }
    Ok(decision)
}

#[derive(Debug, Clone)]
pub(crate) struct SourceScalarReturn {
    pub edge: semantic_vocabulary::EdgeId,
    pub exits: Vec<checked_trees::FlowClaimJoinExit>,
}

pub(crate) fn retain_returns(
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
    checked: &CheckedTrees,
    machine_ids: &[(SymbolHandle, MachineId)],
    machines: &[TerminalMachine],
    sources: &[SourceScalarReturn],
    catalog: &mut ScalarQualificationCatalog,
) -> Result<(), LoweringError> {
    for machine in machines {
        let Some(result) = machine.result.scalar() else {
            continue;
        };
        let set = catalog
            .sets
            .iter()
            .find(|set| set.id == result.qualifications);
        if set.is_none() && !result.qualifications.is_empty() {
            return unsupported("scalar result lost its qualification set");
        }
        // A generated bare helper has no source membership to retain. Only an
        // actual result qualification or a recorded completion edge needs a
        // checked source owner; never invent one from machine position.
        if set.is_none()
            && !machine.blocks.iter().any(|block| {
                matches!(block.terminator, Terminator::Jump { edge, .. }
                if sources.iter().any(|source| source.edge == edge))
            })
        {
            continue;
        }
        let symbol = machine_ids
            .iter()
            .find(|(_, id)| *id == machine.id)
            .map(|(symbol, _)| *symbol)
            .ok_or(LoweringError::Unsupported(
                "qualified result lost its source machine",
            ))?;
        for domain in &catalog.domains {
            let domain_id = &domain.id;
            for block in &machine.blocks {
                let (edge, value, result_id, site) = match &block.terminator {
                    Terminator::Return { edge, value, .. }
                        if !domain.establishment_routes.is_empty()
                            && set.is_some_and(|set| set.domains.contains(domain_id)) =>
                    {
                        (*edge, *value, result.id, ScalarReturnSite::NormalExit)
                    }
                    Terminator::Jump {
                        edge,
                        target,
                        arguments,
                        ..
                    } if sources.iter().any(|source| source.edge == *edge) => {
                        let destination = machine
                            .blocks
                            .iter()
                            .find(|block| block.id == *target)
                            .and_then(|block| block.parameters.last())
                            .ok_or(LoweringError::Unsupported(
                                "scalar return completion lost its destination",
                            ))?;
                        if !catalog.sets.iter().any(|set| {
                            set.id == destination.qualifications && set.domains.contains(domain_id)
                        }) {
                            continue;
                        }
                        let value = *arguments.last().ok_or(LoweringError::Unsupported(
                            "scalar return completion lost its source",
                        ))?;
                        let declaration = machine
                            .parameters
                            .iter()
                            .chain(machine.block_parameters())
                            .chain(
                                machine
                                    .operations()
                                    .filter_map(|operation| operation.result.scalar_ref()),
                            )
                            .find(|source| source.id == value)
                            .ok_or(LoweringError::Unsupported(
                                "scalar return completion source is undeclared",
                            ))?;
                        if catalog.sets.iter().any(|set| {
                            set.id == declaration.qualifications && set.domains.contains(domain_id)
                        }) {
                            continue;
                        }
                        (
                            *edge,
                            value,
                            destination.id,
                            ScalarReturnSite::Completion {
                                argument_ordinal: u32::try_from(arguments.len() - 1).map_err(
                                    |_| {
                                        LoweringError::Unsupported(
                                            "scalar completion argument exceeds u32",
                                        )
                                    },
                                )?,
                            },
                        )
                    }
                    _ => continue,
                };
                let source_exits = sources.iter().find(|source| source.edge == edge).ok_or(
                    LoweringError::Unsupported("qualified return lost its checked source exits"),
                )?;
                let mut origins = Vec::new();
                for exit in &source_exits.exits {
                    if domain.predicate.is_none() && domain.establishment_routes.is_empty() {
                        origins.push(ScalarReturnOrigin::Vacuous);
                        continue;
                    }
                    let decision =
                        checked_exit_decision(checked, symbol, exit, domain.semantic_domain)?;
                    origins.push(match &decision.origin {
                        CheckedScalarReturnOrigin::Forwarded => ScalarReturnOrigin::Forwarded,
                        CheckedScalarReturnOrigin::Established(route) => {
                            let route = super::establishment_route(identities, checked, route)?;
                            let position = domain
                                .establishment_routes
                                .iter()
                                .position(|row| *row == route)
                                .ok_or(LoweringError::Unsupported(
                                    "checked scalar issuer is absent from its domain routes",
                                ))?;
                            ScalarReturnOrigin::Established {
                                route: u32::try_from(position).map_err(|_| {
                                    LoweringError::Unsupported("scalar issuer route exceeds u32")
                                })?,
                            }
                        }
                        CheckedScalarReturnOrigin::Predicate => {
                            return unsupported(
                                "predicate-only checking cannot establish routed scalar provenance",
                            );
                        }
                    });
                }
                origins.sort();
                origins.dedup();
                // A completion phi can already wear the declared result type.
                // That annotation is no establishment evidence: preserve the
                // checker's actual alternatives, including mixed forwarding
                // and issuance, instead of inferring Forwarded from its type.
                if origins.is_empty() {
                    return unsupported("routed scalar result has no checked return membership");
                }
                catalog.returns.push(ScalarReturnQualification {
                    site,
                    machine: machine.id,
                    edge,
                    source: value,
                    result: result_id,
                    domain: *domain_id,
                    origins,
                });
            }
        }
    }
    catalog
        .returns
        .sort_by_key(|row| (row.machine, row.edge, row.domain));
    Ok(())
}

/// Keep both outgoing alternatives of one source statement distinct. The
/// cleanup schedule and checking judgment refer to the same immutable typed
/// occurrence; this join translates its coordinate, not its proof. Which arm a
/// decision belongs to is the checked exit row's own `is_continuation`, copied
/// onto the decision; lowering does not reopen the authored transition.
fn matches_exit(
    checked: &CheckedTrees,
    exit: &checked_trees::FlowClaimJoinExit,
    decision: &checked_trees::CheckedScalarReturnQualification,
) -> bool {
    use checked_trees::FlowClaimJoinExitKind;
    if exit.state_symbol != decision.state || exit.statement_index != decision.statement_index {
        return false;
    }
    match exit.kind {
        // Structural selection ancestry is not a scalar return decision.
        FlowClaimJoinExitKind::SelectionArm { .. }
        | FlowClaimJoinExitKind::SelectionDecision { .. } => false,
        FlowClaimJoinExitKind::FinalExpression => !decision.transition_target.is_valid(),
        FlowClaimJoinExitKind::TransitionTarget | FlowClaimJoinExitKind::TransitionContinuation => {
            decision.transition_target.is_valid()
                && decision.is_continuation
                    == matches!(exit.kind, FlowClaimJoinExitKind::TransitionContinuation)
        }
        FlowClaimJoinExitKind::CallResult { call_ordinal } => checked
            .facts
            .flow
            .control
            .states
            .iter()
            .filter(|(_, state)| {
                state.machine_symbol == decision.machine && state.state_symbol == decision.state
            })
            .flat_map(|(_, state)| checked.facts.flow.control.calls.span_or_empty(state.calls))
            .any(|call| {
                call.statement_index == decision.statement_index
                    && call.call_ordinal == call_ordinal
                    && call.authored_expression == decision.expression
            }),
    }
}

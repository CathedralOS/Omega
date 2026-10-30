//! Representation joins for scalar result provenance at external ingress.
//! Checking owns predicate truth and the original establishment decision.
//! This owner checks that transport has not detached that decision from its
//! domain, exact callable, result value or normal return edge.

use super::{CodecError, TerminalModule, Terminator, malformed};
use terminal_psi::{ScalarDomainEstablishmentRoute, ScalarReturnOrigin, ScalarReturnSite};

pub(super) fn validate(module: &TerminalModule) -> Result<(), CodecError> {
    let catalog = &module.scalar_qualifications;
    for domain in &catalog.domains {
        if !domain.required_memberships_are_well_formed(&catalog.domains) {
            return malformed("scalar domain has ambiguous, cyclic or wrong-carrier prerequisites");
        }
        if module
            .identities
            .text(domain.identity)
            .is_none_or(str::is_empty)
        {
            return malformed("scalar domain has no retained identity");
        }
        if !domain.establishment_routes.is_empty() && domain.predicate.is_none() {
            return malformed("routed scalar domain lost its predicate component");
        }
        if let Some(predicate) = &domain.predicate {
            semantic_vocabulary::PropositionContext::from_value_types([(
                predicate.subject,
                domain.carrier,
            )])
            .and_then(|context| context.validate(&predicate.proposition))
            .map_err(CodecError::MalformedProposition)?;
            let mut exact = predicate.subject.get() == 1;
            exact &= predicate.proposition.visit_value_ids(|value| {
                exact &= value == predicate.subject;
            });
            if !exact {
                return malformed("scalar domain predicate escapes its formal subject");
            }
        }
        if domain.establishment_routes.iter().any(|route| {
            module
                .identities
                .text(route.identity())
                .is_none_or(str::is_empty)
                || module
                    .identities
                    .text(route.declaration().identity())
                    .is_none_or(str::is_empty)
        }) {
            return malformed("scalar domain lost an issuer identity");
        }
    }
    for set in &catalog.sets {
        if set
            .domains
            .iter()
            .any(|id| !catalog.domains.iter().any(|domain| domain.id == *id))
        {
            return malformed("scalar qualification set names an absent domain");
        }
    }
    for coercion in &catalog.coercions {
        let machine = module
            .machines
            .iter()
            .find(|machine| machine.id == coercion.machine)
            .ok_or(CodecError::MalformedStructuralFoundation(
                "scalar coercion lost its machine",
            ))?;
        let source = super::values(machine)
            .find(|value| value.id == coercion.source)
            .ok_or(CodecError::MalformedStructuralFoundation(
                "scalar coercion lost its source",
            ))?;
        let destination = super::values(machine)
            .find(|value| value.id == coercion.destination)
            .ok_or(CodecError::MalformedStructuralFoundation(
                "scalar coercion lost its destination",
            ))?;
        let source_domains = super::qualification_domains(module, source.qualifications).ok_or(
            CodecError::MalformedStructuralFoundation("scalar coercion lost its source set"),
        )?;
        let destination_domains = super::qualification_domains(module, destination.qualifications)
            .ok_or(CodecError::MalformedStructuralFoundation(
                "scalar coercion lost its destination set",
            ))?;
        for id in destination_domains {
            let domain = super::scalar_domain(module, *id).ok_or(
                CodecError::MalformedStructuralFoundation("scalar coercion domain is absent"),
            )?;
            if domain
                .missing_membership_requirements(&catalog.domains, source_domains)
                .is_none_or(|missing| {
                    missing.iter().any(|node| {
                        node.predicate.is_some()
                            || !node.establishment_routes.is_empty()
                            || !node.has_no_required_memberships()
                    })
                })
            {
                return malformed(
                    "scalar coercion cannot establish a predicate or routed qualification",
                );
            }
        }
    }
    for returned in &catalog.returns {
        let machine = module
            .machines
            .iter()
            .find(|machine| machine.id == returned.machine)
            .ok_or(CodecError::MalformedStructuralFoundation(
                "scalar return lost its machine",
            ))?;
        let result = machine
            .result
            .scalar_ref()
            .into_iter()
            .chain(super::values(machine))
            .find(|result| result.id == returned.result)
            .ok_or(CodecError::MalformedStructuralFoundation(
                "scalar return lost its result",
            ))?;
        let domain = super::scalar_domain(module, returned.domain).ok_or(
            CodecError::MalformedStructuralFoundation("scalar return lost its domain"),
        )?;
        let bound = match returned.site {
            ScalarReturnSite::NormalExit => machine.result.scalar_ref().is_some_and(|result| result.id == returned.result)
                && machine.blocks.iter().any(|block| matches!(block.terminator,
                    Terminator::Return { edge, value, .. } if edge == returned.edge && value == returned.source)),
            ScalarReturnSite::Completion { argument_ordinal } => machine.blocks.iter().any(|block| {
                let Terminator::Jump { edge, target, ref arguments, .. } = block.terminator else { return false; };
                edge == returned.edge && arguments.get(argument_ordinal as usize) == Some(&returned.source)
                    && machine.blocks.iter().find(|block| block.id == target)
                        .and_then(|block| block.parameters.get(argument_ordinal as usize))
                        .is_some_and(|destination| destination.id == returned.result && destination.id != returned.source)
            }),
        };
        if result.scalar_type != domain.carrier
            || !catalog
                .sets
                .iter()
                .any(|set| set.id == result.qualifications && set.domains.contains(&domain.id))
            || !bound
        {
            return malformed("scalar membership is not bound to its exact normal result");
        }
        if let Some(predicate) = &domain.predicate
            && returned.site == ScalarReturnSite::NormalExit
        {
            let mut expected = predicate.proposition.clone();
            expected.remap_value_ids(|_| result.id);
            let complete = match &expected {
                semantic_vocabulary::Proposition::Conjunction(predicates) => {
                    predicates.iter().all(|predicate| {
                        machine
                            .contract
                            .ensures
                            .iter()
                            .any(|clause| retains_conjunct(&clause.proposition, predicate))
                    })
                }
                semantic_vocabulary::Proposition::Truth => true,
                _ => machine
                    .contract
                    .ensures
                    .iter()
                    .any(|clause| retains_conjunct(&clause.proposition, &expected)),
            };
            if !complete {
                return malformed("scalar result contract lost its domain predicate");
            }
        }
        let source = super::values(machine)
            .find(|source| source.id == returned.source && source.scalar_type == result.scalar_type)
            .ok_or(CodecError::MalformedStructuralFoundation(
                "scalar return lost its same-carrier source",
            ))?;
        if matches!(returned.site, ScalarReturnSite::Completion { .. })
            && catalog
                .sets
                .iter()
                .any(|set| set.id == source.qualifications && set.domains.contains(&domain.id))
        {
            return malformed("scalar return completion does not add its named membership");
        }
        for origin in &returned.origins {
            let authorized = match origin {
                ScalarReturnOrigin::Vacuous => {
                    matches!(returned.site, ScalarReturnSite::Completion { .. })
                        && domain.predicate.is_none()
                        && domain.establishment_routes.is_empty()
                        && domain.has_no_required_memberships()
                }
                ScalarReturnOrigin::Forwarded => catalog.sets.iter().any(|set| {
                    set.id == source.qualifications
                        && domain
                            .missing_membership_requirements(&catalog.domains, &set.domains)
                            .is_some_and(|missing| missing.is_empty())
                }),
                ScalarReturnOrigin::Established { route } => {
                    let declaration = machine.declaration.as_ref().ok_or(
                        CodecError::MalformedStructuralFoundation(
                            "scalar issuer lost its checked declaration",
                        ),
                    )?;
                    match domain.establishment_routes.get(*route as usize) {
                        Some(ScalarDomainEstablishmentRoute::ExactMachine { declaration: issuer, machine_identity }) => {
                            (*machine_identity == declaration.callable && issuer.identity() == declaration.semantic_declaration)
                                || declaration.specialization.as_ref().is_some_and(|specialization| {
                                    *machine_identity == specialization.template_callable
                                        && issuer.identity() == specialization.template_semantic_declaration
                                        && module.identities.text(specialization.template_callable).is_some_and(|identity| !identity.is_empty())
                                })
                        }
                        Some(ScalarDomainEstablishmentRoute::CheckedRequirement { declaration: issuer, requirement_identity }) => {
                            module.closed_conformance_applications.iter().any(|application| {
                                application.rows.iter().any(|row| {
                                    row.public_requirement_identity == *requirement_identity
                                        && row.requirement_declaration == *issuer
                                        && row.realization_callable_identity == Some(declaration.callable)
                                        && application.realization_callables.iter().any(|callable| {
                                            callable.source_callable_identity == declaration.callable
                                                && callable.callable == terminal_psi::TargetMachineCallable::Machine(machine.id)
                                        })
                                })
                            })
                        }
                        // An admitted boundary crossing is not an ordinary body return.
                        _ => false,
                    }
                }
            };
            let prerequisites = if matches!(origin, ScalarReturnOrigin::Established { .. }) {
                let supplied = super::qualification_domains(module, source.qualifications).ok_or(
                    CodecError::MalformedStructuralFoundation(
                        "scalar issuer lost its source qualification set",
                    ),
                )?;
                domain
                    .missing_membership_requirements(&catalog.domains, supplied)
                    .is_some_and(|missing| {
                        missing.iter().all(|node| {
                            // This return authorizes the named domain only. Each
                            // parent retains its independent issuer obligation.
                            node.id == domain.id || node.establishment_routes.is_empty()
                        })
                    })
            } else {
                true
            };
            if !authorized || !prerequisites {
                return malformed(
                    "scalar return origin does not match its source or authorized issuer",
                );
            }
        }
    }
    for machine in &module.machines {
        for block in &machine.blocks {
            let successors = match &block.terminator {
                Terminator::Jump {
                    edge,
                    target,
                    arguments,
                    ..
                } => vec![(*edge, *target, arguments.as_slice())],
                Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => vec![
                    (
                        when_true.edge,
                        when_true.target,
                        when_true.arguments.as_slice(),
                    ),
                    (
                        when_false.edge,
                        when_false.target,
                        when_false.arguments.as_slice(),
                    ),
                ],
                _ => Vec::new(),
            };
            for (edge, target, arguments) in successors {
                let target = machine
                    .blocks
                    .iter()
                    .find(|block| block.id == target)
                    .ok_or(CodecError::MalformedStructuralFoundation(
                        "scalar successor target is absent",
                    ))?;
                for (ordinal, (argument, destination)) in
                    arguments.iter().zip(&target.parameters).enumerate()
                {
                    let source = super::values(machine)
                        .find(|value| value.id == *argument)
                        .ok_or(CodecError::MalformedStructuralFoundation(
                            "scalar successor source is absent",
                        ))?;
                    for domain in &catalog.domains {
                        let contains = |qualification| {
                            catalog.sets.iter().any(|set| {
                                set.id == qualification && set.domains.contains(&domain.id)
                            })
                        };
                        if contains(destination.qualifications) && !contains(source.qualifications)
                            && !catalog.coercions.iter().any(|row| row.machine == machine.id && row.edge == edge
                                && row.source == source.id && row.destination == destination.id && row.argument_ordinal as usize == ordinal)
                            && !catalog.returns.iter().any(|row| row.machine == machine.id && row.edge == edge
                                && row.source == source.id && row.result == destination.id && row.domain == domain.id
                                && matches!(row.site, ScalarReturnSite::Completion { argument_ordinal } if argument_ordinal as usize == ordinal))
                        {
                            return malformed("scalar successor introduces membership without its checked return completion");
                        }
                    }
                }
            }
        }
        let Some(result) = machine.result.scalar() else {
            continue;
        };
        let Some(set) = catalog
            .sets
            .iter()
            .find(|set| set.id == result.qualifications)
        else {
            continue;
        };
        for domain in set.domains.iter().filter(|id| {
            catalog.domains.iter().any(|domain| {
                domain.id == **id
                    && domain
                        .required_domain_closure(&catalog.domains)
                        .is_none_or(|required| {
                            required
                                .iter()
                                .any(|node| !node.establishment_routes.is_empty())
                        })
            })
        }) {
            for block in &machine.blocks {
                if let Terminator::Return { edge, .. } = block.terminator
                    && !catalog.returns.iter().any(|row| {
                        row.machine == machine.id && row.edge == edge && row.domain == *domain
                    })
                {
                    return malformed(
                        "routed scalar return has no establishment or forwarding row",
                    );
                }
            }
        }
    }
    Ok(())
}

/// Representation containment, not implication search or predicate proof.
fn retains_conjunct(
    actual: &semantic_vocabulary::Proposition,
    expected: &semantic_vocabulary::Proposition,
) -> bool {
    actual == expected
        || matches!(actual, semantic_vocabulary::Proposition::Conjunction(children)
        if children.iter().any(|child| retains_conjunct(child, expected)))
}

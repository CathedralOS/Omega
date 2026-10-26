//! Project already admitted historical folds through existing exact candidates.
use super::{
    CheckedTrees, CheckedUnitProviderCandidate, LoweringError, MachineId, lookup_machine_id,
};
use terminal_psi::{
    FoldedConstantCandidate, FoldedConstantClosure, FoldedConstantDependency,
    FoldedLayoutAlternative, FoldedLayoutObservation, FoldedLayoutOwner, FoldedLayoutRequirement,
    ProviderCandidateConformance, TerminalIdentityTableBuilder,
};

pub(super) fn lower(
    checked: &CheckedTrees,
    identities: &mut TerminalIdentityTableBuilder,
    exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
    sources: &[CheckedUnitProviderCandidate],
    candidates: &[ProviderCandidateConformance],
    machines: &[(symbols::SymbolHandle, MachineId)],
    plans: super::UnitPlans<'_>,
    type_ids: &[(
        typed_trees::type_identity::TypeIdentityId,
        semantic_vocabulary::StructuralTypeId,
    )],
) -> Result<FoldedConstantClosure, LoweringError> {
    let typed = &checked.typed;
    let closure = &typed.constant_recipe_closure;
    let layout_requirements = lower_layout_requirements(checked, plans, type_ids, machines)?;
    if closure.receipts.is_empty() {
        return Ok(FoldedConstantClosure {
            layout_requirements,
            ..Default::default()
        });
    }
    if closure.target_scope.is_empty() {
        return Err(LoweringError::Unsupported(
            "folded constants lost their primary target closure",
        ));
    }
    let mut dependencies = Vec::new();
    for receipt in &closure.receipts {
        let (requirement, candidate) = if receipt.provider_machine.is_valid() {
            let mut matching = sources.iter().filter(|source| {
                source.boundary == receipt.requirement
                    && source.provider == receipt.provider_declaration
                    && source.candidate == receipt.provider_machine
            });
            let source = matching.next().ok_or(LoweringError::Unsupported(
                "folded constant lost its exact selected provider candidate",
            ))?;
            if matching.next().is_some() {
                return Err(LoweringError::Unsupported(
                    "folded constant provider candidate is ambiguous",
                ));
            }
            let machine = lookup_machine_id(machines, receipt.provider_machine)?;
            let declaration = crate::machine_lowering::declarations::nominal(
                identities,
                checked,
                exact_toolchain_sources,
                receipt.provider_declaration,
            )
            .map_err(LoweringError::Unsupported)?;
            let identity = typed.callable_identity_text(source.requirement_identity);
            let mut matching = candidates.iter().filter(|candidate| {
                candidate.candidate == machine
                    && candidate.provider_declaration == declaration
                    && identities.text(candidate.requirement_identity) == Some(identity.as_str())
            });
            let row = matching.next().ok_or(LoweringError::Unsupported(
                "folded constant has no retained canonical provider association",
            ))?;
            if matching.next().is_some() {
                return Err(LoweringError::Unsupported(
                    "folded constant canonical association is ambiguous",
                ));
            }
            (
                row.source_requirement_identity,
                Some(FoldedConstantCandidate {
                    boundary: row.boundary,
                    machine: row.candidate,
                    declaration: row.provider_declaration.clone(),
                }),
            )
        } else {
            if receipt.provider_declaration.is_valid() {
                return Err(LoweringError::Unsupported(
                    "intrinsic fold carries a checked provider declaration without its body",
                ));
            }
            let identity = if let Some((owner, signature)) =
                typed.trait_machine_signature(receipt.requirement)
            {
                typed.trait_requirement_callable_identity(owner, signature)
            } else if let Some(operator) = typed.operator_definition_by_symbol(receipt.requirement)
            {
                typed.boundary_operator_requirement_callable_identity(operator)
            } else if let Some(machine) = typed.machine_by_symbol(receipt.requirement) {
                typed.machine_callable_identity(machine)
            } else {
                return Err(LoweringError::Unsupported(
                    "intrinsic fold lost its exact requirement declaration",
                ));
            };
            (
                identities.intern(&typed.callable_identity_text(identity)),
                None,
            )
        };
        dependencies.push(FoldedConstantDependency {
            target: receipt.target_scope.clone(),
            requirement,
            plan: receipt.plan_identity,
            candidate,
        });
    }
    dependencies.sort();
    dependencies.dedup();
    Ok(FoldedConstantClosure {
        target: Some(closure.target_scope.clone()),
        dependencies,
        layout_requirements,
    })
}

fn lower_layout_requirements(
    checked: &CheckedTrees,
    plans: super::UnitPlans<'_>,
    type_ids: &[(
        typed_trees::type_identity::TypeIdentityId,
        semantic_vocabulary::StructuralTypeId,
    )],
    machines: &[(symbols::SymbolHandle, MachineId)],
) -> Result<Vec<FoldedLayoutRequirement>, LoweringError> {
    let mut requirements = Vec::new();
    for requirement in &checked.facts.constant_layout_requirements {
        let owner = match requirement.owner {
            checked_trees::CheckedLayoutOwner::Machine(machine) => {
                if !machines.iter().any(|(source, _)| *source == machine)
                    && requirement.alternatives.iter().all(|alternative| {
                        checked
                            .facts
                            .constant_layout_requirements
                            .iter()
                            .any(|carrier| {
                                carrier.proof_owner == requirement.proof_owner
                                    && !matches!(
                                        carrier.owner,
                                        checked_trees::CheckedLayoutOwner::Machine(_)
                                    )
                                    && carrier.alternatives.contains(alternative)
                            })
                    })
                {
                    // Static policy execution is erased. Its materialized
                    // result below retains the same judgment's condition.
                    continue;
                }
                FoldedLayoutOwner::Machine(lookup_machine_id(machines, machine)?)
            }
            checked_trees::CheckedLayoutOwner::Structural { machine, identity } => {
                FoldedLayoutOwner::Storage {
                    machine: lookup_machine_id(machines, machine)?,
                    carrier: crate::terminal_identities::lookup_type_id(type_ids, &identity)?,
                }
            }
            checked_trees::CheckedLayoutOwner::Carrier(application) => {
                let mut matches = Vec::new();
                for (identity, carrier) in type_ids {
                    let declaration = super::catalog::selected_declaration(plans, *identity)?;
                    if matches!(&declaration.shape,
                        checked_trees::CheckedUnitStructuralTypeShape::Record {
                            storage: checked_trees::CheckedUnitRecordStorage::TargetLayout { application: bound, .. }, ..
                        } if *bound == application)
                    {
                        matches.push(*carrier);
                    }
                }
                let [carrier] = matches.as_slice() else {
                    return Err(LoweringError::Unsupported(
                        "folded layout proof lost its exact consuming application",
                    ));
                };
                FoldedLayoutOwner::Carrier(*carrier)
            }
        };
        let mut alternatives = Vec::new();
        for alternative in &requirement.alternatives {
            let mut observations = Vec::new();
            for observation in &alternative.observations {
                let mut subjects = checked.facts.layout_query_subjects.iter().filter(|query| {
                    query.expression == observation.expression && query.owner == observation.owner
                });
                let query = subjects.next().ok_or(LoweringError::Unsupported(
                    "folded observation lost its checked query subject",
                ))?;
                if subjects.next().is_some() {
                    return Err(LoweringError::Unsupported(
                        "folded observation has ambiguous checked query subjects",
                    ));
                }
                let carrier = match query.carrier {
                    checked_trees::CheckedLayoutQueryCarrier::Unit => {
                        terminal_psi::LayoutObservationCarrier::Unit
                    }
                    checked_trees::CheckedLayoutQueryCarrier::Scalar(scalar) => {
                        terminal_psi::LayoutObservationCarrier::Scalar(scalar)
                    }
                    checked_trees::CheckedLayoutQueryCarrier::Structural(identity) => {
                        terminal_psi::LayoutObservationCarrier::Structural(
                            crate::terminal_identities::lookup_type_id(type_ids, &identity)?,
                        )
                    }
                };
                let kind = match query.kind {
                    language_core::data_layout_queries::DataLayoutQueryKind::Size => {
                        terminal_psi::LayoutObservationKind::Size
                    }
                    language_core::data_layout_queries::DataLayoutQueryKind::Alignment => {
                        terminal_psi::LayoutObservationKind::Alignment
                    }
                };
                observations.push(FoldedLayoutObservation {
                    kind,
                    carrier,
                    value: observation.value,
                });
            }
            observations.sort();
            observations.dedup();
            alternatives.push(FoldedLayoutAlternative {
                target: alternative.target.as_str().to_owned(),
                observations,
            });
        }
        alternatives.sort();
        alternatives.dedup();
        requirements.push(FoldedLayoutRequirement {
            owner,
            alternatives,
        });
    }
    requirements.sort();
    requirements.dedup();
    Ok(requirements)
}

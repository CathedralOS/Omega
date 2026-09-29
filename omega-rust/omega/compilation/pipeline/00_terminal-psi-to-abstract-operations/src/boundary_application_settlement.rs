//! Settle exact source-free boundary demands and their selected realizations.
//!
//! The sealed Psi scope binds the occurrence roster to the canonical artifact.
//! This Omega-side operation assigns the checked-body or intrinsic realization,
//! replays provider provenance, and retains opaque ABI evidence for the same edges.
//! Inputs borrow source-side selection evidence from the current checked build
//! route; they are not a replacement for canonical native artifact admission.

use checked_trees::CheckedTrees;
use diagnostics::Diagnostic;
use typed_trees::type_identity::ExactOwnerTypeIdentityRequest;

#[cfg(test)]
mod tests;

/// Evidence needed to settle boundary applications for one selected target.
/// No compiler coordinator or mutable checking context is retained.
pub struct BoundaryApplicationSettlement<'input> {
    pub checked: &'input CheckedTrees,
    pub selected_provider_plans: &'input effects::SelectedProviderPlanFacts,
    pub selected_provider_provenance:
        &'input [provider_planning::SelectedProviderReviewProvenance],
    pub exact_toolchain_sources: &'input [(source::SourceId, [u8; 32])],
    pub target_profile: Option<target::TargetProfile>,
    pub console_exit_binding: Option<&'input selected_dispatch::ResolvedAcceptedSemanticBinding>,
    pub process_exit_binding: Option<&'input selected_dispatch::ResolvedAcceptedSemanticBinding>,
    pub calling_plan_realizations:
        &'input [provider_planning::calling_policy_plans::BoundaryCallingPlanRealization],
}

impl BoundaryApplicationSettlement<'_> {
    /// Rejoin the artifact-bound occurrence roster to exact selected realizations.
    /// Demand, realization and opaque-edge errors retain that evaluation order.
    pub fn settle(
        &self,
        artifact: &terminal_codec::CanonicalTerminalArtifact,
        checked_scope: &lowered_psi_to_terminal_psi::CheckedBoundaryOperatorApplicationScope,
    ) -> Result<boundary_applications::TerminalBoundaryApplicationCoverage, Vec<Diagnostic>> {
        checked_scope
            .validate_for_artifact(artifact)
            .map_err(|message| vec![Diagnostic::error(message)])?;
        let demands = project_terminal_boundary_application_demands(self, artifact, checked_scope)?;
        let realizations =
            project_terminal_boundary_application_realizations(self, checked_scope, &demands)?;
        let opaque_applications = project_opaque_applications(self.calling_plan_realizations)
            .map_err(|message| vec![Diagnostic::error(message)])?;
        boundary_applications::TerminalBoundaryApplicationCoverage::new(demands, realizations)
            .map_err(|message| vec![Diagnostic::error(message)])
            .map(|coverage| coverage.with_opaque_applications(opaque_applications))
    }
}

fn project_terminal_boundary_application_realizations(
    settlement: &BoundaryApplicationSettlement<'_>,
    checked_scope: &lowered_psi_to_terminal_psi::CheckedBoundaryOperatorApplicationScope,
    demands: &boundary_applications::TerminalBoundaryApplicationDemands,
) -> Result<boundary_applications::TerminalBoundaryApplicationRealizations, Vec<Diagnostic>> {
    let nongeneric =
        selected_dispatch::derive_checked_nongeneric_operator_application_realizations(
            settlement.checked,
            settlement.selected_provider_plans,
        )?;
    let specialized =
        selected_dispatch::derive_checked_specialized_operator_application_realizations(
            settlement.checked,
            settlement.selected_provider_plans,
        )?;
    let mut rows = Vec::with_capacity(demands.rows().len());
    for (demand, occurrence) in demands.rows().iter().zip(checked_scope.occurrences()) {
        let application = &checked_scope.applications()[occurrence.application_index()];
        let matching_nongeneric = nongeneric
            .iter()
            .filter(|row| {
                row.application_site == application.site
                    && row.requirement_operator == application.requirement_symbol
            })
            .collect::<Vec<_>>();
        let matching_specialized = specialized
            .iter()
            .filter(|row| {
                row.application_site == application.site
                    && row.requirement_operator == application.requirement_symbol
            })
            .collect::<Vec<_>>();
        let (selected_plan_digest, realization) = match (
            matching_nongeneric.as_slice(),
            matching_specialized.as_slice(),
        ) {
            ([row], []) => (
                *row.provider_plan_commitment.as_bytes(),
                boundary_applications::BoundaryApplicationRealization::NongenericCheckedBody {
                    realization_machine: canonical_boundary_nominal_identity(
                        settlement,
                        row.realization_machine,
                        "nongeneric realization machine",
                    )?,
                    realization_state: canonical_boundary_nominal_identity(
                        settlement,
                        row.realization_state,
                        "nongeneric realization state",
                    )?,
                    realization_contract_commitment: row.realization_contract_commitment.as_bytes(),
                },
            ),
            ([], [row]) => (
                *row.provider_plan_commitment.as_bytes(),
                boundary_applications::BoundaryApplicationRealization::SpecializedCheckedBody {
                    realization_template: canonical_boundary_nominal_identity(
                        settlement,
                        row.realization_template,
                        "specialized realization template",
                    )?,
                    realization_machine: canonical_boundary_nominal_identity(
                        settlement,
                        row.realization_machine,
                        "specialized realization machine",
                    )?,
                    realization_state: canonical_boundary_nominal_identity(
                        settlement,
                        row.realization_state,
                        "specialized realization state",
                    )?,
                    specialization_commitment: row.specialization_commitment.as_bytes(),
                    realization_contract_commitment: row.realization_contract_commitment.as_bytes(),
                },
            ),
            ([], []) => {
                project_compiler_intrinsic_application_realization(settlement, application)?
            }
            _ => {
                return Err(vec![Diagnostic::error(
                    "Terminal boundary application rejoins multiple checked-body realizations",
                )]);
            }
        };
        rows.push(
            boundary_applications::BoundaryApplicationRealizationCompanion::new(
                demand.terminal_operation(),
                selected_plan_digest,
                realization,
            )
            .map_err(|message| vec![Diagnostic::error(message)])?,
        );
    }
    boundary_applications::TerminalBoundaryApplicationRealizations::new(demands, rows)
        .map_err(|message| vec![Diagnostic::error(message)])
}

fn project_compiler_intrinsic_application_realization(
    settlement: &BoundaryApplicationSettlement<'_>,
    application: &checked_trees::CheckedBoundaryOperatorApplicationDemand,
) -> Result<
    (
        [u8; 32],
        boundary_applications::BoundaryApplicationRealization,
    ),
    Vec<Diagnostic>,
> {
    let (expression, origin) = match application.site {
        checked_trees::CheckedBoundaryOperatorApplicationUseSite::Expression {
            expression,
            origin,
        }
        | checked_trees::CheckedBoundaryOperatorApplicationUseSite::MatchEquality {
            expression,
            origin,
            ..
        } => (expression, origin),
        _ => {
            return Err(vec![Diagnostic::error(
                "Terminal boundary application without a value occurrence has no supported realization role",
            )]);
        }
    };
    let uses =
        settlement
            .checked
            .facts
            .operators
            .named_uses
            .iter()
            .filter_map(|(_, operator_use)| {
                (matches!(
                    application.site,
                    checked_trees::CheckedBoundaryOperatorApplicationUseSite::Expression { .. }
                ) && operator_use.expression == expression
                    && operator_use.origin == origin
                    && operator_use.selected_operator_symbol == application.requirement_symbol)
                    .then_some(operator_use.origin)
            })
            .chain(settlement.checked.facts.operators.uses.iter().filter_map(
                |(_, operator_use)| {
                    (operator_use.application_site() == application.site
                        && operator_use.selected_operator_symbol == application.requirement_symbol
                        && operator_use.status
                            == checked_trees::CheckedOperatorResolutionStatus::Resolved)
                        .then_some(operator_use.origin)
                },
            ))
            .chain(
                settlement
                    .checked
                    .facts
                    .operators
                    .named_requirement_uses
                    .iter()
                    .filter_map(|(_, requirement_use)| {
                        (matches!(
                        application.site,
                        checked_trees::CheckedBoundaryOperatorApplicationUseSite::Expression { .. }
                    ) && requirement_use.expression == expression
                        && requirement_use.origin == origin
                        && requirement_use.requirement_symbol == application.requirement_symbol)
                        .then_some(requirement_use.origin)
                    }),
            )
            .collect::<Vec<_>>();
    let [use_origin] = uses.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "Terminal intrinsic application retains {} exact selected uses; expected one",
            uses.len(),
        ))]);
    };
    let plans = settlement.selected_provider_plans.plans();
    let provenance = settlement.selected_provider_provenance;
    if plans.len() != provenance.len() {
        return Err(vec![Diagnostic::error(
            "Terminal intrinsic application has misaligned selected-plan provenance",
        )]);
    }
    // This target's selected plan joins the use by requirement identity.
    let Some((plan_index, plan)) = provider_planning::selected_use_plan(
        settlement.checked,
        plans,
        application.requirement_symbol,
        *use_origin,
    ) else {
        return Err(vec![Diagnostic::error(
            "Terminal intrinsic application has no exact selected plan",
        )]);
    };
    let retained = &provenance[plan_index];
    if retained.plan != *plan {
        return Err(vec![Diagnostic::error(
            "Terminal intrinsic application's selected plan was substituted after review",
        )]);
    }
    if retained.provider.row_requirements.len() != plan.rows.len()
        || retained.provider.row_realizations.len() != plan.rows.len()
        || retained.row_compiler_intrinsic_executions.len() != plan.rows.len()
    {
        return Err(vec![Diagnostic::error(
            "Terminal intrinsic application has incomplete row provenance",
        )]);
    }
    let matching_rows = plan
        .rows
        .iter()
        .zip(&retained.provider.row_requirements)
        .zip(&retained.provider.row_realizations)
        .zip(&retained.row_compiler_intrinsic_executions)
        .filter(|(((_, requirement), _), _)| **requirement == application.requirement_symbol)
        .collect::<Vec<_>>();
    let [(((row, requirement), realization), retained_execution)] = matching_rows.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "Terminal intrinsic application rejoins {} selected plan rows; expected one",
            matching_rows.len(),
        ))]);
    };
    if !matches!(
        row.binding,
        effects::provider_plan::ProviderBinding::CompilerIntrinsic { .. }
    ) {
        return Err(vec![Diagnostic::error(
            "Terminal boundary application has no checked-body or compiler-intrinsic realization",
        )]);
    }
    let derived = selected_dispatch::derive_selected_compiler_intrinsic_execution_identity_for_row(
        settlement.checked,
        plan,
        retained.provider.schema,
        row,
        **requirement,
        **realization,
        settlement
            .target_profile
            .map(target::TargetProfile::target_name),
        settlement.console_exit_binding,
        settlement.process_exit_binding,
    )
    .map_err(|diagnostic| vec![diagnostic])?;
    let execution = match (derived, **retained_execution) {
        (
            Some(selected_dispatch::SelectedCompilerIntrinsicExecutionIdentity::Closed(derived)),
            Some(retained),
        ) if derived == retained => derived,
        _ => {
            return Err(vec![Diagnostic::error(
                "Terminal intrinsic application does not retain one independently rederived closed execution",
            )]);
        }
    };
    Ok((
        *plan.identity_digest().as_bytes(),
        boundary_applications::BoundaryApplicationRealization::ExactCompilerIntrinsic { execution },
    ))
}

fn project_terminal_boundary_application_demands(
    settlement: &BoundaryApplicationSettlement<'_>,
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    checked_scope: &lowered_psi_to_terminal_psi::CheckedBoundaryOperatorApplicationScope,
) -> Result<boundary_applications::TerminalBoundaryApplicationDemands, Vec<Diagnostic>> {
    let mut rows = Vec::with_capacity(checked_scope.occurrences().len());
    for occurrence in checked_scope.occurrences() {
        let application = checked_scope
            .applications()
            .get(occurrence.application_index())
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "Terminal boundary application occurrence names an absent checked demand",
                )]
            })?;
        // The demand names a boundary operator or a top-level boundary
        // requirement; the coverage row keys on the same declaration identity
        // and overload coordinate for either species.
        let requirement_view = provider_planning::IntrinsicRequirement::by_symbol(
            &settlement.checked.typed,
            application.requirement_symbol,
        )
        .filter(|view| {
            view.kind != provider_planning::IntrinsicRequirementKind::Operator
                || view.as_operator().is_some_and(|operator| operator.is_boundary)
        })
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "Terminal boundary application demand names no boundary operator or top-level boundary requirement",
            )]
        })?;
        let declaration = canonical_boundary_nominal_identity(
            settlement,
            application.requirement_symbol,
            "operator requirement",
        )?;
        let overload = requirement_view.requirement_identity.clone();
        let requirement =
            boundary_applications::BoundaryOperatorRequirement::new(declaration, overload)
                .map_err(|message| vec![Diagnostic::error(message)])?;
        let projected_application = project_boundary_application(settlement, application)?;
        rows.push(
            boundary_applications::TerminalBoundaryApplicationDemand::new(
                occurrence.terminal_operation(),
                requirement,
                projected_application,
            ),
        );
    }
    boundary_applications::TerminalBoundaryApplicationDemands::new(
        artifact.manifest().semantic(),
        rows,
    )
    .map_err(|message| vec![Diagnostic::error(message)])
}

fn project_boundary_application(
    settlement: &BoundaryApplicationSettlement<'_>,
    application: &checked_trees::CheckedBoundaryOperatorApplicationDemand,
) -> Result<boundary_applications::BoundaryApplication, Vec<Diagnostic>> {
    if application.arguments.is_empty() {
        return Ok(boundary_applications::BoundaryApplication::Empty);
    }
    let mut arguments = Vec::with_capacity(application.arguments.len());
    for (ordinal, argument) in application.arguments.iter().enumerate() {
        let expected_ordinal = u32::try_from(ordinal).map_err(|_| {
            vec![Diagnostic::error(
                "Terminal boundary application exceeds the supported ordinal range",
            )]
        })?;
        match argument {
            checked_trees::CheckedBoundaryOperatorApplicationArgument::Type {
                binder_owner,
                binder_ordinal,
                type_reference,
                ..
            } if *binder_owner == application.requirement_symbol
                && *binder_ordinal == expected_ordinal =>
            {
                arguments.push(
                    boundary_applications::BoundaryApplicationArgument::type_argument(
                        *binder_ordinal,
                        canonical_boundary_type_identity(settlement, *type_reference)?,
                    ),
                );
            }
            checked_trees::CheckedBoundaryOperatorApplicationArgument::Const {
                binder_owner,
                binder_ordinal,
                declared_carrier,
                value,
                ..
            } if *binder_owner == application.requirement_symbol
                && *binder_ordinal == expected_ordinal =>
            {
                validation::validate_exact_const_value_encoding(
                    &settlement.checked.typed,
                    *declared_carrier,
                    value.encoding.as_str(),
                )
                .map_err(|reason| {
                    vec![Diagnostic::error(format!(
                        "Terminal boundary const application has invalid canonical encoding: {reason}",
                    ))]
                })?;
                arguments.push(
                    boundary_applications::BoundaryApplicationArgument::const_argument(
                        *binder_ordinal,
                        canonical_boundary_type_identity(settlement, *declared_carrier)?,
                        value.type_name.clone(),
                        value.encoding.clone(),
                    )
                    .map_err(|message| vec![Diagnostic::error(message)])?,
                );
            }
            _ => {
                return Err(vec![Diagnostic::error(
                    "Terminal boundary application does not rejoin its binder owner, category, and ordinal",
                )]);
            }
        }
    }
    boundary_applications::BoundaryApplication::exact(arguments)
        .map_err(|message| vec![Diagnostic::error(message)])
}

fn canonical_boundary_nominal_identity(
    settlement: &BoundaryApplicationSettlement<'_>,
    symbol: symbols::SymbolHandle,
    role: &str,
) -> Result<boundary_applications::BoundaryNominalIdentity, Vec<Diagnostic>> {
    let identity = settlement
        .checked
        .exact_owner_nominal_identity(symbol, settlement.exact_toolchain_sources)
        .ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "Terminal boundary {role} has no exact package or toolchain owner",
            ))]
        })?;
    boundary_applications::BoundaryNominalIdentity::new(identity.into_string())
        .map_err(|message| vec![Diagnostic::error(message)])
}

fn canonical_boundary_type_identity(
    settlement: &BoundaryApplicationSettlement<'_>,
    type_reference: typed_trees::types::TypeReferenceHandle,
) -> Result<boundary_applications::BoundaryTypeIdentity, Vec<Diagnostic>> {
    let identity = settlement
        .checked
        .exact_owner_type_identity(ExactOwnerTypeIdentityRequest {
            type_reference,
            binders: &[],
            substitutions: &[],
            exact_toolchain_sources: settlement.exact_toolchain_sources,
        })
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "Terminal boundary application type has no exact package or toolchain owner",
            )]
        })?;
    boundary_applications::BoundaryTypeIdentity::new(identity.into_string())
        .map_err(|message| vec![Diagnostic::error(message)])
}

/// Retain one strong application commitment per requirement edge and shape
/// coordinate. Producer and consumer artifacts compare this source-free set;
/// arena identities are deliberately absent.
fn project_opaque_applications(
    calling_plan_realizations: &[provider_planning::calling_policy_plans::BoundaryCallingPlanRealization],
) -> Result<boundary_applications::BoundaryOpaqueRepresentationApplications, &'static str> {
    let rows = calling_plan_realizations
        .iter()
        .flat_map(|realization| {
            let signature = realization.materialized_signature();
            signature.opaque_representation_uses().iter().map(|use_| {
                boundary_applications::BoundaryOpaqueRepresentationApplication {
                    requirement_identity: signature.owner_requirement_identity().to_owned(),
                    shape_root: use_.shape_root(),
                    application_report_fingerprint: use_.application_report_fingerprint(),
                    selected_application_commitment: use_.selected_application_commitment(),
                }
            })
        })
        .collect();
    boundary_applications::BoundaryOpaqueRepresentationApplications::new(rows)
}

/// Retain native coverage through the same exact-scope replay as a Terminal
/// proposal. An absent source scope permits only absent coverage; only a
/// sealed empty scope permits constructing empty coverage without a supplied
/// companion. A nonempty scope always requires its produced coverage.
pub fn retain_terminal_boundary_application_coverage(
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    checked_scope: Option<&lowered_psi_to_terminal_psi::CheckedBoundaryOperatorApplicationScope>,
    supplied: Option<&boundary_applications::TerminalBoundaryApplicationCoverage>,
) -> Result<Option<boundary_applications::TerminalBoundaryApplicationCoverage>, &'static str> {
    use boundary_applications::{
        TerminalBoundaryApplicationCoverage, TerminalBoundaryApplicationDemands,
        TerminalBoundaryApplicationRealizations,
    };

    let Some(checked_scope) = checked_scope else {
        if supplied.is_some() {
            return Err("D29 coverage requires the exact checked source-to-Terminal scope");
        }
        return Ok(None);
    };
    let coverage = match supplied {
        Some(coverage) => coverage.clone(),
        None if checked_scope.is_empty() => {
            let demands = TerminalBoundaryApplicationDemands::new(
                artifact.manifest().semantic(),
                Vec::new(),
            )?;
            let realizations = TerminalBoundaryApplicationRealizations::new(&demands, Vec::new())?;
            TerminalBoundaryApplicationCoverage::new(demands, realizations)?
        }
        None => {
            return Err("a nonempty checked D29 scope requires exact compiler-produced coverage");
        }
    };
    validate_terminal_boundary_application_coverage(artifact, checked_scope, &coverage)?;
    Ok(Some(coverage))
}

/// Replay retained source-free demands against their sealed artifact-bound scope.
/// A public demand roster cannot substitute its own occurrence count or identities.
pub fn validate_terminal_boundary_application_coverage(
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    checked_scope: &lowered_psi_to_terminal_psi::CheckedBoundaryOperatorApplicationScope,
    coverage: &boundary_applications::TerminalBoundaryApplicationCoverage,
) -> Result<(), &'static str> {
    checked_scope.validate_for_artifact(artifact)?;
    coverage.validate_for_terminal(artifact.manifest().semantic())?;
    if coverage.demands().rows().len() != checked_scope.occurrences().len()
        || !coverage
            .demands()
            .rows()
            .iter()
            .zip(checked_scope.occurrences())
            .all(|(demand, occurrence)| {
                demand.terminal_operation() == occurrence.terminal_operation()
            })
    {
        return Err("source-free D29 coverage differs from exact checked occurrence custody");
    }

    Ok(())
}

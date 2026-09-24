//! Rejoin every Terminal boundary application to its checked realization:
//! a nongeneric or specialized checked body, or the exact compiler intrinsic.

use assembled_syntax_to_checked_compilation::CheckedCompilation;
use diagnostics::Diagnostic;
use typed_trees::type_identity::ExactOwnerTypeIdentityRequest;

pub(crate) fn project_terminal_boundary_application_coverage(
    checked: &CheckedCompilation,
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    checked_scope: &lowered_psi_to_terminal_psi::CheckedBoundaryOperatorApplicationScope,
) -> Result<boundary_applications::TerminalBoundaryApplicationCoverage, Vec<Diagnostic>> {
    let demands = project_terminal_boundary_application_demands(checked, artifact, checked_scope)?;
    let realizations =
        project_terminal_boundary_application_realizations(checked, checked_scope, &demands)?;
    // Selected opaque applications belong to the same boundary-edge custody:
    // the checked compilation's canonical commitment set is bound into the
    // coverage so downstream artifacts compare it at each actual edge.
    let opaque_applications = checked
        .boundary_opaque_applications()
        .map_err(|message| vec![Diagnostic::error(message)])?;
    boundary_applications::TerminalBoundaryApplicationCoverage::new(demands, realizations)
        .map_err(|message| vec![Diagnostic::error(message)])
        .map(|coverage| coverage.with_opaque_applications(opaque_applications))
}

fn project_terminal_boundary_application_realizations(
    checked: &CheckedCompilation,
    checked_scope: &lowered_psi_to_terminal_psi::CheckedBoundaryOperatorApplicationScope,
    demands: &boundary_applications::TerminalBoundaryApplicationDemands,
) -> Result<boundary_applications::TerminalBoundaryApplicationRealizations, Vec<Diagnostic>> {
    let nongeneric =
        selected_dispatch::derive_checked_nongeneric_operator_application_realizations(
            checked,
            checked.selected_provider_plans(),
        )?;
    let specialized =
        selected_dispatch::derive_checked_specialized_operator_application_realizations(
            checked,
            checked.selected_provider_plans(),
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
                        checked,
                        row.realization_machine,
                        "nongeneric realization machine",
                    )?,
                    realization_state: canonical_boundary_nominal_identity(
                        checked,
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
                        checked,
                        row.realization_template,
                        "specialized realization template",
                    )?,
                    realization_machine: canonical_boundary_nominal_identity(
                        checked,
                        row.realization_machine,
                        "specialized realization machine",
                    )?,
                    realization_state: canonical_boundary_nominal_identity(
                        checked,
                        row.realization_state,
                        "specialized realization state",
                    )?,
                    specialization_commitment: row.specialization_commitment.as_bytes(),
                    realization_contract_commitment: row.realization_contract_commitment.as_bytes(),
                },
            ),
            ([], []) => project_compiler_intrinsic_application_realization(checked, application)?,
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
    checked: &CheckedCompilation,
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
    let uses = checked
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
                .then_some((
                    operator_use.provider_plan_report_fingerprint,
                    operator_use.provider_plan_commitment,
                ))
        })
        .chain(
            checked
                .facts
                .operators
                .uses
                .iter()
                .filter_map(|(_, operator_use)| {
                    (operator_use.application_site() == application.site
                        && operator_use.selected_operator_symbol == application.requirement_symbol
                        && operator_use.status
                            == checked_trees::CheckedOperatorResolutionStatus::Resolved)
                        .then_some((
                            operator_use.provider_plan_report_fingerprint,
                            operator_use.provider_plan_commitment,
                        ))
                }),
        )
        .chain(
            checked
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
                        .then_some((
                            requirement_use.provider_plan_report_fingerprint,
                            requirement_use.provider_plan_commitment,
                        ))
                }),
        )
        .collect::<Vec<_>>();
    let [(plan_report, plan_commitment)] = uses.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "Terminal intrinsic application retains {} exact selected uses; expected one",
            uses.len(),
        ))]);
    };
    let plans = checked.selected_provider_plans().plans();
    let provenance = checked.selected_provider_provenance();
    if plans.len() != provenance.len() {
        return Err(vec![Diagnostic::error(
            "Terminal intrinsic application has misaligned selected-plan provenance",
        )]);
    }
    let matching_plans = plans
        .iter()
        .zip(provenance)
        .filter(|(plan, retained)| {
            retained.plan == **plan
                && plan.report_fingerprint() == *plan_report
                && plan.identity_digest().as_bytes() == plan_commitment.as_bytes()
        })
        .collect::<Vec<_>>();
    let [(plan, retained)] = matching_plans.as_slice() else {
        return Err(vec![Diagnostic::error(format!(
            "Terminal intrinsic application rejoins {} selected plans; expected one",
            matching_plans.len(),
        ))]);
    };
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
    let derived =
        selected_dispatch::derive_selected_compiler_intrinsic_execution_identity_for_row_with_resolved_binding(
            checked,
            plan,
            retained.provider.schema,
            row,
            **requirement,
            **realization,
            checked
                .selected_target_profile()
                .map(target::TargetProfile::target_name),
            checked.resolved_semantic_binding(
                package_compilation::AcceptedSemanticBindingRole::ConsoleExitProcessI32,
            ),
            checked.resolved_semantic_binding(
                package_compilation::AcceptedSemanticBindingRole::ProcessExitExitProcessI32,
            ),
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
        *plan_commitment.as_bytes(),
        boundary_applications::BoundaryApplicationRealization::ExactCompilerIntrinsic { execution },
    ))
}

fn project_terminal_boundary_application_demands(
    checked: &CheckedCompilation,
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
            &checked.typed,
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
            checked,
            application.requirement_symbol,
            "operator requirement",
        )?;
        let overload = requirement_view.requirement_identity.clone();
        let requirement =
            boundary_applications::BoundaryOperatorRequirement::new(declaration, overload)
                .map_err(|message| vec![Diagnostic::error(message)])?;
        let projected_application = project_boundary_application(checked, application)?;
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
    checked: &CheckedCompilation,
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
                        canonical_boundary_type_identity(checked, *type_reference)?,
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
                    &checked.typed,
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
                        canonical_boundary_type_identity(checked, *declared_carrier)?,
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
    checked: &CheckedCompilation,
    symbol: symbols::SymbolHandle,
    role: &str,
) -> Result<boundary_applications::BoundaryNominalIdentity, Vec<Diagnostic>> {
    let identity = checked
        .exact_owner_nominal_identity(symbol, checked.exact_toolchain_sources())
        .ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "Terminal boundary {role} has no exact package or toolchain owner",
            ))]
        })?;
    boundary_applications::BoundaryNominalIdentity::new(identity.into_string())
        .map_err(|message| vec![Diagnostic::error(message)])
}

fn canonical_boundary_type_identity(
    checked: &CheckedCompilation,
    type_reference: typed_trees::types::TypeReferenceHandle,
) -> Result<boundary_applications::BoundaryTypeIdentity, Vec<Diagnostic>> {
    let identity = checked
        .exact_owner_type_identity(ExactOwnerTypeIdentityRequest {
            type_reference,
            binders: &[],
            substitutions: &[],
            exact_toolchain_sources: checked.exact_toolchain_sources(),
        })
        .ok_or_else(|| {
            vec![Diagnostic::error(
                "Terminal boundary application type has no exact package or toolchain owner",
            )]
        })?;
    boundary_applications::BoundaryTypeIdentity::new(identity.into_string())
        .map_err(|message| vec![Diagnostic::error(message)])
}

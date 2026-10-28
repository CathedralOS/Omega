//! Select realizations from canonical Terminal demands and checked candidates.
//! Psi retains conformance and specialization facts; target Build rows select
//! them here. No checked application scope or source replay crosses this edge.

use diagnostics::Diagnostic;

/// Evidence needed to settle boundary applications for one selected target.
/// No compiler coordinator or mutable checking context is retained.
pub struct BoundaryApplicationSettlement<'input> {
    pub selected_provider_plans: &'input effects::SelectedProviderPlanFacts,
    pub selected_provider_provenance:
        Vec<crate::provider_admission::CompilerIntrinsicPlanProvenance<'input>>,
    pub calling_applications: Vec<calling_conventions::BoundaryCallingApplication>,
}

impl BoundaryApplicationSettlement<'_> {
    /// Rejoin the artifact-bound occurrence roster to exact selected realizations.
    /// Demand, realization and opaque-edge errors retain that evaluation order.
    pub fn settle(
        &self,
        artifact: &terminal_codec::CanonicalTerminalArtifact,
    ) -> Result<boundary_applications::TerminalBoundaryApplicationCoverage, Vec<Diagnostic>> {
        let demands = boundary_applications::TerminalBoundaryApplicationDemands::new(
            artifact.manifest().semantic(),
            artifact
                .semantic_module()
                .boundary_application_demands
                .clone(),
        )
        .map_err(|message| vec![Diagnostic::error(message)])?;
        let realizations = project_terminal_boundary_application_realizations(
            self,
            artifact.semantic_module(),
            &demands,
        )?;
        let opaque_applications =
            project_opaque_applications(&self.calling_applications, artifact.semantic_module())
                .map_err(|message| vec![Diagnostic::error(message)])?;
        boundary_applications::TerminalBoundaryApplicationCoverage::new(demands, realizations)
            .map_err(|message| vec![Diagnostic::error(message)])
            .map(|coverage| coverage.with_opaque_applications(opaque_applications))
    }
}

fn project_terminal_boundary_application_realizations(
    settlement: &BoundaryApplicationSettlement<'_>,
    module: &terminal_psi::TerminalModule,
    demands: &boundary_applications::TerminalBoundaryApplicationDemands,
) -> Result<boundary_applications::TerminalBoundaryApplicationRealizations, Vec<Diagnostic>> {
    use boundary_applications::BoundaryApplicationRealization as Realization;
    use effects::provider_plan::ProviderBinding;
    let error = |message| vec![Diagnostic::error(message)];
    let plans = settlement.selected_provider_plans.plans();
    let provenance = &settlement.selected_provider_provenance;
    if plans.len() != provenance.len() {
        return Err(error(
            "boundary applications have misaligned selected-plan provenance",
        ));
    }
    // Keep row identity and its already-checked execution together. Selection
    // never revisits checked expressions or rederives an intrinsic's meaning.
    let mut selected_rows = Vec::new();
    for (plan, retained) in plans.iter().zip(provenance) {
        if plan != retained.plan || plan.rows.len() != retained.row_executions.len() {
            return Err(error(
                "boundary applications have substituted or incomplete selected rows",
            ));
        }
        for (row, execution) in plan.rows.iter().zip(retained.row_executions) {
            selected_rows.push((plan, row, *execution));
        }
    }
    let mut rows = Vec::with_capacity(demands.rows().len());
    for demand in demands.rows() {
        let requirement = demand.requirement();
        let mut matching = selected_rows.iter().filter(|(plan, row, _)| {
            plan.schema.identity_text(plan.schema.trait_name) == requirement.overload()
                && plan.schema.trait_package_identity == requirement.package()
                && plan.identities.text(row.requirement_identity) == requirement.overload()
        });
        let (Some((plan, row, execution)), None) = (matching.next(), matching.next()) else {
            return Err(error(
                "Terminal boundary application has no unique exact selected row",
            ));
        };
        let realization = match &row.binding {
            ProviderBinding::CompilerIntrinsic { .. } => {
                let mut boundaries = module.boundary_machines.iter().filter(|boundary| {
                    module.identities.text(boundary.identity) == Some(requirement.overload())
                });
                let boundary = boundaries.next();
                if boundaries.next().is_some() {
                    return Err(error(
                        "selected intrinsic has ambiguous Terminal requirements",
                    ));
                }
                let execution = crate::provider_admission::selected_execution(
                    boundary.and_then(|boundary| boundary.numeric_intrinsic),
                    boundary.map_or(&[], |boundary| boundary.external_providers.as_slice()),
                    &module.identities,
                    plan,
                    row,
                    *execution,
                )
                .map_err(error)?;
                Realization::ExactCompilerIntrinsic {
                    execution: execution.ok_or_else(|| {
                        error("selected intrinsic has no canonical or admitted execution identity")
                    })?,
                }
            }
            ProviderBinding::CheckedAdapter {
                machine_identity,
                machine_package_identity,
            } => {
                if *machine_package_identity != plan.origin_package_identity {
                    return Err(error(
                        "selected boundary body belongs to a different package",
                    ));
                }
                let mut candidates =
                    module
                        .boundary_application_candidates
                        .iter()
                        .filter(|candidate| {
                            candidate.requirement == *requirement
                                && candidate.application == *demand.application()
                                && module.identities.text(candidate.selection_identity)
                                    == Some(plan.identity_text(*machine_identity))
                                && candidate.selection_package == *machine_package_identity
                                && candidate
                                    .target
                                    .as_ref()
                                    .is_none_or(|target| target == &plan.target)
                        });
                let (Some(candidate), None) = (candidates.next(), candidates.next()) else {
                    return Err(error(
                        "selected boundary body has no unique checked Terminal candidate",
                    ));
                };
                let body = &candidate.body;
                match &body.specialization {
                    Some(specialization) => Realization::SpecializedCheckedBody {
                        realization_template: specialization.template.clone(),
                        realization_machine: body.machine.clone(),
                        realization_state: body.state.clone(),
                        specialization_commitment: specialization.commitment,
                        realization_contract_commitment: body.contract,
                    },
                    None => Realization::NongenericCheckedBody {
                        realization_machine: body.machine.clone(),
                        realization_state: body.state.clone(),
                        realization_contract_commitment: body.contract,
                    },
                }
            }
            _ => {
                return Err(error(
                    "Terminal boundary application has no checked-body or compiler-intrinsic realization",
                ));
            }
        };
        rows.push(
            boundary_applications::BoundaryApplicationRealizationCompanion::new(
                demand.terminal_operation(),
                *plan.identity_digest().as_bytes(),
                realization,
            )
            .map_err(error)?,
        );
    }
    boundary_applications::TerminalBoundaryApplicationRealizations::new(demands, rows)
        .map_err(error)
}

/// Retain one strong application commitment per requirement edge and shape
/// coordinate. Producer and consumer artifacts compare this source-free set;
/// arena identities are deliberately absent.
fn project_opaque_applications(
    calling_applications: &[calling_conventions::BoundaryCallingApplication],
    module: &terminal_psi::TerminalModule,
) -> Result<boundary_applications::BoundaryOpaqueRepresentationApplications, &'static str> {
    let rows = calling_applications
        .iter()
        .flat_map(|application| {
            application.opaque_applications.iter().map(|use_| {
                module
                    .identities
                    .index_of(&application.requirement_identity)
                    .ok_or(
                        "boundary opaque application names a requirement absent from the module identity table",
                    )
                    .map(|requirement_identity| {
                        boundary_applications::BoundaryOpaqueRepresentationApplication {
                            requirement_identity,
                            shape_root: use_.shape_root,
                            application_report_fingerprint: use_.application_report_fingerprint,
                            selected_application_commitment: use_
                                .selected_application_commitment,
                        }
                    })
            })
        })
        .collect::<Result<_, _>>()?;
    boundary_applications::BoundaryOpaqueRepresentationApplications::new(rows)
}

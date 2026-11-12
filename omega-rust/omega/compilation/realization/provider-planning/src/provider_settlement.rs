/*
We settle which provider implementation each boundary API uses for the target,
record the authority that selected it, and verify attached component
descriptions for `Independent` provider selections.

`settle_checked_providers` attaches the evaluated `BoundaryCallingPlanRealization`
rows to the prepared product providers, independently verifies attached
component descriptions under the build's admission profile (`COMPONENT_DESCRIPTION_SCHEMA_V3`,
with each description's assumptions pending the consumer's lock acceptance), runs whole-map provider selection against the
canonical Terminal artifact, validates synchronous invocation cycles, extracts
normalized external binding rows, and validates assumed bindings and semantic
completion roles.
*/

/*
Recognizing an API as a filesystem or console service never invents an
implementation: we use only the implementations declared by the program and
selected by its build settings. While `settle_checked_providers` proves each
`CompositionMode::Independent` selection is joined to a verified component
description during checking, `verify_selected_compositions_are_realized`
rejects artifact emission for any settled `Independent` edge until the product
defines a symbolic import/export and installation carrier, rather than silently
falling back to a fused image.
*/

pub(crate) mod assumed_bindings;
pub(crate) mod completion;
pub(crate) mod semantic_roles;

use std::collections::BTreeSet;

use crate::calling_policy_plans::BoundaryCallingPlanRealization;
use crate::evaluated_via_bindings::EvaluatedViaBindingTable;
use crate::{CompositionMode, ProviderSelection, SelectedProviderReviewProvenance};
use component_description::{
    AcceptedComponentAssumptions, COMPONENT_DESCRIPTION_SCHEMA_V3, ComponentVerificationRequest,
    VerifiedComponent, verify_component,
};
use diagnostics::Diagnostic;
use effects::SelectedProviderPlanFacts;
use effects::provider_plan::ProviderPlan;
use package_compilation::{IndependentComponentDescription, PackageCompilationInputs};
use semantic_vocabulary::PackageKeyIdentity;

pub struct SettledTargetMachineDeclarations {
    pub provider_defaults: Vec<ProviderSelection>,
    pub origins: Vec<crate::SelectedTargetMachineOrigin>,
}

pub struct CheckedProviderSelection {
    pub callback_roots: Result<terminal_psi::PreparedTerminalRoots, Vec<Diagnostic>>,
    pub settlement: ProviderSettlement<crate::DeferredProviderRows>,
    pub selected_provider_plan_facts: SelectedProviderPlanFacts,
    pub selected_provider_provenance: Vec<SelectedProviderReviewProvenance>,
    pub selected_declarations:
        Vec<Result<language_semantics::provider_declarations::ProviderDeclarations, Diagnostic>>,
    pub native_provider_sources:
        Result<std::sync::Arc<backend_plan::NativeProviderSources>, Vec<Diagnostic>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderSettlement<Rows = crate::PreparedProviderRows> {
    pub calling_program: std::sync::Arc<crate::calling_policy_plans::CallingProgram>,
    pub selected_provider_publication:
        std::sync::Arc<provider_selection::SelectedTargetProviderPublication>,
    pub boundary_provider_demands: Vec<ProviderPlan>,
    pub provider_declarations: std::sync::Arc<[backend_plan::TerminalTargetProviderPlan]>,
    pub provider_publication: crate::target_machines::ProviderPublicationCatalog,
    pub planning_target_profile: Option<target::TargetProfile>,
    pub boundary_calling_plan_realizations: Vec<BoundaryCallingPlanRealization>,
    pub component_calling_applications: Vec<calling_conventions::BoundaryCallingApplication>,
    pub provider_candidates: Vec<crate::ProviderPlanCandidate<Rows>>,
    pub native_build_plans: Vec<crate::DerivedProviderPlan>,
    pub evaluated_via_bindings: EvaluatedViaBindingTable,
    pub external_binding_rows: Vec<calling_conventions::ExternalBindingRow>,
    pub independent_component_descriptions:
        Vec<package_compilation::IndependentComponentDescription>,
    pub verified_independent_components:
        Vec<std::sync::Arc<component_description::VerifiedComponent>>,
}

/// One environmental premise a verified independent component still needs the
/// consuming project to accept: the component's package, the roster digest,
/// and the exact identities of the description rows bound by it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentAssumptionDemand {
    pub component: PackageKeyIdentity,
    pub assumption: [u8; 32],
    pub bound_rows: Vec<String>,
}

impl<Rows> ProviderSettlement<Rows> {
    /*
    Settlement verifies the attached descriptions in the same order it retains
    them, so each verified component pairs with the description, and the
    package, it came from.
    */
    pub fn component_assumption_demands(&self) -> Vec<ComponentAssumptionDemand> {
        self.independent_component_descriptions
            .iter()
            .zip(&self.verified_independent_components)
            .flat_map(|(description, component)| {
                component
                    .assumption_meanings()
                    .into_iter()
                    .map(|(assumption, rows)| ComponentAssumptionDemand {
                        component: description.package(),
                        assumption,
                        bound_rows: rows.into_iter().map(str::to_owned).collect(),
                    })
            })
            .collect()
    }
    pub fn callback_calling_requirements(
        &self,
        declarations: &psi_terminal_production::PreparedExternalProviderDeclarations,
    ) -> Vec<terminal_psi::CallbackRequirementId> {
        declarations.callbacks.calling_requirements(
            self.boundary_calling_plan_realizations
                .iter()
                .map(|realization| {
                    (
                        realization.calling_owner_application(),
                        realization.requirement_machine,
                    )
                }),
        )
    }
}

impl ProviderSettlement<crate::DeferredProviderRows> {
    pub fn finish_row_declarations(self) -> Result<ProviderSettlement, Vec<Diagnostic>> {
        let Self {
            calling_program,
            selected_provider_publication,
            boundary_provider_demands,
            provider_declarations,
            provider_publication,
            planning_target_profile,
            boundary_calling_plan_realizations,
            component_calling_applications,
            provider_candidates,
            native_build_plans,
            evaluated_via_bindings,
            external_binding_rows,
            independent_component_descriptions,
            verified_independent_components,
        } = self;
        let mut prepared = Vec::with_capacity(provider_candidates.len());
        for candidate in provider_candidates {
            prepared.push(
                candidate
                    .finish_row_declarations()
                    .map_err(|diagnostic| vec![diagnostic])?,
            );
        }
        Ok(ProviderSettlement {
            calling_program,
            selected_provider_publication,
            boundary_provider_demands,
            provider_declarations,
            provider_publication,
            planning_target_profile,
            boundary_calling_plan_realizations,
            component_calling_applications,
            provider_candidates: prepared,
            native_build_plans,
            evaluated_via_bindings,
            external_binding_rows,
            independent_component_descriptions,
            verified_independent_components,
        })
    }
}

impl<Intrinsic> ProviderSettlement<Intrinsic> {
    pub fn calling_applications(
        &self,
        exported: &[calling_conventions::BoundaryCallingApplication],
    ) -> Result<Vec<calling_conventions::BoundaryCallingApplication>, String> {
        let explicit = self
            .boundary_calling_plan_realizations
            .iter()
            .map(BoundaryCallingPlanRealization::native_application)
            .collect::<Result<Vec<_>, _>>()?;
        let mut applications = Vec::with_capacity(explicit.len());
        for application in explicit {
            if !applications.contains(&application) {
                applications.push(application);
            }
        }
        for application in self
            .external_binding_rows
            .iter()
            .flat_map(|row| &row.calling_applications)
        {
            if !applications.contains(application) {
                applications.push(application.clone());
            }
        }
        for application in self.component_calling_applications.iter().chain(exported) {
            if !applications.contains(application) {
                applications.push(application.clone());
            }
        }
        Ok(applications)
    }
}

pub fn settle_checked_providers(
    selected_target_profile: Option<target::TargetProfile>,
    package_inputs: Option<&PackageCompilationInputs>,
    prepared_providers: Result<crate::target_machines::PreparedProductProviders, Vec<Diagnostic>>,
    boundary_calling_plan_realizations: Vec<BoundaryCallingPlanRealization>,
    calling_program: std::sync::Arc<crate::calling_policy_plans::CallingProgram>,
    callbacks: Result<
        &psi_terminal_production::PreparedExternalProviderDeclarations,
        &[Diagnostic],
    >,
) -> Result<CheckedProviderSelection, Vec<Diagnostic>> {
    let planning_target = selected_target_profile.or_else(target::TargetProfile::host_if_supported);
    let target_name = planning_target.map(target::TargetProfile::target_name);
    let provider_selection_target = planning_target
        .map(target::TargetProfile::native_target)
        .unwrap_or_else(target::NativeTarget::host);
    let prepared = prepared_providers?.attach_calling(
        &calling_program,
        planning_target,
        &boundary_calling_plan_realizations,
    )?;
    let evaluated_via_bindings = prepared.bindings;
    let product_candidates = prepared.candidates;
    let native_build_plans = product_candidates[prepared.native_start..]
        .iter()
        .map(|candidate| candidate.source.clone())
        .collect();
    let provider_publication = prepared.publication;
    let completion_roles = prepared.completion_roles;
    let independent_components = verify_independent_components(package_inputs)?
        .into_iter()
        .map(std::sync::Arc::new)
        .collect::<Vec<_>>();
    let prepared_assumptions = prepared.assumptions;
    let assumptions = prepared_assumptions.coordinates?;
    let profile = planning_target.unwrap_or_else(target::TargetProfile::host);
    let mut prepared = prepared.choices?;
    prepared.independent_components = independent_components.clone();
    let artifact = calling_program.artifact.view();
    let canonical_roots = prepared.close_application_roots(artifact)?;
    let callback_inputs = callbacks.map(|callbacks| {
        (
            callbacks,
            callbacks.callbacks.calling_requirements(
                boundary_calling_plan_realizations
                    .iter()
                    .map(|realization| {
                        (
                            realization.calling_owner_application(),
                            realization.requirement_machine,
                        )
                    }),
            ),
        )
    });
    let provider_selection::SelectedTargetProviders {
        selection: canonical_selection,
        facts: mut selected_provider_plan_facts,
        demanded_plans: boundary_provider_demands,
        choices,
        invoked_declarations: _,
        callback_roots,
    } = prepared.select(
        artifact,
        provider_selection::ProviderApplicationDemand::Product(
            canonical_roots.as_deref(),
            callback_inputs
                .as_ref()
                .map(|(callbacks, calling)| (*callbacks, calling.as_slice()))
                .map_err(|errors| *errors),
        ),
        &assumptions,
    )?;
    let provider_declarations: std::sync::Arc<[backend_plan::TerminalTargetProviderPlan]> =
        canonical_selection.plans.clone().into();
    let selected_provider_plans = choices;
    let direct_context = crate::provider_planning::external_binding_rows::ExternalBindingContext {
        calling_program: &calling_program,
        selected_target: target_name,
        native_target: provider_selection_target,
        boundary_calling_plan_realizations: &boundary_calling_plan_realizations,
    };
    let provider_candidates = product_candidates
        .into_iter()
        .map(|candidate| {
            candidate
                .rows
                .attach_physical_rows(candidate.source, &direct_context)
        })
        .collect::<Result<Vec<_>, _>>()?;
    backend_plan::synchronous_invocations::validate_declared_synchronous_invocation_cycles(
        &provider_declarations,
        artifact.module(),
    )?;
    let external_binding_rows = crate::extract_native_external_binding_rows(
        target_name,
        provider_selection_target,
        &selected_provider_plans
            .iter()
            .map(|selected| &selected.candidate.plan)
            .collect::<Vec<_>>(),
        &boundary_calling_plan_realizations,
        &calling_program,
    )?;
    let mut selected_declarations = Vec::with_capacity(selected_provider_plans.len());
    let mut selected_families = Vec::with_capacity(selected_provider_plans.len());
    let completed_providers = selected_provider_plans
        .into_iter()
        .map(|selected| {
            let realization = selected.candidate.owner;
            selected_declarations.push(realization.declarations);
            let review = realization.owner;
            selected_families.push(review.families);
            Ok((
                SelectedProviderReviewProvenance {
                    plan: selected.candidate.plan,
                    provider: review.provenance,
                    selected_by:
                        crate::provider_planning::selection_declarations::restore_provenance(
                            selected.selected_by,
                        ),
                    row_compiler_intrinsic_executions: Vec::new(),
                    row_external_completion_leaves: review.completion?,
                },
                review.role,
            ))
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?;
    if package_inputs.is_some_and(PackageCompilationInputs::semantic_role_discovery) {
        completion_roles.validate(&completed_providers)?;
    }
    let selected_provider_provenance = completed_providers
        .into_iter()
        .map(|(provider, _)| provider)
        .collect::<Vec<_>>();
    let native_provider_sources = crate::native_provider_sources::attach_native_provider_sources(
        &selected_provider_provenance,
        selected_families,
        std::sync::Arc::clone(&provider_declarations),
    )
    .map(std::sync::Arc::new);
    for assumption in prepared_assumptions.rows {
        assumption.validate_calling(&boundary_calling_plan_realizations)?;
    }
    let mut required = std::collections::BTreeSet::new();
    for row in &assumptions {
        if !required.insert(row.required.as_str())
            || selected_provider_plan_facts
                .selected_requirement_row(&row.required)
                .is_some()
        {
            return Err(vec![Diagnostic::error(
                "assumed binding has competing explicit supply",
            )]);
        }
    }
    selected_provider_plan_facts = selected_provider_plan_facts.with_assumed_bindings(assumptions);
    for row in selected_provider_plan_facts.assumed_bindings() {
        let supplied = selected_provider_plan_facts
            .assumed_supplier_identity(&row.required)
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
        if selected_provider_plan_facts
            .selected_requirement_row(supplied)
            .is_none()
        {
            return Err(vec![Diagnostic::error(
                "assumed binding has no selected complete supplied provider",
            )]);
        }
    }
    let mut component_providers = Vec::new();
    for selected in &selected_provider_provenance {
        if selected
            .selected_by
            .composition_mode()
            .map_err(|reason| vec![Diagnostic::error(reason)])?
            == crate::CompositionMode::Independent
        {
            component_providers.push(&selected.plan);
        }
    }
    let component_calling_applications =
        crate::calling_policy_plans::component_calling_applications(
            planning_target.unwrap_or_else(target::TargetProfile::host),
            &calling_program,
            &component_providers,
            &boundary_calling_plan_realizations,
        )?;
    let selected_provider_publication =
        std::sync::Arc::new(provider_selection::SelectedTargetProviderPublication {
            profile,
            terminal_psi: calling_program.artifact.view().identity(),
            application_roots: canonical_roots,
            selection: canonical_selection,
        });
    Ok(CheckedProviderSelection {
        callback_roots: callback_roots.ok_or_else(|| {
            vec![Diagnostic::error(
                "product provider selection lost callback preparation",
            )]
        })?,
        settlement: ProviderSettlement {
            calling_program,
            selected_provider_publication,
            boundary_provider_demands,
            provider_declarations,
            provider_publication,
            planning_target_profile: planning_target,
            boundary_calling_plan_realizations,
            component_calling_applications,
            provider_candidates,
            native_build_plans,
            evaluated_via_bindings,
            external_binding_rows,
            verified_independent_components: independent_components,
            independent_component_descriptions: package_inputs
                .map(|inputs| {
                    inputs
                        .independent_component_descriptions()
                        .cloned()
                        .collect()
                })
                .unwrap_or_default(),
        },
        selected_provider_plan_facts,
        selected_provider_provenance,
        selected_declarations,
        native_provider_sources,
    })
}

pub fn verify_selected_compositions_are_realized(
    provenance: &[SelectedProviderReviewProvenance],
) -> Result<(), Vec<Diagnostic>> {
    let diagnostics = provenance
        .iter()
        .filter_map(|row| match row.selected_by.composition_mode() {
            Ok(CompositionMode::Fused) => None,
            Ok(CompositionMode::Independent) => Some(Diagnostic::error(format!(
                "selected provider plan `{}` settled with an independent composition edge to provider `{}`, but the product defines no symbolic import/export, entry/leave, or installation/replacement carrier for it; refusing to emit a fused artifact",
                row.plan.name, row.plan.provider_type,
            ))),
            Err(reason) => Some(Diagnostic::error(format!(
                "selected provider plan `{}` retained no coherent composition mode ({reason}); refusing to emit a fused artifact",
                row.plan.name,
            ))),
        })
        .collect::<Vec<_>>();
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

/*
Product compilation verifies each attached description in full except for the
consumer's decision on its environmental premises. Those stay pending here and
surface as `component_assumption` acceptance rows of the root Product
occurrence: the consuming project's lock retains the reviewed rows, and root
admission rejects any fresh row the lock does not hold before production
(component_publication.md, "Consumer acceptance and Build attenuation").
*/
fn verification_request(
    description: &IndependentComponentDescription,
) -> ComponentVerificationRequest {
    ComponentVerificationRequest {
        expected_subject: description.expected_subject(),
        accepted_schemas: BTreeSet::from([COMPONENT_DESCRIPTION_SCHEMA_V3]),
        accepted_assumptions: AcceptedComponentAssumptions::PendingConsumerAcceptance,
    }
}

pub(crate) fn verify_independent_component_descriptions<'a>(
    descriptions: impl IntoIterator<Item = &'a IndependentComponentDescription>,
    package_name: impl Fn(PackageKeyIdentity) -> Option<&'a str>,
) -> Result<Vec<VerifiedComponent>, Vec<Diagnostic>> {
    let mut components = Vec::new();
    let mut diagnostics = Vec::new();
    for description in descriptions {
        let request = verification_request(description);
        match verify_component(description.description(), &request) {
            Ok(component) => components.push(component),
            Err(rejection) => {
                let package = package_name(description.package()).unwrap_or("<unnamed>");
                diagnostics.push(Diagnostic::error(format!(
                    "component description attached for dependency package `{package}` failed independent verification: {rejection}; the build cannot deploy that package as an independent component",
                )));
            }
        }
    }
    if diagnostics.is_empty() {
        Ok(components)
    } else {
        Err(diagnostics)
    }
}

fn verify_independent_components(
    package_inputs: Option<&PackageCompilationInputs>,
) -> Result<Vec<VerifiedComponent>, Vec<Diagnostic>> {
    let Some(package_inputs) = package_inputs else {
        return Ok(Vec::new());
    };
    verify_independent_component_descriptions(
        package_inputs.independent_component_descriptions(),
        |package| package_inputs.package_name(package),
    )
}

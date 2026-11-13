// Provider settlement is retained target/native data. The old source-driven
// producer and its checked-flow completion/admission algorithms were deleted;
// these readers do not reconstruct acceptance from source declarations.

use crate::calling_policy_plans::BoundaryCallingPlanRealization;
use crate::evaluated_via_bindings::EvaluatedViaBindingTable;
use crate::{CompositionMode, ProviderSelection, SelectedProviderReviewProvenance};
use diagnostics::Diagnostic;
use effects::provider_plan::ProviderPlan;
use semantic_vocabulary::PackageKeyIdentity;

pub struct SettledTargetMachineDeclarations {
    pub provider_defaults: Vec<ProviderSelection>,
    pub origins: Vec<crate::SelectedTargetMachineOrigin>,
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
        declarations: &terminal_psi::PreparedCallbackDeclarations,
    ) -> Vec<terminal_psi::CallbackRequirementId> {
        declarations.calling_requirements(self.boundary_calling_plan_realizations.iter().map(
            |realization| {
                (
                    realization.calling_owner_application(),
                    realization.requirement_machine,
                )
            },
        ))
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

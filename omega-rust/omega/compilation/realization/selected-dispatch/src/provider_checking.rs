//! Translate build-selected providers into Psi checking inputs and bind the
//! resulting checked program. No source checking or execution-plan rebuilding
//! happens here: the caller runs Psi's single checking entrance between these
//! two operations.

use checked_trees::CheckedTrees as CheckedProgram;
use diagnostics::Diagnostic;
use std::sync::Arc;
use typed_trees::TypedTrees;

/// Checked Psi plus the exact predecessor facts that must be captured before
/// typed ownership moves into checking. This is the output of one phase
/// transition, not source-loading state.
pub struct CheckedProviderProgram {
    pub program: Arc<CheckedProgram>,
    pub selected_provider_plan_facts: effects::SelectedProviderPlanFacts,
    pub selected_provider_grants: Vec<trust_model::ResolvedAuthoredSelectedProviderGrant>,
    pub callback_placements: Vec<backend_plan::BoundNominalCallbackPlacement>,
    pub accepted_template_classifications: trust_model::AcceptedTemplateClassifications,
}

/// Build-selected provider inputs. The compiler supplies these; this owner
/// derives checking requests and binds the checked result to those selections.
pub struct ProviderCheckingRequest<'a> {
    pub native_target: Option<target::NativeTarget>,
    pub package_inputs: Option<&'a package_compilation::PackageCompilationInputs>,
    pub build_settings:
        &'a package_compilation::build_execution::build_value::EvaluatedBuildSettings,
    pub selected_build_machine: Option<symbols::SymbolHandle>,
    pub boundary_calling_plan_realizations:
        &'a mut [provider_planning::calling_policy_plans::BoundaryCallingPlanRealization],
    pub opaque_representation_selections:
        &'a [representation_planning::OpaqueRepresentationSelection],
    pub provider_plans: &'a [effects::provider_plan::ProviderPlan],
    pub selected_provider_plan_facts: effects::SelectedProviderPlanFacts,
    pub root_grants: &'a [String],
    pub authored_root_grants: &'a [trust_model::AuthoredRootGrant],
}

/// Owned representation inputs for one Psi check, plus the
/// authored review records captured before specialization changes the trees.
pub struct ProviderCheckingInputs {
    opaque_property_receipts: Vec<validation::OpaqueDataPropertyReceipt>,
    accepted_template_classifications: trust_model::AcceptedTemplateClassifications,
}

impl ProviderCheckingInputs {
    pub fn prepare(
        typed: &TypedTrees,
        settlement: &ProviderCheckingRequest<'_>,
    ) -> Result<Self, Vec<Diagnostic>> {
        // Package authority admits whole reviewed packages, never a standalone
        // machine grant. Reject before deriving any checking inputs.
        if settlement.package_inputs.is_some() {
            trust_model::reject_package_non_provider_grants(
                typed,
                settlement.root_grants,
                settlement.provider_plans,
                &settlement.selected_provider_plan_facts,
            )?;
        }
        let accepted_template_classifications =
            trust_model::AcceptedTemplateClassifications::capture(typed);
        let rederived_opaque_representation_selections =
            representation_planning::rederive_opaque_representation_selections(
                typed,
                settlement.selected_build_machine,
                settlement.opaque_representation_selections,
            )?;
        let opaque_property_receipts = rederived_opaque_representation_selections
            .iter()
            .filter(|&selection| selection.copy_disposition()
                    == representation_planning::OpaqueRepresentationCopyDisposition::CheckedSemanticCopy )
            .map(|selection| validation::OpaqueDataPropertyReceipt::copy(selection.opaque()))
            .collect::<Vec<_>>();

        Ok(Self {
            opaque_property_receipts,
            accepted_template_classifications,
        })
    }

    /// Supply representation evidence and this invocation's exact Build and
    /// package authority. Psi independently checks privileged instructions and
    /// authored declaration selections; preparation does not approve either.
    pub fn checking_request<'request>(
        &'request self,
        settlement: &'request ProviderCheckingRequest<'_>,
    ) -> typed_trees_to_checked_trees::CheckingRequest<'request> {
        let settings = settlement.build_settings;
        typed_trees_to_checked_trees::CheckingRequest::settled()
            .with_opaque_property_receipts(&self.opaque_property_receipts)
            .with_asm_authority(
                typed_trees_to_checked_trees::AsmAuthorityAdmission::from_freestanding(
                    settings.freestanding,
                )
                .with_grants(
                    settings.privileged_services.port_io,
                    settings.privileged_services.interrupt_table,
                ),
            )
            .with_declaration_authority(
                settlement
                    .package_inputs
                    .map(|inputs| inputs as &dyn validation::DeclarationSelectionAuthority),
            )
    }

    /// Bind callback placements and provider grants after Psi has checked the
    /// source. Errors retain the original selected-provider diagnostics.
    pub fn bind(
        self,
        mut program: CheckedProgram,
        settlement: ProviderCheckingRequest<'_>,
    ) -> Result<CheckedProviderProgram, Vec<Diagnostic>> {
        if let Some(native_target) = settlement.native_target {
            provider_planning::calling_policy_plans::close_outbound_callback_materializations(
                &mut program,
                settlement.boundary_calling_plan_realizations,
                native_target,
                settlement.opaque_representation_selections,
                settlement.package_inputs,
            )?;
        }
        let callback_placements =
            provider_planning::calling_policy_plans::validate_nominal_callback_placement_bindings(
                &program,
                settlement.boundary_calling_plan_realizations,
            )?;
        let program = Arc::new(program);
        let selected_provider_binding = provider_planning::bind_selected_provider_plan_facts(
            &program,
            settlement.provider_plans,
            settlement.selected_provider_plan_facts,
            settlement.root_grants,
            settlement.authored_root_grants,
        )?;
        let (program, selected_provider_plan_facts, selected_provider_grants) =
            selected_provider_binding.into_parts();

        Ok(CheckedProviderProgram {
            program,
            selected_provider_plan_facts,
            selected_provider_grants,
            callback_placements,
            accepted_template_classifications: self.accepted_template_classifications,
        })
    }
}

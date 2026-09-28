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
    pub contract_entailment_stand_downs: Vec<validation::ContractEntailmentStandDown>,
}

/// Build-selected provider inputs. The compiler supplies these; this owner
/// derives checking requests and binds the checked result to those selections.
pub struct ProviderCheckingRequest<'a> {
    pub native_target: Option<target::NativeTarget>,
    pub package_inputs: Option<&'a package_compilation::PackageCompilationInputs>,
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
    contract_entailment_stand_downs: Vec<validation::ContractEntailmentStandDown>,
}

impl ProviderCheckingInputs {
    pub fn prepare(
        typed: &TypedTrees,
        settlement: &ProviderCheckingRequest<'_>,
    ) -> Result<Self, Vec<Diagnostic>> {
        let accepted_template_classifications =
            trust_model::AcceptedTemplateClassifications::capture(typed);
        let contract_entailment_stand_downs =
            validation::collect_contract_entailment_stand_downs(typed);
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
            contract_entailment_stand_downs,
        })
    }

    /// Psi derives all candidate specializations from authored source. This
    /// request carries only independently checked representation evidence.
    pub fn checking_request(&self) -> typed_trees_to_checked_trees::CheckingRequest<'_> {
        typed_trees_to_checked_trees::CheckingRequest::settled()
            .with_opaque_property_receipts(&self.opaque_property_receipts)
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
            contract_entailment_stand_downs: self.contract_entailment_stand_downs,
        })
    }
}

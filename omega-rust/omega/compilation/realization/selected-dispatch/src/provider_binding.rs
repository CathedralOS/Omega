/*
Target-selected provider binding and source-free declaration preparation.

PreparedBindingDeclarations captures the declarations needed to choose
executable implementations while checked source is available, reporting each
saved failure at its original binding step so an unused declaration never fails
early.

prepare_selected_execution then binds a target's selected implementations using
those saved declarations without reopening source or choosing a different
implementation. We check grants, calls, and callbacks in order, closing the
existing exact source association once without needing native ABI publication to
count calling declarations.
*/

use crate::SelectedExecution;
use diagnostics::Diagnostic;
use package_compilation::AcceptedSemanticBindingRole;
use provider_planning::service_declarations::ServiceDeclarations;
use std::sync::Arc;

pub struct PreparedBindingDeclarations {
    pub(crate) owner: Result<(), Vec<Diagnostic>>,
    pub(crate) source_owner: std::sync::Weak<checked_trees::CheckedTrees>,
    trust: Result<Arc<language_semantics::trust_declarations::TrustDeclarations>, Vec<Diagnostic>>,
    pub(crate) external:
        Result<Arc<psi_terminal_production::PreparedExternalProviderDeclarations>, Vec<Diagnostic>>,
    pub(crate) issuers: Result<Vec<language_semantics::BoundaryQualificationIssuer>, &'static str>,
    pub(crate) services: Arc<typed_trees::service_declarations::PreparedServiceCatalog>,
    pub has_proof_obligations: bool,
}

impl PreparedBindingDeclarations {
    fn retained_trust_declarations(
        &self,
    ) -> Result<Arc<language_semantics::trust_declarations::TrustDeclarations>, Vec<Diagnostic>>
    {
        self.owner.clone()?;
        self.trust.clone()
    }

    pub fn callback_declarations<'declarations>(
        &'declarations self,
        program: &provider_planning::calling_policy_plans::CallingProgram,
        sources: &[(source::SourceId, [u8; 32])],
    ) -> Result<
        &'declarations Arc<psi_terminal_production::PreparedExternalProviderDeclarations>,
        Vec<Diagnostic>,
    > {
        self.owner.clone()?;
        if !std::sync::Weak::ptr_eq(&self.source_owner, &program.source_owner) {
            return Err(vec![Diagnostic::error(
                "callback demand belongs to another checked source owner",
            )]);
        }
        let declarations = self.external.as_ref().map_err(Clone::clone)?;
        declarations
            .require_prepared_owner(&program.source_owner, sources)
            .map_err(|reason| {
                vec![Diagnostic::error(format!(
                    "callback demand declarations: {reason:?}"
                ))]
            })?;
        Ok(declarations)
    }

    pub fn capture(
        program: &Arc<checked_trees::CheckedTrees>,
        inventory: &Arc<psi_terminal_production::PreparedProviderInventory>,
        exact_toolchain_sources: &[(source::SourceId, [u8; 32])],
    ) -> Self {
        let owner = if Arc::ptr_eq(inventory.program(), program) {
            Ok(())
        } else {
            Err(vec![Diagnostic::error(
                "selected provider inventory belongs to another checked program",
            )])
        };
        let external = (|| {
            owner.clone()?;
            let external = psi_terminal_production::PreparedExternalProviderDeclarations::prepare(
                Arc::clone(program),
                exact_toolchain_sources,
            )
            .map_err(|error| {
                vec![Diagnostic::error(format!(
                    "external provider declarations: {error:?}"
                ))]
            })?;
            external
                .require_owner(program, exact_toolchain_sources)
                .map_err(|error| {
                    vec![Diagnostic::error(format!(
                        "external provider declarations: {error:?}"
                    ))]
                })?;
            Ok(Arc::new(external))
        })();
        Self {
            owner,
            source_owner: Arc::downgrade(program),
            trust: inventory
                .trust_declarations()
                .map(|trust| Arc::new(trust.clone()))
                .map_err(|error| vec![error]),
            external,
            issuers: program
                .facts
                .semantic
                .admitted_qualification_issuers()
                .map(|issuers| issuers.into_iter().cloned().collect()),
            services: inventory.retained_service_catalog(),
            has_proof_obligations: program.facts.proof_applications.has_obligations,
        }
    }
}

pub struct ProviderBindingRequest<'a> {
    pub declarations: &'a PreparedBindingDeclarations,
    pub selected_declarations:
        Vec<Result<language_semantics::provider_declarations::ProviderDeclarations, Diagnostic>>,
    pub native_provider_sources: Result<Arc<backend_plan::NativeProviderSources>, Vec<Diagnostic>>,
    pub entry_service_requirements: Vec<symbols::SymbolHandle>,
    pub package_inputs: Option<&'a package_compilation::PackageCompilationInputs>,
    pub boundary_calling_plan_realizations:
        &'a [provider_planning::calling_policy_plans::BoundaryCallingPlanRealization],
    pub provider_candidates: &'a [provider_planning::ProviderPlanCandidate],
    pub boundary_provider_demands: &'a [effects::provider_plan::ProviderPlan],
    pub provider_declarations: &'a Arc<[backend_plan::TerminalTargetProviderPlan]>,
    pub calling_program: &'a provider_planning::calling_policy_plans::CallingProgram,
    pub exact_toolchain_sources: &'a [(source::SourceId, [u8; 32])],
    pub selected_provider_plan_facts: effects::SelectedProviderPlanFacts,
    pub callback_roots: Result<terminal_psi::PreparedTerminalRoots, Vec<Diagnostic>>,
    pub root_grants: &'a [String],
    pub selected_provider_provenance: Vec<provider_planning::SelectedProviderReviewProvenance>,
    pub authored_root_grants: &'a [trust_model::AuthoredRootGrant],
    pub provider_planning_target_profile: Option<target::TargetProfile>, // derivation target is not a deployment choice for checking-only products
    pub selected_target_profile: Option<target::TargetProfile>,
    pub selected_program_entry:
        Option<provider_planning::program_entry::SelectedCompilerProgramEntry>,
    pub x86_scalar_fma_provider: Option<target::AdmittedX86ScalarFmaProvider>,
}

pub fn prepare_selected_execution(
    mut settlement: ProviderBindingRequest<'_>,
) -> Result<SelectedExecution, Vec<Diagnostic>> {
    settlement.declarations.owner.clone()?;
    if !std::sync::Weak::ptr_eq(
        &settlement.declarations.source_owner,
        &settlement.calling_program.source_owner,
    ) {
        return Err(vec![Diagnostic::error(
            "selected provider inventory belongs to another checked program",
        )]);
    }
    if settlement.selected_target_profile.is_some()
        && settlement.selected_target_profile != settlement.provider_planning_target_profile
    {
        return Err(vec![Diagnostic::error(
            "selected deployment target differs from retained provider planning target",
        )]);
    }
    let trust = settlement.declarations.retained_trust_declarations()?;
    let provider_plans = settlement
        .provider_candidates
        .iter()
        .map(provider_planning::ProviderPlanCandidate::plan)
        .collect::<Vec<_>>();
    if settlement.package_inputs.is_some() && !settlement.root_grants.is_empty() {
        trust_model::reject_package_non_provider_grants(
            &trust,
            settlement.root_grants,
            &provider_plans,
            &settlement.selected_provider_plan_facts,
        )?;
    }
    let external_providers = settlement.declarations.external.clone()?;
    external_providers
        .require_prepared_owner(
            &settlement.calling_program.source_owner,
            settlement.exact_toolchain_sources,
        )
        .map_err(|error| {
            vec![Diagnostic::error(format!(
                "external provider declarations: {error:?}"
            ))]
        })?;
    let declarations = provider_planning::prepare_selected_plan_declarations(
        settlement.selected_declarations,
        &settlement.selected_provider_plan_facts,
        &settlement.selected_provider_provenance,
    );
    let binding = provider_selection::bind_selected_provider_plan_facts(
        provider_selection::BoundaryProviderAdmission {
            demands: settlement.boundary_provider_demands,
            review: &external_providers.boundary_provider_review(),
            identities: &external_providers.identities,
            external_providers: &external_providers.conformances,
        },
        settlement
            .declarations
            .issuers
            .as_ref()
            .map(|issuers| issuers.iter().collect())
            .map_err(|reason| *reason),
        &provider_plans,
        settlement.selected_provider_plan_facts,
        declarations,
        settlement.root_grants,
        settlement.authored_root_grants,
    )?;
    let (selected_provider_plan_facts, selected_provider_grants) = binding.into_parts();
    let selected_provider_plan_facts = Arc::new(selected_provider_plan_facts);
    // The source choice already classified these exact occurrences. Preserve
    // its deferred error here, after the original grant and declaration checks.
    let callback_roots = settlement.callback_roots?;
    let service_declarations = Arc::new(ServiceDeclarations::prepare(
        settlement.declarations.services.declared_traits(),
        settlement.boundary_calling_plan_realizations,
    ));

    let accepted_binding = |role| {
        settlement
            .package_inputs
            .and_then(|inputs| inputs.accepted_semantic_binding(role))
    };
    let resolved_exit_bindings = crate::retain_selected_compiler_intrinsic_review_identities(
        settlement.provider_candidates,
        &external_providers,
        &service_declarations,
        &selected_provider_plan_facts,
        &mut settlement.selected_provider_provenance,
        settlement
            .provider_planning_target_profile
            .map(target::TargetProfile::target_name),
        accepted_binding(AcceptedSemanticBindingRole::ConsoleExitProcessI32),
        accepted_binding(AcceptedSemanticBindingRole::ProcessExitExitProcessI32),
    )?;
    let direct_boundary_leaves = crate::intrinsic_review::produce_direct_boundary_leaves(
        &external_providers,
        &settlement.selected_provider_provenance,
        settlement.provider_candidates,
        &service_declarations,
        settlement
            .provider_planning_target_profile
            .map(target::TargetProfile::target_name),
        &resolved_exit_bindings,
    )?;
    let mut resolved_semantic_bindings = resolved_exit_bindings;
    let entry_role = settlement
        .selected_target_profile
        .map(|profile| profile.program_entry_slot())
        .and_then(|slot| slot.physical_contract_package)
        .map(package_compilation::program_entry_semantic_binding_role);
    for role in [
        Some(AcceptedSemanticBindingRole::FilesystemHostService),
        Some(AcceptedSemanticBindingRole::TimeHostService),
        entry_role,
    ] {
        if let Some(binding) = role.and_then(accepted_binding) {
            let resolved = crate::resolve_accepted_service_binding(&service_declarations, binding)
                .map_err(|diagnostic| vec![diagnostic])?;
            resolved_semantic_bindings.push(resolved);
        }
    }
    Ok(SelectedExecution {
        direct_boundary_leaves,
        service_declarations,
        external_providers,
        native_provider_sources: settlement.native_provider_sources,
        selected_provider_plan_facts,
        entry_service_requirements: settlement.entry_service_requirements,
        trust_declarations: trust,
        has_proof_obligations: settlement.declarations.has_proof_obligations,
        selected_target_profile: settlement.selected_target_profile,
        selected_program_entry: settlement.selected_program_entry,
        x86_scalar_fma_provider: settlement.x86_scalar_fma_provider,
        selected_provider_grants,
        callback_roots,
        selected_provider_provenance: settlement.selected_provider_provenance,
        resolved_semantic_bindings,
    })
}

#[cfg(test)]
mod trust_custody_tests {
    use super::*;

    // This is the remaining legacy constructor, not an accepted route or a
    // product factory. Its inventory alias must retain the same source owner.
    #[test]
    fn an_equal_program_cannot_supply_another_owners_trust_roster() {
        let program = Arc::new(checked_trees::CheckedTrees::default());
        let other = Arc::new(program.as_ref().clone());
        let inventory = Arc::new(checked_trees::PreparedProviderInventory::new(other, &[]));
        let declarations = PreparedBindingDeclarations::capture(&program, &inventory, &[]);
        let errors = declarations.retained_trust_declarations().unwrap_err();
        assert_eq!(errors.len(), 1);
        assert_eq!(
            errors[0].message,
            "selected provider inventory belongs to another checked program"
        );
    }

    #[test]
    fn selected_declarations_propagate_the_complete_unavailable_trust_error() {
        let mut symbols = symbols::SymbolTableBuilder::new();
        let symbol = symbols.insert_root(
            symbols::SymbolKind::Machine,
            symbols::SymbolNameRef::Static("Claim"),
        );
        let mut typed = checked_trees::typed_trees::TypedTrees {
            symbols: symbols.finish(),
            ..Default::default()
        };
        typed.push_machine(checked_trees::typed_trees::machine::Machine {
            symbol,
            name: "Claim".into(),
            supply_mode: language_semantics::MachineSupplyMode::AdmissionClaim,
            ..Default::default()
        });
        let program = Arc::new(checked_trees::CheckedTrees::with_roots(
            typed,
            checked_trees::CheckFacts::default(),
        ));
        let inventory = Arc::new(checked_trees::PreparedProviderInventory::new(
            Arc::clone(&program),
            &[],
        ));
        let expected = inventory.trust_declarations().unwrap_err();
        let declarations = PreparedBindingDeclarations::capture(&program, &inventory, &[]);
        let errors = declarations.retained_trust_declarations().unwrap_err();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].message, expected.message);
        assert!(errors[0].message.contains("no product route yet"));
        assert!(errors[0].message.ends_with("Claim"));
    }
}

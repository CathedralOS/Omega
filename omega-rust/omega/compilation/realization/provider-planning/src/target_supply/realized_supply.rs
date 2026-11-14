//! Keep the selected provider plans and their exact native execution inputs.
//!
//! Plan construction and native admission remain separate: this owner checks
//! complete roster correspondence before publication, and receiving admission
//! rejoins each row to the canonical artifact before execution.

use std::sync::Arc;

use backend_plan::{DirectBoundaryLeaf, TerminalTargetProviderPlan};
use effects::CompilerIntrinsicExecutionIdentity;
use terminal_psi::TerminalModule;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderPlanRealizationInput {
    pub composition_mode: language_semantics::declaration_selection::CompositionMode,
    pub schema_declaration: terminal_psi::TerminalIdentityIndex,
    pub schema_application: terminal_psi::TerminalIdentityIndex,
    pub declaration: TerminalTargetProviderPlan,
    pub row_executions: Vec<Option<CompilerIntrinsicExecutionIdentity>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetSupplyRealizationInput {
    pub reached: Vec<semantic_vocabulary::BoundaryMachineId>,
    pub target: String,
    pub plans: Vec<ProviderPlanRealizationInput>,
    pub direct_boundary_leaves: Vec<DirectBoundaryLeaf>,
    pub calling_applications: Vec<calling_conventions::BoundaryCallingApplication>,
    pub external_bindings: Vec<calling_conventions::ExternalBindingRow>,
    pub evaluated_native_bindings: Vec<effects::provider_plan::EvaluatedNativeBinding>,
    pub evaluated_binding_identities: terminal_psi::TerminalIdentityTable,
    pub terminal_authority_permissions: Vec<effects::ServiceTerminalAuthorityPermission>,
    pub terminal_authority_permission_identities: terminal_psi::TerminalIdentityTable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PlanRealization {
    composition_mode: language_semantics::declaration_selection::CompositionMode,
    provider_declaration: Option<String>,
    numeric: Vec<Option<String>>,
    executions: Vec<Option<CompilerIntrinsicExecutionIdentity>>,
}

/// The effect closure owns the sole retained plan roster. The small parallel
/// rows retain native execution information that is not an effect-plan field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RealizedTargetSupply {
    target: String,
    plans: Arc<effects::SelectedProviderPlanFacts>,
    realizations: Vec<PlanRealization>,
    direct_boundary_leaves: Vec<DirectBoundaryLeaf>,
    family_realizations: Vec<backend_plan::SelectedFamilyRealizationSource>,
    calling_applications: Vec<calling_conventions::BoundaryCallingApplication>,
    external_bindings: Vec<calling_conventions::ExternalBindingRow>,
    evaluated_native_bindings: Vec<effects::provider_plan::EvaluatedNativeBinding>,
    evaluated_binding_identities: terminal_psi::TerminalIdentityTable,
    terminal_authority_permissions: Vec<effects::ServiceTerminalAuthorityPermission>,
    terminal_authority_permission_identities: terminal_psi::TerminalIdentityTable,
}

impl RealizedTargetSupply {
    pub fn from_checked(
        module: &TerminalModule,
        mut input: TargetSupplyRealizationInput,
    ) -> Result<Self, String> {
        let profile = target::TargetProfile::from_canonical_target_name(&input.target)
            .map_err(|error| error.to_string())?;
        let native_target = profile.native_realization();
        for selected in &input.plans {
            let declaration = &selected.declaration;
            let plan = &declaration.plan;
            let schema =
                module.service_schema(selected.schema_declaration, selected.schema_application)?;
            if schema.surface.as_ref() != plan.schema.declaration.as_ref() {
                return Err(
                    "selected provider changes its canonical complete service surface".into(),
                );
            }
            let choice = module.provider_selection(
                &input.target,
                selected.schema_declaration,
                selected.schema_application,
            )?;
            if selected.composition_mode != choice.composition_mode {
                return Err("provider realization changes its selected composition mode".into());
            }
            if choice.composition_mode
                == language_semantics::declaration_selection::CompositionMode::Independent
            {
                return Err("reached independent provider composition needs its canonical verified component evidence".into());
            }
            let provider_application = choice.provider_application.map(|identity| {
                module.identities.text(identity).filter(|value| !value.is_empty())
                    .ok_or("selected provider value application has an unresolved canonical identity")
            }).transpose()?;
            if module.identities.text(choice.provider_declaration)
                != declaration.provider_declaration.as_deref()
                || module.identities.text(choice.provider_identity)
                    != Some(plan.provider_type.as_str())
                || choice.provider_package != plan.provider_type_package_identity
                || provider_application != plan.application_identity()
            {
                return Err(
                    "provider realization differs from the exact normalized selected value".into(),
                );
            }
            if plan.target != input.target {
                return Err("selected provider plan belongs to a different target".into());
            }
            if plan.rows.len() != selected.row_executions.len()
                || plan.rows.len() != declaration.row_numeric_realizations.len()
            {
                return Err(
                    "selected provider execution changes the complete plan row roster".into(),
                );
            }
            let reasons = plan.validate_against_schema();
            if !reasons.is_empty() {
                return Err(reasons.join("; "));
            }
            if plan.rows.iter().any(|row| {
                matches!(
                    row.binding,
                    effects::provider_plan::ProviderBinding::CheckedAdapter { .. }
                )
            }) {
                let owner = declaration
                    .provider_declaration
                    .as_deref()
                    .filter(|owner| !owner.is_empty())
                    .ok_or("selected checked provider has no exact choosing declaration")?;
                let candidates =
                    backend_plan::selected_provider_candidate_rows(plan, owner, module)?;
                if candidates
                    .iter()
                    .any(|row| row.candidate_positions.is_empty())
                {
                    return Err("selected checked provider has an unproved required row".into());
                }
            }
            for ((row, execution), numeric) in plan
                .rows
                .iter()
                .zip(&selected.row_executions)
                .zip(&declaration.row_numeric_realizations)
            {
                if !matches!(
                    row.binding,
                    effects::provider_plan::ProviderBinding::CompilerIntrinsic { .. }
                ) && (execution.is_some() || numeric.is_some())
                {
                    return Err(
                        "non-intrinsic provider row carries unrelated intrinsic execution".into(),
                    );
                }
            }
        }
        for leaf in &input.direct_boundary_leaves {
            leaf.validate_calling_custody()?;
            if leaf.plan.target != input.target {
                return Err("direct boundary leaf belongs to a different target".into());
            }
            let mut matches = module.boundary_machines.iter().filter(|boundary| {
                boundary.declaration.as_ref().is_some_and(|declaration| {
                    module
                        .identities
                        .text(declaration.source.machine.identity())
                        == Some(leaf.declaration_identity.as_str())
                        && module.identities.text(declaration.source.callable)
                            == Some(leaf.callable_identity.as_str())
                })
            });
            let (Some(boundary), None) = (matches.next(), matches.next()) else {
                return Err("direct leaf has no unique checked boundary declaration".into());
            };
            let declared = boundary
                .declaration
                .as_ref()
                .ok_or("direct leaf lost its checked declaration")?;
            let mut external = module.external_providers.iter().filter(|source| {
                source.realization == declared.source
                    && source
                        .target
                        .as_deref()
                        .is_none_or(|target| target == input.target)
            });
            let (Some(source), None) = (external.next(), external.next()) else {
                return Err("direct leaf lost its unique admitted external conformance".into());
            };
            source.validate(&module.identities)?;
            match &leaf.execution {
                backend_plan::DirectBoundaryExecution::Intrinsic(retained) => {
                    if source.binding
                        != language_semantics::ExternalBindingIdentity::CompilerIntrinsic
                        || effects::external_providers::canonical_external_execution(
                            source,
                            &input.target,
                        )? != *retained
                    {
                        return Err(
                            "direct leaf substituted its admitted native execution meaning".into(),
                        );
                    }
                }
                backend_plan::DirectBoundaryExecution::Foreign(Ok(binding)) => {
                    if let calling_conventions::ExternalBindingKind::Syscall { number } =
                        &binding.binding
                    {
                        let number = *number;
                        if source.binding
                            != (language_semantics::ExternalBindingIdentity::Syscall { number })
                        {
                            return Err("direct syscall changes its admitted native binding".into());
                        }
                        let receipts = input.evaluated_native_bindings.iter().filter(|binding| {
                            matches!(binding, effects::provider_plan::EvaluatedNativeBinding::Syscall(syscall)
                                if syscall.number() == number && syscall.target().target_name() == input.target)
                                && binding.receipt().build_origin().is_some_and(|origin| {
                                    input.evaluated_binding_identities.text(origin.requirement_identity)
                                        == Some(leaf.callable_identity.as_str())
                                })
                        }).count();
                        if receipts != 1 {
                            return Err(
                                "direct syscall has no unique completed Build locator receipt"
                                    .into(),
                            );
                        }
                    }
                }
                backend_plan::DirectBoundaryExecution::Foreign(Err(_)) => {}
            }
        }
        for application in &input.calling_applications {
            if Some(application.target) != native_target
                || application.recomputed_commitment()? != application.commitment
            {
                return Err(
                    "selected calling application changes its target or complete commitment".into(),
                );
            }
        }
        if input
            .external_bindings
            .iter()
            .any(|row| row.target_name != input.target)
        {
            return Err("selected foreign binding belongs to a different target".into());
        }
        for binding in &input.evaluated_native_bindings {
            if binding.target().target_name() != input.target {
                return Err("evaluated native binding belongs to a different target".into());
            }
            // Reinterning validates every producer coordinate against the
            // retained table; the original table remains the receipt owner.
            binding.reintern_producer(
                &input.evaluated_binding_identities,
                &mut terminal_psi::TerminalIdentityTableBuilder::default(),
            )?;
        }
        for permission in &input.terminal_authority_permissions {
            input
                .terminal_authority_permission_identities
                .text(permission.requirement_identity())
                .filter(|identity| !identity.is_empty())
                .ok_or("terminal authority permission lost its requirement identity")?;
        }
        input.plans.sort_by(|left, right| {
            left.declaration
                .plan
                .selection_order(&right.declaration.plan)
        });
        let mut plans = Vec::with_capacity(input.plans.len());
        let mut realizations = Vec::with_capacity(input.plans.len());
        for selected in input.plans {
            plans.push(selected.declaration.plan);
            realizations.push(PlanRealization {
                composition_mode: selected.composition_mode,
                provider_declaration: selected.declaration.provider_declaration,
                numeric: selected.declaration.row_numeric_realizations,
                executions: selected.row_executions,
            });
        }
        let plans = effects::SelectedProviderPlanFacts::from_selected_plans(plans)?;
        let family_realizations = super::plan_realization::family_realizations(
            module,
            &input.target,
            &input.reached,
            &plans,
        )?;
        Ok(Self {
            target: input.target,
            plans: Arc::new(plans),
            realizations,
            direct_boundary_leaves: input.direct_boundary_leaves,
            family_realizations,
            calling_applications: input.calling_applications,
            external_bindings: input.external_bindings,
            evaluated_native_bindings: input.evaluated_native_bindings,
            evaluated_binding_identities: input.evaluated_binding_identities,
            terminal_authority_permissions: input.terminal_authority_permissions,
            terminal_authority_permission_identities: input
                .terminal_authority_permission_identities,
        })
    }

    pub fn target(&self) -> &str {
        &self.target
    }
    pub fn plans(&self) -> &effects::SelectedProviderPlanFacts {
        &self.plans
    }
    pub fn shared_plans(&self) -> &Arc<effects::SelectedProviderPlanFacts> {
        &self.plans
    }
    pub fn provider_declarations(&self) -> Vec<TerminalTargetProviderPlan> {
        self.plans
            .plans()
            .iter()
            .zip(&self.realizations)
            .map(|(plan, realization)| TerminalTargetProviderPlan {
                plan: plan.clone(),
                provider_declaration: realization.provider_declaration.clone(),
                row_numeric_realizations: realization.numeric.clone(),
            })
            .collect()
    }
    pub fn composition_modes(
        &self,
    ) -> impl ExactSizeIterator<Item = language_semantics::declaration_selection::CompositionMode> + '_
    {
        self.realizations.iter().map(|row| row.composition_mode)
    }
    pub fn row_executions(
        &self,
    ) -> impl ExactSizeIterator<Item = &[Option<CompilerIntrinsicExecutionIdentity>]> {
        self.realizations
            .iter()
            .map(|row| row.executions.as_slice())
    }
    pub fn family_realizations(&self) -> &[backend_plan::SelectedFamilyRealizationSource] {
        &self.family_realizations
    }
    pub fn direct_boundary_leaves(&self) -> &[DirectBoundaryLeaf] {
        &self.direct_boundary_leaves
    }
    pub fn calling_applications(&self) -> &[calling_conventions::BoundaryCallingApplication] {
        &self.calling_applications
    }
    pub fn external_bindings(&self) -> &[calling_conventions::ExternalBindingRow] {
        &self.external_bindings
    }
    pub fn evaluated_native_bindings(&self) -> &[effects::provider_plan::EvaluatedNativeBinding] {
        &self.evaluated_native_bindings
    }
    pub fn evaluated_binding_identities(&self) -> &terminal_psi::TerminalIdentityTable {
        &self.evaluated_binding_identities
    }
    pub fn terminal_authority_permissions(&self) -> &[effects::ServiceTerminalAuthorityPermission] {
        &self.terminal_authority_permissions
    }
    pub fn terminal_authority_permission_identities(&self) -> &terminal_psi::TerminalIdentityTable {
        &self.terminal_authority_permission_identities
    }
}

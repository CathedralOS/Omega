//! Keep the selected provider plans and their exact native execution inputs.
//!
//! Plan construction and native admission remain separate: this owner checks
//! complete roster correspondence before publication, and receiving admission
//! rejoins each row to the canonical artifact before execution.

use std::sync::Arc;

use build_providers::plans::ProviderPlan;
use direct_boundary_bindings::DirectBoundaryLeaf;
use native_intrinsics::CompilerIntrinsicExecutionIdentity;
use terminal_psi::TerminalModule;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ProviderPlanRealizationInput {
    pub component: Option<verified_optimization_input::ComponentImport>,
    pub composition_mode: semantic_vocabulary::source_selections::CompositionMode,
    pub schema_declaration: terminal_psi::TerminalIdentityIndex,
    pub schema_application: terminal_psi::TerminalIdentityIndex,
    pub declaration: ProviderPlan,
    pub row_executions: Vec<Option<CompilerIntrinsicExecutionIdentity>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PlanRealization {
    component: Option<verified_optimization_input::ComponentImport>,
    composition_mode: semantic_vocabulary::source_selections::CompositionMode,
    executions: Vec<Option<CompilerIntrinsicExecutionIdentity>>,
}

/// Stage 03 owns this value while it resolves a target's complete supply.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RealizedTargetSupply(SelectedProviderRealization);

/// The effect closure owns the sole retained plan roster. The small parallel
/// rows retain native execution information that is not an effect-plan field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedProviderRealization {
    program: terminal_psi::TerminalPsiIdentity,
    target: Option<String>,
    plans: Arc<build_providers::selections::SelectedProviderPlanFacts>,
    realizations: Vec<PlanRealization>,
    direct_boundary_leaves: Vec<DirectBoundaryLeaf>,
    calling_applications: Vec<calling_conventions::BoundaryCallingApplication>,
    external_bindings: Vec<calling_conventions::ExternalBindingRow>,
    evaluated_native_bindings: Vec<build_providers::plans::EvaluatedNativeBinding>,
    evaluated_binding_identities: terminal_psi::TerminalIdentityTable,
    terminal_authority_permissions:
        Vec<build_providers::acceptance::ServiceTerminalAuthorityPermission>,
    terminal_authority_permission_identities: terminal_psi::TerminalIdentityTable,
}

impl std::ops::Deref for RealizedTargetSupply {
    type Target = SelectedProviderRealization;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub(super) fn validate_direct_leaves<'a>(
    module: &TerminalModule,
    target: &str,
    input: &super::TargetSupplyExecutionInputs,
    leaves: impl Iterator<Item = &'a direct_boundary_bindings::DirectBoundaryLeaf>,
    primitive_leaves: &[direct_boundary_bindings::DirectBoundaryLeaf],
) -> Result<(), String> {
    let primitive_keys = super::sorted_leaf_keys(primitive_leaves);
    // Index checked boundary declarations by the key a leaf must carry, so
    // each leaf finds its boundary by binary search.
    let mut boundary_keys = module
        .interfaces
        .boundary_machines
        .iter()
        .enumerate()
        .filter_map(|(position, boundary)| {
            let declaration = boundary.declaration.as_ref()?;
            let key = super::boundary_leaf_key(&module.identities, boundary.kind, declaration)?;
            Some((key, position))
        })
        .collect::<Vec<_>>();
    boundary_keys.sort_unstable();
    for leaf in leaves {
        leaf.validate_calling_custody()?;
        if leaf.plan.target != target {
            return Err("direct boundary leaf belongs to a different target".into());
        }
        let key = super::direct_leaf_key(leaf);
        let [(_, position)] = super::entries_with_key(&boundary_keys, key) else {
            return Err("direct leaf has no unique checked boundary declaration".into());
        };
        let boundary = &module.interfaces.boundary_machines[*position];
        if boundary.declaration.as_ref().is_some_and(|declaration| {
            matches!(
                declaration.execution,
                terminal_psi::CheckedBoundaryExecution::HostedPrimitive { .. }
                    | terminal_psi::CheckedBoundaryExecution::NumericPrimitive
            )
        }) {
            leaf.validate_bound_calling_custody(module)?;
            if !super::entries_with_key(&primitive_keys, key)
                .iter()
                .any(|(_, position)| primitive_leaves[*position] == *leaf)
            {
                return Err(
                    "direct compiler primitive changes its canonical checked meaning".into(),
                );
            }
            continue;
        }
        if boundary.kind == semantic_vocabulary::BoundaryRequirementKind::Trait {
            leaf.validate_plain_execution(
                module,
                &input.evaluated_native_bindings,
                &input.evaluated_binding_identities,
            )?;
            let direct_boundary_bindings::DirectBoundaryExecution::Foreign(Ok(binding)) =
                &leaf.execution
            else {
                return Err("plain native requirement lost its completed execution".into());
            };
            if input
                .external_bindings
                .iter()
                .filter(|row| {
                    row.requirement_identity == binding.requirement_identity
                        && row.target_name == target
                })
                .count()
                != 1
                || !input.external_bindings.contains(binding)
            {
                return Err("plain native leaf changes its retained external binding".into());
            }
            continue;
        }
        let declared = boundary
            .declaration
            .as_ref()
            .ok_or("direct leaf lost its checked declaration")?;
        let mut external = module
            .interfaces
            .external_providers
            .iter()
            .filter(|source| {
                source.realization == declared.source
                    && source
                        .target
                        .as_deref()
                        .is_none_or(|declared_target| declared_target == target)
            });
        let (Some(source), None) = (external.next(), external.next()) else {
            return Err("direct leaf lost its unique admitted external conformance".into());
        };
        source.validate(&module.identities)?;
        match &leaf.execution {
            direct_boundary_bindings::DirectBoundaryExecution::Intrinsic(retained) => {
                if source.binding != semantic_vocabulary::ExternalBindingIdentity::CompilerIntrinsic
                    || provider_correspondence::external_providers::canonical_external_execution(
                        source, target,
                    )? != *retained
                {
                    return Err(
                        "direct leaf substituted its admitted native execution meaning".into(),
                    );
                }
            }
            direct_boundary_bindings::DirectBoundaryExecution::Foreign(Ok(binding)) => {
                if let calling_conventions::ExternalBindingKind::Syscall { number } =
                    &binding.binding
                {
                    let number = *number;
                    if source.binding
                        != (semantic_vocabulary::ExternalBindingIdentity::Syscall { number })
                    {
                        return Err("direct syscall changes its admitted native binding".into());
                    }
                    let receipts = input.evaluated_native_bindings.iter().filter(|binding| {
                        matches!(binding, build_providers::plans::EvaluatedNativeBinding::Syscall(syscall)
                            if syscall.number() == number && syscall.target().target_name() == target)
                            && binding.receipt().build_origin().is_some_and(|origin| {
                                input.evaluated_binding_identities.text(origin.requirement_identity)
                                    == Some(leaf.callable_identity.as_str())
                            })
                    }).count();
                    if receipts != 1 {
                        return Err(
                            "direct syscall has no unique completed Build locator receipt".into(),
                        );
                    }
                }
            }
            direct_boundary_bindings::DirectBoundaryExecution::Foreign(Err(_)) => {}
        }
    }
    Ok(())
}

impl RealizedTargetSupply {
    pub(crate) fn into_result(self) -> SelectedProviderRealization {
        self.0
    }

    /// An unscoped check cannot choose a target's provider. Its actual roots
    /// must reach no boundary before a no-demand realization can be retained.
    pub fn unscoped(
        artifact: terminal_codec::TerminalArtifactView<'_>,
        roots: &[semantic_vocabulary::MachineId],
    ) -> Result<Self, String> {
        // The scalar roots and every root's boundary reach read one set of
        // callable values instead of building them once per root.
        let analyses = terminal_semantics::synchronous_calls::call_analyses(artifact.module())?;
        let graph = analyses.calls();
        let mut demanded_roots = roots.to_vec();
        for root in graph.scalar_evaluation_roots(roots, None)? {
            match root {
                terminal_psi::TargetMachineCallable::Machine(machine) => {
                    demanded_roots.push(machine)
                }
                terminal_psi::TargetMachineCallable::Boundary(_) => {
                    return Err(
                        "boundary requirements require an explicit deployment target".into(),
                    );
                }
            }
        }
        demanded_roots.sort_unstable();
        demanded_roots.dedup();
        for root in demanded_roots {
            let reach = analyses.boundary_call_reach(root, &[])?;
            if !reach.boundaries.is_empty() {
                return Err("boundary requirements require an explicit deployment target".into());
            }
        }
        Self::no_demand(artifact.identity(), None)
    }

    pub(super) fn no_demand(
        program: terminal_psi::TerminalPsiIdentity,
        target: Option<&str>,
    ) -> Result<Self, String> {
        if let Some(target) = target {
            target::TargetProfile::from_canonical_target_name(target)
                .map_err(|error| error.to_string())?;
        }
        Ok(Self(SelectedProviderRealization {
            program,
            target: target.map(str::to_owned),
            plans: Arc::new(
                build_providers::selections::SelectedProviderPlanFacts::from_selected_plans(
                    Vec::new(),
                )?,
            ),
            realizations: Vec::new(),
            direct_boundary_leaves: Vec::new(),
            calling_applications: Vec::new(),
            external_bindings: Vec::new(),
            evaluated_native_bindings: Vec::new(),
            evaluated_binding_identities: Default::default(),
            terminal_authority_permissions: Vec::new(),
            terminal_authority_permission_identities: Default::default(),
        }))
    }

    /// `program` is the identity of `module` that `realize_target_supply`
    /// already checked against the closed logical providers.
    pub(super) fn from_checked(
        module: &TerminalModule,
        program: terminal_psi::TerminalPsiIdentity,
        profile: target::TargetProfile,
        mut selected_plans: Vec<ProviderPlanRealizationInput>,
        direct_boundary_leaves: Vec<DirectBoundaryLeaf>,
        input: super::TargetSupplyExecutionInputs,
    ) -> Result<Self, String> {
        let target = profile.target_name();
        let native_target = profile.native_realization();
        for selected in &selected_plans {
            let plan = &selected.declaration;
            let schema =
                module.service_schema(selected.schema_declaration, selected.schema_application)?;
            if schema.surface.as_ref() != plan.schema.declaration.as_ref() {
                return Err(
                    "selected provider changes its canonical complete service surface".into(),
                );
            }
            let choice = module.provider_selection(
                target,
                selected.schema_declaration,
                selected.schema_application,
            )?;
            if selected.composition_mode != choice.composition_mode {
                return Err("provider realization changes its selected composition mode".into());
            }
            use semantic_vocabulary::source_selections::CompositionMode;
            match (choice.composition_mode, &selected.component) {
                (CompositionMode::Fused, None) => {}
                (CompositionMode::Independent, Some(component)) if component.matches(program, plan) => {}
                (CompositionMode::Independent, _) => return Err(
                    "reached independent provider composition needs its canonical verified component evidence".into()
                ),
                (CompositionMode::Fused, Some(_)) => return Err(
                    "a fused provider cannot import a separate component".into()
                ),
            }
            let provider_application = choice.provider_application.map(|identity| {
                module.identities.text(identity).filter(|value| !value.is_empty())
                    .ok_or("selected provider value application has an unresolved canonical identity")
            }).transpose()?;
            if module.identities.text(choice.provider_declaration)
                != plan.provider_declaration.as_deref()
                || module.identities.text(choice.provider_identity)
                    != Some(plan.provider_type.as_str())
                || choice.provider_package != plan.provider_type_package_identity
                || provider_application != plan.application_identity()
            {
                return Err(
                    "provider realization differs from the exact normalized selected value".into(),
                );
            }
            if plan.target != target {
                return Err("selected provider plan belongs to a different target".into());
            }
            if plan.rows.len() != selected.row_executions.len()
                || plan.rows.len() != plan.row_numeric_realizations.len()
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
                    build_providers::plans::ProviderBinding::CheckedAdapter { .. }
                )
            }) {
                plan.provider_declaration
                    .as_deref()
                    .filter(|owner| !owner.is_empty())
                    .ok_or("selected checked provider has no exact choosing declaration")?;
                let candidates = target_provider_admission::selected_provider_candidate_rows(plan, module)?;
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
                .zip(&plan.row_numeric_realizations)
            {
                if !matches!(
                    row.binding,
                    build_providers::plans::ProviderBinding::CompilerIntrinsic { .. }
                ) && (execution.is_some() || numeric.is_some())
                {
                    return Err(
                        "non-intrinsic provider row carries unrelated intrinsic execution".into(),
                    );
                }
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
            .any(|row| row.target_name != target)
        {
            return Err("selected foreign binding belongs to a different target".into());
        }
        for binding in &input.evaluated_native_bindings {
            if binding.target().target_name() != target {
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
        selected_plans.sort_by(|left, right| left.declaration.selection_order(&right.declaration));
        let mut plans = Vec::with_capacity(selected_plans.len());
        let mut realizations = Vec::with_capacity(selected_plans.len());
        for selected in selected_plans {
            plans.push(selected.declaration);
            realizations.push(PlanRealization {
                component: selected.component,
                composition_mode: selected.composition_mode,
                executions: selected.row_executions,
            });
        }
        let plans =
            build_providers::selections::SelectedProviderPlanFacts::from_selected_plans(plans)?;
        Ok(Self(SelectedProviderRealization {
            program,
            target: Some(target.to_owned()),
            plans: Arc::new(plans),
            realizations,
            direct_boundary_leaves,
            calling_applications: input.calling_applications,
            external_bindings: input.external_bindings,
            evaluated_native_bindings: input.evaluated_native_bindings,
            evaluated_binding_identities: input.evaluated_binding_identities,
            terminal_authority_permissions: input.terminal_authority_permissions,
            terminal_authority_permission_identities: input
                .terminal_authority_permission_identities,
        }))
    }

    /// `program` is the identity of `module`, which must be the program this
    /// supply was realized for. `analyses` are the module's call analyses or
    /// the refusal that building them produced.
    pub(crate) fn resolve_installation_reach(
        mut self,
        module: &TerminalModule,
        analyses: Result<&terminal_semantics::ModuleAnalyses<'_>, &String>,
        program: terminal_psi::TerminalPsiIdentity,
        installation_requirements: &[semantic_vocabulary::BoundaryMachineId],
    ) -> Result<Self, String> {
        self.require_program(program)?;
        let plans = super::installation_reach::resolve_installation_reach(
            module,
            analyses,
            installation_requirements,
            self.plans.as_ref().clone(),
        )
        .map_err(|diagnostics| {
            diagnostics
                .into_iter()
                .map(|diagnostic| diagnostic.message)
                .collect::<Vec<_>>()
                .join("; ")
        })?;
        self.0.plans = Arc::new(plans);
        Ok(self)
    }

    pub fn with_folded_program(
        &self,
        artifact: terminal_codec::TerminalArtifactView<'_>,
    ) -> Result<Self, String> {
        Self::replay_folded_program(&self.0, artifact).map(Self)
    }

    /// Bind the same plans to a derived program whose only change is the exact
    /// folded dependency closure these plans establish. Different code or an
    /// incomplete/foreign closure cannot reuse this realization.
    pub(crate) fn replay_folded_program(
        realization: &SelectedProviderRealization,
        artifact: terminal_codec::TerminalArtifactView<'_>,
    ) -> Result<SelectedProviderRealization, String> {
        let target = realization
            .target()
            .ok_or("folded constants require an explicit target")?;
        let mut original = artifact.module().clone();
        let folded = std::mem::take(&mut original.scalars.folded_constants);
        realization.require_program(
            terminal_codec::terminal_psi_identity(&original).map_err(|error| error.to_string())?,
        )?;
        let root = original.program_entry_contract(target)?.entry;
        let expected = crate::folded_entry::folded_constant_dependencies(
            &original,
            target,
            realization.plans().plans(),
            &[root],
        )?;
        if folded != expected || folded.dependencies.is_empty() {
            return Err(
                "folded constant closure differs from this exact provider realization".into(),
            );
        }
        realization.rebind_components(artifact)
    }

    pub(crate) fn close_folded_entry(
        realization: &SelectedProviderRealization,
        original: &terminal_codec::CanonicalTerminalArtifact,
        evidence: Option<&terminal_codec::EvidenceSeals>,
        reach: &crate::folded_entry::FoldedEntryReach,
    ) -> Result<Option<(SelectedProviderRealization, crate::FoldedTargetProgram)>, String> {
        realization.require_program(original.view().identity())?;
        let target = realization
            .target()
            .ok_or("folded constants require an explicit target")?;
        let closure = reach.bind(original.view(), target, realization.plans().plans())?;
        if closure.dependencies.is_empty() {
            return Ok(None);
        }
        let folded = crate::FoldedTargetProgram::close(original, evidence, closure)?;
        let derived = realization.rebind_components(folded.artifact().view())?;
        Ok(Some((derived, folded)))
    }
}

impl SelectedProviderRealization {
    fn rebind_components(
        &self,
        artifact: terminal_codec::TerminalArtifactView<'_>,
    ) -> Result<Self, String> {
        let mut derived = self.clone();
        for (plan, row) in self
            .provider_declarations()
            .iter()
            .zip(&mut derived.realizations)
        {
            if let Some(previous) = &row.component {
                row.component = Some(verified_optimization_input::ComponentImport::admit(
                    artifact,
                    Arc::clone(previous.component()),
                    plan,
                )?);
            }
        }
        derived.program = artifact.identity();
        Ok(derived)
    }

    pub fn program(&self) -> terminal_psi::TerminalPsiIdentity {
        self.program
    }

    pub fn require_program(
        &self,
        program: terminal_psi::TerminalPsiIdentity,
    ) -> Result<(), String> {
        if self.program != program {
            return Err("provider realization belongs to a different accepted program".into());
        }
        Ok(())
    }

    pub fn target(&self) -> Option<&str> {
        self.target.as_deref()
    }
    pub fn plans(&self) -> &build_providers::selections::SelectedProviderPlanFacts {
        &self.plans
    }
    pub fn shared_plans(&self) -> &Arc<build_providers::selections::SelectedProviderPlanFacts> {
        &self.plans
    }
    pub fn provider_declarations(&self) -> &[ProviderPlan] {
        self.plans.plans()
    }
    pub fn component_imports(
        &self,
    ) -> impl Iterator<Item = (&ProviderPlan, &verified_optimization_input::ComponentImport)> {
        self.provider_declarations()
            .iter()
            .zip(&self.realizations)
            .filter_map(|(plan, row)| row.component.as_ref().map(|component| (plan, component)))
    }
    pub fn composition_modes(
        &self,
    ) -> impl ExactSizeIterator<Item = semantic_vocabulary::source_selections::CompositionMode> + '_
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
    pub fn direct_boundary_leaves(&self) -> &[DirectBoundaryLeaf] {
        &self.direct_boundary_leaves
    }
    pub fn calling_applications(&self) -> &[calling_conventions::BoundaryCallingApplication] {
        &self.calling_applications
    }
    pub fn external_bindings(&self) -> &[calling_conventions::ExternalBindingRow] {
        &self.external_bindings
    }
    pub fn evaluated_native_bindings(&self) -> &[build_providers::plans::EvaluatedNativeBinding] {
        &self.evaluated_native_bindings
    }
    pub fn evaluated_binding_identities(&self) -> &terminal_psi::TerminalIdentityTable {
        &self.evaluated_binding_identities
    }
    pub fn terminal_authority_permissions(
        &self,
    ) -> &[build_providers::acceptance::ServiceTerminalAuthorityPermission] {
        &self.terminal_authority_permissions
    }
    pub fn terminal_authority_permission_identities(&self) -> &terminal_psi::TerminalIdentityTable {
        &self.terminal_authority_permission_identities
    }
}

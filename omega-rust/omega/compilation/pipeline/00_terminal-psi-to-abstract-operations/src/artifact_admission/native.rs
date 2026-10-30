//! Native authority is established by ordinary verification, never by promotion
//! of optimizer-only input.

use super::ArtifactLoweringError;
use super::retention::{AdmittedOptimizationArtifact, retain_optimization_context};
use crate::lowering::lower_decoded_module;
use crate::optimization::{VerifiedPsiOptimizationContext, VerifiedPsiOptimizationInput};
use abstract_operations::AbstractOperationPlan;
use terminal_installation::TerminalPlacedViewEstablishment;

#[cfg(test)]
#[path = "native_scope_tests.rs"]
mod scope_tests;

/// Native-admitted input and the provider establishments bound to its
/// direct-entry placed-view roster rows. Construction is private:
/// `AdmittedNativeArtifact::try_into_native_input` joins one supply per
/// declared row, so an empty roster takes an empty supply.
#[derive(Debug, Clone)]
pub struct VerifiedNativeArtifactInput {
    optimization_input: VerifiedPsiOptimizationInput,
    placed_view_establishments: Vec<TerminalPlacedViewEstablishment>,
    private_entry_ram: Vec<terminal_installation::CheckedPrivateEntryRamRecipe>,
}

/// Invocation choices needed before executable lowering. These select work;
/// provider admission still owns the authority to install the selected supply.
pub struct NativeExecutionSelection<'request> {
    pub scalar_applications: Option<super::NativeScalarApplications<'request>>,
    pub providers: &'request [crate::SelectedProviderAdapter],
    pub plans: &'request effects::SelectedProviderPlanFacts,
    pub callbacks: &'request [backend_plan::TerminalCallbackOccurrenceProposal],
    pub entry_call: Option<&'request crate::SelectedProgramEntryCall>,
}

/// One native consumer's selected work, before scalar evaluation or lowering.
/// It retains the exact target and scope rather than rediscovering them after
/// evaluation. This is work selection, not executable or provider authority.
pub(crate) struct NativeScopePreparation {
    profile: target::TargetProfile,
    entry: semantic_vocabulary::MachineId,
    pub(crate) targets: terminal_installation::TerminalTargetContext,
    pub(crate) layouts: terminal_installation::AdmittedTargetLayouts,
    pub(crate) scope: crate::native_execution_scope::NativeExecutionScope,
    pub(crate) scalar_demand: Vec<semantic_vocabulary::ScalarApplicationId>,
}

impl NativeScopePreparation {
    pub(crate) fn prepare(
        artifact: &terminal_codec::CanonicalTerminalArtifact,
        profile: target::TargetProfile,
        root: terminal_psi::TerminalCallable,
        selection: &NativeExecutionSelection<'_>,
    ) -> Result<Self, ArtifactLoweringError> {
        profile
            .admit_address_representation()
            .map_err(ArtifactLoweringError::TargetFamily)?;
        let module = artifact.semantic_module();
        let selections = terminal_installation::target_context(
            artifact.view(),
            profile.target_name(),
        );
        let entry = selections
            .entry(artifact.view().identity(), module, root)
            .map_err(|error| ArtifactLoweringError::TargetFamily(error.to_string()))?;
        let layout_target = profile.layout_target_facts().ok_or_else(|| {
            ArtifactLoweringError::TargetFamily(
                "selected profile has no admitted target layout facts".into(),
            )
        })?;
        let layouts = terminal_installation::admit_target_layouts(
            artifact.view(),
            &selections,
            &layout_target,
        )
        .map_err(|error| ArtifactLoweringError::TargetFamily(error.to_string()))?;
        let assumptions = selection
            .plans
            .assumed_binding_coordinates(module)
            .map_err(ArtifactLoweringError::TargetFamily)?;
        let scope = crate::native_execution_scope::NativeExecutionScope::derive(
            module,
            entry,
            &selections,
            selection.entry_call,
            selection.providers,
            &assumptions,
            selection.callbacks,
        )
        .map_err(|error| ArtifactLoweringError::TargetFamily(error.to_string()))?;
        let scalar_demand = scope
            .scalar_application_demand(module, &selections, &layouts)
            .map_err(ArtifactLoweringError::TargetFamily)?;
        Ok(Self {
            profile,
            entry,
            targets: selections,
            layouts,
            scope,
            scalar_demand,
        })
    }

    pub(crate) fn lower(
        self,
        artifact: &terminal_codec::CanonicalTerminalArtifact,
        establishments: &[TerminalPlacedViewEstablishment],
        selection: &NativeExecutionSelection<'_>,
    ) -> Result<VerifiedNativeArtifactInput, ArtifactLoweringError> {
        let Self {
            profile,
            entry,
            targets: selections,
            layouts,
            scope,
            scalar_demand,
        } = self;
        let module = artifact.semantic_module();
        let scalar_applications = selection
            .scalar_applications
            .as_ref()
            .map(|inputs| {
                inputs.bind(
                    artifact,
                    profile,
                    &selections,
                    &layouts,
                    selection,
                    &scalar_demand,
                )
            })
            .transpose()
            .map_err(ArtifactLoweringError::TargetFamily)?;
        let mut structural_types = module.structural_types.clone();
        let geometry = scalar_applications.map_or_else(
            || layouts.resolved_records(),
            |closed| closed.geometry().as_ref(),
        );
        for resolved in geometry {
            let declaration = structural_types
                .iter_mut()
                .find(|row| row.id == resolved.id)
                .ok_or_else(|| {
                    ArtifactLoweringError::TargetFamily(
                        "selected layout lost its executable structural declaration".into(),
                    )
                })?;
            *declaration = resolved.clone();
        }
        let plan = crate::lowering::lower_selected_module(
            module,
            entry,
            structural_types,
            Some(&scope),
            scalar_applications,
        )
        .map_err(ArtifactLoweringError::Lowering)?;
        let mut input = retain_native_plan(module, plan, Some(selections))
            .try_into_native_input(establishments)?;
        input.optimization_input.selected_program_entry_call = selection.entry_call.cloned();
        input.optimization_input.target_layout_selections = Some(layouts);
        input.optimization_input.scalar_applications = scalar_applications.cloned();
        Ok(input)
    }
}

impl VerifiedNativeArtifactInput {
    pub fn target_machine_selections(
        &self,
    ) -> Option<&terminal_installation::TerminalTargetContext> {
        self.optimization_input.target_machine_selections.as_ref()
    }

    /// The exact scalar judgment bound before this input's executable values
    /// and storage geometry were lowered. Runtime capture borrows this result,
    /// never a separately supplied replacement judgment.
    pub fn scalar_applications(
        &self,
    ) -> Option<&terminal_installation::AdmittedScalarApplications> {
        self.optimization_input.scalar_applications.as_ref()
    }

    /// Select and lower one exact native consumer. Callers that must evaluate
    /// scalar inputs first retain NativeScopePreparation across that work.
    pub fn prepare(
        artifact: &terminal_codec::CanonicalTerminalArtifact,
        profile: target::TargetProfile,
        root: terminal_psi::TerminalCallable,
        establishments: &[TerminalPlacedViewEstablishment],
        selection: &NativeExecutionSelection<'_>,
    ) -> Result<Self, ArtifactLoweringError> {
        NativeScopePreparation::prepare(artifact, profile, root, selection)?.lower(
            artifact,
            establishments,
            selection,
        )
    }

    /// Original guest-owned initialized storage the invocation boundary must construct.
    /// Opaque establishments alone never answer this physical lending demand.
    pub fn private_entry_ram(&self) -> &[terminal_installation::CheckedPrivateEntryRamRecipe] {
        &self.private_entry_ram
    }

    pub fn prepare_with_private_entry_ram(
        artifact: &terminal_codec::CanonicalTerminalArtifact,
        profile: target::TargetProfile,
        root: terminal_psi::TerminalCallable,
        recipes: &[terminal_installation::CheckedPrivateEntryRamRecipe],
        selection: &NativeExecutionSelection<'_>,
    ) -> Result<Self, ArtifactLoweringError> {
        NativeScopePreparation::prepare(artifact, profile, root, selection)?
            .lower_with_private_entry_ram(artifact, recipes, selection)
    }

    pub fn plan(&self) -> &AbstractOperationPlan {
        self.optimization_input.plan()
    }
}

impl NativeScopePreparation {
    pub(crate) fn lower_with_private_entry_ram(
        self,
        artifact: &terminal_codec::CanonicalTerminalArtifact,
        recipes: &[terminal_installation::CheckedPrivateEntryRamRecipe],
        selection: &NativeExecutionSelection<'_>,
    ) -> Result<VerifiedNativeArtifactInput, ArtifactLoweringError> {
        if recipes
            .iter()
            .any(|recipe| recipe.terminal() != artifact.view().identity())
        {
            return Err(ArtifactLoweringError::TargetFamily(
                "private entry RAM recipe belongs to another Terminal artifact".into(),
            ));
        }
        if !recipes.is_empty() {
            if recipes
                .iter()
                .any(|recipe| recipe.layouts() != &self.layouts)
            {
                return Err(ArtifactLoweringError::TargetFamily(
                    "private entry RAM recipe belongs to another target or selected geometry"
                        .into(),
                ));
            }
        }
        let establishments = recipes
            .iter()
            .enumerate()
            .map(|(index, recipe)| {
                let (identity, _) = terminal_installation::private_entry_ram_identity_pair(
                    artifact.view().module(),
                    index,
                )
                .ok_or_else(|| {
                    ArtifactLoweringError::TargetFamily(
                        "private entry RAM identity namespace is exhausted".into(),
                    )
                })?;
                Ok(TerminalPlacedViewEstablishment {
                    input: recipe.input().clone(),
                    referent: terminal_installation::TerminalStructuralValue::minted(
                        identity,
                        recipe.schema(),
                        Vec::new(),
                    ),
                })
            })
            .collect::<Result<Vec<_>, ArtifactLoweringError>>()?;
        let mut input = self.lower(artifact, &establishments, selection)?;
        input.private_entry_ram = recipes.to_vec();
        Ok(input)
    }
}

impl VerifiedNativeArtifactInput {
    /// Retain selected requirement coordinates. Family aliases additionally
    /// require the exact artifact member receipt; closed Imports retain the
    /// source-authenticated roster the proposal already derived.
    pub(crate) fn bind_selected_provider_coordinates(
        mut self,
        artifact: &terminal_codec::CanonicalTerminalArtifact,
        selected: &effects::SelectedProviderPlanFacts,
    ) -> Result<Self, String> {
        let selections = self
            .optimization_input
            .target_machine_selections
            .as_ref()
            .ok_or_else(|| "selected family providers require target selection".to_owned())?;
        for row in selected.family_realization_rows() {
            let member = selections
                .member(terminal_psi::TerminalCallable::Boundary(row.head))
                .map_err(|error| error.to_string())?;
            let module = artifact.semantic_module();
            if row.terminal != artifact.manifest().semantic()
                || row.terminal != selections.terminal_psi()
                || row.target != selections.target()
                || member.callable() != terminal_psi::TerminalCallable::Boundary(row.member)
                || terminal_psi::find_boundary_machine(&module.boundary_machines, row.head)
                    .is_none_or(|head| {
                        module.identities.text(head.identity)
                            != selected.identities().text(row.head_identity)
                    })
                || terminal_psi::find_boundary_machine(&module.boundary_machines, row.member)
                    .is_none_or(|member| {
                        module.identities.text(member.identity)
                            != selected.identities().text(row.member_identity)
                    })
            {
                return Err(
                    "selected family provider row disagrees with the exact artifact member"
                        .to_owned(),
                );
            }
        }
        self.optimization_input.target_family_realizations =
            selected.family_realization_rows().to_vec();
        self.optimization_input.external_requirement_applications =
            selected.external_requirement_applications().to_vec();
        self.optimization_input
            .selected_provider_requirement_identities = selected.identities().clone();
        Ok(self)
    }

    pub const fn context(&self) -> &VerifiedPsiOptimizationContext {
        self.optimization_input.context()
    }

    pub(crate) fn retain_admitted_assumed_bindings(
        &mut self,
        calls: Vec<installation_evidence::InstalledAssumedBindingEvidence>,
    ) {
        self.optimization_input.admitted_assumed_bindings = calls;
    }

    pub(crate) fn retain_admitted_native_callbacks(
        &mut self,
        callbacks: Vec<crate::AdmittedNativeCallbackArgument>,
    ) {
        self.optimization_input.admitted_native_callbacks = callbacks;
    }

    pub const fn optimization_input(&self) -> &VerifiedPsiOptimizationInput {
        &self.optimization_input
    }

    /// The provider establishments bound to this input's direct-entry roster
    /// rows, in roster order. Empty for a program that declared none.
    pub fn placed_view_establishments(&self) -> &[TerminalPlacedViewEstablishment] {
        &self.placed_view_establishments
    }

    /// Establishment bindings are invocation-scoped executable evidence and
    /// do not transfer into optimizer authority; the provider keeps custody
    /// of its supply.
    pub fn into_optimization_input(self) -> VerifiedPsiOptimizationInput {
        self.optimization_input
    }
}

/// One canonical native-admitted plan with its exact retained verifier context.
/// Placed-view rows are semantic custody, not backing or access-event authority.
#[derive(Debug, Clone)]
pub struct AdmittedNativeArtifact {
    optimization_input: VerifiedPsiOptimizationInput,
}

impl AdmittedNativeArtifact {
    pub fn plan(&self) -> &AbstractOperationPlan {
        self.optimization_input.plan()
    }

    pub const fn context(&self) -> &VerifiedPsiOptimizationContext {
        self.optimization_input.context()
    }

    pub fn placed_view_inputs(&self) -> &[terminal_psi::TerminalPlacedViewInput] {
        &self.context().module().placed_view_inputs
    }

    /// Downgrade authority while preserving the complete semantic roster.
    pub fn into_optimization_artifact(self) -> AdmittedOptimizationArtifact {
        AdmittedOptimizationArtifact {
            input: self.optimization_input,
        }
    }

    /// The lowered plan alone. The plan carries no placed-view rows or
    /// verifier context; consumers that need either keep the admission and
    /// read `placed_view_inputs` or `context` instead.
    pub fn into_plan(self) -> AbstractOperationPlan {
        self.optimization_input.plan
    }

    /// The executable-image realization input: join one provider
    /// establishment to each placed-view roster row the direct entry
    /// declares. A program that declared none takes an empty supply. The
    /// bound supplies ride inside the native input so the realized entry
    /// boundary can lend the exact qualified backing for the invocation's
    /// duration; a supply that answers no declared row, answers one twice,
    /// carries noncanonical qualifications, or overlaps another exclusive
    /// referent rejects here rather than at access, and a row left
    /// unanswered still fails custody — a roster or a pointer is not the
    /// authority that lends the referent. This semantic establishment does not realize a native host
    /// entry lender; publication retains that physical-entry refusal.
    pub(crate) fn try_into_native_input(
        self,
        establishments: &[TerminalPlacedViewEstablishment],
    ) -> Result<VerifiedNativeArtifactInput, ArtifactLoweringError> {
        let placed_view_establishments = super::establishment::establish_native_placed_view_inputs(
            self.context().module(),
            self.plan().entry,
            establishments,
        )?;
        Ok(VerifiedNativeArtifactInput {
            optimization_input: self.optimization_input,
            placed_view_establishments,
            private_entry_ram: Vec::new(),
        })
    }
}

pub(super) fn lower_decoded_native_module(
    module: &terminal_psi::TerminalModule,
    entry: semantic_vocabulary::MachineId,
    selections: Option<terminal_installation::TerminalTargetContext>,
) -> Result<AdmittedNativeArtifact, ArtifactLoweringError> {
    let plan = lower_decoded_module(module, entry).map_err(ArtifactLoweringError::Lowering)?;
    Ok(retain_native_plan(module, plan, selections))
}

fn retain_native_plan(
    module: &terminal_psi::TerminalModule,
    plan: AbstractOperationPlan,
    selections: Option<terminal_installation::TerminalTargetContext>,
) -> AdmittedNativeArtifact {
    let context = retain_optimization_context(module);
    AdmittedNativeArtifact {
        optimization_input: VerifiedPsiOptimizationInput {
            scalar_applications: None,
            selected_program_entry_call: None,
            plan,
            context,
            admitted_native_callbacks: Vec::new(),
            admitted_assumed_bindings: Vec::new(),
            target_machine_selections: selections,
            target_layout_selections: None,
            target_family_realizations: Vec::new(),
            external_requirement_applications: Vec::new(),
            selected_provider_requirement_identities: terminal_psi::TerminalIdentityTable::from(
                Vec::new(),
            ),
        },
    }
}

#[cfg(test)]
mod private_entry_target_tests {
    use super::*;

    // A data-only consumer of the actual public CLI's canonical output. This
    // deliberately runs no Source compiler inside the Rust test process.
    #[test]
    #[ignore = "explicit qualification consumes OMEGA_ENTRY_RAM_CANONICAL_ARTIFACT"]
    fn checked_private_entry_recipe_cannot_rebind_target() {
        let path = std::env::var_os("OMEGA_ENTRY_RAM_CANONICAL_ARTIFACT")
            .expect("qualification must supply the actual canonical artifact");
        let bytes = std::fs::read(path).expect("read actual retained canonical bytes");
        let artifact = terminal_codec::CanonicalTerminalArtifact::from_bytes(&bytes)
            .expect("independent canonical admission");
        let terminal_psi::TerminalCallable::Machine(entry) = artifact.semantic_module().entry
        else {
            panic!("fixture must select an ordinary machine")
        };
        let row = artifact
            .semantic_module()
            .placed_view_inputs
            .iter()
            .find(|row| row.machine == entry)
            .expect("actual direct placed row")
            .clone();
        let selections =
            terminal_installation::target_context(artifact.view(), "linux_x86_64");
        let layout_target = target::TargetProfile::LinuxX64
            .layout_target_facts()
            .expect("Linux layout facts");
        let layouts = terminal_installation::admit_target_layouts(
            artifact.view(),
            &selections,
            &layout_target,
        )
        .expect("actual Linux selected geometry");
        // Public receiving data controls use the actual canonical entry row.
        // A fresh private allocation cannot answer a stale or changed demand.
        let mut wrong_schema = row.clone();
        assert_ne!(wrong_schema.schema_identity, wrong_schema.policy_identity);
        wrong_schema.schema_identity = wrong_schema.policy_identity;
        let mut wrong_seal = row.clone();
        wrong_seal.placement_commitment[0] ^= 1;
        let mut wrong_source_position = row.clone();
        wrong_source_position.position += 1;
        let mut wrong_runtime_position = row.clone();
        wrong_runtime_position.runtime_position += 1;
        let mut wrong_access = row.clone();
        wrong_access.access = terminal_psi::StructuralAccess::Owned;
        let mut wrong_binding = row.clone();
        wrong_binding.binding_is_mutable = !wrong_binding.binding_is_mutable;
        let mut stale_machine = row.clone();
        stale_machine.machine = semantic_vocabulary::MachineId::new(u64::MAX).unwrap();
        for input in [
            wrong_schema,
            wrong_seal,
            wrong_source_position,
            wrong_runtime_position,
            wrong_access,
            wrong_binding,
            stale_machine,
        ] {
            let failure = terminal_installation::admit_private_entry_ram(
                artifact.view(),
                &layouts,
                entry,
                &terminal_installation::PrivateEntryRamRecipe {
                    input,
                    contents: vec![41, 0, 0, 0, 3, 0, 0, 0],
                },
            )
            .expect_err("changed row must not lend private RAM");
            assert!(failure.contains("exact borrowed entry row"), "{failure}");
        }
        let recipe = terminal_installation::admit_private_entry_ram(
            artifact.view(),
            &layouts,
            entry,
            &terminal_installation::PrivateEntryRamRecipe {
                input: row,
                contents: vec![41, 0, 0, 0, 3, 0, 0, 0],
            },
        )
        .expect("complete real checked typed initializer");
        assert_eq!(recipe.layouts(), &layouts);
        assert_eq!(recipe.target(), "linux_x86_64");
        for profile in [target::TargetProfile::MacosArm64] {
            let other_selections = terminal_installation::target_context(
                artifact.view(),
                profile.target_name(),
            );
            let other = terminal_installation::admit_target_layouts(
                artifact.view(),
                &other_selections,
                &profile
                    .layout_target_facts()
                    .expect("other target layout facts"),
            )
            .expect("other target has independently admitted geometry");
            assert_ne!(recipe.layouts(), &other);
            let failure = VerifiedNativeArtifactInput::prepare_with_private_entry_ram(
                &artifact,
                profile,
                artifact.semantic_module().entry,
                std::slice::from_ref(&recipe),
                &NativeExecutionSelection {
                    scalar_applications: None,
                    providers: &[],
                    plans: &effects::SelectedProviderPlanFacts::default(),
                    callbacks: &[],
                    entry_call: None,
                },
            )
            .expect_err("same checked receipt cannot be rebound to another target");
            assert!(
                format!("{failure:?}").contains("another target or selected geometry"),
                "{failure:?}"
            );
        }
    }
}

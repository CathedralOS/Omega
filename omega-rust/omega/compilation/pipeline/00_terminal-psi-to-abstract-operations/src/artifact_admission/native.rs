//! Native authority is established by ordinary verification, never by promotion
//! of optimizer-only input.

use super::ArtifactLoweringError;
use super::retention::{AdmittedOptimizationArtifact, retain_optimization_context};
use crate::lowering::lower_decoded_module;
use crate::optimization::{VerifiedPsiOptimizationContext, VerifiedPsiOptimizationInput};
use abstract_operations::AbstractOperationPlan;
use terminal_installation::TerminalPlacedViewEstablishment;

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

impl VerifiedNativeArtifactInput {
    /// Resolve the portable entry for this exact deployment, lower its
    /// executable projection, and bind supplied direct-entry placed-view loans.
    /// Canonical semantics remain unchanged and no executable plan exists with
    /// an unresolved entry. Selection supplies no provider or backing authority.
    pub fn prepare(
        artifact: &terminal_codec::CanonicalTerminalArtifact,
        profile: target::TargetProfile,
        root: terminal_psi::TargetMachineCallable,
        establishments: &[TerminalPlacedViewEstablishment],
    ) -> Result<Self, ArtifactLoweringError> {
        let module = artifact.semantic_module();
        let selections = terminal_installation::admit_target_machine_selections(
            artifact.view(),
            profile.target_name(),
        );
        let entry = selections
            .entry(artifact.view().identity(), module, root)
            .map_err(|error| ArtifactLoweringError::TargetFamily(error.to_string()))?;
        lower_decoded_native_module(module, entry, Some(selections))?
            .try_into_native_input(establishments)
    }

    /// Original guest-owned initialized storage the invocation boundary must construct.
    /// Opaque establishments alone never answer this physical lending demand.
    pub fn private_entry_ram(&self) -> &[terminal_installation::CheckedPrivateEntryRamRecipe] {
        &self.private_entry_ram
    }

    pub fn prepare_with_private_entry_ram(
        artifact: &terminal_codec::CanonicalTerminalArtifact,
        profile: target::TargetProfile,
        root: terminal_psi::TargetMachineCallable,
        recipes: &[terminal_installation::CheckedPrivateEntryRamRecipe],
    ) -> Result<Self, ArtifactLoweringError> {
        if recipes
            .iter()
            .any(|recipe| recipe.terminal() != artifact.view().identity())
        {
            return Err(ArtifactLoweringError::TargetFamily(
                "private entry RAM recipe belongs to another Terminal artifact".into(),
            ));
        }
        if !recipes.is_empty() {
            let selections = terminal_installation::admit_target_machine_selections(
                artifact.view(),
                profile.target_name(),
            );
            let layouts = terminal_installation::admit_target_layouts(artifact.view(), &selections)
                .map_err(|reason| {
                    ArtifactLoweringError::TargetFamily(format!(
                        "private entry RAM selected geometry is unavailable: {reason:?}"
                    ))
                })?;
            if recipes.iter().any(|recipe| recipe.layouts() != &layouts) {
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
        let mut input = Self::prepare(artifact, profile, root, &establishments)?;
        input.private_entry_ram = recipes.to_vec();
        Ok(input)
    }

    pub fn plan(&self) -> &AbstractOperationPlan {
        self.optimization_input.plan()
    }

    /// Select one deployment's executable closure before physical work.
    /// Entry artifacts and private callback artifacts use this same projection;
    /// canonical storage and actual callable IDs never change. Selection grants
    /// no provider installation authority. A self-contained callback supplies
    /// an empty adapter roster and still requires ordinary target lowering.
    pub fn select_target_execution(
        self,
        artifact: &terminal_codec::CanonicalTerminalArtifact,
        profile: target::TargetProfile,
        selected: &[crate::SelectedProviderAdapter],
        assumptions: &effects::SelectedProviderPlanFacts,
        receiving_provider_roots: &[semantic_vocabulary::BoundaryMachineId],
        callbacks: &[backend_plan::TerminalCallbackOccurrenceProposal],
    ) -> Result<Self, crate::NativeExecutionScopeError> {
        if artifact.view().identity() != self.plan().psi {
            return Err(crate::NativeExecutionScopeError::TerminalIdentityMismatch);
        }
        let mut input = self.select_target_families(profile);
        // Selection can change the executable scope. It grants no callback
        // authority; only admission of the newly selected input can restore it.
        input.optimization_input.admitted_native_callbacks.clear();
        input.optimization_input.admitted_assumed_bindings.clear();
        let selections = terminal_installation::admit_target_machine_selections(
            artifact.view(),
            profile.target_name(),
        );
        selections
            .entry(
                artifact.view().identity(),
                artifact.semantic_module(),
                terminal_psi::TargetMachineCallable::Machine(input.plan().entry),
            )
            .map_err(|error| crate::NativeExecutionScopeError::TargetFamily(error.to_string()))?;
        let layouts = terminal_installation::admit_target_layouts(artifact.view(), &selections)
            .map_err(|error| crate::NativeExecutionScopeError::TargetLayout(error.to_string()))?;
        if !layouts.resolved_records().is_empty() {
            let declarations = input.optimization_input.plan.structural_types.make_mut();
            for resolved in layouts.resolved_records() {
                let declaration = declarations
                    .iter_mut()
                    .find(|row| row.id == resolved.id)
                    .ok_or_else(|| {
                        crate::NativeExecutionScopeError::TargetLayout(
                            "selected layout lost its executable structural declaration".to_owned(),
                        )
                    })?;
                *declaration = resolved.clone();
            }
        }
        input.optimization_input.target_machine_selections = Some(selections);
        input.optimization_input.target_layout_selections = Some(layouts);
        let scope = crate::native_execution_scope::NativeExecutionScope::derive(
            &input.optimization_input,
            selected,
            &assumptions
                .assumed_binding_coordinates(artifact.semantic_module())
                .map_err(crate::NativeExecutionScopeError::TargetFamily)?,
            receiving_provider_roots,
            callbacks,
        )?;
        input.retain_execution_scope(&scope);
        Ok(input)
    }

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
                .member(terminal_psi::TargetMachineCallable::Boundary(row.head))
                .map_err(|error| error.to_string())?;
            let module = artifact.semantic_module();
            if row.terminal != artifact.manifest().semantic()
                || row.terminal != selections.terminal_psi()
                || row.target != selections.target()
                || member.callable() != terminal_psi::TargetMachineCallable::Boundary(row.member)
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

    /// Restrict physical work to this deployment's retained implementations.
    /// Canonical Terminal storage and actual member identities remain intact;
    /// this does not redirect calls or substitute a body at a family head.
    /// A surviving call to an excluded member still fails ordinary closure
    /// admission. Portable head calls keep their original occurrence and bind
    /// to the exact selected member through separate internal call custody.
    fn select_target_families(mut self, profile: target::TargetProfile) -> Self {
        let members = self
            .optimization_input
            .context
            .module()
            .target_machine_families
            .iter()
            .flat_map(|family| &family.members)
            .filter_map(|member| match member.callable {
                terminal_psi::TargetMachineCallable::Machine(machine) => {
                    Some((machine, member.target.as_str() == profile.target_name()))
                }
                terminal_psi::TargetMachineCallable::Boundary(_) => None,
            })
            .collect::<Vec<_>>();
        self.optimization_input.plan.functions.retain(|function| {
            members
                .iter()
                .find(|(machine, _)| *machine == function.machine)
                .is_none_or(|(_, selected)| *selected)
        });
        self
    }

    /// Narrow only the invocation's executable projection. The immutable
    /// canonical module remains available to every contract/evidence consumer.
    fn retain_execution_scope(
        &mut self,
        scope: &crate::native_execution_scope::NativeExecutionScope,
    ) {
        self.optimization_input
            .plan
            .functions
            .retain(|function| scope.contains_machine(function.machine));
        self.optimization_input
            .plan
            .boundary_machines
            .retain(|boundary| scope.contains_boundary(boundary.id));
        let mut candidate_position = 0;
        self.optimization_input
            .plan
            .provider_candidates
            .retain(|_| {
                let retained = scope.contains_candidate_position(candidate_position);
                candidate_position += 1;
                retained
            });
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
    selections: Option<terminal_installation::AdmittedTargetMachineSelections>,
) -> Result<AdmittedNativeArtifact, ArtifactLoweringError> {
    let plan = lower_decoded_module(module, entry).map_err(ArtifactLoweringError::Lowering)?;
    let context = retain_optimization_context(module);
    Ok(AdmittedNativeArtifact {
        optimization_input: VerifiedPsiOptimizationInput {
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
    })
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
        let terminal_psi::TargetMachineCallable::Machine(entry) = artifact.semantic_module().entry
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
            terminal_installation::admit_target_machine_selections(artifact.view(), "linux_x86_64");
        let layouts = terminal_installation::admit_target_layouts(artifact.view(), &selections)
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
            let other_selections = terminal_installation::admit_target_machine_selections(
                artifact.view(),
                profile.target_name(),
            );
            let other =
                terminal_installation::admit_target_layouts(artifact.view(), &other_selections)
                    .expect("other target has independently admitted geometry");
            assert_ne!(recipe.layouts(), &other);
            let failure = VerifiedNativeArtifactInput::prepare_with_private_entry_ram(
                &artifact,
                profile,
                artifact.semantic_module().entry,
                std::slice::from_ref(&recipe),
            )
            .expect_err("same checked receipt cannot be rebound to another target");
            assert!(
                format!("{failure:?}").contains("another target or selected geometry"),
                "{failure:?}"
            );
        }
    }
}

/*
  Canonical Terminal artifact preparation, deployment execution-scope selection,
  and distinct ordinary, optimizer, and native admission (`lower_artifact`,
  `VerifiedNativeArtifactInput`, `AdmittedNativeArtifact`).

  `lower_artifact` is the single loose-byte entrance that runs
  `terminal_codec::decode_module`; callers holding a `CanonicalTerminalArtifact`
  read its already-decoded `semantic_module()`. Every admitted result retains
  its exact access-view input roster (`TerminalAccessViewEstablishment`) and
  can downgrade from native authority to optimizer custody, never the reverse.
*/
mod error {
    use crate::lowering::LoweringError;

    #[derive(Debug)]
    pub enum ArtifactLoweringError {
        TargetSelectionRequired,
        TargetFamily(String),
        SemanticDecode(terminal_codec::CodecError),
        AccessViewInputsRequireCustodyLowering,
        AccessViewEstablishmentUnexpected {
            machine: semantic_vocabulary::MachineId,
            position: u32,
        },
        AccessViewEstablishmentDuplicate {
            machine: semantic_vocabulary::MachineId,
            position: u32,
        },
        AccessViewEstablishmentQualificationsNonCanonical,
        AccessViewEstablishmentBackingUndeclared(semantic_vocabulary::StructuralTypeId),
        AccessViewEstablishmentRangeUnresolved,
        AccessViewEstablishmentQualificationUndeclared(semantic_vocabulary::StructuralDomainId),
        AccessViewEstablishmentQualificationCarrier(semantic_vocabulary::StructuralDomainId),
        AccessViewEstablishmentAliasing(u64),
        Lowering(LoweringError),
        /// Evidence seals name a different module than the one being lowered,
        /// for example seals from an earlier build.
        EvidenceForAnotherModule,
    }

    diagnostics::debug_display!(ArtifactLoweringError);

    impl std::error::Error for ArtifactLoweringError {}
}
mod establishment {
    pub use terminal_installation::TerminalAccessViewEstablishment;

    use super::ArtifactLoweringError;
    use std::collections::BTreeSet;
    use terminal_installation::TerminalStructuralValue;
    use terminal_psi::{StructuralAccess, TerminalModule};

    const fn exclusive(access: StructuralAccess) -> bool {
        matches!(access, terminal_psi::exclusive_borrow_pair_pattern!())
    }

    fn referent_overlaps(left: &TerminalStructuralValue, right: &TerminalStructuralValue) -> bool {
        left.opaque_identity == right.opaque_identity
            && (left.path.starts_with(&right.path) || right.path.starts_with(&left.path))
    }

    pub(super) fn establish_native_access_view_inputs(
        module: &TerminalModule,
        entry: semantic_vocabulary::MachineId,
        establishments: &[TerminalAccessViewEstablishment],
    ) -> Result<Vec<TerminalAccessViewEstablishment>, ArtifactLoweringError> {
        let entry_rows = module
            .access_view_inputs
            .iter()
            .filter(|row| row.machine == entry)
            .collect::<Vec<_>>();
        let mut seen = BTreeSet::new();
        for establishment in establishments {
            if !entry_rows.contains(&&establishment.input) {
                return Err(ArtifactLoweringError::AccessViewEstablishmentUnexpected {
                    machine: establishment.input.machine,
                    position: establishment.input.position,
                });
            }
            if !seen.insert(&establishment.input) {
                return Err(ArtifactLoweringError::AccessViewEstablishmentDuplicate {
                    machine: establishment.input.machine,
                    position: establishment.input.position,
                });
            }
            let referent = &establishment.referent;
            terminal_semantics::validate_access_view_referent(
                module,
                referent.structural_type,
                &referent.path,
                &referent.qualifications,
            )
            .map_err(|error| match error {
                terminal_semantics::AccessViewReferentError::QualificationsNonCanonical => {
                    ArtifactLoweringError::AccessViewEstablishmentQualificationsNonCanonical
                }
                terminal_semantics::AccessViewReferentError::BackingUndeclared(carrier) => {
                    ArtifactLoweringError::AccessViewEstablishmentBackingUndeclared(carrier)
                }
                terminal_semantics::AccessViewReferentError::RangeUnresolved => {
                    ArtifactLoweringError::AccessViewEstablishmentRangeUnresolved
                }
                terminal_semantics::AccessViewReferentError::QualificationUndeclared(domain) => {
                    ArtifactLoweringError::AccessViewEstablishmentQualificationUndeclared(domain)
                }
                terminal_semantics::AccessViewReferentError::QualificationCarrier(domain) => {
                    ArtifactLoweringError::AccessViewEstablishmentQualificationCarrier(domain)
                }
            })?;
        }
        let mut bound: Vec<(
            &terminal_psi::TerminalAccessViewInput,
            &TerminalAccessViewEstablishment,
        )> = Vec::with_capacity(entry_rows.len());
        for row in entry_rows {
            let Some(establishment) = establishments
                .iter()
                .find(|establishment| establishment.input == *row)
            else {
                return Err(ArtifactLoweringError::AccessViewInputsRequireCustodyLowering);
            };
            for (previous_row, previous) in &bound {
                if (exclusive(row.access) || exclusive(previous_row.access))
                    && referent_overlaps(&establishment.referent, &previous.referent)
                {
                    return Err(ArtifactLoweringError::AccessViewEstablishmentAliasing(
                        establishment.referent.opaque_identity,
                    ));
                }
            }
            bound.push((row, establishment));
        }
        Ok(bound
            .into_iter()
            .map(|(_, establishment)| establishment.clone())
            .collect())
    }
}
mod native {
    use super::ArtifactLoweringError;
    use super::retention::AdmittedOptimizationArtifact;
    use crate::lowering::lower_decoded_module;
    use abstract_operations::AbstractOperationPlan;
    use terminal_installation::TerminalAccessViewEstablishment;
    use terminal_installation::{AdmittedScalarApplications, AdmittedTargetMachineSelections};
    use verified_optimization_input::{
        VerifiedPsiOptimizationContext, VerifiedPsiOptimizationInput,
    };

    #[derive(Debug, Clone)]
    pub struct VerifiedNativeArtifactInput {
        optimization_input: VerifiedPsiOptimizationInput,
        access_view_establishments: Vec<TerminalAccessViewEstablishment>,
        private_entry_ram: Vec<terminal_installation::CheckedPrivateEntryRamRecipe>,
    }

    pub struct NativeExecutionSelection<'request> {
        pub scope: Option<&'request crate::NativeExecutionScope>,
        pub scalar_applications: Option<super::NativeScalarApplications<'request>>,
        pub providers: &'request [crate::SelectedProviderAdapter],
        pub plans: &'request effects::SelectedProviderPlanFacts,
        pub callbacks: &'request [backend_plan::TerminalCallbackOccurrenceProposal],
        pub entry_call: Option<&'request crate::SelectedProgramEntryCall>,
    }

    pub(crate) struct NativeScopePreparation {
        profile: target::TargetProfile,
        entry: semantic_vocabulary::MachineId,
        pub(crate) targets: AdmittedTargetMachineSelections,
        pub(crate) layouts: terminal_installation::AdmittedTargetLayouts,
        pub(crate) scope: Option<crate::NativeExecutionScope>,
        pub(crate) scalar_demand: Vec<semantic_vocabulary::ScalarApplicationId>,
    }

    impl NativeScopePreparation {
        pub(crate) fn prepare(
            artifact: &terminal_codec::CanonicalTerminalArtifact,
            profile: target::TargetProfile,
            root: terminal_psi::TargetMachineCallable,
            selection: &NativeExecutionSelection<'_>,
        ) -> Result<Self, ArtifactLoweringError> {
            profile
                .admit_address_representation()
                .map_err(ArtifactLoweringError::TargetFamily)?;
            let module = artifact.semantic_module();
            let selections = terminal_installation::admit_target_machine_selections(
                artifact.view(),
                profile.target_name(),
            );
            let entry = selections
                .entry(artifact.view().identity(), module, root)
                .map_err(|error| ArtifactLoweringError::TargetFamily(error.to_string()))?;
            let target_facts = profile.layout_target_facts().ok_or_else(|| {
                ArtifactLoweringError::TargetFamily(
                    "selected native target has no representation facts".into(),
                )
            })?;
            let layouts = terminal_installation::admit_target_layouts(
                artifact.view(),
                &selections,
                &target_facts,
            )
            .map_err(|error| ArtifactLoweringError::TargetFamily(error.to_string()))?;
            let scope = if let Some(scope) = selection.scope {
                scope
                    .require_source_input(
                        artifact.view(),
                        entry,
                        &selections,
                        selection.plans,
                        selection.providers,
                    )
                    .map_err(|reason| ArtifactLoweringError::TargetFamily(reason.into()))?;
                if scope.entry_call() != selection.entry_call
                    || scope.callbacks() != selection.callbacks
                {
                    return Err(ArtifactLoweringError::TargetFamily(
                        "native preparation changes its checked entry or callback inputs".into(),
                    ));
                }
                Some(scope.clone())
            } else {
                // Raw choices do not establish complete callback, entry and
                // provider coverage. Keep the ordinary full-module contract.
                None
            };
            let scalar_demand = match scope.as_ref() {
                Some(scope) => scope.scalar_application_demand(module, &selections, &layouts),
                None => crate::NativeExecutionScope::full_module_scalar_demand(
                    module,
                    entry,
                    &selections,
                    &layouts,
                ),
            }
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
            establishments: &[TerminalAccessViewEstablishment],
            selection: &NativeExecutionSelection<'_>,
            evidence: Option<&terminal_codec::EvidenceSeals>,
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
            let full_machines;
            let machines = match scope.as_ref() {
                Some(scope) => scope.machines(),
                None => {
                    full_machines = module
                        .machines
                        .iter()
                        .map(|machine| machine.id)
                        .collect::<Vec<_>>();
                    &full_machines
                }
            };
            let evidence = evidence
                .map(|evidence| evidence.project_for_machines(module, machines))
                .transpose()
                .map_err(|error| {
                    ArtifactLoweringError::TargetFamily(format!(
                        "native operation evidence: {error}"
                    ))
                })?;
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
                        machines,
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
                scope.as_ref(),
                scalar_applications,
                Some(&layouts),
            )
            .map_err(ArtifactLoweringError::Lowering)?;
            let mut input = retain_native_plan(module, plan, Some(selections))
                .try_into_native_input(establishments)?;
            input
                .optimization_input
                .retain_native_selections(
                    selection.entry_call.cloned(),
                    layouts,
                    scalar_applications.cloned(),
                    scope
                        .as_ref()
                        .map(|scope| std::sync::Arc::new(scope.clone())),
                    scope.as_ref().map_or_else(
                        || *selection.plans.identity_digest().as_bytes(),
                        crate::NativeExecutionScope::selected_provider_closure,
                    ),
                )
                .map_err(|reason| ArtifactLoweringError::TargetFamily(reason.into()))?;
            if let Some(evidence) = evidence {
                input.attach_evidence(evidence).map_err(|error| {
                    ArtifactLoweringError::TargetFamily(format!(
                        "native operation evidence: {error}"
                    ))
                })?;
            }
            Ok(input)
        }
    }

    impl VerifiedNativeArtifactInput {
        pub fn target_machine_selections(&self) -> Option<&AdmittedTargetMachineSelections> {
            self.optimization_input.target_machine_selections()
        }

        pub fn scalar_applications(&self) -> Option<&AdmittedScalarApplications> {
            self.optimization_input.scalar_applications()
        }

        pub fn prepare(
            artifact: &terminal_codec::CanonicalTerminalArtifact,
            profile: target::TargetProfile,
            root: terminal_psi::TargetMachineCallable,
            establishments: &[TerminalAccessViewEstablishment],
            selection: &NativeExecutionSelection<'_>,
        ) -> Result<Self, ArtifactLoweringError> {
            NativeScopePreparation::prepare(artifact, profile, root, selection)?.lower(
                artifact,
                establishments,
                selection,
                None,
            )
        }

        pub fn private_entry_ram(&self) -> &[terminal_installation::CheckedPrivateEntryRamRecipe] {
            &self.private_entry_ram
        }

        pub fn prepare_with_private_entry_ram(
            artifact: &terminal_codec::CanonicalTerminalArtifact,
            profile: target::TargetProfile,
            root: terminal_psi::TargetMachineCallable,
            recipes: &[terminal_installation::CheckedPrivateEntryRamRecipe],
            selection: &NativeExecutionSelection<'_>,
        ) -> Result<Self, ArtifactLoweringError> {
            NativeScopePreparation::prepare(artifact, profile, root, selection)?
                .lower_with_private_entry_ram(artifact, recipes, selection, None)
        }

        pub fn plan(&self) -> &AbstractOperationPlan {
            self.optimization_input.plan()
        }

        pub fn module(&self) -> &terminal_psi::TerminalModule {
            self.optimization_input.context().module()
        }

        pub fn admission_source(&self) -> provider_admission::AdmissionSource<'_> {
            let input = &self.optimization_input;
            provider_admission::AdmissionSource {
                plan: self.plan(),
                module: self.module(),
                target_machine_selections: input.target_machine_selections(),
                program_entry: input.selected_program_entry_call().map(|call| {
                    provider_admission::AdmittedProgramEntryCall {
                        boundary: call.boundary(),
                        entry: call.entry(),
                        provider_plan: call.provider_plan(),
                    }
                }),
                private_entry_ram: self.private_entry_ram(),
            }
        }
    }

    impl NativeScopePreparation {
        pub(crate) fn lower_with_private_entry_ram(
            self,
            artifact: &terminal_codec::CanonicalTerminalArtifact,
            recipes: &[terminal_installation::CheckedPrivateEntryRamRecipe],
            selection: &NativeExecutionSelection<'_>,
            evidence: Option<&terminal_codec::EvidenceSeals>,
        ) -> Result<VerifiedNativeArtifactInput, ArtifactLoweringError> {
            if recipes
                .iter()
                .any(|recipe| recipe.terminal() != artifact.view().identity())
            {
                return Err(ArtifactLoweringError::TargetFamily(
                    "private entry RAM recipe belongs to another Terminal artifact".into(),
                ));
            }
            if !recipes.is_empty()
                && recipes
                    .iter()
                    .any(|recipe| recipe.layouts() != &self.layouts)
            {
                return Err(ArtifactLoweringError::TargetFamily(
                    "private entry RAM recipe belongs to another target or selected geometry"
                        .into(),
                ));
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
                    Ok(TerminalAccessViewEstablishment {
                        input: recipe.input().clone(),
                        referent: terminal_installation::TerminalStructuralValue::minted(
                            identity,
                            recipe.schema(),
                            Vec::new(),
                        ),
                    })
                })
                .collect::<Result<Vec<_>, ArtifactLoweringError>>()?;
            let mut input = self.lower(artifact, &establishments, selection, evidence)?;
            input.private_entry_ram = recipes.to_vec();
            Ok(input)
        }
    }

    impl VerifiedNativeArtifactInput {
        pub(crate) fn bind_selected_provider_coordinates(
            mut self,
            artifact: &terminal_codec::CanonicalTerminalArtifact,
            selected: &effects::SelectedProviderPlanFacts,
        ) -> Result<Self, String> {
            let selections = self
                .optimization_input
                .target_machine_selections()
                .ok_or_else(|| "selected family providers require target selection".to_owned())?;
            for row in selected.family_realization_rows() {
                let member = selections
                    .member(terminal_psi::TargetMachineCallable::Boundary(row.head))
                    .map_err(|error| error.to_string())?;
                let module = artifact.semantic_module();
                if row.terminal != artifact.manifest().semantic()
                    || row.terminal != selections.terminal_psi()
                    || row.target != selections.target()
                    || member.callable()
                        != terminal_psi::TargetMachineCallable::Boundary(row.member)
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
            self.optimization_input
                .retain_selected_providers(
                    selected.family_realization_rows().to_vec(),
                    selected.external_requirement_applications().to_vec(),
                    selected.identities().clone(),
                    *selected.identity_digest().as_bytes(),
                )
                .map_err(str::to_owned)?;
            Ok(self)
        }

        pub const fn context(&self) -> &VerifiedPsiOptimizationContext {
            self.optimization_input.context()
        }

        pub(crate) fn retain_admitted_assumed_bindings(
            &mut self,
            calls: Vec<installation_evidence::InstalledAssumedBindingEvidence>,
        ) {
            self.optimization_input
                .retain_admitted_assumed_bindings(calls);
        }

        pub(crate) fn retain_admitted_native_callbacks(
            &mut self,
            callbacks: provider_admission::AdmittedNativeCallbackCohort,
        ) {
            self.optimization_input
                .retain_admitted_native_callbacks(callbacks);
        }

        pub(crate) fn retain_admitted_installed_interrupts(
            &mut self,
            entries: provider_admission::AdmittedInstalledInterruptCohort,
        ) {
            self.optimization_input
                .retain_admitted_installed_interrupts(entries);
        }

        pub const fn optimization_input(&self) -> &VerifiedPsiOptimizationInput {
            &self.optimization_input
        }

        /// Attaches seals that name exactly the module this input was
        /// lowered from, as `lower_module_with_evidence` does for a module
        /// lowered on its own: the product's application is lowered from
        /// the artifact in memory, so its seals are still beside it.
        pub fn attach_evidence(
            &mut self,
            evidence: terminal_codec::EvidenceSeals,
        ) -> Result<(), &'static str> {
            if self.optimization_input.plan().psi != evidence.module {
                return Err("the evidence seals name a different module than the one lowered");
            }
            self.optimization_input
                .attach_in_memory_evidence(evidence.seals);
            Ok(())
        }

        pub fn access_view_establishments(&self) -> &[TerminalAccessViewEstablishment] {
            &self.access_view_establishments
        }

        pub fn into_optimization_input(self) -> VerifiedPsiOptimizationInput {
            self.optimization_input
        }
    }

    #[derive(Debug, Clone)]
    pub struct AdmittedNativeArtifact {
        optimization_input: VerifiedPsiOptimizationInput,
    }

    impl AdmittedNativeArtifact {
        /// Attaches seals that name exactly this module's identity.
        pub(crate) fn attach_evidence(
            &mut self,
            evidence: terminal_codec::EvidenceSeals,
        ) -> Result<(), ArtifactLoweringError> {
            if self.optimization_input.plan().psi != evidence.module {
                return Err(ArtifactLoweringError::EvidenceForAnotherModule);
            }
            self.optimization_input
                .attach_in_memory_evidence(evidence.seals);
            Ok(())
        }

        pub fn plan(&self) -> &AbstractOperationPlan {
            self.optimization_input.plan()
        }

        pub const fn context(&self) -> &VerifiedPsiOptimizationContext {
            self.optimization_input.context()
        }

        pub fn access_view_inputs(&self) -> &[terminal_psi::TerminalAccessViewInput] {
            &self.context().module().access_view_inputs
        }

        pub fn into_optimization_artifact(self) -> AdmittedOptimizationArtifact {
            AdmittedOptimizationArtifact {
                input: self.optimization_input,
            }
        }

        pub fn into_plan(self) -> AbstractOperationPlan {
            self.optimization_input.into_plan()
        }

        pub(crate) fn try_into_native_input(
            self,
            establishments: &[TerminalAccessViewEstablishment],
        ) -> Result<VerifiedNativeArtifactInput, ArtifactLoweringError> {
            let access_view_establishments =
                super::establishment::establish_native_access_view_inputs(
                    self.context().module(),
                    self.plan().entry,
                    establishments,
                )?;
            Ok(VerifiedNativeArtifactInput {
                optimization_input: self.optimization_input,
                access_view_establishments,
                private_entry_ram: Vec::new(),
            })
        }
    }

    pub(super) fn lower_decoded_native_module(
        module: &terminal_psi::TerminalModule,
        entry: semantic_vocabulary::MachineId,
        selections: Option<AdmittedTargetMachineSelections>,
    ) -> Result<AdmittedNativeArtifact, ArtifactLoweringError> {
        let plan = lower_decoded_module(module, entry).map_err(ArtifactLoweringError::Lowering)?;
        Ok(retain_native_plan(module, plan, selections))
    }

    fn retain_native_plan(
        module: &terminal_psi::TerminalModule,
        plan: AbstractOperationPlan,
        selections: Option<AdmittedTargetMachineSelections>,
    ) -> AdmittedNativeArtifact {
        AdmittedNativeArtifact {
            optimization_input: VerifiedPsiOptimizationInput::lowered(
                plan,
                module.clone(),
                selections,
            ),
        }
    }

    #[cfg(test)]
    mod private_entry_target_tests {
        use super::*;

        #[test]
        #[ignore = "explicit qualification consumes OMEGA_ENTRY_RAM_CANONICAL_ARTIFACT"]
        fn checked_private_entry_recipe_cannot_rebind_target() {
            let path = std::env::var_os("OMEGA_ENTRY_RAM_CANONICAL_ARTIFACT")
                .expect("qualification must supply the actual canonical artifact");
            let bytes = std::fs::read(path).expect("read actual retained canonical bytes");
            let artifact = terminal_codec::CanonicalTerminalArtifact::from_bytes(&bytes)
                .expect("independent canonical admission");
            let Some(terminal_psi::TargetMachineCallable::Machine(entry)) =
                artifact.semantic_module().entry
            else {
                panic!("fixture must select an ordinary machine")
            };
            let row = artifact
                .semantic_module()
                .access_view_inputs
                .iter()
                .find(|row| row.machine == entry)
                .expect("actual direct placed row")
                .clone();
            let selections = terminal_installation::admit_target_machine_selections(
                artifact.view(),
                "linux_x86_64",
            );
            let layouts = terminal_installation::admit_target_layouts(
                artifact.view(),
                &selections,
                &target::TargetProfile::LinuxX64
                    .layout_target_facts()
                    .expect("Linux catalog geometry"),
            )
            .expect("actual Linux selected geometry");
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
            {
                let profile = target::TargetProfile::MacosArm64;
                let other_selections = terminal_installation::admit_target_machine_selections(
                    artifact.view(),
                    profile.target_name(),
                );
                let other = terminal_installation::admit_target_layouts(
                    artifact.view(),
                    &other_selections,
                    &profile
                        .layout_target_facts()
                        .expect("other catalog geometry"),
                )
                .expect("other target has independently admitted geometry");
                assert_ne!(recipe.layouts(), &other);
                let failure = VerifiedNativeArtifactInput::prepare_with_private_entry_ram(
                    &artifact,
                    profile,
                    terminal_psi::TargetMachineCallable::Machine(entry),
                    std::slice::from_ref(&recipe),
                    &NativeExecutionSelection {
                        scope: None,
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

    #[cfg(test)]
    mod scope_tests {
        use super::*;
        use semantic_vocabulary::{
            BlockId, ContractId, EdgeId, IntegerSign, IntegerType, IntegerValue, MachineId,
            OperationId, ScalarType, ValueId,
        };
        use terminal_codec::CanonicalTerminalArtifact;
        use terminal_psi::{
            Block, MachineContract, Operation, OperationKind, OperationResult, TerminalMachine,
            TerminalMachineResult, TerminalModule, Terminator, ValueDeclaration,
        };

        fn machine_id(value: u64) -> MachineId {
            MachineId::new(value).unwrap()
        }

        fn scalar_type() -> ScalarType {
            ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 64).unwrap())
        }

        fn machine(identity: u64, kind: OperationKind) -> TerminalMachine {
            let result = ValueDeclaration {
                id: ValueId::new(1).unwrap(),
                scalar_type: scalar_type(),
                qualifications: Default::default(),
            };
            TerminalMachine {
                id: machine_id(identity),
                attachment: None,
                nominal_cleanup_subject: None,
                structural_places: Vec::new(),
                block_entry_claims: Vec::new(),
                loan_completions: Vec::new(),
                entry: BlockId::new(1).unwrap(),
                blocks: vec![Block {
                    id: BlockId::new(1).unwrap(),
                    parameters: Vec::new(),
                    erased_scalar_formals: Vec::new(),
                    erased_proof_formals: Vec::new(),
                    structural_parameters: Vec::new(),
                    operations: vec![Operation::unsuspended(
                        OperationId::new(1).unwrap(),
                        OperationResult::Scalar(result),
                        kind,
                    )],
                    terminator: Terminator::Return {
                        edge: EdgeId::new(1).unwrap(),
                        value: result.id,
                        cleanup_actions: Vec::new(),
                    },
                }],
                ranked_scc: None,
                content_identity_reshuffles: Vec::new(),
                content_partition_compositions: Vec::new(),
                contract: terminal_psi::MachineContract {
                    selected_application: None,
                    declaration: None,
                    parameters: Vec::new(),
                    parameter_order: Vec::new(),
                    structural_parameters: Vec::new(),
                    entry_claims: Vec::new(),
                    content_entry_claims: Vec::new(),
                    result: TerminalMachineResult::Scalar(result),
                    declared_service_reach: None,
                    closed_reach_application: None,
                    published_synchronous_entries: Vec::new(),
                    published_service_ceiling: Vec::new(),
                    ..MachineContract::empty(ContractId::new(identity).unwrap())
                },
                assembly_target_requirement: terminal_psi::RegisterTargetApplicability::Any,
            }
        }

        fn call(callee: u64) -> OperationKind {
            OperationKind::Call {
                callee: machine_id(callee),
                arguments: Vec::new(),
                erased_arguments: Vec::new(),
                erased_proof_arguments: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            }
        }

        fn graph() -> TerminalModule {
            let mut module = TerminalModule::for_entry(machine_id(1));
            let application = module
                .scalar_applications
                .insert(terminal_psi::ScalarApplication::Evaluate {
                    entry: terminal_psi::TargetMachineCallable::Machine(machine_id(1)),
                    scalar_type: scalar_type(),
                })
                .unwrap();
            module.machines = vec![
                machine(
                    1,
                    OperationKind::IntegerConstant {
                        value: IntegerValue::Unsigned(7),
                    },
                ),
                machine(2, call(3)),
                machine(3, OperationKind::ReadScalarApplication { application }),
            ];
            module
        }

        fn prepare(
            module: TerminalModule,
        ) -> Result<VerifiedNativeArtifactInput, ArtifactLoweringError> {
            prepare_at(module, target::TargetProfile::MacosArm64)
        }

        fn prepare_at(
            module: TerminalModule,
            profile: target::TargetProfile,
        ) -> Result<VerifiedNativeArtifactInput, ArtifactLoweringError> {
            let record =
                terminal_codec::build_identity_optimization_execution_record(&module).unwrap();
            let artifact = CanonicalTerminalArtifact::from_parts(module, record, None).unwrap();
            let reloaded = CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes()).unwrap();
            VerifiedNativeArtifactInput::prepare(
                &reloaded,
                profile,
                reloaded
                    .semantic_module()
                    .entry
                    .expect("fixture declares an entry"),
                &[],
                &NativeExecutionSelection {
                    scope: None,
                    scalar_applications: None,
                    providers: &[],
                    plans: &effects::SelectedProviderPlanFacts::default(),
                    callbacks: &[],
                    entry_call: None,
                },
            )
        }

        #[test]
        fn raw_native_input_keeps_full_module_checking_without_a_selected_scope() {
            assert!(matches!(
                prepare(graph()),
                Err(ArtifactLoweringError::Lowering(
                    crate::LoweringError::UnclosedScalarApplication(_)
                ))
            ));
            let mut module = graph();
            module.machines.truncate(1);
            let input = prepare(module).unwrap();
            let plan = input.plan().clone();
            let facts = input.into_optimization_input().into_lowering_facts();
            facts
                .require_lowering_context(&plan, target::NativeTarget::macos_arm64())
                .unwrap();
            assert!(
                facts
                    .native_execution_scope_for(
                        plan.psi,
                        target::NativeTarget::macos_arm64(),
                        plan.entry,
                        *effects::SelectedProviderPlanFacts::default()
                            .identity_digest()
                            .as_bytes()
                    )
                    .unwrap()
                    .is_none()
            );
            assert!(
                facts
                    .require_lowering_context(&plan, target::NativeTarget::windows_x64())
                    .is_err()
            );
            let mut other_entry = plan.clone();
            other_entry.entry = machine_id(2);
            assert!(
                facts
                    .require_lowering_context(&other_entry, target::NativeTarget::macos_arm64())
                    .is_err()
            );
        }

        #[test]
        fn optimizer_only_facts_do_not_claim_an_empty_native_scope() {
            let mut module = graph();
            module.machines.truncate(1);
            let bytes = terminal_codec::encode_module(&module).unwrap();
            let input = crate::lower_artifact(crate::ArtifactSections {
                semantic_bytes: &bytes,
            })
            .unwrap()
            .into_optimization_artifact()
            .into_optimization_input();
            let plan = input.plan().clone();
            let facts = input.into_lowering_facts();
            assert!(
                facts
                    .native_execution_scope_for(
                        plan.psi,
                        target::NativeTarget::macos_arm64(),
                        plan.entry,
                        [0; 32],
                    )
                    .unwrap()
                    .is_none(),
                "no native admission must remain distinct from an empty selection"
            );
        }
    }
}
mod retention {
    use abstract_operations::AbstractOperationPlan;
    use verified_optimization_input::{
        VerifiedPsiOptimizationContext, VerifiedPsiOptimizationInput,
    };

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct AdmittedOptimizationArtifact {
        pub(super) input: VerifiedPsiOptimizationInput,
    }

    impl AdmittedOptimizationArtifact {
        pub const fn plan(&self) -> &AbstractOperationPlan {
            self.input.plan()
        }

        pub const fn context(&self) -> &VerifiedPsiOptimizationContext {
            self.input.context()
        }

        pub fn into_optimization_input(self) -> VerifiedPsiOptimizationInput {
            self.input
        }
    }
}
mod scalar_applications {
    use terminal_installation::{
        AdmittedScalarApplications, AdmittedTargetLayouts, AdmittedTargetMachineSelections,
    };

    pub struct NativeScalarApplications<'request> {
        pub completed: &'request AdmittedScalarApplications,
        pub provider_declarations: &'request [backend_plan::TerminalTargetProviderPlan],
    }

    impl<'request> NativeScalarApplications<'request> {
        pub(super) fn bind(
            &self,
            artifact: &terminal_codec::CanonicalTerminalArtifact,
            profile: target::TargetProfile,
            targets: &AdmittedTargetMachineSelections,
            layouts: &AdmittedTargetLayouts,
            selection: &super::NativeExecutionSelection<'_>,
            scalar_demand: &[semantic_vocabulary::ScalarApplicationId],
            execution_machines: &[semantic_vocabulary::MachineId],
        ) -> Result<&'request AdmittedScalarApplications, String> {
            let completed = self.completed;
            completed.require_product_formation()?;
            let primary = completed.primary();
            if completed.terminal() != artifact.view().identity()
                || primary.targets() != Some(targets)
                || primary.layouts() != Some(layouts)
            {
                return Err(
                    "native scalar closure belongs to another artifact, target or layout selection"
                        .into(),
                );
            }
            for &machine in execution_machines {
                if artifact
                    .semantic_module()
                    .machine(machine)
                    .is_some_and(|row| row.declaration.is_some())
                    && !primary.admits_machine(artifact.semantic_module(), machine)
                {
                    return Err("native execution reaches an authored body outside the completed scalar application closure".into());
                }
            }
            for inputs in std::iter::once(primary).chain(completed.supplemental()) {
                let selected_target = inputs
                    .targets()
                    .ok_or("native scalar closure has no deployment selection")?
                    .target();
                let profile = target::TargetProfile::from_canonical_target_name(selected_target)
                    .map_err(|error| format!("scalar closure address-model target: {error:?}"))?;
                if inputs.carrier_layout()
                    != profile
                        .native_realization()
                        .map(runtime_abi::target_carrier_layout)
                {
                    return Err(format!(
                        "scalar closure for {} has different target carrier geometry",
                        selected_target
                    ));
                }
                let Some(model) = inputs.address_model() else {
                    continue;
                };
                let expected = target::TargetSemantics::close_address_model(profile)
                    .ok_or("scalar closure requires an unavailable address model")?;
                if model.target != profile.target_name()
                    || model.carrier != expected.carrier()
                    || model.semantics_version != expected.semantics_version()
                    || model.commitment != expected.commitment()
                {
                    return Err(format!(
                        "scalar closure for {} has a different target address model",
                        selected_target,
                    ));
                }
            }
            let providers = backend_plan::admit_selected_checked_providers(
                artifact.view(),
                profile.target_name(),
                selection.plans,
                self.provider_declarations,
            )?;
            if primary.providers() != &providers {
                return Err(
                    "native scalar closure differs from exact provider installation custody".into(),
                );
            }
            let full_adapters =
                crate::providers::provider_admission::retain_selected_provider_adapters(
                    selection.plans,
                    self.provider_declarations,
                )?;
            if selection
                .providers
                .iter()
                .any(|adapter| !full_adapters.contains(adapter))
            {
                return Err(
                    "native local provider differs from the completed scalar selection".into(),
                );
            }
            for &application in scalar_demand {
                if completed.values().value(application).is_none() {
                    return Err(format!(
                        "native execution requires uncompleted scalar application {application}"
                    ));
                }
            }
            Ok(completed)
        }
    }
}

pub(crate) use error::ArtifactLoweringError;
pub use establishment::TerminalAccessViewEstablishment;
pub(crate) use native::AdmittedNativeArtifact;
pub(crate) use native::NativeScopePreparation;
pub use native::{NativeExecutionSelection, VerifiedNativeArtifactInput};
pub use scalar_applications::NativeScalarApplications;

#[derive(Clone, Copy)]
pub struct ArtifactSections<'artifact> {
    pub semantic_bytes: &'artifact [u8],
}

pub fn lower_artifact(
    sections: ArtifactSections<'_>,
) -> Result<AdmittedNativeArtifact, ArtifactLoweringError> {
    let module = terminal_codec::decode_module(sections.semantic_bytes)
        .map_err(ArtifactLoweringError::SemanticDecode)?;
    lower_decoded_artifact(&module)
}

/// Lowers an in-memory module, from the lowered-form route, with the evidence
/// seals produced beside it. The module goes through the same canonical
/// encoding and decoding as `lower_artifact`, so it is validated the same way;
/// the seals are attached only when they name this module's identity. This is
/// the only way an input gets seals: an input lowered from bytes has
/// `ObligationEvidence::ReadFromBytes`.
pub fn lower_module_with_evidence(
    module: &terminal_psi::TerminalModule,
    evidence: terminal_codec::EvidenceSeals,
) -> Result<AdmittedNativeArtifact, ArtifactLoweringError> {
    let bytes =
        terminal_codec::encode_module(module).map_err(ArtifactLoweringError::SemanticDecode)?;
    let mut lowered = lower_artifact(ArtifactSections {
        semantic_bytes: &bytes,
    })?;
    lowered.attach_evidence(evidence)?;
    Ok(lowered)
}

pub(crate) fn lower_decoded_artifact(
    module: &terminal_psi::TerminalModule,
) -> Result<AdmittedNativeArtifact, ArtifactLoweringError> {
    let Some(terminal_psi::TargetMachineCallable::Machine(entry)) = module.entry else {
        return Err(ArtifactLoweringError::TargetSelectionRequired);
    };
    native::lower_decoded_native_module(module, entry, None)
}

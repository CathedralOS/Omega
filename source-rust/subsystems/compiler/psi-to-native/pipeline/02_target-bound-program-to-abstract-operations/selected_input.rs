//! Retain the authorized program and its selected execution scope through native lowering.
//! The checked access establishments belong to the same entry as the lowered plan.
mod error {
    use crate::operations::LoweringError;

    #[derive(Debug)]
    pub enum ArtifactLoweringError {
        #[cfg(any(test, feature = "test-support"))]
        TargetSelectionRequired,
        TargetContext(String),
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
        #[cfg(any(test, feature = "test-support"))]
        EvidenceForAnotherModule,
    }

    impl std::fmt::Display for ArtifactLoweringError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::TargetContext(reason) => f.write_str(reason),
                Self::Lowering(error) => write!(f, "{error}"),
                Self::AccessViewEstablishmentUnexpected { machine, position } => {
                    write!(f, "unexpected access view at {machine:?} input {position}")
                }
                Self::AccessViewEstablishmentDuplicate { machine, position } => {
                    write!(f, "duplicate access view at {machine:?} input {position}")
                }
                Self::AccessViewEstablishmentBackingUndeclared(ty) => {
                    write!(f, "access view backing type {ty:?} is undeclared")
                }
                Self::AccessViewEstablishmentQualificationUndeclared(domain) => {
                    write!(f, "access view qualification {domain:?} is undeclared")
                }
                Self::AccessViewEstablishmentQualificationCarrier(domain) => {
                    write!(
                        f,
                        "access view qualification {domain:?} has a different carrier"
                    )
                }
                Self::AccessViewEstablishmentAliasing(identity) => {
                    write!(f, "exclusive access views overlap on referent {identity}")
                }
                Self::AccessViewInputsRequireCustodyLowering => {
                    f.write_str("entry access view requires checked custody")
                }
                Self::AccessViewEstablishmentQualificationsNonCanonical => {
                    f.write_str("access view qualifications are noncanonical")
                }
                Self::AccessViewEstablishmentRangeUnresolved => {
                    f.write_str("access view range is unresolved")
                }
                #[cfg(any(test, feature = "test-support"))]
                Self::TargetSelectionRequired => f.write_str("a default machine entry is required"),
                #[cfg(any(test, feature = "test-support"))]
                Self::EvidenceForAnotherModule => {
                    f.write_str("evidence seals name a different module")
                }
            }
        }
    }

    impl std::error::Error for ArtifactLoweringError {}
}
mod establishment {
    pub use terminal_semantics::entry_values::TerminalAccessViewEstablishment;

    use super::ArtifactLoweringError;
    use std::collections::BTreeSet;
    use terminal_semantics::entry_values::TerminalStructuralValue;
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
            .access.access_view_inputs
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
    use abstract_operations::AbstractOperationPlan;
    use std::borrow::Cow;
    use psi_target_layouts::TerminalTargetContext;
    use psi_scalar_applications::AdmittedScalarApplications;
    use terminal_semantics::entry_values::TerminalAccessViewEstablishment;
    use verified_optimization_input::{
        VerifiedPsiOptimizationContext, VerifiedPsiOptimizationInput,
    };

    #[derive(Debug, Clone)]
    pub struct VerifiedNativeArtifactInput {
        optimization_input: VerifiedPsiOptimizationInput,
        access_view_establishments: Vec<TerminalAccessViewEstablishment>,
        private_entry_ram: Vec<psi_entry_memory::CheckedPrivateEntryRamRecipe>,
    }

    pub struct NativeExecutionSelection<'request> {
        pub scope: Option<&'request crate::NativeExecutionScope>,
        pub scalar_applications: Option<super::NativeScalarApplications<'request>>,
        pub providers: &'request [crate::SelectedProviderAdapter],
        pub plans: &'request selected_provider_plans::SelectedProviderPlanFacts,
        pub callbacks:
            &'request [provider_correspondence::callbacks::TerminalCallbackOccurrenceProposal],
        pub entry_call: Option<&'request crate::SelectedProgramEntryCall>,
    }

    impl<'request> NativeExecutionSelection<'request> {
        pub(crate) fn checked(
            scope: &'request crate::NativeExecutionScope,
            plans: &'request selected_provider_plans::SelectedProviderPlanFacts,
            scalar_applications: Option<super::NativeScalarApplications<'request>>,
        ) -> Self {
            Self {
                scope: Some(scope),
                scalar_applications,
                providers: scope.local_adapters(),
                plans,
                callbacks: scope.callbacks(),
                entry_call: scope.entry_call(),
            }
        }
    }

    pub(crate) struct NativeScopePreparation<'request> {
        authorization: &'request psi_authorization::AuthorizedTerminalArtifact,
        selection: &'request NativeExecutionSelection<'request>,
        profile: target::TargetProfile,
        entry: semantic_vocabulary::MachineId,
        targets: TerminalTargetContext,
        layouts: Cow<'request, psi_target_layouts::AdmittedTargetLayouts>,
        scalar_demand: Cow<'request, [semantic_vocabulary::ScalarApplicationId]>,
    }

    impl<'request> NativeScopePreparation<'request> {
        pub(crate) fn prepare(
            authorization: &'request psi_authorization::AuthorizedTerminalArtifact,
            profile: target::TargetProfile,
            root: terminal_psi::TargetMachineCallable,
            selection: &'request NativeExecutionSelection<'request>,
        ) -> Result<Self, ArtifactLoweringError> {
            let artifact = authorization.artifact();
            let module = artifact.semantic_module();
            let selections = selection.scope.map_or_else(
                || {
                    psi_target_layouts::terminal_target_context(artifact.view(), profile.target_name())
                },
                |scope| scope.reach().target_context().clone(),
            );
            let entry = selections
                .entry(artifact.view().identity(), module, root)
                .map_err(|error| ArtifactLoweringError::TargetContext(error.to_string()))?;
            let (layouts, scalar_demand) = if let Some(scope) = selection.scope {
                scope
                    .require_source_input(
                        artifact,
                        entry,
                        profile,
                        selection.plans,
                        selection.providers,
                    )
                    .map_err(|reason| ArtifactLoweringError::TargetContext(reason.into()))?;
                if scope.entry_call() != selection.entry_call
                    || scope.callbacks() != selection.callbacks
                {
                    return Err(ArtifactLoweringError::TargetContext(
                        "native preparation changes its checked entry or callback inputs".into(),
                    ));
                }
                (
                    Cow::Borrowed(scope.reach().layouts()),
                    Cow::Borrowed(scope.reach().scalar_application_demand()),
                )
            } else {
                profile
                    .admit_address_representation()
                    .map_err(ArtifactLoweringError::TargetContext)?;
                let facts = profile.layout_target_facts().ok_or_else(|| {
                    ArtifactLoweringError::TargetContext(
                        "selected native target has no representation facts".into(),
                    )
                })?;
                let layouts =
                    psi_target_layouts::admit_target_layouts(artifact.view(), &selections, &facts)
                        .map_err(|error| ArtifactLoweringError::TargetContext(error.to_string()))?;
                let demand = target_bound_program::TargetBoundReach::full_module_scalar_demand(module, &layouts)
                    .map_err(ArtifactLoweringError::TargetContext)?;
                (Cow::Owned(layouts), Cow::Owned(demand))
            };
            Ok(Self {
                authorization,
                selection,
                profile,
                entry,
                targets: selections,
                layouts,
                scalar_demand,
            })
        }

        pub(crate) fn lower(
            self,
            establishments: &[TerminalAccessViewEstablishment],
            scalar_providers: &[crate::SelectedProviderAdapter],
            evidence: Option<&terminal_codec::EvidenceSeals>,
        ) -> Result<VerifiedNativeArtifactInput, ArtifactLoweringError> {
            let Self {
                authorization,
                profile,
                entry,
                targets: selections,
                layouts,
                selection,
                scalar_demand,
            } = self;
            let artifact = authorization.artifact();
            let scope = selection.scope;
            let module = artifact.semantic_module();
            let full_machines;
            let machines = match scope {
                Some(scope) => scope.reach().machines(),
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
                    ArtifactLoweringError::TargetContext(format!(
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
                        selection.plans,
                        scalar_providers,
                        &scalar_demand,
                        machines,
                    )
                })
                .transpose()
                .map_err(ArtifactLoweringError::TargetContext)?;
            let mut structural_types = module.types.structural_types.clone();
            let geometry = scalar_applications.map_or_else(
                || layouts.resolved_records(),
                |closed| closed.geometry().as_ref(),
            );
            for resolved in geometry {
                let declaration = structural_types
                    .iter_mut()
                    .find(|row| row.id == resolved.id)
                    .ok_or_else(|| {
                        ArtifactLoweringError::TargetContext(
                            "selected layout lost its executable structural declaration".into(),
                        )
                    })?;
                *declaration = resolved.clone();
            }
            let plan = crate::operations::lower_selected_module(
                module,
                entry,
                structural_types,
                scope.map(crate::NativeExecutionScope::reach),
                scalar_applications,
                Some(&layouts),
            )
            .map_err(ArtifactLoweringError::Lowering)?;
            let optimization_input = VerifiedPsiOptimizationInput::lowered(
                plan,
                authorization.clone(),
                Some(selections),
            )
            .map_err(|reason| ArtifactLoweringError::TargetContext(reason.to_owned()))?;
            let access_view_establishments =
                super::establishment::establish_native_access_view_inputs(
                    optimization_input.context().module(),
                    optimization_input.plan().entry,
                    establishments,
                )?;
            let mut input = VerifiedNativeArtifactInput {
                optimization_input,
                access_view_establishments,
                private_entry_ram: Vec::new(),
            };
            input
                .optimization_input
                .retain_native_selections(
                    selection.entry_call.cloned(),
                    scalar_applications.cloned(),
                    scope.cloned().map(std::sync::Arc::new),
                    scope.map_or_else(
                        || *selection.plans.identity_digest().as_bytes(),
                        crate::NativeExecutionScope::selected_provider_closure,
                    ),
                )
                .map_err(|reason| ArtifactLoweringError::TargetContext(reason.into()))?;
            if let Some(evidence) = evidence {
                input.attach_evidence(evidence).map_err(|error| {
                    ArtifactLoweringError::TargetContext(format!(
                        "native operation evidence: {error}"
                    ))
                })?;
            }
            Ok(input)
        }
    }

    impl VerifiedNativeArtifactInput {
        pub fn target_context(&self) -> Option<&TerminalTargetContext> {
            self.optimization_input.target_context()
        }

        pub fn scalar_applications(&self) -> Option<&AdmittedScalarApplications> {
            self.optimization_input.scalar_applications()
        }

        #[cfg(any(test, feature = "test-support"))]
        pub fn prepare(
            artifact: &psi_authorization::AuthorizedTerminalArtifact,
            profile: target::TargetProfile,
            root: terminal_psi::TargetMachineCallable,
            establishments: &[TerminalAccessViewEstablishment],
            selection: &NativeExecutionSelection<'_>,
        ) -> Result<Self, ArtifactLoweringError> {
            NativeScopePreparation::prepare(artifact, profile, root, selection)?.lower(
                establishments,
                selection.providers,
                None,
            )
        }

        pub fn private_entry_ram(&self) -> &[psi_entry_memory::CheckedPrivateEntryRamRecipe] {
            &self.private_entry_ram
        }

        #[cfg(any(test, feature = "test-support"))]
        pub fn prepare_with_private_entry_ram(
            artifact: &psi_authorization::AuthorizedTerminalArtifact,
            profile: target::TargetProfile,
            root: terminal_psi::TargetMachineCallable,
            recipes: &[psi_entry_memory::CheckedPrivateEntryRamRecipe],
            selection: &NativeExecutionSelection<'_>,
        ) -> Result<Self, ArtifactLoweringError> {
            NativeScopePreparation::prepare(artifact, profile, root, selection)?
                .lower_with_private_entry_ram(recipes, selection.providers, None)
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
                closed_module: input.context().closed_module(),
                target_context: input.target_context(),
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

    impl NativeScopePreparation<'_> {
        pub(crate) fn lower_with_private_entry_ram(
            self,
            recipes: &[psi_entry_memory::CheckedPrivateEntryRamRecipe],
            scalar_providers: &[crate::SelectedProviderAdapter],
            evidence: Option<&terminal_codec::EvidenceSeals>,
        ) -> Result<VerifiedNativeArtifactInput, ArtifactLoweringError> {
            let artifact = self.authorization.artifact();
            if recipes
                .iter()
                .any(|recipe| recipe.terminal() != artifact.view().identity())
            {
                return Err(ArtifactLoweringError::TargetContext(
                    "private entry RAM recipe belongs to another Terminal artifact".into(),
                ));
            }
            if !recipes.is_empty()
                && recipes
                    .iter()
                    .any(|recipe| recipe.layouts() != self.layouts.as_ref())
            {
                return Err(ArtifactLoweringError::TargetContext(
                    "private entry RAM recipe belongs to another target or selected geometry"
                        .into(),
                ));
            }
            let establishments = recipes
                .iter()
                .enumerate()
                .map(|(index, recipe)| {
                    let (identity, _) = psi_entry_memory::private_entry_ram_identity_pair(
                        artifact.view().module(),
                        index,
                    )
                    .ok_or_else(|| {
                        ArtifactLoweringError::TargetContext(
                            "private entry RAM identity namespace is exhausted".into(),
                        )
                    })?;
                    Ok(TerminalAccessViewEstablishment {
                        input: recipe.input().clone(),
                        referent: terminal_semantics::entry_values::TerminalStructuralValue::minted(
                            identity,
                            recipe.schema(),
                            Vec::new(),
                        ),
                    })
                })
                .collect::<Result<Vec<_>, ArtifactLoweringError>>()?;
            let mut input = self.lower(&establishments, scalar_providers, evidence)?;
            input.private_entry_ram = recipes.to_vec();
            Ok(input)
        }
    }

    impl VerifiedNativeArtifactInput {
        pub(crate) fn bind_selected_provider_coordinates(
            mut self,
            artifact: &terminal_codec::CanonicalTerminalArtifact,
            selected: &selected_provider_plans::SelectedProviderPlanFacts,
        ) -> Result<Self, String> {
            self.context()
                .authorization()
                .require_artifact(artifact)
                .map_err(|error| error.to_string())?;
            self.optimization_input
                .retain_selected_providers(
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
            calls: Vec<operation_evidence::provider_calls::InstalledAssumedBindingEvidence>,
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

        /// Retains optional seals for this exact authorized module. Seals do
        /// not replace the host producer authorization already held by input.
        pub fn attach_evidence(
            &mut self,
            evidence: terminal_codec::EvidenceSeals,
        ) -> Result<(), &'static str> {
            if self.optimization_input.plan().psi != evidence.module {
                return Err("the evidence seals name a different module than the one lowered");
            }
            self.optimization_input
                .retain_evidence_seals(evidence.seals);
            Ok(())
        }

        pub fn access_view_establishments(&self) -> &[TerminalAccessViewEstablishment] {
            &self.access_view_establishments
        }

        pub fn into_optimization_input(self) -> VerifiedPsiOptimizationInput {
            self.optimization_input
        }
    }

    #[cfg(test)]
    fn authorize_test_artifact(
        artifact: &terminal_codec::CanonicalTerminalArtifact,
    ) -> psi_authorization::AuthorizedTerminalArtifact {
        let producer = psi_authorization::HostProducerTrust::accept("controlled native fixture");
        psi_authorization::authorize_terminal_artifact(
            std::sync::Arc::new(artifact.clone()),
            Some(&producer),
        )
        .unwrap()
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
                .access.access_view_inputs
                .iter()
                .find(|row| row.machine == entry)
                .expect("actual direct placed row")
                .clone();
            let selections =
                psi_target_layouts::terminal_target_context(artifact.view(), "linux_x86_64");
            let layouts = psi_target_layouts::admit_target_layouts(
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
                let failure = psi_entry_memory::admit_private_entry_ram(
                    artifact.view(),
                    &layouts,
                    entry,
                    &psi_entry_memory::PrivateEntryRamRecipe {
                        input,
                        contents: vec![41, 0, 0, 0, 3, 0, 0, 0],
                    },
                )
                .expect_err("changed row must not lend private RAM");
                assert!(failure.contains("exact borrowed entry row"), "{failure}");
            }
            let recipe = psi_entry_memory::admit_private_entry_ram(
                artifact.view(),
                &layouts,
                entry,
                &psi_entry_memory::PrivateEntryRamRecipe {
                    input: row,
                    contents: vec![41, 0, 0, 0, 3, 0, 0, 0],
                },
            )
            .expect("complete real checked typed initializer");
            assert_eq!(recipe.layouts(), &layouts);
            assert_eq!(recipe.target(), "linux_x86_64");
            {
                let profile = target::TargetProfile::MacosArm64;
                let other_selections = psi_target_layouts::terminal_target_context(
                    artifact.view(),
                    profile.target_name(),
                );
                let other = psi_target_layouts::admit_target_layouts(
                    artifact.view(),
                    &other_selections,
                    &profile
                        .layout_target_facts()
                        .expect("other catalog geometry"),
                )
                .expect("other target has independently admitted geometry");
                assert_ne!(recipe.layouts(), &other);
                let failure = VerifiedNativeArtifactInput::prepare_with_private_entry_ram(
                    &authorize_test_artifact(&artifact),
                    profile,
                    terminal_psi::TargetMachineCallable::Machine(entry),
                    std::slice::from_ref(&recipe),
                    &NativeExecutionSelection {
                        scope: None,
                        scalar_applications: None,
                        providers: &[],
                        plans: &selected_provider_plans::SelectedProviderPlanFacts::default(),
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
                structural_places: Vec::new(),
                call_formations: Vec::new(),
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
                .scalars.scalar_applications
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
                &authorize_test_artifact(&reloaded),
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
                    plans: &selected_provider_plans::SelectedProviderPlanFacts::default(),
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
                    crate::operations::LoweringError::UnclosedScalarApplication(_)
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
                        *selected_provider_plans::SelectedProviderPlanFacts::default()
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
            assert!(
                facts
                    .borrowed_storage_layouts(target::NativeTarget::macos_arm64())
                    .is_ok()
            );
            assert_eq!(
                facts.borrowed_storage_layouts(target::NativeTarget::windows_x64()),
                Err("lowering facts belong to another target selection"),
            );
            let mut other_entry = plan.clone();
            other_entry.entry = machine_id(2);
            assert!(
                facts
                    .require_lowering_context(&other_entry, target::NativeTarget::macos_arm64())
                    .is_err()
            );
        }

        // The source compiler lowers authored overlays to byte-region observations.
        // A separately produced Psi artifact can still request a record copy directly.
        fn byte_record(field: terminal_psi::StructuralFieldType) -> TerminalModule {
            use semantic_vocabulary::{
                PlaceId, StructuralFieldId, StructuralPlaceKind, StructuralTypeId,
            };
            use terminal_psi::{
                ArrayExtent, BindingRelevance, BoundaryParameterKind, StructuralAccess,
                StructuralFieldDeclaration, StructuralMultiplicity, StructuralOperationResult,
                StructuralParameterDeclaration, StructuralPathSegment, StructuralPlaceDeclaration,
                StructuralRecordStorage, StructuralTypeDeclaration, StructuralTypeShape,
                TerminalIdentityTableBuilder,
            };

            let mut names = TerminalIdentityTableBuilder::default();
            let byte = StructuralTypeId::new(1).unwrap();
            let array = StructuralTypeId::new(2).unwrap();
            let record = StructuralTypeId::new(3).unwrap();
            let field = StructuralFieldDeclaration {
                declaration_identity: Default::default(),
                inline_carrier: None,
                id: StructuralFieldId::new(1).unwrap(),
                identity: names.intern("digit"),
                relevance: BindingRelevance::Relevant,
                field_type: field,
                storage: None,
            };
            let mut module = TerminalModule::for_entry(machine_id(1));
            module.layouts.native_layouts.insert(
                record,
                names.intern("Record"),
                terminal_psi::TerminalIdentityIndex::ABSENT,
                terminal_psi::TerminalIdentityIndex::ABSENT,
                [terminal_psi::NativeLayoutFieldIdentity {
                    field: field.id,
                    declaration_identity: field.identity,
                }],
                [],
            );
            let mut declaration = |id, name, shape| StructuralTypeDeclaration {
                declaration_identity: Default::default(),
                quotient_carrier: None,
                id,
                identity: names.intern(name),
                owns_linear_claim: false,
                multiplicity: StructuralMultiplicity::Unrestricted,
                no_code_affine_cleanup: false,
                descriptor_cleanup: false,
                nominal_cleanup: None,
                shape,
                scalar_restriction: None,
                requires: Vec::new(),
            };
            module.types.structural_types = vec![
                declaration(
                    byte,
                    "u8",
                    StructuralTypeShape::PrimitiveScalar(ScalarType::Integer(
                        IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
                    )),
                ),
                declaration(
                    array,
                    "Bytes",
                    StructuralTypeShape::FixedArray {
                        element: byte,
                        length: ArrayExtent::Literal(2),
                    },
                ),
                declaration(
                    record,
                    "Record",
                    StructuralTypeShape::Record {
                        fields: vec![field],
                        storage: StructuralRecordStorage::Packed,
                    },
                ),
            ];
            module.identities = names.build();
            let mut entry = machine(
                1,
                OperationKind::IntegerConstant {
                    value: IntegerValue::Unsigned(0),
                },
            );
            let source = PlaceId::new(1).unwrap();
            let copied = PlaceId::new(2).unwrap();
            let producer = OperationId::new(2).unwrap();
            entry
                .parameter_order
                .push(BoundaryParameterKind::Structural);
            entry
                .structural_parameters
                .push(StructuralParameterDeclaration {
                    place: source,
                    position: 0,
                    structural_type: array,
                    multiplicity: StructuralMultiplicity::Unrestricted,
                    access: StructuralAccess::SharedBorrow,
                    qualifications: vec![],
                    projected_qualifications: vec![],
                    additional_input_qualifications: vec![],
                });
            entry.structural_places = vec![
                StructuralPlaceDeclaration {
                    id: source,
                    kind: StructuralPlaceKind::Parameter { position: 0 },
                },
                StructuralPlaceDeclaration {
                    id: copied,
                    kind: StructuralPlaceKind::OperationResult {
                        producer,
                        structural_type: record,
                    },
                },
            ];
            entry.blocks[0].operations.push(Operation::unsuspended(
                producer,
                OperationResult::Structural(StructuralOperationResult {
                    place: copied,
                    structural_type: record,
                    multiplicity: StructuralMultiplicity::Unrestricted,
                    qualifications: vec![],
                    projected_qualifications: vec![],
                    additional_input_qualifications: vec![],
                    qualification_establishments: vec![],
                    claims: vec![],
                }),
                OperationKind::StructuralLeafCopy {
                    source,
                    path: vec![StructuralPathSegment::FixedIndex(1)],
                },
            ));
            module.machines.push(entry);
            module
        }

        #[test]
        fn byte_record_requires_every_field_to_accept_arbitrary_bytes() {
            use semantic_vocabulary::BoundedIntegerType;
            use terminal_psi::StructuralFieldType;

            let byte = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
            let plain = byte_record(StructuralFieldType::Scalar(ScalarType::Integer(byte)));
            let input = prepare(plain.clone()).unwrap();
            assert!(
                input.plan().functions[0].blocks[0]
                    .operations
                    .iter()
                    .any(|operation| {
                        matches!(
                            operation,
                            abstract_operations::AbstractOperation::StructuralLeafCopy { .. }
                        )
                    })
            );
            let mut constrained_record = plain;
            constrained_record.types.structural_types[2]
                .requires
                .push(semantic_vocabulary::Proposition::Falsehood);
            let digit =
                BoundedIntegerType::new(byte, IntegerValue::Unsigned(0), IntegerValue::Unsigned(9))
                    .unwrap();
            for module in [
                byte_record(StructuralFieldType::BoundedInteger(digit)),
                byte_record(StructuralFieldType::Scalar(ScalarType::Boolean)),
                constrained_record,
            ] {
                let result = prepare(module);
                assert!(
                    matches!(
                        &result,
                        Err(ArtifactLoweringError::Lowering(
                            crate::operations::LoweringError::InvalidStructuralLeafCopy {
                                reason: "raw bytes do not establish the record's required values",
                                ..
                            }
                        ))
                    ),
                    "expected a byte-record validity refusal, got {:?}",
                    result.err()
                );
            }
        }

        #[test]
        fn optimizer_only_facts_do_not_claim_an_empty_native_scope() {
            let mut module = graph();
            module.machines.truncate(1);
            let record =
                terminal_codec::build_identity_optimization_execution_record(&module).unwrap();
            let artifact = CanonicalTerminalArtifact::from_parts(module, record, None).unwrap();
            let input =
                crate::test_support::lower_artifact(&authorize_test_artifact(&artifact), None)
                    .unwrap();
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
mod scalar_applications {
    use psi_target_layouts::{AdmittedTargetLayouts, TerminalTargetContext};
    use psi_scalar_applications::AdmittedScalarApplications;

    pub struct NativeScalarApplications<'request> {
        pub completed: &'request AdmittedScalarApplications,
    }

    impl<'request> NativeScalarApplications<'request> {
        pub(super) fn bind(
            &self,
            artifact: &terminal_codec::CanonicalTerminalArtifact,
            profile: target::TargetProfile,
            targets: &TerminalTargetContext,
            layouts: &AdmittedTargetLayouts,
            plans: &selected_provider_plans::SelectedProviderPlanFacts,
            scalar_providers: &[crate::SelectedProviderAdapter],
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
            let providers = target_provider_admission::admit_selected_checked_providers(
                artifact.view(),
                profile.target_name(),
                plans,
            )?;
            if primary.providers() != &providers {
                return Err(
                    "native scalar closure differs from exact provider installation custody".into(),
                );
            }
            let full_adapters =
                provider_admission::retain_selected_provider_adapters(
                    plans,
                )?;
            if scalar_providers
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
pub(crate) use native::NativeScopePreparation;
pub use native::{NativeExecutionSelection, VerifiedNativeArtifactInput};
pub use scalar_applications::NativeScalarApplications;

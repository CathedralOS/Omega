/*
Catalog provider and assumed-binding installation (`provider_installation`),
component providers and memberships (`component_provider` and
`component_memberships`), and tagged target-machine family selection
(`target_families`).
*/

pub(crate) mod provider_installation {
    use semantic_vocabulary::{BoundaryMachineId, MachineId};
    use std::collections::{BTreeMap, BTreeSet};

    #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record)]
    pub struct ProviderInstallationSelection {
        pub boundary: BoundaryMachineId,
        pub provider_identity: String,
        pub provider_declaration: String,
        pub candidate: MachineId,
    }

    #[derive(
        Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compiler_storage::Record,
    )]
    pub struct AssumedBindingSelection {
        pub required: BoundaryMachineId,
        pub supplied_requirement: BoundaryMachineId,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct AdmittedAssumedBinding {
        pub required: BoundaryMachineId,
        pub supplied_requirement: BoundaryMachineId,
        pub implementation: MachineId,
        pub selected_requirement: BoundaryMachineId,
        pub supply_path: Vec<BoundaryMachineId>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct AdmittedProviderSelections {
        code_identity: terminal_codec::TerminalExecutionIdentity,
        providers: BTreeMap<BoundaryMachineId, AdmittedProviderSelection>,
        assumed_bindings: Vec<AdmittedAssumedBinding>,
        folded_constant_target: Option<String>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct AdmittedProviderSelection {
        candidate: MachineId,
        provider_declaration: terminal_psi::NominalDeclarationIdentity,
        declaration_application: terminal_psi::TerminalIdentityIndex,
        receiving_application: terminal_psi::TerminalIdentityIndex,
        receiving_region_arguments: Vec<terminal_psi::TerminalIdentityIndex>,
    }

    impl AdmittedProviderSelections {
        pub fn is_subset_of(&self, complete: &Self) -> bool {
            self.code_identity == complete.code_identity
                && self
                    .providers
                    .iter()
                    .all(|(boundary, selected)| complete.providers.get(boundary) == Some(selected))
                && self
                    .assumed_bindings
                    .iter()
                    .all(|assumed| complete.assumed_bindings.contains(assumed))
                && self
                    .folded_constant_target
                    .as_ref()
                    .is_none_or(|target| complete.folded_constant_target.as_ref() == Some(target))
        }

        pub fn admit_folded_constant_closure(
            mut self,
            artifact: terminal_codec::TerminalArtifactView<'_>,
            target: &str,
            plan_preimages: &[Vec<u8>],
        ) -> Result<Self, String> {
            if self.code_identity
                != terminal_codec::TerminalExecutionIdentity::portable(artifact.identity())
            {
                return Err("folded constant closure names another Terminal artifact".into());
            }
            let module = artifact.module();
            module
                .folded_constants
                .validate(module)
                .map_err(str::to_owned)?;
            if module.folded_constants.target.as_deref() != Some(target) {
                return Err("folded constant closure belongs to another target".into());
            }
            let expected: Vec<_> = module
                .folded_constants
                .dependencies
                .iter()
                .filter(|dependency| dependency.target == target)
                .cloned()
                .collect();
            module
                .folded_constants
                .validate_plan_inputs(target, plan_preimages)
                .map_err(str::to_owned)?;
            let mut historical = Vec::new();
            for dependency in &expected {
                let Some(association) = &dependency.candidate else {
                    continue;
                };
                let candidate = module
                    .provider_candidates
                    .iter()
                    .find(|candidate| {
                        candidate.boundary == association.boundary
                            && candidate.candidate == association.machine
                            && candidate.provider_declaration == association.declaration
                            && candidate.source_requirement_identity == dependency.requirement
                    })
                    .ok_or("folded constant historical candidate is absent")?;
                if self.providers.contains_key(&candidate.boundary)
                    && !self.contains_candidate(candidate)
                {
                    return Err(
                        "runtime provider substitutes the folded constant realization".into(),
                    );
                }
                historical.push(ProviderInstallationSelection {
                    boundary: candidate.boundary,
                    candidate: candidate.candidate,
                    provider_identity: module
                        .identities
                        .text(candidate.provider_identity)
                        .ok_or("historical provider identity is absent")?
                        .into(),
                    provider_declaration: module
                        .identities
                        .text(candidate.provider_declaration.identity())
                        .ok_or("historical selecting declaration is absent")?
                        .into(),
                });
            }
            historical.sort();
            historical.dedup();
            admit_provider_selections(
                module,
                terminal_codec::TerminalExecutionIdentity::portable(artifact.identity()),
                &historical,
                &[],
            )
            .map_err(|error| format!("folded constant catalog admission rejected: {error:?}"))?;
            self.folded_constant_target = Some(target.to_owned());
            Ok(self)
        }

        pub fn folded_constant_target(&self) -> Option<&str> {
            self.folded_constant_target.as_deref()
        }
        pub fn contains_candidate(
            &self,
            candidate: &terminal_psi::ProviderCandidateConformance,
        ) -> bool {
            self.providers
                .get(&candidate.boundary)
                .is_some_and(|installed| {
                    installed.candidate == candidate.candidate
                        && installed.provider_declaration == candidate.provider_declaration
                        && installed.declaration_application == candidate.declaration_application
                        && installed.receiving_application == candidate.receiving_application
                        && installed.receiving_region_arguments
                            == candidate.receiving_region_arguments
                })
        }

        pub const fn terminal_psi(&self) -> terminal_psi::TerminalPsiIdentity {
            self.code_identity.semantic()
        }

        pub fn execution_identity(&self) -> &terminal_codec::TerminalExecutionIdentity {
            &self.code_identity
        }

        pub fn assumed_bindings(&self) -> &[AdmittedAssumedBinding] {
            &self.assumed_bindings
        }

        pub fn receiving_application(
            &self,
            boundary: BoundaryMachineId,
        ) -> terminal_psi::TerminalIdentityIndex {
            self.providers.get(&boundary).map_or(
                terminal_psi::TerminalIdentityIndex::default(),
                |selection| selection.receiving_application,
            )
        }

        pub fn receiving_region_arguments(
            &self,
            boundary: BoundaryMachineId,
        ) -> &[terminal_psi::TerminalIdentityIndex] {
            self.providers.get(&boundary).map_or(&[], |selection| {
                selection.receiving_region_arguments.as_slice()
            })
        }

        pub fn providers(&self) -> impl Iterator<Item = (BoundaryMachineId, MachineId)> + '_ {
            self.providers
                .iter()
                .map(|(&boundary, selection)| (boundary, selection.candidate))
        }
    }

    #[derive(Debug)]
    pub enum ProviderInstallationError {
        SemanticDecode(terminal_codec::CodecError),
        UnknownOrDuplicateSelection {
            boundary: BoundaryMachineId,
            candidate: MachineId,
        },
        AssumedBindingRejected {
            required: BoundaryMachineId,
            supplied_requirement: BoundaryMachineId,
            reason: AssumedBindingRejection,
        },
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum AssumedBindingRejection {
        UnknownRequirement,
        UnknownSuppliedRequirement,
        CompetingCheckedSupply,
        CompetingAssumption,
        UnselectedSupply,
        CyclicSupply,
        IncompatibleRuntimeShape,
        IncompatibleOperationalContract,
        UnsupportedAssumedAuthority,
    }

    pub fn admit_provider_installation_from_artifact(
        semantic_bytes: &[u8],
        selections: &[ProviderInstallationSelection],
    ) -> Result<AdmittedProviderSelections, ProviderInstallationError> {
        admit_provider_installation_with_assumed_bindings_from_artifact(
            semantic_bytes,
            selections,
            &[],
        )
    }

    pub fn admit_provider_execution_installation(
        code: terminal_codec::TerminalExecutionView<'_>,
        selections: &[ProviderInstallationSelection],
        assumptions: &[AssumedBindingSelection],
    ) -> Result<AdmittedProviderSelections, ProviderInstallationError> {
        admit_provider_selections(
            code.module(),
            code.custody_identity(),
            selections,
            assumptions,
        )
    }

    pub fn admit_provider_installation(
        artifact: terminal_codec::TerminalArtifactView<'_>,
        selections: &[ProviderInstallationSelection],
        assumptions: &[AssumedBindingSelection],
    ) -> Result<AdmittedProviderSelections, ProviderInstallationError> {
        admit_provider_selections(
            artifact.module(),
            terminal_codec::TerminalExecutionIdentity::portable(artifact.identity()),
            selections,
            assumptions,
        )
    }

    pub fn admit_provider_installation_with_assumed_bindings_from_artifact(
        semantic_bytes: &[u8],
        selections: &[ProviderInstallationSelection],
        assumptions: &[AssumedBindingSelection],
    ) -> Result<AdmittedProviderSelections, ProviderInstallationError> {
        let module = terminal_codec::decode_module(semantic_bytes)
            .map_err(ProviderInstallationError::SemanticDecode)?;
        let identity = terminal_codec::terminal_psi_identity(&module)
            .map_err(ProviderInstallationError::SemanticDecode)?;
        admit_provider_selections(
            &module,
            terminal_codec::TerminalExecutionIdentity::portable(identity),
            selections,
            assumptions,
        )
    }

    fn admit_provider_selections(
        module: &terminal_psi::TerminalModule,
        identity: terminal_codec::TerminalExecutionIdentity,
        selections: &[ProviderInstallationSelection],
        assumptions: &[AssumedBindingSelection],
    ) -> Result<AdmittedProviderSelections, ProviderInstallationError> {
        let mut installed = BTreeMap::new();
        if !selections.is_empty() {
            let mut candidates_by_boundary = BTreeMap::<
                BoundaryMachineId,
                Vec<&terminal_psi::ProviderCandidateConformance>,
            >::new();
            for candidate in &module.provider_candidates {
                candidates_by_boundary
                    .entry(candidate.boundary)
                    .or_default()
                    .push(candidate);
            }
            let callable_boundaries: BTreeSet<BoundaryMachineId> = module
                .boundary_machines
                .iter()
                .filter(|boundary| boundary.role == terminal_psi::BoundaryDeclarationRole::Callable)
                .map(|boundary| boundary.id)
                .collect();
            for selection in selections {
                let mut candidates = candidates_by_boundary
                    .get(&selection.boundary)
                    .into_iter()
                    .flatten()
                    .copied()
                    .filter(|candidate| {
                        module.identities.text(candidate.provider_identity)
                            == Some(selection.provider_identity.as_str())
                            && module
                                .identities
                                .text(candidate.provider_declaration.identity())
                                == Some(selection.provider_declaration.as_str())
                            && candidate.candidate == selection.candidate
                    });
                let candidate = candidates.next();
                if selection.provider_identity.is_empty()
                    || selection.provider_declaration.is_empty()
                    || !callable_boundaries.contains(&selection.boundary)
                    || installed.contains_key(&selection.boundary)
                    || candidates.next().is_some()
                {
                    return Err(ProviderInstallationError::UnknownOrDuplicateSelection {
                        boundary: selection.boundary,
                        candidate: selection.candidate,
                    });
                }
                let Some(candidate) = candidate else {
                    return Err(ProviderInstallationError::UnknownOrDuplicateSelection {
                        boundary: selection.boundary,
                        candidate: selection.candidate,
                    });
                };
                installed.insert(
                    selection.boundary,
                    AdmittedProviderSelection {
                        candidate: candidate.candidate,
                        provider_declaration: candidate.provider_declaration.clone(),
                        declaration_application: candidate.declaration_application,
                        receiving_application: candidate.receiving_application,
                        receiving_region_arguments: candidate.receiving_region_arguments.clone(),
                    },
                );
            }
        }
        let assumed_bindings = admit_assumed_bindings(module, &installed, assumptions)?;
        Ok(AdmittedProviderSelections {
            code_identity: identity,
            providers: installed,
            assumed_bindings,
            folded_constant_target: None,
        })
    }

    fn same_structural_input_carriers(
        required: &[terminal_psi::StructuralParameterDeclaration],
        supplied: &[terminal_psi::StructuralParameterDeclaration],
    ) -> bool {
        required.len() == supplied.len()
            && required.iter().zip(supplied).all(|(required, supplied)| {
                required.position == supplied.position
                    && required.structural_type == supplied.structural_type
                    && required.multiplicity == supplied.multiplicity
                    && required.access == supplied.access
            })
    }

    fn admit_assumed_bindings(
        module: &terminal_psi::TerminalModule,
        installed: &BTreeMap<BoundaryMachineId, AdmittedProviderSelection>,
        assumptions: &[AssumedBindingSelection],
    ) -> Result<Vec<AdmittedAssumedBinding>, ProviderInstallationError> {
        let boundary_ids: BTreeSet<BoundaryMachineId> = module
            .boundary_machines
            .iter()
            .filter(|boundary| boundary.role == terminal_psi::BoundaryDeclarationRole::Callable)
            .map(|boundary| boundary.id)
            .collect();
        let mut admitted = Vec::new();
        for assumption in assumptions {
            let reject = |reason| ProviderInstallationError::AssumedBindingRejected {
                required: assumption.required,
                supplied_requirement: assumption.supplied_requirement,
                reason,
            };
            if !boundary_ids.contains(&assumption.required) {
                return Err(reject(AssumedBindingRejection::UnknownRequirement));
            }
            if installed.contains_key(&assumption.required) {
                return Err(reject(AssumedBindingRejection::CompetingCheckedSupply));
            }
            if admitted
                .iter()
                .any(|binding: &AdmittedAssumedBinding| binding.required == assumption.required)
            {
                return Err(reject(AssumedBindingRejection::CompetingAssumption));
            }
            if !boundary_ids.contains(&assumption.supplied_requirement) {
                return Err(reject(AssumedBindingRejection::UnknownSuppliedRequirement));
            }
            let required =
                terminal_psi::find_boundary_machine(&module.boundary_machines, assumption.required)
                    .unwrap();
            let supplied = terminal_psi::find_boundary_machine(
                &module.boundary_machines,
                assumption.supplied_requirement,
            )
            .unwrap();
            let qualified_result = |boundary: &terminal_psi::BoundaryMachineDeclaration| {
                matches!(&boundary.result, terminal_psi::BoundaryMachineResult::Structural(result)
                if !result.qualifications.is_empty() || !result.projected_qualifications.is_empty())
            };
            if !required.program_local_root_introductions.is_empty()
                || !supplied.program_local_root_introductions.is_empty()
                || !required.content_guarantees.is_empty()
                || !supplied.content_guarantees.is_empty()
                || qualified_result(required)
                || qualified_result(supplied)
            {
                return Err(reject(AssumedBindingRejection::UnsupportedAssumedAuthority));
            }
            if required.parameter_order != supplied.parameter_order
                || required.scalar_parameters != supplied.scalar_parameters
                || !same_structural_input_carriers(
                    &required.structural_parameters,
                    &supplied.structural_parameters,
                )
                || required.result != supplied.result
                || required.reference_outputs != supplied.reference_outputs
                || required.attachment != supplied.attachment
            {
                return Err(reject(AssumedBindingRejection::IncompatibleRuntimeShape));
            }
            if required.kind != supplied.kind
                || required.completion != supplied.completion
                || (required.unconditional_termination && !supplied.unconditional_termination)
                || required.crash_routes != supplied.crash_routes
                || supplied.published_service_ceiling.iter().any(|service| {
                    Some(*service) != supplied.owning_service
                        && !required.published_service_ceiling.contains(service)
                })
            {
                return Err(reject(
                    AssumedBindingRejection::IncompatibleOperationalContract,
                ));
            }
            let mut visiting = BTreeSet::from([assumption.required]);
            let mut supply_path = Vec::new();
            let mut requirement = assumption.supplied_requirement;
            let implementation = loop {
                if let Some(candidate) = installed.get(&requirement) {
                    break candidate.candidate;
                }
                if !visiting.insert(requirement) {
                    return Err(reject(AssumedBindingRejection::CyclicSupply));
                }
                let Some(next) = assumptions.iter().find(|link| link.required == requirement)
                else {
                    return Err(reject(AssumedBindingRejection::UnselectedSupply));
                };
                supply_path.push(requirement);
                requirement = next.supplied_requirement;
            };
            admitted.push(AdmittedAssumedBinding {
                required: assumption.required,
                supplied_requirement: assumption.supplied_requirement,
                implementation,
                selected_requirement: requirement,
                supply_path,
            });
        }
        Ok(admitted)
    }
}

pub(crate) mod component_provider {
    use semantic_vocabulary::{BoundaryMachineId, MachineId};
    use terminal_codec::{TerminalArtifactView, TerminalExecutionIdentity, TerminalExecutionView};
    use terminal_psi::TerminalPsiIdentity;

    use crate::{
        AdmittedProviderSelections, ProviderInstallationSelection, admit_provider_installation,
    };

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct AdmittedComponentProvider {
        receiver: TerminalExecutionIdentity,
        receiving_boundary: BoundaryMachineId,
        provider: TerminalPsiIdentity,
        provider_boundary: BoundaryMachineId,
        candidate: MachineId,
        selection: AdmittedProviderSelections,
        input_memberships: crate::component_memberships::InputMemberships,
        result_memberships: crate::component_memberships::Memberships,
    }

    impl AdmittedComponentProvider {
        pub const fn receiver(&self) -> &TerminalExecutionIdentity {
            &self.receiver
        }
        pub const fn receiving_boundary(&self) -> BoundaryMachineId {
            self.receiving_boundary
        }
        pub const fn provider(&self) -> TerminalPsiIdentity {
            self.provider
        }
        pub const fn provider_boundary(&self) -> BoundaryMachineId {
            self.provider_boundary
        }
        pub const fn candidate(&self) -> MachineId {
            self.candidate
        }
        pub const fn selection(&self) -> &AdmittedProviderSelections {
            &self.selection
        }

        pub fn input_membership(
            &self,
            input: usize,
            domain: semantic_vocabulary::StructuralDomainId,
        ) -> Option<semantic_vocabulary::StructuralDomainId> {
            self.input_memberships
                .get(input)?
                .iter()
                .find(|(source, _)| *source == domain)
                .map(|(_, target)| *target)
        }
        pub fn result_membership(
            &self,
            domain: semantic_vocabulary::StructuralDomainId,
        ) -> Option<semantic_vocabulary::StructuralDomainId> {
            self.result_memberships
                .iter()
                .find(|(source, _)| *source == domain)
                .map(|(_, target)| *target)
        }
    }

    pub fn admit_component_provider<'receiver>(
        receiver: impl Into<TerminalExecutionView<'receiver>>,
        receiving_boundary: BoundaryMachineId,
        provider: TerminalArtifactView<'_>,
        selected: &ProviderInstallationSelection,
    ) -> Result<AdmittedComponentProvider, String> {
        let receiver = receiver.into();
        let receiving = receiver
            .module()
            .boundary_machines
            .iter()
            .find(|row| row.id == receiving_boundary)
            .ok_or("component import names no receiving boundary")?;
        let supplying = provider
            .module()
            .boundary_machines
            .iter()
            .find(|row| row.id == selected.boundary)
            .ok_or("component export names no producing boundary")?;
        let left = &receiver.module().identities;
        let right = &provider.module().identities;
        if !left
            .text(receiving.identity)
            .zip(right.text(supplying.identity))
            .is_some_and(|(left, right)| !left.is_empty() && left == right)
            || !receiving
                .owner
                .same_identity_as(left, &supplying.owner, right)
        {
            return Err("component import/export name different requirement declarations".into());
        }
        let (Some(receiving_contract), Some(supplying_contract)) =
            (&receiving.declaration, &supplying.declaration)
        else {
            return Err("component import/export lacks a checked callable contract".into());
        };
        if !receiving_contract.same_identity_as(left, supplying_contract, right)
            || receiving.completion != supplying.completion
            || receiving.unconditional_termination != supplying.unconditional_termination
            || receiving.parameter_order != supplying.parameter_order
            || receiving.scalar_parameters != supplying.scalar_parameters
        {
            return Err("component import/export checked callable contracts differ".into());
        }
        let selection = admit_provider_installation(provider, std::slice::from_ref(selected), &[])
            .map_err(|error| format!("component export provider admission: {error:?}"))?;
        let input_memberships = crate::component_memberships::admit_input_memberships(
            receiver.module(),
            receiving,
            provider.module(),
            supplying,
        )?;
        let result_memberships = crate::component_memberships::admit_result_memberships(
            receiver.module(),
            receiving,
            provider.module(),
            supplying,
        )?;
        Ok(AdmittedComponentProvider {
            receiver: receiver.custody_identity(),
            receiving_boundary,
            provider: provider.identity(),
            provider_boundary: selected.boundary,
            candidate: selected.candidate,
            selection,
            input_memberships,
            result_memberships,
        })
    }
}

pub(crate) mod component_memberships {
    use semantic_vocabulary::{StructuralDomainId, StructuralTypeId};
    use terminal_psi::{
        BoundaryMachineDeclaration, BoundaryMachineResult, NominalDeclarationIdentity,
        StructuralDomainDeclaration, StructuralEstablishmentRoute, TerminalModule,
    };

    pub(crate) type Memberships = Vec<(StructuralDomainId, StructuralDomainId)>;
    pub(crate) type InputMemberships = Vec<Memberships>;

    pub(crate) fn admit_input_memberships(
        receiver: &TerminalModule,
        required: &BoundaryMachineDeclaration,
        provider: &TerminalModule,
        supplied: &BoundaryMachineDeclaration,
    ) -> Result<InputMemberships, String> {
        if required.structural_parameters.len() != supplied.structural_parameters.len() {
            return Err("component input membership arities differ".into());
        }
        required
            .structural_parameters
            .iter()
            .zip(&supplied.structural_parameters)
            .map(|(left, right)| {
                admit_memberships(
                    receiver,
                    left.structural_type,
                    &left.qualifications,
                    provider,
                    right.structural_type,
                    &right.qualifications,
                )
                .map_err(|reason| format!("component input {reason}"))
            })
            .collect()
    }

    pub(crate) fn admit_result_memberships(
        receiver: &TerminalModule,
        required: &BoundaryMachineDeclaration,
        provider: &TerminalModule,
        supplied: &BoundaryMachineDeclaration,
    ) -> Result<Memberships, String> {
        match (&supplied.result, &required.result) {
            (BoundaryMachineResult::Unit, BoundaryMachineResult::Unit) => Ok(Vec::new()),
            (BoundaryMachineResult::Scalar(left), BoundaryMachineResult::Scalar(right))
                if left == right =>
            {
                Ok(Vec::new())
            }
            (BoundaryMachineResult::Structural(left), BoundaryMachineResult::Structural(right)) => {
                if !left.projected_qualifications.is_empty()
                    || !right.projected_qualifications.is_empty()
                {
                    return Err("component result requires projected membership transfer".into());
                }
                admit_memberships(
                    provider,
                    left.structural_type,
                    &left.qualifications,
                    receiver,
                    right.structural_type,
                    &right.qualifications,
                )
                .map_err(|reason| format!("component result {reason}"))
            }
            _ => Err("component result kinds or scalar contracts differ".into()),
        }
    }

    fn admit_memberships(
        source_module: &TerminalModule,
        source_type: StructuralTypeId,
        source_required: &[StructuralDomainId],
        target_module: &TerminalModule,
        target_type: StructuralTypeId,
        target_required: &[StructuralDomainId],
    ) -> Result<Memberships, String> {
        let mut bindings = Vec::new();
        for source in source_module
            .structural_domains
            .iter()
            .filter(|row| row.carrier == source_type)
        {
            let mut matches = target_module.structural_domains.iter().filter(|row| {
                row.carrier == target_type && same_domain(source_module, source, target_module, row)
            });
            if let Some(destination) = matches.next() {
                if matches.next().is_some() {
                    return Err("has ambiguous membership theories".into());
                }
                bindings.push((source.id, destination.id));
            }
        }
        let mut mapped = source_required
            .iter()
            .map(|source| {
                bindings
                    .iter()
                    .find(|(domain, _)| domain == source)
                    .map(|(_, domain)| *domain)
                    .ok_or_else(|| "membership theories differ".to_owned())
            })
            .collect::<Result<Vec<_>, _>>()?;
        mapped.sort();
        if mapped != target_required {
            return Err("membership requirements differ".into());
        }
        Ok(bindings)
    }

    fn same_domain(
        left: &TerminalModule,
        source: &StructuralDomainDeclaration,
        right: &TerminalModule,
        target: &StructuralDomainDeclaration,
    ) -> bool {
        if source
            .required_domain_closure(&left.structural_domains)
            .is_none()
            || target
                .required_domain_closure(&right.structural_domains)
                .is_none()
        {
            return false;
        }
        let mut pending = vec![(source, target)];
        let mut paired = Vec::new();
        while let Some((source, target)) = pending.pop() {
            if paired.iter().any(|(a, b)| {
                (*a == source.id || *b == target.id) && (*a != source.id || *b != target.id)
            }) {
                return false;
            }
            if paired.contains(&(source.id, target.id)) {
                continue;
            }
            if !same_domain_definition(left, source, right, target) {
                return false;
            }
            paired.push((source.id, target.id));
            let (Some(source_required), Some(target_required)) =
                (&source.required_memberships, &target.required_memberships)
            else {
                return false;
            };
            if source_required.len() != target_required.len() {
                return false;
            }
            for required in source_required {
                let Some(source_parent) = required_domain(left, source, *required) else {
                    return false;
                };
                let mut matches = target_required.iter().filter_map(|required| {
                    required_domain(right, target, *required).filter(|parent| {
                        NominalDeclarationIdentity::new(source_parent.identity).same_identity_as(
                            &left.identities,
                            &NominalDeclarationIdentity::new(parent.identity),
                            &right.identities,
                        )
                    })
                });
                let Some(target_parent) = matches.next() else {
                    return false;
                };
                if matches.next().is_some() {
                    return false;
                }
                pending.push((source_parent, target_parent));
            }
        }
        true
    }

    fn required_domain<'a>(
        module: &'a TerminalModule,
        child: &StructuralDomainDeclaration,
        domain: semantic_vocabulary::DomainSemanticId,
    ) -> Option<&'a StructuralDomainDeclaration> {
        let mut matches = module
            .structural_domains
            .iter()
            .filter(|row| row.semantic_domain == domain && row.carrier == child.carrier);
        let row = matches.next()?;
        matches.next().is_none().then_some(row)
    }

    fn same_domain_definition(
        left: &TerminalModule,
        source: &StructuralDomainDeclaration,
        right: &TerminalModule,
        target: &StructuralDomainDeclaration,
    ) -> bool {
        let same_name = |a, b| {
            NominalDeclarationIdentity::new(a).same_identity_as(
                &left.identities,
                &NominalDeclarationIdentity::new(b),
                &right.identities,
            )
        };
        let carrier = |module: &TerminalModule, id| {
            module
                .structural_types
                .iter()
                .find(|row| row.id == id)
                .map(|row| row.identity)
        };
        if !source.complete_predicate_theory
            || !target.complete_predicate_theory
            || !same_name(source.identity, target.identity)
            || !carrier(left, source.carrier)
                .zip(carrier(right, target.carrier))
                .is_some_and(|(a, b)| same_name(a, b))
            || source.byte_predicate != target.byte_predicate
            || source
                .content_projection
                .as_ref()
                .map(|row| (&row.algebra, &row.expression))
                != target
                    .content_projection
                    .as_ref()
                    .map(|row| (&row.algebra, &row.expression))
        {
            return false;
        }
        match (&source.predicate, &target.predicate) {
            (Some(a), Some(b))
                if a.proposition == b.proposition
                    && terminal_semantics::domain_predicate_observes_no_storage(
                        left.structural_types.iter(),
                        source.carrier,
                        a,
                    )
                    && terminal_semantics::domain_predicate_observes_no_storage(
                        right.structural_types.iter(),
                        target.carrier,
                        b,
                    ) => {}
            _ => return false,
        }
        let routes = |module: &TerminalModule, domain: &StructuralDomainDeclaration| {
            let mut rows = domain
                .establishment_routes
                .iter()
                .map(|route| {
                    let kind = match route {
                        StructuralEstablishmentRoute::CheckedRequirement { .. } => 0,
                        StructuralEstablishmentRoute::ExactMachine { .. } => 1,
                        StructuralEstablishmentRoute::BoundaryRequirement { .. } => 2,
                    };
                    let declaration = route.declaration();
                    let identity = route.identity();
                    if module.identities.is_invocation_local(identity)
                        || module
                            .identities
                            .is_invocation_local(declaration.identity())
                    {
                        return None;
                    }
                    let owner = module
                        .identities
                        .text(declaration.identity())
                        .filter(|name| !name.is_empty())?;
                    module
                        .identities
                        .text(identity)
                        .filter(|name| !name.is_empty())
                        .map(|name| (kind, owner.to_owned(), name.to_owned()))
                })
                .collect::<Option<Vec<_>>>()?;
            rows.sort();
            Some(rows)
        };
        routes(left, source)
            .zip(routes(right, target))
            .is_some_and(|(a, b)| a == b)
    }
}

pub(crate) mod target_families {
    use terminal_codec::TerminalArtifactView;
    use terminal_psi::{TargetMachineCallable, TargetMachineSupply, TerminalPsiIdentity};

    pub fn admitted_boundary_member(
        module: &terminal_psi::TerminalModule,
        selections: Option<&AdmittedTargetMachineSelections>,
        boundary: semantic_vocabulary::BoundaryMachineId,
    ) -> Result<Option<terminal_psi::TargetMachineCallable>, String> {
        let head = terminal_psi::TargetMachineCallable::Boundary(boundary);
        let families = &module.target_machine_families;
        let Ok(position) = families.binary_search_by_key(&head, |family| family.canonical) else {
            return Ok(None);
        };
        if families[position]
            .members
            .iter()
            .any(|member| member.callable == head)
        {
            return Ok(None);
        }
        let selections = selections.ok_or_else(|| {
            format!("target-family head {boundary:?} has no invocation deployment selection")
        })?;
        let member = selections.member(head).map_err(|error| error.to_string())?;
        match (member.callable(), member.supply()) {
            (
                terminal_psi::TargetMachineCallable::Machine(machine),
                terminal_psi::TargetMachineSupply::CheckedBody
                | terminal_psi::TargetMachineSupply::Boundary,
            ) => Ok(Some(terminal_psi::TargetMachineCallable::Machine(machine))),
            (terminal_psi::TargetMachineCallable::Boundary(member), supply)
                if supply != terminal_psi::TargetMachineSupply::CheckedBody =>
            {
                Ok(Some(terminal_psi::TargetMachineCallable::Boundary(member)))
            }
            _ => Err(format!(
                "target-family head {boundary:?} selected incompatible callable {:?} for `{}`",
                member.callable(),
                selections.target(),
            )),
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct TargetMachineSelection {
        head: TargetMachineCallable,
        callable: TargetMachineCallable,
        supply: TargetMachineSupply,
    }

    impl TargetMachineSelection {
        pub const fn head(&self) -> TargetMachineCallable {
            self.head
        }

        pub const fn callable(&self) -> TargetMachineCallable {
            self.callable
        }

        pub const fn supply(&self) -> TargetMachineSupply {
            self.supply
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct AdmittedTargetMachineSelections {
        terminal_psi: TerminalPsiIdentity,
        target: String,
        selections: Vec<TargetMachineSelection>,
    }

    impl AdmittedTargetMachineSelections {
        pub fn entry(
            &self,
            identity: TerminalPsiIdentity,
            module: &terminal_psi::TerminalModule,
            root: TargetMachineCallable,
        ) -> Result<semantic_vocabulary::MachineId, TargetMachineSelectionError> {
            if self.terminal_psi != identity {
                return Err(TargetMachineSelectionError::TerminalIdentityMismatch);
            }
            match root {
                TargetMachineCallable::Machine(machine) => {
                    if module.machine(machine).is_none() {
                        return Err(TargetMachineSelectionError::NonExecutableEntry {
                            head: root,
                            target: self.target.clone(),
                        });
                    }
                    for family in &module.target_machine_families {
                        if family.members.iter().any(|member| member.callable == root)
                            && self.member(family.canonical)?.callable() != root
                        {
                            return Err(TargetMachineSelectionError::NonExecutableEntry {
                                head: family.canonical,
                                target: self.target.clone(),
                            });
                        }
                    }
                    Ok(machine)
                }
                head @ TargetMachineCallable::Boundary(_) => {
                    let member = self.member(head)?;
                    executable_entry(head, member, &self.target)
                }
            }
        }

        pub const fn terminal_psi(&self) -> TerminalPsiIdentity {
            self.terminal_psi
        }

        pub fn target(&self) -> &str {
            &self.target
        }

        pub fn selections(&self) -> &[TargetMachineSelection] {
            &self.selections
        }

        pub fn member(
            &self,
            head: TargetMachineCallable,
        ) -> Result<&TargetMachineSelection, TargetMachineSelectionError> {
            self.selections
                .binary_search_by_key(&head, |selection| selection.head)
                .map(|position| &self.selections[position])
                .map_err(|_| TargetMachineSelectionError::MissingMember {
                    head,
                    target: self.target.clone(),
                })
        }
    }

    fn executable_entry(
        head: TargetMachineCallable,
        member: &TargetMachineSelection,
        target: &str,
    ) -> Result<semantic_vocabulary::MachineId, TargetMachineSelectionError> {
        match (member.callable(), member.supply()) {
            (
                TargetMachineCallable::Machine(machine),
                TargetMachineSupply::CheckedBody | TargetMachineSupply::Boundary,
            ) => Ok(machine),
            _ => Err(TargetMachineSelectionError::NonExecutableEntry {
                head,
                target: target.to_owned(),
            }),
        }
    }

    fn selected_member(
        family: &terminal_psi::TargetMachineFamily,
        target: &str,
    ) -> Option<TargetMachineSelection> {
        let member = borrowed_selected_member(family, target)?;
        Some(TargetMachineSelection {
            head: family.canonical,
            callable: member.callable,
            supply: member.supply,
        })
    }

    fn borrowed_selected_member<'family>(
        family: &'family terminal_psi::TargetMachineFamily,
        target: &str,
    ) -> Option<&'family terminal_psi::TargetMachineFamilyMember> {
        let position = family
            .members
            .binary_search_by(|member| member.target.as_str().cmp(target))
            .ok()?;
        family.members.get(position)
    }

    pub fn resolve_target_machine_member<'module>(
        module: &'module terminal_psi::TerminalModule,
        callable: TargetMachineCallable,
        target: &str,
    ) -> Result<Option<&'module terminal_psi::TargetMachineFamilyMember>, TargetMachineSelectionError>
    {
        let Some(family) = module
            .target_machine_families
            .iter()
            .find(|family| family.canonical == callable)
        else {
            return Ok(None);
        };
        if family
            .members
            .iter()
            .any(|member| member.callable == callable)
        {
            return Ok(None);
        }
        borrowed_selected_member(family, target)
            .map(Some)
            .ok_or_else(|| TargetMachineSelectionError::MissingMember {
                head: callable,
                target: target.to_owned(),
            })
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum TargetMachineSelectionError {
        TerminalIdentityMismatch,
        NonExecutableEntry {
            head: TargetMachineCallable,
            target: String,
        },
        MissingMember {
            head: TargetMachineCallable,
            target: String,
        },
    }

    impl std::fmt::Display for TargetMachineSelectionError {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::TerminalIdentityMismatch => {
                    formatter.write_str("entry selection names a different Terminal artifact")
                }
                Self::NonExecutableEntry { head, target } => write!(
                    formatter,
                    "target family {head:?} has no executable entry for deployment tag `{target}`",
                ),
                Self::MissingMember { head, target } => write!(
                    formatter,
                    "target family {head:?} has no member for deployment tag `{target}`"
                ),
            }
        }
    }

    impl std::error::Error for TargetMachineSelectionError {}

    // Selection reads only the module's target families and its identity, so
    // portable artifacts and invocation-published code select the same way.
    pub fn admit_target_machine_selections<'code>(
        artifact: impl Into<terminal_codec::TerminalExecutionView<'code>>,
        target: &str,
    ) -> AdmittedTargetMachineSelections {
        let artifact = artifact.into();
        let selections = artifact
            .module()
            .target_machine_families
            .iter()
            .filter_map(|family| selected_member(family, target))
            .collect();
        AdmittedTargetMachineSelections {
            terminal_psi: artifact.identity(),
            target: target.to_owned(),
            selections,
        }
    }
}

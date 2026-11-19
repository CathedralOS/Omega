//! Verify requested providers and assumed bindings against one artifact’s callable
//! catalog. The admitted selection retains the execution identity, receiving
//! applications, and concrete supply paths needed by its consumers.
#![forbid(unsafe_code)]

use semantic_vocabulary::{BoundaryMachineId, MachineId};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, record_storage::Record)]
pub struct ProviderInstallationSelection {
    pub boundary: BoundaryMachineId,
    pub provider_identity: String,
    pub provider_declaration: String,
    pub candidate: MachineId,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, record_storage::Record,
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
struct AdmittedProviderSelection {
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

/// Whether the supplied requirement's operational contract can stand in
/// for the required one. Its published reach ceiling must fit inside the
/// required ceiling member for member: the owning service is catalogue
/// ownership, not reach (`wiki/spec/language/effects.md`), so a supplied
/// ceiling that names its own owner reaches that service like any other
/// member and needs the required ceiling to permit it.
fn operational_contract_fits(
    required: &terminal_psi::BoundaryMachineDeclaration,
    supplied: &terminal_psi::BoundaryMachineDeclaration,
) -> bool {
    required.kind == supplied.kind
        && required.completion == supplied.completion
        && (!required.unconditional_termination || supplied.unconditional_termination)
        && required.crash_routes == supplied.crash_routes
        && supplied
            .published_service_ceiling
            .iter()
            .all(|service| required.published_service_ceiling.contains(service))
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
            || !terminal_semantics::boundary_result_lifetimes_match(
                module, required, module, supplied,
            )
            .unwrap_or(false)
            || required.reference_outputs != supplied.reference_outputs
            || required.attachment != supplied.attachment
        {
            return Err(reject(AssumedBindingRejection::IncompatibleRuntimeShape));
        }
        if !operational_contract_fits(required, supplied) {
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

#[cfg(test)]
mod tests {
    use super::operational_contract_fits;
    use semantic_vocabulary::{BoundaryMachineId, ServiceId};
    use terminal_psi::{
        BoundaryDeclarationRole, BoundaryMachineDeclaration, BoundaryMachineResult,
        NominalDeclarationIdentity, TerminalIdentityTableBuilder,
    };

    fn service(id: u64) -> ServiceId {
        ServiceId::new(id).unwrap()
    }

    fn requirement(id: u64, owner: u64, ceiling: &[u64]) -> BoundaryMachineDeclaration {
        let identity = TerminalIdentityTableBuilder::default().intern("test::Slot::call");
        let ceiling: Vec<ServiceId> = ceiling.iter().copied().map(service).collect();
        BoundaryMachineDeclaration {
            id: BoundaryMachineId::new(id).unwrap(),
            declaration: None,
            numeric_intrinsic: None,
            role: BoundaryDeclarationRole::Callable,
            kind: semantic_vocabulary::BoundaryRequirementKind::Trait,
            completion: semantic_vocabulary::BoundaryCompletion::NormalReturn,
            unconditional_termination: true,
            identity,
            owner: NominalDeclarationIdentity::new(identity),
            attachment: None,
            parameter_order: Vec::new(),
            scalar_parameters: Vec::new(),
            crash_routes: Vec::new(),
            structural_parameters: Vec::new(),
            result: BoundaryMachineResult::Unit,
            result_reference_regions: Vec::new(),
            result_lifetimes: None,
            requires: Vec::new(),
            membership_ensures: Vec::new(),
            normal_qualifications: Vec::new(),
            scalar_domain_requirements: Vec::new(),
            scalar_requires: Vec::new(),
            scalar_ensures: Vec::new(),
            reference_outputs: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            fixed_service_reach: ceiling.clone(),
            service_reach_is_installation_bound: false,
            owning_service: Some(service(owner)),
            published_service_ceiling: ceiling,
            published_synchronous_entries: Vec::new(),
        }
    }

    // A supply whose ceiling names its own owning slot reaches that slot;
    // a requirement declared `reaches none` cannot be stood in for by it.
    #[test]
    fn supplied_owner_in_its_ceiling_must_fit_the_required_ceiling() {
        let required = requirement(1, 10, &[]);
        assert!(operational_contract_fits(
            &required,
            &requirement(2, 20, &[])
        ));
        assert!(!operational_contract_fits(
            &required,
            &requirement(2, 20, &[20])
        ));
        assert!(operational_contract_fits(
            &requirement(1, 10, &[20]),
            &requirement(2, 20, &[20]),
        ));
        assert!(!operational_contract_fits(
            &required,
            &requirement(2, 20, &[30])
        ));
    }

    // The required owner held only as catalogue ownership contributes
    // nothing a narrower supply must match.
    #[test]
    fn required_owner_outside_its_ceiling_permits_nothing_extra() {
        let required = requirement(1, 10, &[]);
        assert!(!operational_contract_fits(
            &required,
            &requirement(2, 20, &[10])
        ));
    }
}

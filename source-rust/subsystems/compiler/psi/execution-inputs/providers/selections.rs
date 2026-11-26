//! Verify requested providers against one artifact’s callable
//! catalog. The admitted selection retains the execution identity, receiving
//! applications, and concrete supply paths needed by its consumers. A component
//! selection also checks the receiving catalog against the supplying artifact.
#![forbid(unsafe_code)]

pub mod dependency_reach;
pub mod logical_reach;

mod component;
pub use component::{AdmittedComponentProvider, admit_component_provider};

use semantic_vocabulary::{BoundaryMachineId, MachineId};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, record_storage::Record)]
pub struct ProviderInstallationSelection {
    pub boundary: BoundaryMachineId,
    pub provider_identity: String,
    pub provider_declaration: String,
    pub candidate: MachineId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedProviderSelections {
    code_identity: terminal_codec::TerminalExecutionIdentity,
    providers: BTreeMap<BoundaryMachineId, AdmittedProviderSelection>,
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
            .scalars.folded_constants
            .validate(module)
            .map_err(str::to_owned)?;
        if module.scalars.folded_constants.target.as_deref() != Some(target) {
            return Err("folded constant closure belongs to another target".into());
        }
        let expected: Vec<_> = module
            .scalars.folded_constants
            .dependencies
            .iter()
            .filter(|dependency| dependency.target == target)
            .cloned()
            .collect();
        module
            .scalars.folded_constants
            .validate_plan_inputs(target, plan_preimages)
            .map_err(str::to_owned)?;
        let mut historical = Vec::new();
        for dependency in &expected {
            let Some(association) = &dependency.candidate else {
                continue;
            };
            let candidate = module
                .interfaces.provider_candidates
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
}

pub fn admit_provider_execution_installation(
    code: terminal_codec::TerminalExecutionView<'_>,
    selections: &[ProviderInstallationSelection],
) -> Result<AdmittedProviderSelections, ProviderInstallationError> {
    admit_provider_selections(
        code.module(),
        code.custody_identity(),
        selections,
    )
}

pub fn admit_provider_installation(
    artifact: terminal_codec::TerminalArtifactView<'_>,
    selections: &[ProviderInstallationSelection],
) -> Result<AdmittedProviderSelections, ProviderInstallationError> {
    admit_provider_selections(
        artifact.module(),
        terminal_codec::TerminalExecutionIdentity::portable(artifact.identity()),
        selections,
    )
}

pub fn admit_provider_installation_from_artifact(
    semantic_bytes: &[u8],
    selections: &[ProviderInstallationSelection],
) -> Result<AdmittedProviderSelections, ProviderInstallationError> {
    let module = terminal_codec::decode_module(semantic_bytes)
        .map_err(ProviderInstallationError::SemanticDecode)?;
    let identity = terminal_codec::terminal_psi_identity(&module)
        .map_err(ProviderInstallationError::SemanticDecode)?;
    admit_provider_selections(
        &module,
        terminal_codec::TerminalExecutionIdentity::portable(identity),
        selections,
    )
}

fn admit_provider_selections(
    module: &terminal_psi::TerminalModule,
    identity: terminal_codec::TerminalExecutionIdentity,
    selections: &[ProviderInstallationSelection],
) -> Result<AdmittedProviderSelections, ProviderInstallationError> {
    let mut installed = BTreeMap::new();
    if !selections.is_empty() {
        let mut candidates_by_boundary = BTreeMap::<
            BoundaryMachineId,
            Vec<&terminal_psi::ProviderCandidateConformance>,
        >::new();
        for candidate in &module.interfaces.provider_candidates {
            candidates_by_boundary
                .entry(candidate.boundary)
                .or_default()
                .push(candidate);
        }
        let callable_boundaries: BTreeSet<BoundaryMachineId> = module
            .interfaces.boundary_machines
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
            terminal_semantics::provider_candidates::conservation::verify(module, candidate)
                .map_err(|reason| ProviderInstallationError::SemanticDecode(
                    terminal_codec::CodecError::MalformedStructuralFoundation(reason),
                ))?;
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
    Ok(AdmittedProviderSelections {
        code_identity: identity,
        providers: installed,
        folded_constant_target: None,
    })
}

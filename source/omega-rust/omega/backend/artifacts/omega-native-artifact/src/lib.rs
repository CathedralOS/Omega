#![forbid(unsafe_code)]

//! Authority-free canonical Terminal-Psi to native artifact handoff.
//!
//! This crate deliberately has no dependency on source, syntax, typed,
//! checked, or source-derived provider-plan carriers.
//! It owns only canonical Terminal bytes, target artifacts, and the exact
//! source-free identity projections needed to replay their joins.

use std::collections::BTreeSet;

use omega_installation_evidence::ProviderExecutionEvidence;

/// One selected provider plan projected into source-free native-artifact
/// identity. Requirements are canonical, strictly ordered, and complete for
/// this selected plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeSelectedProviderPlan {
    identity: u64,
    requirement_identities: Vec<String>,
}

impl NativeSelectedProviderPlan {
    pub fn new(identity: u64, mut requirement_identities: Vec<String>) -> Self {
        requirement_identities.sort();
        requirement_identities.dedup();
        Self {
            identity,
            requirement_identities,
        }
    }

    pub const fn identity(&self) -> u64 {
        self.identity
    }

    pub fn requirement_identities(&self) -> &[String] {
        &self.requirement_identities
    }
}

/// One exact admitted provider execution selected during native
/// realization. This is an owned identity projection, not a provider
/// occurrence or installation receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeProviderExecution {
    requirement_identity: String,
    provider_plan: u64,
    provider_execution_identity: u64,
    provider_execution_fingerprint: u64,
    normalized_root_identity: u64,
    boundary_contract_fingerprint: u64,
}

impl NativeProviderExecution {
    pub fn from_evidence(evidence: &dyn ProviderExecutionEvidence) -> Self {
        Self {
            requirement_identity: evidence.requirement_identity().to_owned(),
            provider_plan: evidence.provider_plan(),
            provider_execution_identity: evidence.provider_execution_identity(),
            provider_execution_fingerprint: evidence.provider_execution_fingerprint(),
            normalized_root_identity: evidence.normalized_root_identity(),
            boundary_contract_fingerprint: evidence.boundary_contract_fingerprint(),
        }
    }
}

impl ProviderExecutionEvidence for NativeProviderExecution {
    fn requirement_identity(&self) -> &str {
        &self.requirement_identity
    }

    fn provider_plan(&self) -> u64 {
        self.provider_plan
    }

    fn provider_execution_identity(&self) -> u64 {
        self.provider_execution_identity
    }

    fn provider_execution_fingerprint(&self) -> u64 {
        self.provider_execution_fingerprint
    }

    fn normalized_root_identity(&self) -> u64 {
        self.normalized_root_identity
    }

    fn boundary_contract_fingerprint(&self) -> u64 {
        self.boundary_contract_fingerprint
    }
}

/// Complete source-independent Terminal-Psi native realization.
#[derive(Debug)]
#[must_use = "a native artifact owns the canonical semantic and target artifact join"]
pub struct NativeArtifact {
    target: omega_target::NativeTarget,
    psi_artifact: psi_terminal_codec::CanonicalTerminalArtifact,
    object: omega_image_emission::ObjectArtifact,
    image: omega_image_emission::ExecutableImage,
    selected_provider_closure_identity: u64,
    selected_provider_plans: Vec<NativeSelectedProviderPlan>,
    provider_executions: Vec<NativeProviderExecution>,
}

#[derive(Debug)]
pub struct NativeArtifactParts {
    pub target: omega_target::NativeTarget,
    pub psi_artifact: psi_terminal_codec::CanonicalTerminalArtifact,
    pub object: omega_image_emission::ObjectArtifact,
    pub image: omega_image_emission::ExecutableImage,
    pub selected_provider_closure_identity: u64,
    pub selected_provider_plans: Vec<NativeSelectedProviderPlan>,
    pub provider_executions: Vec<NativeProviderExecution>,
}

impl NativeArtifact {
    /// Rejoin already verified proof admission with target artifacts while
    /// replaying every source-free identity and byte relation retained here.
    pub fn from_replayed_parts(parts: NativeArtifactParts) -> Result<Self, &'static str> {
        let artifact = Self {
            target: parts.target,
            psi_artifact: parts.psi_artifact,
            object: parts.object,
            image: parts.image,
            selected_provider_closure_identity: parts.selected_provider_closure_identity,
            selected_provider_plans: parts.selected_provider_plans,
            provider_executions: parts.provider_executions,
        };
        artifact.validate()?;
        Ok(artifact)
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        self.psi_artifact
            .validate()
            .map_err(|_| "native artifact contains an invalid canonical artifact")?;
        let semantic = self.psi_artifact.manifest().semantic();
        if self.object.psi() != semantic || self.image.psi() != semantic {
            return Err("native artifact semantic identity disagrees with its object or image");
        }
        if self.object.target() != self.target || self.image.target() != self.target {
            return Err("native artifact target disagrees with its object or image");
        }
        omega_image_emission::validate_executable_image(&self.object, &self.image)
            .map_err(|_| "native artifact image failed object-to-image replay")?;
        let module = psi_terminal_codec::decode_module(self.psi_artifact.semantic_bytes())
            .map_err(|_| "native artifact canonical semantics failed to decode")?;
        if module.entry != self.object.entry() {
            return Err("native artifact entry disagrees with canonical semantics");
        }
        if self.selected_provider_closure_identity == 0 {
            return Err("native artifact selected provider closure has the reserved zero identity");
        }

        let mut prior_plan = None;
        for plan in &self.selected_provider_plans {
            if plan.identity == 0 || prior_plan.is_some_and(|prior| prior >= plan.identity) {
                return Err("native artifact selected provider plans are not canonical and unique");
            }
            prior_plan = Some(plan.identity);
            if plan.requirement_identities.is_empty()
                || plan
                    .requirement_identities
                    .windows(2)
                    .any(|pair| pair[0] >= pair[1])
            {
                return Err(
                    "native artifact selected provider requirements are not canonical and unique",
                );
            }
        }

        let mut prior_execution = None;
        let mut seen_requirements = BTreeSet::new();
        let mut admitted_executions = BTreeSet::new();
        for execution in &self.provider_executions {
            let key = (
                execution.requirement_identity(),
                execution.provider_plan(),
                execution.provider_execution_identity(),
            );
            if prior_execution.is_some_and(|prior| prior >= key) {
                return Err("native artifact provider executions are not in canonical order");
            }
            prior_execution = Some(key);
            if !seen_requirements.insert(execution.requirement_identity()) {
                return Err("native artifact contains duplicate provider requirement executions");
            }
            let Some(plan) = self
                .selected_provider_plans
                .iter()
                .find(|plan| plan.identity == execution.provider_plan())
            else {
                return Err("native artifact provider execution names an unselected plan");
            };
            if plan
                .requirement_identities
                .binary_search_by(|identity| {
                    identity.as_str().cmp(execution.requirement_identity())
                })
                .is_err()
            {
                return Err("native artifact provider execution is absent from its selected plan");
            }
            admitted_executions.insert((
                execution.requirement_identity().to_owned(),
                execution.provider_plan(),
                execution.provider_execution_identity(),
                execution.provider_execution_fingerprint(),
                execution.normalized_root_identity(),
                execution.boundary_contract_fingerprint(),
            ));
        }
        let required_executions = self
            .image
            .boundary_settlements()
            .iter()
            .map(|installed| {
                let execution = installed.settlement.provider_execution;
                module
                    .boundary_machines
                    .iter()
                    .find(|boundary| boundary.id == installed.settlement.boundary)
                    .map(|boundary| {
                        (
                            boundary.identity.clone(),
                            execution.provider_plan,
                            execution.provider_execution_identity,
                            execution.provider_execution_fingerprint,
                            execution.normalized_root_identity,
                            execution.boundary_contract_fingerprint,
                        )
                    })
            })
            .collect::<Option<BTreeSet<_>>>()
            .ok_or("native artifact image settlement names an absent boundary requirement")?;
        if admitted_executions != required_executions {
            return Err("native artifact provider execution closure disagrees with its image");
        }
        Ok(())
    }

    pub const fn target(&self) -> omega_target::NativeTarget {
        self.target
    }

    pub fn semantic_bytes(&self) -> &[u8] {
        self.psi_artifact.semantic_bytes()
    }

    pub fn proof_bytes(&self) -> &[u8] {
        self.psi_artifact.proof_bytes()
    }

    pub const fn psi_artifact(&self) -> &psi_terminal_codec::CanonicalTerminalArtifact {
        &self.psi_artifact
    }

    pub const fn object(&self) -> &omega_image_emission::ObjectArtifact {
        &self.object
    }

    pub const fn image(&self) -> &omega_image_emission::ExecutableImage {
        &self.image
    }

    pub const fn selected_provider_closure_identity(&self) -> u64 {
        self.selected_provider_closure_identity
    }

    pub fn selected_provider_plans(&self) -> &[NativeSelectedProviderPlan] {
        &self.selected_provider_plans
    }

    pub fn provider_executions(&self) -> &[NativeProviderExecution] {
        &self.provider_executions
    }

    pub fn into_parts(self) -> NativeArtifactParts {
        NativeArtifactParts {
            target: self.target,
            psi_artifact: self.psi_artifact,
            object: self.object,
            image: self.image,
            selected_provider_closure_identity: self.selected_provider_closure_identity,
            selected_provider_plans: self.selected_provider_plans,
            provider_executions: self.provider_executions,
        }
    }
}

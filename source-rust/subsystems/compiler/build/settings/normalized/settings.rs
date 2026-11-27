//! The normalized target and runtime choices produced by a returned Build value.
#![forbid(unsafe_code)]

use build_model::values::BuildTimeValue;
use optimization_selections::OptimizationSelections;
use target::{ApplicationIdentifier, HostedApplicationIntent};

/// Default summand limit for automatic compatibility-proof product expansion
/// (`wiki/spec/build/configuration.md#proof-work-policy`).
pub const DEFAULT_PROOF_INDEX_EXPANSION_TERMS: u64 = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluatedBuildSettings {
    pub application_intent: Option<HostedApplicationIntent>, // Authored hosted presentation intent, independent of PE loader metadata.
    pub application_identifier: Option<ApplicationIdentifier>, // Build-validated authored application identifier (`None` when omitted or empty).
    pub subsystem: u16, // PE optional-header Subsystem word (console 3 when unstated).
    pub optimizations: OptimizationSelections, // Exact root-build optimization selections.
    pub x86_deployment_claim: X86DeploymentClaim, // Authored x86 deployment-feature claim admitted per realized x86 profile.
    pub elf_interpreters: ElfInterpreterSelections, // Authored per-target ELF loader selections (`PT_INTERP`).
    pub tests_disabled: bool, // Authored `builder.tests.disabled`: registered groups stay checked but are reported disabled, never run or passed.
    pub proof_index_expansion_terms: u64, // Authored `builder.proof.index_expansion_terms` (positive, default 64).
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ElfInterpreterSelections {
    explicit: Vec<target::NormalizedElfInterpreterPlan>, // At most one normalized `PT_INTERP` pathname per ELF target profile.
}

impl ElfInterpreterSelections {
    pub fn explicit(
        &self,
        target: target::TargetProfile,
    ) -> Option<&target::NormalizedElfInterpreterPlan> {
        self.explicit.iter().find(|plan| plan.target() == target)
    }

    pub fn effective(
        &self,
        target: target::TargetProfile,
    ) -> Option<target::NormalizedElfInterpreterPlan> {
        self.explicit(target)
            .cloned()
            .or_else(|| target::declared_elf_interpreter_default(target))
    }

    /// Read explicit loader paths from the per-target fields of the returned Build value.
    pub fn from_build_rows(rows: &[(String, BuildTimeValue)]) -> Result<Self, String> {
        let mut explicit = Vec::new();
        for target in target::ELF_INTERPRETER_TARGET_PROFILES {
            let name = target.target_name();
            let Some((_, value)) = rows.iter().find(|(row, _)| row == name) else {
                continue;
            };
            let BuildTimeValue::Text(bytes) = value else {
                return Err(format!(
                    "Build.elf_interpreter.{name} is not bytes: {value:?}"
                ));
            };
            if bytes.is_empty() {
                continue;
            }
            let plan = target::normalize_elf_interpreter_plan(bytes.clone(), target)
                .map_err(|error| format!("Build.elf_interpreter.{name} is invalid: {error}"))?;
            explicit.push(plan);
        }
        Ok(Self { explicit })
    }
}

impl Default for EvaluatedBuildSettings {
    fn default() -> Self {
        Self {
            application_intent: Some(HostedApplicationIntent::Console),
            application_identifier: None,
            subsystem: 3,
            optimizations: OptimizationSelections::default(),
            x86_deployment_claim: X86DeploymentClaim::Baseline,
            elf_interpreters: ElfInterpreterSelections::default(),
            tests_disabled: false,
            proof_index_expansion_terms: DEFAULT_PROOF_INDEX_EXPANSION_TERMS,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum X86DeploymentClaim {
    #[default]
    Baseline,
    AvxFma3,
}

impl X86DeploymentClaim {
    pub fn admit_for(
        self,
        profile: Option<target::TargetProfile>,
    ) -> Result<Option<target::AdmittedX86ScalarFmaProvider>, String> {
        match (self, profile) {
            (Self::Baseline, _) | (Self::AvxFma3, None) => Ok(None),
            (Self::AvxFma3, Some(profile))
                if profile.native_target().architecture != target::Architecture::X86_64 =>
            {
                Ok(None)
            }
            (Self::AvxFma3, Some(profile)) => {
                target::AdmittedX86ScalarFmaProvider::from_deployment_claim(
                    profile,
                    &target::X86_SCALAR_FMA_REQUIRED_FEATURES,
                )
                .map(Some)
                .map_err(|error| {
                    format!(
                        "Build.x86_deployment_features cannot admit AVX+FMA3 for exact profile `{}`: {error:?}",
                        profile.target_name()
                    )
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use target::TargetProfile::{LinuxArm64, LinuxX64};

    #[test]
    fn loader_selection_only_overrides_its_named_target() {
        let defaults = EvaluatedBuildSettings::default();
        let arm_loader = defaults.elf_interpreters.effective(LinuxArm64).unwrap();
        let custom = b"/runtime/ld-omega.so";
        let choices = ElfInterpreterSelections::from_build_rows(&[(
            LinuxX64.target_name().into(),
            BuildTimeValue::Text(custom.to_vec()),
        )])
        .unwrap();
        assert_eq!(
            choices.explicit(LinuxX64).unwrap().interpreter_path(),
            custom
        );
        assert_eq!(choices.explicit(LinuxArm64), None);
        assert_eq!(choices.effective(LinuxArm64).unwrap(), arm_loader);
        assert!(
            ElfInterpreterSelections::from_build_rows(&[(
                LinuxX64.target_name().into(),
                BuildTimeValue::Text(b"relative/ld.so".to_vec()),
            )])
            .is_err()
        );
    }
}

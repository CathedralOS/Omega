// Normalized Build settings decoded from an evaluated `BuildTimeValue`, separate from executed selection receipts.
//
// `EvaluatedBuildSettings` captures the authored presentation intent (`application_intent`),
// optional `application_identifier`, PE `subsystem` word, `freestanding` flag, root
// `optimizations`, `x86_deployment_claim`, granular `privileged_services` grants, and
// per-target `elf_interpreters`. Optional facets (`identifier`, `privileged_services`,
// `elf_interpreter`) default cleanly when omitted by builds that predate those surfaces.
// The `x86_deployment_claim` is a single Build value that each realized x86 profile
// admits independently via `X86DeploymentClaim::admit_for`, allowing a single Build
// to bind AArch64 targets alongside x86 targets without conflict.

use super::optimization;
use build_time_evaluation::BuildTimeValue;
use optimization_core::OptimizationSelections;
use target::{ApplicationIdentifier, HostedApplicationIntent};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluatedBuildSettings {
    pub application_intent: Option<HostedApplicationIntent>, // Authored hosted presentation intent, independent of PE loader metadata.
    pub application_identifier: Option<ApplicationIdentifier>, // Build-validated authored application identifier (`None` when omitted or empty).
    pub subsystem: u16, // PE optional-header Subsystem word (console 3 when unstated).
    pub freestanding: bool, // Freestanding image: no ambient host packages or import thunks.
    pub optimizations: OptimizationSelections, // Exact root-build optimization selections.
    pub x86_deployment_claim: X86DeploymentClaim, // Authored x86 deployment-feature claim admitted per realized x86 profile.
    pub privileged_services: PrivilegedServicesGrants, // Authored granular privileged-service grants (`port_io`, `interrupt_table`).
    pub elf_interpreters: ElfInterpreterSelections, // Authored per-target ELF loader selections (`PT_INTERP`).
    pub tests_disabled: bool, // Authored `builder.tests.disabled`: registered groups stay checked but are reported disabled, never run or passed.
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

    fn from_build_rows(rows: &[(String, BuildTimeValue)]) -> Result<Self, String> {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PrivilegedServicesGrants {
    pub port_io: bool, // Admit port I/O instructions (`in`/`out` authority class).
    pub interrupt_table: bool, // Admit interrupt-table publication instructions (`lidt` class).
}

impl Default for EvaluatedBuildSettings {
    fn default() -> Self {
        Self {
            application_intent: Some(HostedApplicationIntent::Console),
            application_identifier: None,
            subsystem: 3,
            freestanding: false,
            optimizations: OptimizationSelections::default(),
            x86_deployment_claim: X86DeploymentClaim::Baseline,
            privileged_services: PrivilegedServicesGrants::default(),
            elf_interpreters: ElfInterpreterSelections::default(),
            tests_disabled: false,
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

pub(crate) fn extract_build_settings(
    build: &BuildTimeValue,
    optimization_admission: optimization::BuildOptimizationAdmission,
    has_activation_vocabulary: bool,
) -> Result<
    (
        EvaluatedBuildSettings,
        optimization_core::OptimizationReportRequest,
    ),
    String,
> {
    let BuildTimeValue::Struct { fields, .. } = build else {
        return Err(format!("expected a Build struct, got {build:?}"));
    };
    let field = |name: &str| -> Result<&BuildTimeValue, String> {
        fields
            .iter()
            .find(|(field, _)| field == name)
            .map(|(_, value)| value)
            .ok_or_else(|| format!("the Build carries no `{name}` field"))
    };

    let x86_deployment_claim = if has_activation_vocabulary {
        let BuildTimeValue::Case { variant, payload } = field("x86_deployment_features")? else {
            return Err(
                "Build.x86_deployment_features is not an X86DeploymentFeatures case".to_owned(),
            );
        };
        if !payload.is_empty() {
            return Err(format!(
                "Build.x86_deployment_features case `{variant}` unexpectedly carries a payload"
            ));
        }
        match variant.rsplit("::").next().unwrap_or(variant) {
            "Baseline" => X86DeploymentClaim::Baseline,
            "AvxFma3" => X86DeploymentClaim::AvxFma3,
            other => {
                return Err(format!(
                    "Build.x86_deployment_features has unknown X86DeploymentFeatures case `{other}`"
                ));
            }
        }
    } else {
        X86DeploymentClaim::Baseline
    };

    let (application_intent, subsystem) = match field("subsystem")? {
        BuildTimeValue::Case { variant, payload } => {
            match variant.rsplit("::").next().unwrap_or(variant) {
                "Console" => (Some(HostedApplicationIntent::Console), 3u16),
                "Gui" => (Some(HostedApplicationIntent::Gui), 2),
                "EfiApplication" => (None, 10),
                "Unspecified" => match payload.iter().find(|(name, _)| name == "value") {
                    Some((_, BuildTimeValue::Int(value))) => (
                        None,
                        u16::try_from(*value).map_err(|_| {
                            format!("Unspecified subsystem value {value} exceeds a u16")
                        })?,
                    ),
                    other => {
                        return Err(format!(
                            "Unspecified subsystem carries no integer value: {other:?}"
                        ));
                    }
                },
                other => return Err(format!("unknown Subsystem case `{other}`")),
            }
        }
        other => {
            return Err(format!(
                "Build.subsystem is not a Subsystem case: {other:?}"
            ));
        }
    };

    let application_identifier = match field("identifier") {
        Ok(BuildTimeValue::Text(bytes)) => {
            if bytes.is_empty() {
                None
            } else {
                Some(ApplicationIdentifier::new(bytes).map_err(|reason| {
                    format!("Build.identifier is not a valid application identifier: {reason}")
                })?)
            }
        }
        Ok(other) => return Err(format!("Build.identifier is not bytes: {other:?}")),
        Err(_) => None,
    };

    let freestanding = match field("freestanding")? {
        BuildTimeValue::Bool(value) => *value,
        other => return Err(format!("Build.freestanding is not a bool: {other:?}")),
    };

    let privileged_services = match field("privileged_services") {
        Ok(BuildTimeValue::Struct { fields, .. }) => {
            let flag = |name: &str| -> Result<bool, String> {
                match fields
                    .iter()
                    .find(|(field, _)| field == name)
                    .map(|(_, value)| value)
                {
                    Some(BuildTimeValue::Bool(value)) => Ok(*value),
                    Some(other) => Err(format!(
                        "Build.privileged_services.{name} is not a bool: {other:?}"
                    )),
                    None => Ok(false),
                }
            };
            PrivilegedServicesGrants {
                port_io: flag("port_io")?,
                interrupt_table: flag("interrupt_table")?,
            }
        }
        Ok(other) => {
            return Err(format!(
                "Build.privileged_services is not a PrivilegedServices struct: {other:?}"
            ));
        }
        Err(_) => PrivilegedServicesGrants::default(),
    };

    let elf_interpreters = match field("elf_interpreter") {
        Ok(BuildTimeValue::Struct { fields, .. }) => {
            ElfInterpreterSelections::from_build_rows(fields)?
        }
        Ok(other) => {
            return Err(format!(
                "Build.elf_interpreter is not an ElfInterpreterSelection struct: {other:?}"
            ));
        }
        Err(_) => ElfInterpreterSelections::default(),
    };

    // Testing is enabled unless authored otherwise; builds that predate the
    // facet carry no `tests` row and keep that default.
    let tests_disabled = match field("tests") {
        Ok(BuildTimeValue::Struct { fields, .. }) => match fields
            .iter()
            .find(|(field, _)| field == "disabled")
            .map(|(_, value)| value)
        {
            Some(BuildTimeValue::Bool(value)) => *value,
            Some(other) => return Err(format!("Build.tests.disabled is not a bool: {other:?}")),
            None => false,
        },
        Ok(other) => return Err(format!("Build.tests is not a BuildTests struct: {other:?}")),
        Err(_) => false,
    };

    let (optimizations, optimization_report) = optimization_admission.extract(build)?;

    Ok((
        EvaluatedBuildSettings {
            application_intent,
            application_identifier,
            subsystem,
            freestanding,
            optimizations,
            x86_deployment_claim,
            privileged_services,
            elf_interpreters,
            tests_disabled,
        },
        optimization_report,
    ))
}

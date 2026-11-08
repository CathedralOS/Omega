//! Normalized Build settings, separate from executed selection receipts.

use super::optimization;
use build_time_evaluation::BuildTimeValue;
use optimization_core::OptimizationSelections;
use target::{ApplicationIdentifier, HostedApplicationIntent};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluatedBuildSettings {
    /// Authored hosted presentation intent, independent of PE loader metadata.
    /// EFI and raw `Unspecified` words carry no hosted application intent.
    pub application_intent: Option<HostedApplicationIntent>,
    /// Build-validated authored application identifier. `None` means the
    /// build supplied no `identifier` bytes; console output then uses the
    /// validated executable leaf as its ad-hoc signing label.
    pub application_identifier: Option<ApplicationIdentifier>,
    /// PE optional-header Subsystem word (console 3 when unstated).
    pub subsystem: u16,
    /// Freestanding image: no ambient host packages or import thunks.
    pub freestanding: bool,
    /// Exact root-build optimization selections. Empty is the ordinary
    /// compiler path and constructs no optimizer machinery.
    pub optimizations: OptimizationSelections,
    /// The authored x86 deployment-feature claim. It is one Build value for
    /// every target; each realized x86 profile admits it separately
    /// (`X86DeploymentClaim::admit_for`).
    pub x86_deployment_claim: X86DeploymentClaim,
    /// Authored granular privileged-service grants
    /// (wiki/spec/build/permissions.md#privileged-services): each flag admits
    /// exactly one mediated privileged class in the produced image's
    /// assembly authority discharge. Machine-owner authority has no granular
    /// grant and stays `freestanding`-only; `freestanding` already covers
    /// both mediated classes.
    pub privileged_services: PrivilegedServicesGrants,
    /// Authored per-target ELF loader selections
    /// (wiki/spec/build/configuration.md#elf-executable-loader-selection).
    /// Each is validated when the Build is read back; realization combines
    /// them with the selected target environment's declared default.
    pub elf_interpreters: ElfInterpreterSelections,
}

/// Rows of the authored `Build.elf_interpreter` selection: at most one exact
/// `PT_INTERP` pathname per ELF target profile, each already normalized.
///
/// An absent row keeps that target environment's declared default. A row
/// never requests dynamic linking or makes foreign imports compatible; the
/// image owner consumes the effective loader only for an object that retains
/// normalized ELF imports.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ElfInterpreterSelections {
    explicit: Vec<target::NormalizedElfInterpreterPlan>,
}

impl ElfInterpreterSelections {
    /// The authored row for one target, when the Build selected one.
    pub fn explicit(
        &self,
        target: target::TargetProfile,
    ) -> Option<&target::NormalizedElfInterpreterPlan> {
        self.explicit.iter().find(|plan| plan.target() == target)
    }

    /// The effective loader of one realized target: the authored row
    /// overrides the target environment's declared default. `None` means
    /// neither supplies one, so a dynamically linked output for this target
    /// must reject for missing output configuration.
    pub fn effective(
        &self,
        target: target::TargetProfile,
    ) -> Option<target::NormalizedElfInterpreterPlan> {
        self.explicit(target)
            .cloned()
            .or_else(|| target::declared_elf_interpreter_default(target))
    }

    /// Read the `ElfInterpreterSelection` rows keyed by canonical target
    /// name. Empty bytes mean no authored row; every nonempty row must be an
    /// absolute pathname without NUL bytes.
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

/// The independent mediated privileged-service grants retained from the
/// evaluated `Build.privileged_services` flags. Each admits only its own
/// asm authority class; neither implies the other nor machine-owner
/// authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PrivilegedServicesGrants {
    /// Admit port I/O instructions (`in`/`out` authority class).
    pub port_io: bool,
    /// Admit interrupt-table publication instructions (`lidt` class).
    pub interrupt_table: bool,
}

impl Default for EvaluatedBuildSettings {
    fn default() -> Self {
        Self {
            application_intent: Some(HostedApplicationIntent::Console),
            application_identifier: None,
            subsystem: 3, // IMAGE_SUBSYSTEM_WINDOWS_CUI -- the Console case's meaning
            freestanding: false,
            optimizations: OptimizationSelections::default(),
            x86_deployment_claim: X86DeploymentClaim::Baseline,
            privileged_services: PrivilegedServicesGrants::default(),
            elf_interpreters: ElfInterpreterSelections::default(),
        }
    }
}

/// The authored `Build.x86_deployment_features` claim.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum X86DeploymentClaim {
    #[default]
    Baseline,
    AvxFma3,
}

impl X86DeploymentClaim {
    /// Admit the claim for one realized profile. `None` is the generic SSE2
    /// baseline and grants no FMA route; the admitted carrier retains the
    /// canonical semantic cancellation-vector admission only, not a native
    /// differential execution receipt. The claim describes x86 deployments
    /// only, so one Build may bind AArch64 targets beside it: their realization
    /// admits nothing and keeps its baseline FMA route.
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

    // The x86 deployment claim is one Build value; it becomes an admitted
    // provider only for an x86 profile a realization selects.
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

    // An authored Build that predates the identifier surface carries no
    // `identifier` field; omission and an empty authored value both mean no
    // authored identity. Validation here is separate from the executable
    // name and never feeds the PE subsystem word.
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

    // An authored Build that predates the privileged-service surface carries
    // no `privileged_services` field; omission grants no mediated class.
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

    // An authored Build that predates the loader surface carries no
    // `elf_interpreter` field; omission selects no explicit loader.
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
        },
        optimization_report,
    ))
}

//! Concrete build intent and extraction from the evaluated Build value.

use crate::{RootBinding, WireCompatibilityDemand, optimization};
use build_time_evaluation::BuildTimeValue;
use optimization_core::OptimizationSelections;
use provider_planning::ProviderSelection;
use representation_planning::OpaqueRepresentationSelection;

/// The image facts the pipeline consumes, extracted from the augmented
/// `Build`. ZII: the default IS the zero value's meaning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildConfig {
    /// Authored hosted presentation intent, independent of PE loader metadata.
    /// EFI and raw `Unspecified` words carry no hosted application intent.
    pub application_intent: Option<HostedApplicationIntent>,
    /// PE optional-header Subsystem word (console 3 when unstated).
    pub subsystem: u16,
    /// Freestanding image: no ambient host packages or import thunks.
    pub freestanding: bool,
    /// Exact root-build optimization selections. Empty is the ordinary
    /// compiler path and constructs no optimizer machinery.
    pub optimizations: OptimizationSelections,
    /// Explicit x86 deployment-feature opt-in admitted for the exact selected
    /// profile. `None` is the generic SSE2 baseline and grants no FMA route.
    /// This carrier retains the canonical semantic cancellation-vector
    /// admission only; it is not a native differential execution receipt.
    pub x86_scalar_fma_provider: Option<target::AdmittedX86ScalarFmaProvider>,
    /// CH10 ROOT GRANTS (GR3): the symbol paths the final build accepted
    /// via `b.accept_boundary<pkg::symbol>();` -- harvested STATICALLY
    /// from the build machine's marker calls (grants are declarations,
    /// not runtime effects; the evaluator serves the marker as a no-op).
    pub grants: Vec<trust_model::AuthoredRootGrant>,
    /// PRV4c: explicit provider-type choices for boundary slots. These are
    /// declarations harvested from the authoritative build machine; they are
    /// validated against derived candidates before selection grants anything.
    pub provider_selections: Vec<ProviderSelection>,
    /// Exact named conformances activated as physical carriers for
    /// boundary-opaque values. The compiler derives shape from each carrier.
    pub opaque_representation_selections: Vec<OpaqueRepresentationSelection>,
    /// Channel/store compatibility demands. Each marker names the
    /// edge, format lineage, local and peer schemas, and the directional facts
    /// the final build requires.
    pub wire_compatibility_demands: Vec<WireCompatibilityDemand>,
    /// Target-owned inbound root slots bound by the authoritative build
    /// machine. The binding names an exact source machine; no entry discovery
    /// or naming convention participates once a binding is present.
    pub root_bindings: Vec<RootBinding>,
}

/// Portable presentation requested by the authored Console or Gui case.
/// This does not select an execution environment or enable bundle publication
/// independently of the selected target and requested output product.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostedApplicationIntent {
    Console,
    Gui,
}

impl Default for BuildConfig {
    fn default() -> Self {
        Self {
            application_intent: Some(HostedApplicationIntent::Console),
            subsystem: 3, // IMAGE_SUBSYSTEM_WINDOWS_CUI -- the Console case's meaning
            freestanding: false,
            optimizations: OptimizationSelections::default(),
            x86_scalar_fma_provider: None,
            grants: Vec::new(),
            provider_selections: Vec::new(),
            opaque_representation_selections: Vec::new(),
            wire_compatibility_demands: Vec::new(),
            root_bindings: Vec::new(),
        }
    }
}

pub(super) fn extract_build_config(
    build: &BuildTimeValue,
    optimization_admission: optimization::BuildOptimizationAdmission,
    selected_target_profile: Option<target::TargetProfile>,
    has_target_vocabulary: bool,
) -> Result<(BuildConfig, optimization_core::OptimizationReportRequest), String> {
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

    if has_target_vocabulary {
        let expected = selected_target_profile.ok_or_else(|| {
            "toolchain Build.target exists without an exact invocation target".to_owned()
        })?;
        let BuildTimeValue::Case { variant, payload } = field("target")? else {
            return Err("Build.target is not a TargetProfile case".to_owned());
        };
        if !payload.is_empty() {
            return Err(format!(
                "Build.target case `{variant}` unexpectedly carries a payload"
            ));
        }
        let case = variant.rsplit("::").next().unwrap_or(variant);
        let actual = target::TargetProfile::from_build_case_name(case)
            .ok_or_else(|| format!("Build.target has unknown TargetProfile case `{case}`"))?;
        if actual != expected {
            return Err(format!(
                "Build.target is compiler-owned and immutable: invocation supplied `{}`, but build evaluation returned `{}`",
                expected.build_case_name(),
                actual.build_case_name(),
            ));
        }
    } else if selected_target_profile.is_some() {
        return Err(
            "exact invocation target has no admitted toolchain Build.target vocabulary".to_owned(),
        );
    }

    let x86_scalar_fma_provider = if has_target_vocabulary {
        let profile = selected_target_profile.ok_or_else(|| {
            "toolchain Build.x86_deployment_features exists without an exact invocation target"
                .to_owned()
        })?;
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
            "Baseline" => None,
            "AvxFma3" => Some(
                target::AdmittedX86ScalarFmaProvider::from_deployment_claim(
                    profile,
                    &target::X86_SCALAR_FMA_REQUIRED_FEATURES,
                )
                .map_err(|error| {
                    format!(
                        "Build.x86_deployment_features cannot admit AVX+FMA3 for exact profile `{}`: {error:?}",
                        profile.target_name()
                    )
                })?,
            ),
            other => {
                return Err(format!(
                    "Build.x86_deployment_features has unknown X86DeploymentFeatures case `{other}`"
                ));
            }
        }
    } else {
        None
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

    let freestanding = match field("freestanding")? {
        BuildTimeValue::Bool(value) => *value,
        other => return Err(format!("Build.freestanding is not a bool: {other:?}")),
    };

    let (optimizations, optimization_report) = optimization_admission.extract(build)?;

    Ok((
        BuildConfig {
            application_intent,
            subsystem,
            freestanding,
            optimizations,
            x86_scalar_fma_provider,
            grants: Vec::new(),
            provider_selections: Vec::new(),
            opaque_representation_selections: Vec::new(),
            wire_compatibility_demands: Vec::new(),
            root_bindings: Vec::new(),
        },
        optimization_report,
    ))
}

#[cfg(test)]
mod tests {
    use super::BuildConfig;

    #[test]
    fn gui_intent_is_not_an_unspecified_pe_subsystem() {
        let mut typed = typed_trees::TypedTrees::default();
        typed.push_data_definition(typed_trees::data::DataDefinition {
            name: "Build".into(),
            ..Default::default()
        });
        let extract = |variant: &str, payload| {
            let build = super::BuildTimeValue::Struct {
                type_name: "Build".into(),
                fields: vec![
                    (
                        "subsystem".into(),
                        super::BuildTimeValue::Case {
                            variant: variant.into(),
                            payload,
                        },
                    ),
                    ("freestanding".into(), super::BuildTimeValue::Bool(false)),
                ],
            };
            super::extract_build_config(
                &build,
                super::optimization::BuildOptimizationAdmission::admit(&typed).unwrap(),
                None,
                false,
            )
            .unwrap()
            .0
        };
        let gui = extract("Gui", vec![]);
        let unspecified = extract(
            "Unspecified",
            vec![("value".into(), super::BuildTimeValue::Int(2))],
        );
        assert_eq!(gui.subsystem, unspecified.subsystem);
        assert_ne!(
            gui, unspecified,
            "equal PE words do not establish GUI application intent"
        );
        assert_eq!(
            gui.application_intent,
            Some(super::HostedApplicationIntent::Gui)
        );
        assert_eq!(unspecified.application_intent, None);
        let console = extract("Console", vec![]);
        assert_eq!(
            console.application_intent,
            Some(super::HostedApplicationIntent::Console)
        );
        assert_eq!(console.subsystem, 3);
        assert_eq!(console, BuildConfig::default());
        let efi = extract("EfiApplication", vec![]);
        assert_eq!(efi.application_intent, None);
        assert_eq!(efi.subsystem, 10);
        assert!(
            !efi.freestanding,
            "presentation extraction must not override environment policy"
        );
        for word in [0, 2, 3, 10, 65535] {
            let raw = extract(
                "Unspecified",
                vec![("value".into(), super::BuildTimeValue::Int(word))],
            );
            assert_eq!(raw.application_intent, None);
            assert_eq!(raw.subsystem, word as u16);
        }
    }
}

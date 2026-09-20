//! Concrete build intent and extraction from the evaluated Build value.

use crate::admission::behavior_exclusions::AuthoredBehaviorExclusion;
use crate::admission::component_assumptions::AuthoredComponentAssumptionAcceptance;
use crate::{RootBinding, WireCompatibilityDemand, optimization};
use build_time_evaluation::BuildTimeValue;
use optimization_core::OptimizationSelections;
use provider_planning::ProviderSelection;
use representation_planning::OpaqueRepresentationSelection;

/// The authored application identifier: it supplies the GUI CodeDirectory
/// signing identity and `CFBundleIdentifier`
/// (wiki/spec/build/macos_application.md). Validation is deliberately
/// separate from the executable-name rule: nonempty ASCII in
/// `A-Z a-z 0-9 . -` with no empty `.`-separated segment.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ApplicationIdentifier(String);

impl ApplicationIdentifier {
    /// Validate authored `builder.identifier` bytes. An empty value is not
    /// admitted here: omission and an empty field both produce `None` at
    /// extraction, so `new` only sees an explicitly authored identity.
    pub fn new(bytes: &[u8]) -> Result<Self, &'static str> {
        if bytes.is_empty() {
            return Err("identifier is empty");
        }
        if !bytes.is_ascii() {
            return Err("identifier is not ASCII");
        }
        if bytes.iter().any(|byte| {
            !matches!(
                byte,
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'.' | b'-'
            )
        }) {
            return Err("identifier contains a byte outside `A-Z a-z 0-9 . -`");
        }
        if bytes
            .split(|byte| *byte == b'.')
            .any(|segment| segment.is_empty())
        {
            return Err("identifier has an empty `.`-separated segment");
        }
        Ok(Self(
            String::from_utf8(bytes.to_vec()).expect("ASCII identifier bytes are UTF-8"),
        ))
    }

    /// The validated identifier spelling bound into CodeDirectory signing and
    /// `CFBundleIdentifier` publication.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The image facts the pipeline consumes, extracted from the augmented
/// `Build`. ZII: the default IS the zero value's meaning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildConfig {
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
    /// Optional proof-carrying product requests
    /// (wiki/spec/proofs/publication.md). Both flags are independent and
    /// default to false; they request adjacent `.proof` sidecars beside the
    /// ordinary artifacts and never weaken ordinary checking or select a
    /// different pipeline.
    pub pcc: PccRequests,
    /// Behavior exclusions (wiki/spec/build/behavior_exclusions.md): the
    /// authored `builder.exclude_crash(...)` product-admission requirements
    /// harvested statically from the root build machine's checked call scope,
    /// each retaining its exact toolchain case identity and authored span.
    /// These rows declare what the selected executable composition must not
    /// reach; the admission join itself consumes them later.
    pub behavior_exclusions: Vec<AuthoredBehaviorExclusion>,
    /// Accepted component-assumption digests
    /// (wiki/spec/build/component_publication.md): the authored
    /// `builder.accept_component_assumption(...)` declarations harvested
    /// statically from the root build machine's checked call scope. An
    /// attached independent-component description that binds an inseparable
    /// assumption verifies only when its exact digest appears here; the
    /// roster carries no accepted digest until the build authors one.
    pub accepted_component_assumptions: Vec<AuthoredComponentAssumptionAcceptance>,
}

/// The two independent optional proof-product selections retained from
/// normalized Build. Each requests the matching artifact/`.proof` companion
/// pair at publication; neither grants receiving authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PccRequests {
    /// Publish the Psi artifact beside a `.proof` companion. A Psi request
    /// retains the Psi artifact even during source-to-native compilation.
    pub psi: bool,
    /// Publish the native artifact beside a standalone `.proof` companion.
    /// A stop that excludes native production cannot satisfy this request.
    pub native: bool,
}

impl PccRequests {
    /// True when either optional proof product was requested.
    pub const fn any(self) -> bool {
        self.psi || self.native
    }
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
            application_identifier: None,
            subsystem: 3, // IMAGE_SUBSYSTEM_WINDOWS_CUI -- the Console case's meaning
            freestanding: false,
            optimizations: OptimizationSelections::default(),
            x86_scalar_fma_provider: None,
            grants: Vec::new(),
            provider_selections: Vec::new(),
            opaque_representation_selections: Vec::new(),
            wire_compatibility_demands: Vec::new(),
            root_bindings: Vec::new(),
            pcc: PccRequests::default(),
            behavior_exclusions: Vec::new(),
            accepted_component_assumptions: Vec::new(),
        }
    }
}

pub(crate) fn extract_build_config(
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

    // An authored Build that predates the PCC surface carries no `pcc`
    // field; omission means false for both independent requests.
    let pcc = match field("pcc") {
        Ok(BuildTimeValue::Struct { fields, .. }) => {
            let flag = |name: &str| -> Result<bool, String> {
                match fields
                    .iter()
                    .find(|(field, _)| field == name)
                    .map(|(_, value)| value)
                {
                    Some(BuildTimeValue::Bool(value)) => Ok(*value),
                    Some(other) => Err(format!("Build.pcc.{name} is not a bool: {other:?}")),
                    None => Ok(false),
                }
            };
            PccRequests {
                psi: flag("psi")?,
                native: flag("native")?,
            }
        }
        Ok(other) => return Err(format!("Build.pcc is not a Pcc struct: {other:?}")),
        Err(_) => PccRequests::default(),
    };

    let (optimizations, optimization_report) = optimization_admission.extract(build)?;

    Ok((
        BuildConfig {
            application_intent,
            application_identifier,
            subsystem,
            freestanding,
            optimizations,
            x86_scalar_fma_provider,
            grants: Vec::new(),
            provider_selections: Vec::new(),
            opaque_representation_selections: Vec::new(),
            wire_compatibility_demands: Vec::new(),
            root_bindings: Vec::new(),
            pcc,
            behavior_exclusions: Vec::new(),
            accepted_component_assumptions: Vec::new(),
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

    #[test]
    fn identifier_extraction_validates_authored_bytes() {
        let mut typed = typed_trees::TypedTrees::default();
        typed.push_data_definition(typed_trees::data::DataDefinition {
            name: "Build".into(),
            ..Default::default()
        });
        let extract = |identifier: Option<super::BuildTimeValue>| {
            let mut fields = vec![
                (
                    "subsystem".into(),
                    super::BuildTimeValue::Case {
                        variant: "Gui".into(),
                        payload: vec![],
                    },
                ),
                ("freestanding".into(), super::BuildTimeValue::Bool(false)),
            ];
            if let Some(identifier) = identifier {
                fields.push(("identifier".into(), identifier));
            }
            super::extract_build_config(
                &super::BuildTimeValue::Struct {
                    type_name: "Build".into(),
                    fields,
                },
                super::optimization::BuildOptimizationAdmission::admit(&typed).unwrap(),
                None,
                false,
            )
        };
        // A Build predating the field, or an empty authored value, means no
        // authored identity.
        assert_eq!(extract(None).unwrap().0.application_identifier, None);
        assert_eq!(
            extract(Some(super::BuildTimeValue::Text(Vec::new())))
                .unwrap()
                .0
                .application_identifier,
            None
        );
        let gui = extract(Some(super::BuildTimeValue::Text(
            b"com.omega.window-app".to_vec(),
        )))
        .unwrap()
        .0;
        assert_eq!(
            gui.application_identifier
                .as_ref()
                .map(super::ApplicationIdentifier::as_str),
            Some("com.omega.window-app")
        );
        // The identifier never feeds the PE subsystem word or hosted intent.
        assert_eq!(gui.subsystem, 2);
        assert_eq!(
            gui.application_intent,
            Some(super::HostedApplicationIntent::Gui)
        );
        for invalid in [
            b"has_underscore".as_slice(),
            b"has space".as_slice(),
            b"double..dot".as_slice(),
            b".leading".as_slice(),
            b"trailing.".as_slice(),
            &[0x80, 0x81][..],
        ] {
            let diagnostic = extract(Some(super::BuildTimeValue::Text(invalid.to_vec())))
                .expect_err("invalid identifier bytes must reject");
            assert!(
                diagnostic.contains("Build.identifier"),
                "unexpected diagnostic: {diagnostic}"
            );
        }
        let non_bytes = extract(Some(super::BuildTimeValue::Int(3)))
            .expect_err("non-bytes identifier must reject");
        assert!(non_bytes.contains("Build.identifier is not bytes"));
    }
}

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
    /// Optional proof-carrying product requests
    /// (wiki/spec/proofs/publication.md). Both flags are independent and
    /// default to false; they request adjacent `.proof` sidecars beside the
    /// ordinary artifacts and never weaken ordinary checking or select a
    /// different pipeline.
    pub pcc: PccRequests,
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
            pcc: PccRequests::default(),
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
    /// differential execution receipt.
    pub fn admit_for(
        self,
        profile: Option<target::TargetProfile>,
    ) -> Result<Option<target::AdmittedX86ScalarFmaProvider>, String> {
        match (self, profile) {
            (Self::Baseline, _) | (Self::AvxFma3, None) => Ok(None),
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
            pcc,
            privileged_services,
            elf_interpreters,
        },
        optimization_report,
    ))
}

#[cfg(test)]
mod tests {
    use super::EvaluatedBuildSettings;

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
            super::extract_build_settings(
                &build,
                super::optimization::BuildOptimizationAdmission::admit(&typed).unwrap(),
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
        assert_eq!(console, EvaluatedBuildSettings::default());
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
            super::extract_build_settings(
                &super::BuildTimeValue::Struct {
                    type_name: "Build".into(),
                    fields,
                },
                super::optimization::BuildOptimizationAdmission::admit(&typed).unwrap(),
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

    #[test]
    fn privileged_services_extraction_reads_each_grant_independently() {
        let mut typed = typed_trees::TypedTrees::default();
        typed.push_data_definition(typed_trees::data::DataDefinition {
            name: "Build".into(),
            ..Default::default()
        });
        let extract = |privileged_services: Option<super::BuildTimeValue>| {
            let mut fields = vec![
                (
                    "subsystem".into(),
                    super::BuildTimeValue::Case {
                        variant: "Console".into(),
                        payload: vec![],
                    },
                ),
                ("freestanding".into(), super::BuildTimeValue::Bool(false)),
            ];
            if let Some(privileged_services) = privileged_services {
                fields.push(("privileged_services".into(), privileged_services));
            }
            super::extract_build_settings(
                &super::BuildTimeValue::Struct {
                    type_name: "Build".into(),
                    fields,
                },
                super::optimization::BuildOptimizationAdmission::admit(&typed).unwrap(),
                false,
            )
        };
        let grants = |fields: Vec<(&str, bool)>| super::BuildTimeValue::Struct {
            type_name: "PrivilegedServices".into(),
            fields: fields
                .into_iter()
                .map(|(name, value)| (name.into(), super::BuildTimeValue::Bool(value)))
                .collect(),
        };
        // A Build predating the field grants nothing.
        assert_eq!(
            extract(None).unwrap().0.privileged_services,
            super::PrivilegedServicesGrants::default()
        );
        // Each flag admits only its own class; an unset flag defaults false.
        assert_eq!(
            extract(Some(grants(vec![("port_io", true)])))
                .unwrap()
                .0
                .privileged_services,
            super::PrivilegedServicesGrants {
                port_io: true,
                interrupt_table: false,
            }
        );
        assert_eq!(
            extract(Some(grants(vec![("interrupt_table", true)])))
                .unwrap()
                .0
                .privileged_services,
            super::PrivilegedServicesGrants {
                port_io: false,
                interrupt_table: true,
            }
        );
        assert_eq!(
            extract(Some(grants(vec![
                ("port_io", true),
                ("interrupt_table", true)
            ])))
            .unwrap()
            .0
            .privileged_services,
            super::PrivilegedServicesGrants {
                port_io: true,
                interrupt_table: true,
            }
        );
        let non_bool = extract(Some(super::BuildTimeValue::Struct {
            type_name: "PrivilegedServices".into(),
            fields: vec![("port_io".into(), super::BuildTimeValue::Int(1))],
        }))
        .expect_err("non-bool privileged_services flag must reject");
        assert!(non_bool.contains("Build.privileged_services.port_io is not a bool"));
        let non_struct = extract(Some(super::BuildTimeValue::Bool(true)))
            .expect_err("non-struct privileged_services must reject");
        assert!(
            non_struct.contains("Build.privileged_services is not a PrivilegedServices struct")
        );
    }

    #[test]
    fn elf_interpreter_rows_override_target_environment_defaults() {
        use target::TargetProfile;

        let mut typed = typed_trees::TypedTrees::default();
        typed.push_data_definition(typed_trees::data::DataDefinition {
            name: "Build".into(),
            ..Default::default()
        });
        let extract = |selection: Option<super::BuildTimeValue>| {
            let mut fields = vec![
                (
                    "subsystem".into(),
                    super::BuildTimeValue::Case {
                        variant: "Console".into(),
                        payload: vec![],
                    },
                ),
                ("freestanding".into(), super::BuildTimeValue::Bool(false)),
            ];
            if let Some(selection) = selection {
                fields.push(("elf_interpreter".into(), selection));
            }
            super::extract_build_settings(
                &super::BuildTimeValue::Struct {
                    type_name: "Build".into(),
                    fields,
                },
                super::optimization::BuildOptimizationAdmission::admit(&typed).unwrap(),
                false,
            )
            .map(|(settings, _)| settings.elf_interpreters)
        };
        let rows = |arm64: &[u8], x86_64: &[u8]| super::BuildTimeValue::Struct {
            type_name: "ElfInterpreterSelection".into(),
            fields: vec![
                (
                    "linux_arm64".into(),
                    super::BuildTimeValue::Text(arm64.to_vec()),
                ),
                (
                    "linux_x86_64".into(),
                    super::BuildTimeValue::Text(x86_64.to_vec()),
                ),
            ],
        };
        let path = |plan: Option<target::NormalizedElfInterpreterPlan>| {
            plan.map(|plan| plan.interpreter_path().to_vec())
        };

        // A Build predating the field and an all-empty selection both keep
        // every target environment's declared default.
        for unselected in [
            extract(None).unwrap(),
            extract(Some(rows(b"", b""))).unwrap(),
        ] {
            assert_eq!(unselected, super::ElfInterpreterSelections::default());
            assert_eq!(unselected.explicit(TargetProfile::LinuxX64), None);
            assert_eq!(
                path(unselected.effective(TargetProfile::LinuxX64)),
                Some(b"/lib64/ld-linux-x86-64.so.2".to_vec())
            );
            assert_eq!(
                path(unselected.effective(TargetProfile::LinuxArm64)),
                Some(b"/lib/ld-linux-aarch64.so.1".to_vec())
            );
            assert_eq!(unselected.effective(TargetProfile::MacosArm64), None);
        }

        // One target's explicit row overrides only that target's default.
        let musl = extract(Some(rows(b"", b"/lib/ld-musl-x86_64.so.1"))).unwrap();
        let selected = musl
            .explicit(TargetProfile::LinuxX64)
            .expect("the authored linux_x86_64 row is retained");
        assert_eq!(selected.target(), TargetProfile::LinuxX64);
        assert_eq!(
            path(musl.effective(TargetProfile::LinuxX64)),
            Some(b"/lib/ld-musl-x86_64.so.1".to_vec())
        );
        assert_eq!(musl.explicit(TargetProfile::LinuxArm64), None);
        assert_eq!(
            path(musl.effective(TargetProfile::LinuxArm64)),
            Some(b"/lib/ld-linux-aarch64.so.1".to_vec())
        );

        // Relative and NUL-bearing pathnames reject when the Build is read
        // back, naming the exact target row.
        let relative = extract(Some(rows(b"", b"lib64/ld-linux-x86-64.so.2")))
            .expect_err("a relative interpreter pathname must reject");
        assert!(
            relative.contains("Build.elf_interpreter.linux_x86_64 is invalid")
                && relative.contains("not absolute"),
            "unexpected diagnostic: {relative}"
        );
        let nul = extract(Some(rows(b"/lib/ld\0-linux-aarch64.so.1", b"")))
            .expect_err("a NUL-bearing interpreter pathname must reject");
        assert!(
            nul.contains("Build.elf_interpreter.linux_arm64 is invalid")
                && nul.contains("NUL byte at offset 7"),
            "unexpected diagnostic: {nul}"
        );
        let non_bytes = extract(Some(super::BuildTimeValue::Struct {
            type_name: "ElfInterpreterSelection".into(),
            fields: vec![("linux_x86_64".into(), super::BuildTimeValue::Int(1))],
        }))
        .expect_err("a non-bytes interpreter row must reject");
        assert!(non_bytes.contains("Build.elf_interpreter.linux_x86_64 is not bytes"));
        let non_struct = extract(Some(super::BuildTimeValue::Bool(true)))
            .expect_err("a non-struct selection must reject");
        assert!(non_struct.contains("Build.elf_interpreter is not an ElfInterpreterSelection"));
    }
}

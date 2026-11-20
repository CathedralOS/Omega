// Normalized Build settings decoded from an evaluated `BuildTimeValue`, separate from executed selection receipts.
//
// `EvaluatedBuildSettings` captures the authored presentation intent (`application_intent`),
// optional `application_identifier`, PE `subsystem` word, root
// `optimizations`, `x86_deployment_claim`, and per-target `elf_interpreters`.
// Optional facets (`identifier`, `elf_interpreter`) default cleanly when omitted
// by builds that predate those surfaces.
// The `x86_deployment_claim` is a single Build value that each realized x86 profile
// admits independently via `X86DeploymentClaim::admit_for`, allowing a single Build
// to bind AArch64 targets alongside x86 targets without conflict.

use super::optimization;
use normalized_build_settings::{ElfInterpreterSelections, EvaluatedBuildSettings, X86DeploymentClaim};
use build_time_values::BuildTimeValue;
use target::{ApplicationIdentifier, HostedApplicationIntent};

pub(crate) fn extract_build_settings(
    build: &BuildTimeValue,
    optimization_admission: optimization::BuildOptimizationAdmission,
    has_activation_vocabulary: bool,
) -> Result<
    (
        EvaluatedBuildSettings,
        optimization_requests::OptimizationReportRequest,
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

    // Keep validating the authored legacy field until its declaration is retired.
    // It supplies no normalized output choice or per-output hardware authority.
    match field("freestanding")? {
        BuildTimeValue::Bool(_) => {}
        other => return Err(format!("Build.freestanding is not a bool: {other:?}")),
    }

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
            optimizations,
            x86_deployment_claim,
            elf_interpreters,
            tests_disabled,
        },
        optimization_report,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build_execution::build_value::BuildValueAdmission;

    // Use the production source assembler: it loads the real toolchain prelude
    // and its Core dependencies with their ordinary source custody. No local
    // declaration or unchecked constructor stands in for Build admission.
    fn admitted_build() -> (BuildValueAdmission, BuildTimeValue) {
        let temporary = platform_custody::temporary_directory::TemporaryDirectory::create(
            "build-legacy-setting",
            false,
        )
        .expect("create an owned Build fixture directory");
        let root = temporary.path();
        let main = root.join("main.omg");
        std::fs::write(&main, "machine main() {}\n").expect("stage Product source");
        std::fs::write(
            root.join("build.omg"),
            build_declarations::default_build_source(),
        )
        .expect("stage the ordinary Build entry");
        let mut timings = artifacts::compile_timings::CompileTimings::default();
        let mut catalogue = source_compiler::source_inputs::SourceCatalogue::default();
        let checkpoint = crate::source_assembly::ImmutableSourceParseCheckpoint::prepare(
            &main,
            None,
            None,
            &mut timings,
            &mut catalogue,
        )
        .expect("admit the fixture's actual sources");
        let (_, syntax) = checkpoint
            .assemble(None, &mut timings, &mut catalogue)
            .expect("assemble the real Build vocabulary");
        let build_source = syntax.build_source.expect("production Build selection");
        let profile = target::TargetProfile::host_if_supported().expect("supported Build host");
        let (source, _) = source_compiler::ResolveAndTypeRequest {
            syntax: syntax.syntax_trees,
            source_items: syntax.source_items,
            root_sources: syntax.root_sources,
            frontends: Default::default(),
            build_helper_identity: profile.target_name(),
            build_sources: syntax.build_scope_sources,
            build_stage: None,
            sources: syntax.sources,
            top_level_bindings: syntax.source_scoped_top_level_bindings,
            selection_authority: None,
        }
        .prepare(&mut source_compiler::UnmeasuredProduction)
        .expect("type the production Build declaration");
        let typed = source.typed();
        let machine =
            crate::build_execution::select_build_machine(typed, Some(build_source.source_id()))
                .expect("select the source-owned Build entry")
                .expect("the authored Build entry");
        let admission = BuildValueAdmission::admit(typed).expect("real Build admission");
        let scope = crate::build_execution::BuildMachineFilesystemScope::for_root(
            &main,
            root.join("output"),
            None,
        );
        let value = admission.initial_value(typed, machine, &scope);
        (admission, value)
    }

    fn fields(value: &mut BuildTimeValue) -> &mut Vec<(String, BuildTimeValue)> {
        let BuildTimeValue::Struct { fields, .. } = value else {
            panic!("the admitted initial Build is a record");
        };
        fields
    }

    fn set(value: &mut BuildTimeValue, name: &str, changed: BuildTimeValue) {
        let (_, field) = fields(value)
            .iter_mut()
            .find(|(field, _)| field == name)
            .expect("the production Build has the tested field");
        *field = changed;
    }

    #[test]
    fn legacy_freestanding_is_validated_without_becoming_a_choice() {
        let (admission, mut original) = admitted_build();
        assert!(
            !fields(&mut original)
                .iter()
                .any(|(name, _)| name == "privileged_services")
        );
        let baseline = admission
            .extract(&original)
            .expect("decode the real initial Build");
        assert_eq!(baseline.0, EvaluatedBuildSettings::default());

        let mut changed = original.clone();
        set(
            &mut changed,
            "subsystem",
            BuildTimeValue::Case {
                variant: "Gui".into(),
                payload: Vec::new(),
            },
        );
        let gui = admission
            .extract(&changed)
            .expect("decode a different retained choice");
        assert_ne!(
            gui.0, baseline.0,
            "real normalized choices still distinguish settings"
        );
        assert_eq!(gui.0.subsystem, 2);
        assert_eq!(gui.1, baseline.1);

        let mut missing = original.clone();
        fields(&mut missing).retain(|(name, _)| name != "freestanding");
        assert_eq!(
            admission.extract(&missing).unwrap_err(),
            "the Build carries no `freestanding` field"
        );
        for wrong in [
            BuildTimeValue::Unit,
            BuildTimeValue::Int(1),
            BuildTimeValue::Text(b"yes".to_vec()),
        ] {
            let mut malformed = original.clone();
            let expected = format!("Build.freestanding is not a bool: {wrong:?}");
            set(&mut malformed, "freestanding", wrong);
            // The later invalid setting must not hide this first refusal.
            set(&mut malformed, "elf_interpreter", BuildTimeValue::Unit);
            assert_eq!(admission.extract(&malformed).unwrap_err(), expected);
        }

        let mut malformed = original.clone();
        set(&mut malformed, "elf_interpreter", BuildTimeValue::Unit);
        assert_eq!(
            admission.extract(&malformed).unwrap_err(),
            "Build.elf_interpreter is not an ElfInterpreterSelection struct: Unit"
        );

        let mut changed = original.clone();
        set(&mut changed, "freestanding", BuildTimeValue::Bool(true));
        assert_eq!(
            admission
                .extract(&changed)
                .expect("accept the legacy Boolean"),
            baseline,
            "the legacy Boolean must not distinguish normalized Build choices"
        );
    }
}

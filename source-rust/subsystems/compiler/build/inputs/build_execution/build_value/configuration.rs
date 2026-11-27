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
use build_model::values::BuildTimeValue;
use crate::normalized_settings::{
    ElfInterpreterSelections, EvaluatedBuildSettings, X86DeploymentClaim,
};
use target::{ApplicationIdentifier, HostedApplicationIntent};

pub(crate) fn extract_build_settings(
    build: &BuildTimeValue,
) -> Result<
    (
        EvaluatedBuildSettings,
        build_model::optimization_requests::OptimizationReportRequest,
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

    let x86_deployment_claim = {
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

    let proof_index_expansion_terms = match field("proof") {
        Ok(BuildTimeValue::Struct { fields, .. }) => match fields
            .iter()
            .find(|(field, _)| field == "index_expansion_terms")
            .map(|(_, value)| value)
        {
            Some(BuildTimeValue::Int(value)) => {
                if *value <= 0 {
                    return Err(format!(
                        "Build.proof.index_expansion_terms must be a positive integer (got {value}); 0 is rejected rather than unlimited"
                    ));
                }
                *value as u64
            }
            Some(other) => {
                return Err(format!(
                    "Build.proof.index_expansion_terms is not an integer: {other:?}"
                ));
            }
            None => crate::normalized_settings::DEFAULT_PROOF_INDEX_EXPANSION_TERMS,
        },
        Ok(other) => {
            return Err(format!(
                "Build.proof is not a ProofWorkPolicy struct: {other:?}"
            ));
        }
        Err(_) => crate::normalized_settings::DEFAULT_PROOF_INDEX_EXPANSION_TERMS,
    };

    let (optimizations, optimization_report) = optimization::extract(field("optimizations")?)?;

    Ok((
        EvaluatedBuildSettings {
            application_intent,
            application_identifier,
            subsystem,
            optimizations,
            x86_deployment_claim,
            elf_interpreters,
            tests_disabled,
            proof_index_expansion_terms,
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
            "build-settings",
            false,
        )
        .expect("create an owned Build fixture directory");
        let root = temporary.path();
        let main = root.join("main.omg");
        std::fs::write(&main, "machine main() {}\n").expect("stage Product source");
        std::fs::write(
            root.join("build.omg"),
            "machine build(builder: @mutable BuildPlan) {}",
        )
        .expect("stage the ordinary Build entry");
        let mut timings = artifacts::compile_timings::CompileTimings::default();
        let mut catalogue =
            source_compiler::representations::source_inputs::SourceCatalogue::default();
        let checkpoint = crate::source_assembly::ImmutableSourceParseCheckpoint::prepare(
            crate::source_assembly::CapturedBuildEntry::Initial(&root.join("build.omg")),
            None,
            None,
            &mut timings,
            &mut catalogue,
        )
        .expect("admit the fixture's actual sources");
        let (_, syntax) = checkpoint
            .assemble(None, &Default::default(), &mut timings, &mut catalogue)
            .expect("assemble the real Build vocabulary");
        let profile = target::TargetProfile::host_if_supported().expect("supported Build host");
        let interval = syntax
            .build_stage_source()
            .unwrap()
            .prepare(&Default::default(), profile.target_name(), None)
            .unwrap();
        let typed = interval.program.as_ref();
        let admission = BuildValueAdmission::admit(typed).expect("real Build admission");
        drop(interval);
        let value = admission.initial_value();
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
    fn initial_build_contains_only_current_settings() {
        let (admission, mut original) = admitted_build();
        assert!(
            !fields(&mut original)
                .iter()
                .any(|(name, _)| matches!(name.as_str(), "privileged_services" | "freestanding" | "inputs"))
        );
        assert!(
            fields(&mut original)
                .iter()
                .any(|(name, value)| name == "identifier"
                    && matches!(value, BuildTimeValue::Text(bytes) if bytes.is_empty()))
        );
        let baseline = admission
            .extract(&original)
            .expect("decode the real initial Build");
        assert_eq!(baseline.0, EvaluatedBuildSettings::default());

        assert_eq!(
            baseline.1,
            build_model::optimization_requests::OptimizationReportRequest::Suppressed
        );

        let mut requested = original.clone();
        let (_, options) = fields(&mut requested)
            .iter_mut()
            .find(|(name, _)| name == "optimizations")
            .unwrap();
        let option = optimization_selections::Optimization::CopyPropagation;
        let mut counters =
            vec![BuildTimeValue::Int(0); optimization_selections::Optimization::ALL.len()];
        let slot = optimization_selections::Optimization::ALL
            .iter()
            .position(|candidate| *candidate == option)
            .unwrap();
        counters[slot] = BuildTimeValue::Int(1);
        set(options, "selected", BuildTimeValue::Array(counters));
        set(options, "human_report", BuildTimeValue::Int(1));
        let (settings, report) = admission.extract(&requested).unwrap();
        assert_eq!(settings.optimizations.as_slice(), &[option]);
        assert_eq!(
            report,
            build_model::optimization_requests::OptimizationReportRequest::EmitHumanText
        );

        fields(&mut requested).retain(|(name, _)| name != "optimizations");
        assert_eq!(
            admission.extract(&requested).unwrap_err(),
            "the Build carries no `optimizations` field"
        );

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

        let mut malformed = original.clone();
        set(&mut malformed, "elf_interpreter", BuildTimeValue::Unit);
        assert_eq!(
            admission.extract(&malformed).unwrap_err(),
            "Build.elf_interpreter is not an ElfInterpreterSelection struct: Unit"
        );

        assert_eq!(
            baseline.0.proof_index_expansion_terms,
            crate::normalized_settings::DEFAULT_PROOF_INDEX_EXPANSION_TERMS
        );
        let mut raised = original.clone();
        let (_, proof) = fields(&mut raised)
            .iter_mut()
            .find(|(name, _)| name == "proof")
            .unwrap();
        set(proof, "index_expansion_terms", BuildTimeValue::Int(128));
        let (raised_settings, _) = admission.extract(&raised).unwrap();
        assert_eq!(raised_settings.proof_index_expansion_terms, 128);

        let mut zeroed = original.clone();
        let (_, proof) = fields(&mut zeroed)
            .iter_mut()
            .find(|(name, _)| name == "proof")
            .unwrap();
        set(proof, "index_expansion_terms", BuildTimeValue::Int(0));
        assert!(
            admission
                .extract(&zeroed)
                .unwrap_err()
                .contains("index_expansion_terms must be a positive integer"),
            "0 must be rejected rather than treated as unlimited"
        );
    }
}

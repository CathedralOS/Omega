#![forbid(unsafe_code)]

/*
  Omega Stage 00 (`00_terminal-psi-to-abstract-operations`): canonical Terminal
  Psi admission, native execution-scope closure, boundary/callback/entry
  settlement, Psi optimization-unit construction, and lowering into
  `AbstractOperationPlan`.

  We admit canonical Terminal artifacts once at the Psi-to-Omega boundary and
  produce the source-free inputs required by downstream pipeline stages.
  `artifact_admission` decodes loose canonical section bytes (`lower_artifact`)
  or reads a `CanonicalTerminalArtifact`'s retained module, enforces access-view
  establishment custody, and separates native realization authority from
  optimizer-only custody. `native_execution_scope` computes the reachable
  machine, boundary, callback, cleanup, and interrupt closure and sequences
  provider admission (`admit_native_realization`). `retained_terminal` and its
  `retained_terminal_codec` / `retained_terminal_vocabulary` companions join and
  persist compiler-private native derivation proposals without replaying source
  joins. `settlement`, `callback_settlement`, and `entry_settlement` validate
  boundary applications, comparisons, callback placements, and `ProgramEntry`
  sidecars, and `lowering` (`lowering_scalars`, `lowering_storage`) lowers each
  machine into `AbstractOperationPlan`, retained with its admitted selections as
  the `VerifiedPsiOptimizationInput` that Omega 01 builds its unit from.
*/

/*
  Decoding and verifying a canonical Terminal module is expensive, and source
  trees must never cross into the native backend. By decoding the canonical
  module at a single byte entrance (pinned by `terminal_decode_once`) and
  joining provider, callback, comparison, and entry evidence once in
  `retained_terminal`, we avoid redundant module decodes and prevent later
  stages from re-running source-level provider or boundary selection. Pruning
  unreachable machines and unused target bodies in `native_execution_scope`
  before lowering ensures that abstract optimization, target lowering, and
  instruction selection operate only on the executable closure of the selected
  entry.
*/

/*
  Consumes `CanonicalTerminalArtifact` (`PSITERM\0`) from Psi Stage 08 alongside
  build-evaluated target selections, calling plans, and provider evidence from
  `provider-admission` and `terminal-installation`. Produces
  `AdmittedNativeRealization`, `VerifiedNativeArtifactInput`,
  `VerifiedPsiOptimizationInput`, and `AbstractOperationPlan` for Omega Stage 01
  (`01_abstract-operations-to-abstract-operations`) and Omega Stage 02
  (`02_abstract-operations-to-target-operations`).
*/
mod artifact_admission;
mod lowering;
mod native_execution_scope;
use ::provider_admission as providers;
pub use ::provider_admission::AdmittedInstalledEntryExecution;
pub use ::provider_admission::provider_admission;
pub(crate) use native_execution_scope::native_admission;
mod retained_terminal;
mod settlement;
pub use installation_evidence::{
    BootstrapAssumptionInterval, BootstrapEnvironmentAssumption, InstallationEnvironmentAssumption,
    InterruptDeliveryInterval, X86InterruptDeliveryAssumption, X86InterruptDeliveryProfile,
};
pub use native_admission::{
    AdmittedNativeRealization, NativeAdmissionRequest, admit_native_realization,
    admit_prepared_native_realization, artifact_lowering_error,
};
pub use providers::provider_admission::{
    AdmittedBoundaryExecution, AdmittedBoundarySettlement, AdmittedNativeCallbackArgument,
    InstalledInterruptNativeEntryProposal, NativeCallbackArgumentProposal, SettledCompilerBuiltin,
};
pub use retained_terminal::{
    AdmittedNativeComponentCall, ComponentExportInput, ComponentNativeInputs,
    PreparedNativeExecution, PreparedNativeWork, PreparedRetainedNativeProviders,
    RetainedComponentNativeInputs, RetainedNativeProviderInputs, RetainedTerminalArtifact,
    SelectedFamilyRealizationSource, TerminalCallbackOccurrenceProposal,
    TerminalCallbackThunkArtifact, TerminalCompilerBuiltinProposal,
    TerminalIeeeFloatComparisonOccurrenceProposal, TerminalIntegerComparisonOccurrenceProposal,
    TerminalNativeBoundaryApplications, TerminalNativeOccurrences, TerminalNativeProposalRequest,
    TerminalNativeProviders, TerminalNativeRealizationProposal, TerminalTargetProviderProposal,
    decode_installation_assumptions, encode_installation_assumptions,
    project_terminal_native_realization_proposal, recover_native_derivation_inputs,
    retain_native_derivation_inputs,
};

pub(crate) use artifact_admission::ArtifactLoweringError;
pub use artifact_admission::{
    ArtifactSections, NativeExecutionSelection, NativeScalarApplications,
    TerminalAccessViewEstablishment, VerifiedNativeArtifactInput, lower_artifact,
    lower_module_with_evidence,
};
pub use lowering::LoweringError;
pub use providers::provider_installation::SelectedProviderAdapter;
pub use providers::provider_installation::{
    AdmittedProviderInstallation, ProviderInstallationError,
};
pub use settlement::boundary_application_settlement::BoundaryApplicationSettlement;
pub use settlement::callback_settlement::settle_native_callback_placements;
pub(crate) use settlement::callback_settlement::{
    CallbackRegistrarOccurrence, settle_callback_registrar_imports,
};
pub use settlement::entry_settlement::{
    NativeProgramEntrySettlement, NativeProgramEntrySettlementError, SelectedProgramEntryCall,
    ValidatedNativeProgramEntrySettlement, validate_native_program_entry_settlement,
};
pub use verified_optimization_input::{
    ObligationEvidence, VerifiedPsiOptimizationContext, VerifiedPsiOptimizationInput,
};

#[cfg(test)]
mod terminal_decode_once {
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::{Path, PathBuf};

    #[test]
    fn provider_installation_uses_retained_artifact_custody() {
        let source =
            include_str!("../../../realization/provider-admission/src/provider_installation.rs");
        assert!(source.contains("TerminalArtifactView<'_>"));
        assert!(!source.contains("admit_provider_installation_from_artifact("));
        assert!(!source.contains("semantic_bytes"));
    }

    const DECODE_MODULE_CALL: &str = concat!("decode_module", "(");
    const ARTIFACT_REPLAY_CALL: &str =
        concat!("CanonicalTerminalArtifact", "::", "from_bytes", "(");
    const FROM_BYTES_CALL: &str = concat!("from_bytes", "(");
    const TO_BYTES_OUTPUT: &str = concat!("to_bytes", "()");

    const PERMITTED_MODULE_DECODES: &[(&str, usize, &str)] = &[(
        "omega-rust/omega/compilation/pipeline/00_terminal-psi-to-abstract-operations/src/artifact_admission.rs",
        1,
        "Omega 00 admission: `lower_artifact` decodes loose canonical sections",
    )];

    const PERMITTED_ARTIFACT_REPLAYS: &[(&str, usize, &str)] = &[
        (
            "omega-rust/omega/application/cli/src/inspection/mod.rs",
            1,
            "inspect path: `inspect_terminal_artifact` admits externally supplied envelope bytes",
        ),
        (
            "omega-rust/omega/compilation/backend/artifacts/component-description/src/component_description.rs",
            2,
            "describe path: `decode_component_description` and `description_subject` admit the description's embedded artifact",
        ),
    ];

    fn collect_rust_sources(root: &Path, sources: &mut Vec<PathBuf>) {
        let entries = fs::read_dir(root)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", root.display()));
        for entry in entries {
            let path = entry
                .unwrap_or_else(|error| panic!("cannot list {}: {error}", root.display()))
                .path();
            if path.is_dir() {
                let name = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or_default();
                if name.starts_with('.') || matches!(name, "tests" | "target" | "examples") {
                    continue;
                }
                collect_rust_sources(&path, sources);
            } else if path.extension().and_then(|extension| extension.to_str()) == Some("rs")
                && !path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name == "tests.rs" || name.ends_with("_tests.rs"))
            {
                sources.push(path);
            }
        }
    }

    fn scanned_tree() -> (PathBuf, PathBuf) {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
        let repository = manifest
            .ancestors()
            .nth(5)
            .unwrap_or_else(|| panic!("{manifest:?} has no repository-root ancestor"))
            .to_path_buf();
        let omega = manifest
            .ancestors()
            .nth(3)
            .unwrap_or_else(|| panic!("{manifest:?} has no omega/ ancestor"))
            .to_path_buf();
        (repository, omega)
    }

    fn code_lines(text: &str) -> Vec<(usize, &str)> {
        let mut lines = text.lines().enumerate().peekable();
        let mut code = Vec::new();
        while let Some((number, line)) = lines.next() {
            if line.trim() == "#[cfg(test)]"
                && let Some((_, item)) = lines.peek()
                && item.trim_start().starts_with("mod ")
                && item.trim_end().ends_with('{')
            {
                let indentation = &item[..item.len() - item.trim_start().len()];
                let closing = format!("{indentation}}}");
                for (_, skipped) in lines.by_ref() {
                    if skipped.trim_end() == closing {
                        break;
                    }
                }
                continue;
            }
            if !line.trim_start().starts_with("//") {
                code.push((number, line));
            }
        }
        code
    }

    fn count_occurrences(
        repository: &Path,
        sources: &[PathBuf],
        needle: &str,
    ) -> BTreeMap<String, usize> {
        let mut found = BTreeMap::new();
        for source in sources {
            let text = fs::read_to_string(source)
                .unwrap_or_else(|error| panic!("cannot read {}: {error}", source.display()));
            let count = code_lines(&text)
                .into_iter()
                .map(|(_, line)| line.matches(needle).count())
                .sum::<usize>();
            if count > 0 {
                let relative = source
                    .strip_prefix(repository)
                    .unwrap_or_else(|_| {
                        panic!("{} escapes {}", source.display(), repository.display())
                    })
                    .to_string_lossy()
                    .replace('\\', "/");
                found.insert(relative, count);
            }
        }
        found
    }

    fn expected(sites: &[(&str, usize, &str)]) -> BTreeMap<String, usize> {
        sites
            .iter()
            .map(|(path, count, _)| ((*path).to_owned(), *count))
            .collect()
    }

    fn describe(sites: &[(&str, usize, &str)]) -> String {
        sites
            .iter()
            .map(|(path, count, role)| format!("  {count}x {path} — {role}"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn realization_decodes_terminal_module_only_at_stage_00_admission() {
        let (repository, omega) = scanned_tree();
        let mut sources = Vec::new();
        collect_rust_sources(&omega, &mut sources);
        let found = count_occurrences(&repository, &sources, DECODE_MODULE_CALL);
        let permitted = expected(PERMITTED_MODULE_DECODES);
        assert_eq!(
            found,
            permitted,
            "canonical module decode moved off Omega 00 admission; every consumer holding a \
         CanonicalTerminalArtifact reads semantic_module() instead of re-decoding, and only the \
         loose-bytes entrances may call decode_module. Permitted sites:\n{}",
            describe(PERMITTED_MODULE_DECODES),
        );
    }

    #[test]
    fn realization_never_rebuilds_the_terminal_artifact() {
        let (repository, omega) = scanned_tree();
        let mut sources = Vec::new();
        collect_rust_sources(&omega, &mut sources);
        let found = count_occurrences(&repository, &sources, ARTIFACT_REPLAY_CALL);
        let permitted = expected(PERMITTED_ARTIFACT_REPLAYS);
        assert_eq!(
            found,
            permitted,
            "a Terminal artifact replay entered Omega non-test source outside the describe path; the \
         realization path holds the artifact and must not rebuild it from bytes. Permitted \
         sites:\n{}",
            describe(PERMITTED_ARTIFACT_REPLAYS),
        );
    }

    #[test]
    fn no_source_replays_encoded_bytes_through_from_bytes() {
        let (repository, omega) = scanned_tree();
        let mut sources = Vec::new();
        collect_rust_sources(&omega, &mut sources);
        let mut violations = Vec::new();
        for source in &sources {
            let text = fs::read_to_string(source)
                .unwrap_or_else(|error| panic!("cannot read {}: {error}", source.display()));
            for (line_number, line) in code_lines(&text) {
                if line.contains(FROM_BYTES_CALL) && line.contains(TO_BYTES_OUTPUT) {
                    let relative = source
                        .strip_prefix(&repository)
                        .unwrap_or_else(|_| {
                            panic!("{} escapes {}", source.display(), repository.display())
                        })
                        .to_string_lossy()
                        .replace('\\', "/");
                    violations.push(format!("{relative}:{}", line_number + 1));
                }
            }
        }
        assert!(
            violations.is_empty(),
            "from_bytes fed by to_bytes() re-admits bytes a value just encoded — the byte round trip \
         this stage removed; clone or share the value instead. Sites: {violations:?}"
        );
    }
}

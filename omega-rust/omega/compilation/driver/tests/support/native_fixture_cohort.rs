//! One reviewed source cohort supplies every requested native application.
//!
//! Source realization and executable publication consume the same admitted
//! products. Optional target observations retain each application's real root
//! against the cohort's unchanged canonical Terminal artifact.
use super::{entry_inputs, fixture_package_inputs, terminal_execution};
use compiler::{CompileRequest, OptimizationRollback, RequestedCompileProduct};
use diagnostics::Diagnostic;
use std::path::Path;

pub(super) struct NativeFixtureCohort {
    pub primary: compilation_products::CompileReport,
    companions: Vec<compilation_products::CompileReport>,
    compiled_roots: bool,
}

impl NativeFixtureCohort {
    pub fn target_roots(
        &self,
    ) -> Result<Vec<&compilation_products::TerminalRuntimeProvisions>, Vec<Diagnostic>> {
        if !self.compiled_roots {
            return Ok(Vec::new());
        }
        let artifact = retained_native_terminal(&self.primary).ok_or_else(|| {
            vec![Diagnostic::error(
                "target cohort lost its canonical Terminal artifact",
            )]
        })?;
        let mut roots = Vec::new();
        for report in std::iter::once(&self.primary).chain(&self.companions) {
            let sibling_artifact = retained_native_terminal(report).ok_or_else(|| {
                vec![Diagnostic::error(
                    "target cohort lost a retained Terminal product",
                )]
            })?;
            if !std::ptr::eq(artifact, sibling_artifact) {
                return Err(vec![Diagnostic::error(
                    "target cohort did not retain one shared canonical Terminal artifact",
                )]);
            }
            roots.push(report.runtime_provisions().ok_or_else(|| {
                vec![Diagnostic::error(
                    "target cohort lost its exact admitted runtime root",
                )]
            })?);
        }
        Ok(roots)
    }
}

pub(super) fn produce(
    discovered: fixture_package_inputs::DiscoveredFixture,
    build_dir: &Path,
    target: &str,
    rollback: Option<&OptimizationRollback>,
    fixture_dir: Option<&Path>,
) -> Result<NativeFixtureCohort, Vec<Diagnostic>> {
    let target_profile = target::TargetProfile::from_omega_target_name(Some(target))
        .map_err(|diagnostic| vec![diagnostic])?;
    let compiled_roots = fixture_dir
        .map(terminal_execution::compiled_target_provisions)
        .transpose()
        .map_err(|message| vec![diagnostics::Diagnostic::error(message)])?
        .unwrap_or(false);
    let mut targets = vec![Some(target_profile)];
    if compiled_roots {
        let fixture_dir = fixture_dir.ok_or_else(|| {
            vec![diagnostics::Diagnostic::error(
                "compiled target cohort lost its fixture",
            )]
        })?;
        for name in terminal_execution::requested_target_names(fixture_dir)
            .map_err(|message| vec![diagnostics::Diagnostic::error(message)])?
        {
            let profile = target::TargetProfile::from_omega_target_name(Some(&name))
                .map_err(|diagnostic| vec![diagnostic])?;
            if !targets.contains(&Some(profile)) {
                targets.push(Some(profile));
            }
        }
    }
    let profiles = targets
        .iter()
        .map(|profile| profile.unwrap_or(target_profile))
        .collect::<Vec<_>>();
    let root_path = discovered.authored_root_path.clone();
    let native = compiler::CompilationProduct::Native(compilation_products::NativeProductRequest {
        private_entry_ram: entry_inputs::load(&root_path)?,
        terminal_authority_policy: terminal_psi_to_abstract_operations::provider_admission::current_terminal_authority_policy(),
        terminal_authority_permission_policy: None,
        optimization_rollback: rollback.cloned().unwrap_or_default(),
    });
    let mut prepared = fixture_package_inputs::review_fixture(discovered, &profiles, native)?;
    let reports = if prepared.package_inputs.is_some() {
        fixture_package_inputs::produce_fixture_products(&mut prepared)?
            .into_iter()
            .map(|(profile, output)| {
                output
                    .into_report()
                    .map(|report| (Some(profile), report))
                    .map_err(compiler::CompilationProductError::into_diagnostics)
            })
            .collect::<Result<Vec<_>, _>>()?
    } else {
        let execution = prepared
            .execution
            .take()
            .expect("standalone fixture inputs");
        let mut request =
            CompileRequest::for_targets(prepared.root_path, build_dir.to_path_buf(), targets)
                .with_requested_product(RequestedCompileProduct::NativeArtifact)
                .with_private_entry_ram(entry_inputs::load(&root_path)?);
        if let Some(rollback) = rollback {
            request = request.with_optimization_rollback(rollback.clone());
        }
        if let Some(snapshot) = execution.snapshot {
            request = request.with_build_snapshot(snapshot);
        }
        let outcomes = omega_build::compile_sources(prepared.session.compiler(), request)?;
        let mut reports = Vec::new();
        let mut failures = Vec::new();
        for outcome in outcomes.into_outcomes().into_vec() {
            let profile = outcome.target_profile();
            match outcome.into_result() {
                Ok(report) => reports.push((profile, report)),
                Err(diagnostics) => failures.extend(diagnostics),
            }
        }
        if !failures.is_empty() {
            return Err(failures);
        }
        reports
    };

    let mut primary = None;
    let mut companions = Vec::new();
    for (profile, report) in reports {
        if profile == Some(target_profile) {
            if primary.replace(report).is_some() {
                return Err(vec![Diagnostic::error(
                    "native cohort duplicated its requested target product",
                )]);
            }
        } else {
            companions.push(report);
        }
    }
    let primary = primary.ok_or_else(|| {
        vec![Diagnostic::error(
            "native cohort lost its requested target product",
        )]
    })?;
    Ok(NativeFixtureCohort {
        primary,
        companions,
        compiled_roots,
    })
}

/// Both native product classes retain the source-free canonical artifact.
pub(super) fn retained_native_terminal(
    report: &compilation_products::CompileReport,
) -> Option<&terminal_codec::CanonicalTerminalArtifact> {
    report
        .retained_native_artifact()
        .map(|native| native.psi_artifact())
        .or_else(|| {
            report
                .retained_dynamic_elf_artifact()
                .map(|native| native.psi_artifact())
        })
}

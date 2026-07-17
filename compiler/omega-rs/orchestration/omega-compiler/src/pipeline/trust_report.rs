//! GR5 (the chapter-10 carrier's report surface): one trust-report row per
//! admitted semantic commitment. Today's rows are the SEALED-DOMAIN
//! INTRODUCTIONS: every domain declared in the compilation unit is
//! own-package and dev-active (grant locality v1, mirroring the
//! MintAuthority consult in omega-validation's recasts), so each carries
//! the standing warning until GR3's root grants land and flip its
//! provenance. Progress profiles, accepted facts, and provider plans join
//! as their consumers wire in (GR6).

use crate::pipeline::compile_options::CompileOptions;
use omega_artifacts::{ArtifactWriter, TrustReport, TrustReportRow};
use omega_core::diagnostics::Diagnostic;
use omega_typed_trees::TypedTrees;

pub(super) fn write_trust_report(
    options: &CompileOptions,
    typed: &TypedTrees,
) -> Result<(), Vec<Diagnostic>> {
    let mut report = TrustReport::default();
    for domain in typed.domain_definitions() {
        if !domain.semantic_id.is_valid() {
            continue;
        }
        report.rows.push(TrustReportRow {
            commitment: format!("domain introduction: {}", domain.name.as_str()),
            provenance: "own-package (dev-active)".to_owned(),
            standing_warning: true,
        });
    }

    let writer =
        ArtifactWriter::new(&options.build_dir()).map_err(|diagnostic| vec![diagnostic])?;
    writer
        .write_trust_report(&report)
        .map_err(|diagnostic| vec![diagnostic])
}

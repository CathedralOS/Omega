use super::output::{Hex, Output};
use super::sources::{package_key, path, resolution};
use super::{PackagePolicyDecisionSubject as Subject, PackagePolicyReviewError};
use crate::review::{
    PackagePolicyChangeKind, PackagePolicyChangeSet, PackagePolicyReplacementSite,
};
use std::fmt::{self, Write};

pub(super) fn template(
    changes: &PackagePolicyChangeSet,
    maximum_bytes: usize,
) -> Result<Output, PackagePolicyReviewError> {
    let mut output = Output::new(maximum_bytes);
    let result = render(&mut output, changes);
    output.finish(result)
}

fn render(output: &mut Output, changes: &PackagePolicyChangeSet) -> fmt::Result {
    writeln!(
        output,
        "omega-package-review 1\ncomparison {}",
        Hex(&changes.fingerprint().digest())
    )?;
    writeln!(
        output,
        "# Edit each pending decision to accept or reject. Findings are generated."
    )?;
    writeln!(
        output,
        "# Decisions record project acceptance, not proof that an audit occurred."
    )?;
    match changes.baseline_source_subject() {
        Some(subject) => writeln!(output, "baseline {}", subject.to_hex())?,
        None => writeln!(output, "baseline none")?,
    }
    writeln!(
        output,
        "candidate {}",
        changes.candidate_source_subject().to_hex()
    )?;
    writeln!(output, "audit-recommended {}", changes.audit_recommended())?;

    if let Some(change) = changes.root_role_change() {
        writeln!(
            output,
            "\nroot-role-change {}",
            change.broken_contract().as_str()
        )?;
        package_key(output, "root", change.root())?;
        writeln!(
            output,
            "- role {}\n+ role {}",
            role(change.baseline_role()),
            role(change.candidate_role())
        )?;
        output.choice(Subject::RootRole)?;
    }
    for replacement in changes.source_replacements() {
        writeln!(output, "\nsource-replacement")?;
        match replacement.site() {
            PackagePolicyReplacementSite::Root => writeln!(output, "binding root")?,
            PackagePolicyReplacementSite::Dependency {
                requester,
                purpose,
                alias,
            } => {
                package_key(output, "requester", requester)?;
                writeln!(output, "purpose {}", purpose.name())?;
                writeln!(output, "binding {:?}", alias.as_str())?;
            }
        }
        package_key(output, "- package", replacement.baseline())?;
        package_key(output, "+ package", replacement.candidate())?;
        output.choice(Subject::SourceReplacement(
            replacement.fingerprint().digest(),
        ))?;
    }
    for package in changes.packages() {
        writeln!(output)?;
        package_key(output, "package", package.key())?;
        resolution(output, "- source", package.baseline_resolution())?;
        resolution(output, "+ source", package.candidate_resolution())?;
        path(output, "- path", package.baseline_path())?;
        path(output, "+ path", package.candidate_path())?;
        purposes(output, "-", package.baseline_occurrence_purposes())?;
        purposes(output, "+", package.candidate_occurrence_purposes())?;
        writeln!(output, "source-changed {}", package.source_changed())?;
        writeln!(
            output,
            "source-association-changed {}",
            package.source_association_changed()
        )?;
        writeln!(output, "audit-recommended {}", package.audit_recommended())?;
        for request in package.restricted_build_requests() {
            build_request(output, request)?;
        }
        for row in package.rows() {
            let change = match row.change() {
                PackagePolicyChangeKind::Added => "added",
                PackagePolicyChangeKind::Removed => "removed",
                PackagePolicyChangeKind::Changed => "changed",
            };
            writeln!(output, "change {} {change}", row.kind().as_str())?;
            writeln!(output, "audit-recommended {}", row.audit_recommended())?;
            policy(output, "-", row.baseline())?;
            policy(output, "+", row.candidate())?;
            if row.requires_decision() {
                output.choice(Subject::Row(row.fingerprint().digest()))?;
            }
        }
        writeln!(output, "end-package")?;
    }
    writeln!(output, "end-review")
}

fn purposes(
    output: &mut Output,
    prefix: &str,
    values: Option<&[crate::declarations::DependencyPurpose]>,
) -> fmt::Result {
    write!(output, "{prefix} purposes")?;
    match values {
        Some(values) => {
            for value in values {
                write!(output, " {}", value.name())?;
            }
        }
        None => write!(output, " none")?,
    }
    writeln!(output)
}

fn build_request(
    output: &mut Output,
    request: &build_evaluation::RestrictedBuildRequest,
) -> fmt::Result {
    use build_evaluation::{
        RestrictedBuildGrant, RestrictedBuildGrantRoot, RestrictedBuildOperation,
    };
    let operation = match request.operation() {
        RestrictedBuildOperation::ScopedFilesystemExecution => "scoped-filesystem-execution",
        RestrictedBuildOperation::UnscopedFilesystemExecution => "unscoped-filesystem-execution",
    };
    writeln!(output, "build-request {operation}")?;
    let grant = |output: &mut Output, access: &str, grant: &RestrictedBuildGrant| {
        write!(output, "{access} ")?;
        match grant.root() {
            RestrictedBuildGrantRoot::SourceInventory => write!(output, "source-inventory")?,
            RestrictedBuildGrantRoot::StagedOutput => write!(output, "staged-output")?,
            RestrictedBuildGrantRoot::Other(identity) => write!(output, "grant-root-{identity}")?,
        }
        if grant.narrowed() {
            write!(output, " narrowed")?;
        }
        if let Some(captured) = grant.captured() {
            write!(
                output,
                " captured entries={} bytes={}",
                captured.entry_count(),
                captured.file_bytes()
            )?;
        }
        writeln!(output)
    };
    for entry in request.read_grants() {
        grant(output, "read", entry)?;
    }
    for entry in request.write_grants() {
        grant(output, "write", entry)?;
    }
    let bounds = request.bounds();
    match bounds.filesystem_sponsor_limits() {
        Some(limits) => writeln!(
            output,
            "bounds filesystem-sponsor entries={} logical-bytes={} extent={}",
            limits.maximum_entries,
            limits.maximum_total_logical_bytes,
            limits.maximum_object_extent
        )?,
        None => writeln!(output, "bounds filesystem-sponsor none")?,
    }
    match bounds.evaluation_sponsor_limits() {
        Some(limits) => writeln!(
            output,
            "bounds evaluation-sponsor fuel={} log-bytes={} filesystem-attempts={} \
             filesystem-handles={} live-cells={} live-text-bytes={} result-cells={} \
             result-text-bytes={}",
            limits.maximum_fuel_units(),
            limits.maximum_build_log_bytes(),
            limits.maximum_filesystem_operation_attempts(),
            limits.maximum_live_filesystem_handles(),
            limits.maximum_live_cells(),
            limits.maximum_live_text_bytes(),
            limits.maximum_result_cells(),
            limits.maximum_result_text_bytes()
        )?,
        None => writeln!(output, "bounds evaluation-sponsor none")?,
    }
    for name in bounds.required_outputs() {
        writeln!(output, "required-output {}", String::from_utf8_lossy(name))?;
    }
    writeln!(output, "artifact-only {}", bounds.artifact_only())?;
    let profile = |profile: Option<target::TargetProfile>| {
        profile
            .map(|profile| profile.target_name())
            .unwrap_or("none")
    };
    writeln!(
        output,
        "build-profile {}",
        profile(request.build_execution_profile())
    )?;
    writeln!(
        output,
        "target {}",
        profile(request.selected_target_profile())
    )?;
    writeln!(output, "end-build-request")
}

fn role(role: crate::declarations::BuildDeclarationKind) -> &'static str {
    use crate::declarations::BuildDeclarationKind;
    match role {
        BuildDeclarationKind::Package => "package",
        BuildDeclarationKind::Application => "application",
        BuildDeclarationKind::Workspace => "workspace",
    }
}

fn policy(
    output: &mut Output,
    prefix: &str,
    row: Option<&crate::lock::PackageAcceptanceRow>,
) -> fmt::Result {
    match row {
        Some(row) => {
            // The evidence codec owns readable policy syntax and escaping.
            // Prefix every line so no policy text becomes a decision directive.
            for line in row.canonical_text().lines() {
                writeln!(output, "{prefix} {line}")?;
            }
            Ok(())
        }
        None => writeln!(output, "{prefix} none"),
    }
}

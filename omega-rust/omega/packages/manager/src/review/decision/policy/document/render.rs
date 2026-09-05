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
            PackagePolicyReplacementSite::Dependency { requester, alias } => {
                package_key(output, "requester", requester)?;
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
        writeln!(output, "source-changed {}", package.source_changed())?;
        writeln!(
            output,
            "source-association-changed {}",
            package.source_association_changed()
        )?;
        writeln!(output, "audit-recommended {}", package.audit_recommended())?;
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
    row: Option<&package_evidence::record::PackagePolicyRow>,
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

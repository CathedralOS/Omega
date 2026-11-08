/*
    We render a restartable, byte-bounded package review document
    (`render_package_policy_review`) with `pending` beside every required policy
    change, and recover user decisions (`recover_package_policy_review`) against
    freshly obtained comparison findings.

    Recovery regenerates the canonical template from `PackagePolicyChangeSet` and
    verifies that only the `pending` decision tokens were edited to `accept` or
    `reject`, preventing accidental resume against edited or stale displayed
    findings. Package-origin strings are always quoted data, and policy rows are
    line-prefixed so no policy text can become a decision directive.
*/

use super::{
    PackagePolicyDecision, PackagePolicyDecisionError, PackagePolicyDecisionSubject,
    PackagePolicyResolution, ReviewOnlyRootPolicyDisposition, resolve_package_policy_decisions,
};
use crate::Hex;
use crate::declarations::PackageKey;
use crate::lock::{PackageAcceptanceRow, PackageCheckedContext};
use crate::review::timings;
use crate::review::{
    PackagePolicyChangeKind, PackagePolicyChangeSet, PackagePolicyDependencyPath,
    PackagePolicyReplacementSite,
};
use package_compilation::build_execution::RestrictedBuildRequest;
use package_source::{GitTransport, ImmutableSourceResolution, SourceLineage};
use std::fmt::{self, Write};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PackagePolicyReviewError {
    ByteLimit,
    AllocationFailed,
    ChangedFindings,
    InvalidDecision,
    UnresolvedDecision(PackagePolicyDecisionSubject),
    Decisions(PackagePolicyDecisionError),
}

impl fmt::Display for PackagePolicyReviewError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ByteLimit => formatter.write_str("package review document exceeds its byte limit"),
            Self::AllocationFailed => formatter.write_str("package review document allocation failed"),
            Self::ChangedFindings => formatter.write_str(
                "package review findings or framing changed; regenerate the document and edit only decisions",
            ),
            Self::InvalidDecision => formatter.write_str("package review decision must be accept or reject"),
            Self::UnresolvedDecision(subject) => write!(formatter, "package review decision remains pending: {subject:?}"),
            Self::Decisions(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for PackagePolicyReviewError {}

pub(crate) fn render_package_policy_review(
    changes: &PackagePolicyChangeSet,
    maximum_bytes: usize,
) -> Result<String, PackagePolicyReviewError> {
    let _stage = timings::stage("review_document_rendering");
    Ok(template(changes, maximum_bytes)?.text)
}

pub(crate) fn recover_package_policy_review(
    changes: &PackagePolicyChangeSet,
    text: &str,
    maximum_bytes: usize,
) -> Result<PackagePolicyResolution, PackagePolicyReviewError> {
    let _stage = timings::stage("review_document_recovery");
    use PackagePolicyReviewError as Error;
    if text.len() > maximum_bytes {
        return Err(Error::ByteLimit);
    }
    if !text.ends_with('\n') || text.contains('\r') {
        return Err(Error::ChangedFindings);
    }
    let template = template(changes, maximum_bytes)?;
    let mut actual = text.lines();
    let mut subjects = template.subjects.iter();
    let mut decisions = Vec::new();
    let mut unresolved = None;
    crate::reserve!(decisions, template.subjects.len(), Error);
    for expected in template.text.lines() {
        let line = actual.next().ok_or(Error::ChangedFindings)?;
        if expected.starts_with("comparison ") && line != expected {
            return Err(Error::Decisions(
                PackagePolicyDecisionError::WrongComparison,
            ));
        }
        if expected.starts_with("decision ") {
            let prefix = expected
                .strip_suffix("pending")
                .expect("generated pending choice");
            let subject = *subjects.next().expect("generated choice subject");
            let disposition = match line.strip_prefix(prefix).ok_or(Error::ChangedFindings)? {
                "accept" => ReviewOnlyRootPolicyDisposition::AcceptCandidateChange,
                "reject" => ReviewOnlyRootPolicyDisposition::RejectCandidateChange,
                "pending" => {
                    unresolved.get_or_insert(subject);
                    continue;
                }
                _ => return Err(Error::InvalidDecision),
            };
            decisions.push(PackagePolicyDecision {
                subject,
                disposition,
            });
        } else if line != expected {
            return Err(Error::ChangedFindings);
        }
    }
    if actual.next().is_some() {
        return Err(Error::ChangedFindings);
    }
    if let Some(subject) = unresolved {
        return Err(Error::UnresolvedDecision(subject));
    }
    resolve_package_policy_decisions(changes, changes.fingerprint().digest(), &decisions)
        .map_err(Error::Decisions)
}

struct Output {
    text: String,
    subjects: Vec<PackagePolicyDecisionSubject>,
    maximum_bytes: usize,
    error: Option<PackagePolicyReviewError>,
}

impl Output {
    fn new(maximum_bytes: usize) -> Self {
        Self {
            text: String::new(),
            subjects: Vec::new(),
            maximum_bytes,
            error: None,
        }
    }

    fn finish(self, result: fmt::Result) -> Result<Self, PackagePolicyReviewError> {
        result.map_err(|_| {
            self.error
                .unwrap_or(PackagePolicyReviewError::AllocationFailed)
        })?;
        Ok(self)
    }

    fn choice(&mut self, subject: PackagePolicyDecisionSubject) -> fmt::Result {
        use PackagePolicyDecisionSubject as Subject;
        match subject {
            Subject::RootRole => self.write_str("decision root-role pending\n")?,
            Subject::SourceReplacement(digest) => {
                writeln!(self, "decision source-replacement {} pending", Hex(&digest))?
            }
            Subject::Row(digest) => writeln!(self, "decision row {} pending", Hex(&digest))?,
        }
        self.subjects.try_reserve(1).map_err(|_| {
            self.error = Some(PackagePolicyReviewError::AllocationFailed);
            fmt::Error
        })?;
        self.subjects.push(subject);
        Ok(())
    }
}

impl Write for Output {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        if value.len() > self.maximum_bytes.saturating_sub(self.text.len()) {
            self.error = Some(PackagePolicyReviewError::ByteLimit);
            return Err(fmt::Error);
        }
        self.text.try_reserve(value.len()).map_err(|_| {
            self.error = Some(PackagePolicyReviewError::AllocationFailed);
            fmt::Error
        })?;
        self.text.push_str(value);
        Ok(())
    }
}

fn template(
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
        output.choice(PackagePolicyDecisionSubject::RootRole)?;
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
        output.choice(PackagePolicyDecisionSubject::SourceReplacement(
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
        purposes(output, "-", package.baseline_occurrence_contexts())?;
        purposes(output, "+", package.candidate_occurrence_contexts())?;
        for context in package.baseline_occurrence_contexts() {
            checked_context(output, "-", Some(*context))?;
        }
        for context in package.candidate_occurrence_contexts() {
            checked_context(output, "+", Some(*context))?;
        }
        writeln!(
            output,
            "occurrence-contexts-changed {}",
            package.occurrence_contexts_changed()
        )?;
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
            checked_context(output, "-", row.baseline_context())?;
            checked_context(output, "+", row.candidate_context())?;
            writeln!(output, "audit-recommended {}", row.audit_recommended())?;
            policy(output, "-", row.baseline())?;
            policy(output, "+", row.candidate())?;
            if row.requires_decision() {
                output.choice(PackagePolicyDecisionSubject::Row(
                    row.fingerprint().digest(),
                ))?;
            }
        }
        writeln!(output, "end-package")?;
    }
    writeln!(output, "end-review")
}

fn checked_context(
    output: &mut Output,
    prefix: &str,
    context: Option<PackageCheckedContext>,
) -> fmt::Result {
    match context {
        Some(context) => writeln!(
            output,
            "{prefix} occurrence {} target {} execution {}",
            context.purpose().name(),
            context.target().target_name(),
            context
                .build_execution_profile()
                .map_or("none", |profile| profile.target_name())
        ),
        None => writeln!(output, "{prefix} occurrence none"),
    }
}

fn purposes(output: &mut Output, prefix: &str, values: &[PackageCheckedContext]) -> fmt::Result {
    write!(output, "{prefix} purposes")?;
    if values.is_empty() {
        write!(output, " none")?;
    } else {
        for value in values {
            write!(output, " {}", value.purpose().name())?;
        }
    }
    writeln!(output)
}

fn build_request(output: &mut Output, request: &RestrictedBuildRequest) -> fmt::Result {
    use package_compilation::build_execution::{
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
            "bounds evaluation-sponsor work={} log-bytes={} filesystem-attempts={} \
             filesystem-handles={} live-cells={} live-text-bytes={} result-cells={} \
             result-text-bytes={}",
            limits.work_budget(),
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

fn policy(output: &mut Output, prefix: &str, row: Option<&PackageAcceptanceRow>) -> fmt::Result {
    match row {
        Some(row) => {
            for line in row.canonical_text().lines() {
                writeln!(output, "{prefix} {line}")?;
            }
            Ok(())
        }
        None => writeln!(output, "{prefix} none"),
    }
}

fn package_key(output: &mut Output, prefix: &str, key: &PackageKey) -> fmt::Result {
    write!(
        output,
        "{prefix} {:?} {} ",
        key.name().as_str(),
        Hex(&key.identity().digest())
    )?;
    match key.source_lineage() {
        SourceLineage::GitHub(source) => writeln!(
            output,
            "github {:?} {:?}",
            source.owner(),
            source.repository()
        ),
        SourceLineage::GitLab(source) => writeln!(output, "gitlab {:?}", source.repository_path()),
        SourceLineage::Git(source) => {
            let transport = match source.transport() {
                GitTransport::Https => "https",
                GitTransport::SshUrl => "ssh",
                GitTransport::ScpLike => "scp",
            };
            writeln!(
                output,
                "git {transport} {:?} {:?} {:?} {:?}",
                source.user(),
                source.host(),
                source.port(),
                source.repository_path()
            )
        }
        SourceLineage::Workspace(source) => {
            writeln!(output, "workspace {:?}", source.member_path().as_str())
        }
        SourceLineage::ExternalLocal(source) => {
            writeln!(output, "local {:?}", source.canonical_absolute_path())
        }
    }
}

fn resolution(
    output: &mut Output,
    prefix: &str,
    resolution: Option<&ImmutableSourceResolution>,
) -> fmt::Result {
    match resolution {
        None => writeln!(output, "{prefix} none"),
        Some(ImmutableSourceResolution::Git {
            commit,
            tree,
            content,
        }) => writeln!(
            output,
            "{prefix} git commit {} tree {} content {}",
            commit.to_hex(),
            tree.to_hex(),
            content.to_hex()
        ),
        Some(ImmutableSourceResolution::Workspace { content }) => {
            writeln!(output, "{prefix} workspace content {}", content.to_hex())
        }
        Some(ImmutableSourceResolution::ExternalLocal { content }) => {
            writeln!(output, "{prefix} local content {}", content.to_hex())
        }
    }
}

fn path(
    output: &mut Output,
    prefix: &str,
    path: Option<&PackagePolicyDependencyPath>,
) -> fmt::Result {
    let Some(path) = path else {
        return writeln!(output, "{prefix} none");
    };
    write!(output, "{prefix} {}", Hex(&path.root().digest()))?;
    for step in path.steps() {
        write!(
            output,
            " -> {:?} {}",
            step.alias(),
            Hex(&step.target().digest())
        )?;
    }
    writeln!(output)
}

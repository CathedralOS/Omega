//! Readable inspection only; accepted policy remains historical project data.

use crate::declarations::PackageKey;
use crate::lock::PackageLockTarget;
use crate::resolution::graph::CanonicalSourceClosureSubject;
use crate::review::{
    CompilerIssuedPackageReviewSet, PackagePolicyChangeSet, PackagePolicyDependencyPath,
};
use package_evidence::record::{PackageReviewNominalIdentity, PackageReviewNominalOwner};
use package_source::{GitTransport, ImmutableSourceResolution, SourceLineage};
use std::fmt::{self, Write};
use target::TargetProfile;

mod policy;
#[cfg(test)]
mod tests;

pub(super) fn render(
    target: TargetProfile,
    accepted: Option<&PackageLockTarget>,
    fresh: Option<(
        &CanonicalSourceClosureSubject,
        &CompilerIssuedPackageReviewSet,
        &PackagePolicyChangeSet,
    )>,
    unavailable: Option<&str>,
    maximum_bytes: usize,
    verbose: bool,
) -> Result<String, String> {
    if accepted.is_some_and(|accepted| accepted.target() != target) {
        return Err("package inspection accepted target mismatch".into());
    }
    if fresh.is_some() && unavailable.is_some() {
        return Err(
            "package inspection cannot have fresh findings and unavailable analysis".into(),
        );
    }
    if let Some((source, reviews, changes)) = fresh
        && (source.target_profile() != target
            || changes.candidate_source_subject() != source.fingerprint()
            || changes.baseline_source_subject()
                != accepted.map(|accepted| accepted.source().fingerprint())
            || reviews.reviews().len() != source.packages().len()
            || source.packages().iter().any(|package| {
                reviews.review(package.key()).is_none_or(|review| {
                    review.resolution() != package.resolution()
                        || review.policy().package() != package.key().identity()
                        || review.policy().target() != target
                })
            }))
    {
        return Err("package inspection fresh analysis context mismatch".into());
    }
    let mut output = Output::new(maximum_bytes);
    let result = contents(&mut output, target, accepted, fresh, unavailable, verbose);
    result.map_err(|_| {
        output
            .error
            .unwrap_or("package inspection rendering failed")
            .to_owned()
    })?;
    Ok(output.text)
}

fn contents(
    output: &mut Output,
    target: TargetProfile,
    accepted: Option<&PackageLockTarget>,
    fresh: Option<(
        &CanonicalSourceClosureSubject,
        &CompilerIssuedPackageReviewSet,
        &PackagePolicyChangeSet,
    )>,
    unavailable: Option<&str>,
    verbose: bool,
) -> fmt::Result {
    writeln!(output, "target {}", target.identity().as_str())?;
    if !verbose {
        writeln!(
            output,
            "policy-summary: typed compiler findings; --details shows complete canonical policy, signatures, contracts, and identity payloads"
        )?;
    }
    match fresh {
        Some((_, _, changes)) => {
            writeln!(output, "fresh-analysis complete")?;
            writeln!(output, "requires-review {}", changes.requires_decision())?;
        }
        None => writeln!(
            output,
            "fresh-analysis unavailable: {:?}",
            unavailable.unwrap_or("fresh analysis was not supplied")
        )?,
    }
    if let Some(accepted) = accepted {
        writeln!(
            output,
            "\naccepted lock: historical project record; not proof or fresh compiler findings"
        )?;
        graph(output, "accepted", accepted.source())?;
        for (package, baseline) in accepted
            .source()
            .packages()
            .iter()
            .zip(accepted.baselines())
        {
            package_key(output, "\naccepted package", package.key())?;
            if let Some((_, _, changes)) = fresh {
                let Some(change) = changes
                    .packages()
                    .iter()
                    .find(|change| change.key() == package.key())
                else {
                    return output
                        .fail("package inspection is missing an accepted dependency path");
                };
                path(output, "accepted dependency-path", change.baseline_path())?;
            }
            if fresh.is_some_and(|(_, reviews, _)| {
                reviews
                    .review(package.key())
                    .is_some_and(|review| review.policy() == baseline)
            }) {
                writeln!(
                    output,
                    "accepted-policy equal-to-fresh (shown with fresh findings)"
                )?;
            } else {
                policy::render(
                    output,
                    "accepted-policy (historical meaning)",
                    baseline,
                    accepted.source(),
                    verbose,
                )?;
            }
        }
    } else {
        writeln!(output, "accepted none\naccepted-policy none")?;
    }

    let Some((source, reviews, changes)) = fresh else {
        return Ok(());
    };
    writeln!(output, "\nfresh compiler findings for current sources")?;
    graph(output, "fresh", source)?;
    for package in source.packages() {
        package_key(output, "\nfresh package", package.key())?;
        let Some(change) = changes
            .packages()
            .iter()
            .find(|change| change.key() == package.key())
        else {
            return output.fail("package inspection is missing a fresh dependency path");
        };
        path(output, "fresh dependency-path", change.candidate_path())?;
        writeln!(output, "source-changed {}", change.source_changed())?;
        writeln!(output, "audit-recommended {}", change.audit_recommended())?;
        let Some(review) = reviews.review(package.key()) else {
            return output.fail("package inspection is missing fresh compiler policy");
        };
        policy::render(output, "fresh-policy", review.policy(), source, verbose)?;
        policy::observations(output, review.projection(), source)?;
    }
    Ok(())
}

fn graph(output: &mut Output, label: &str, source: &CanonicalSourceClosureSubject) -> fmt::Result {
    writeln!(
        output,
        "{label} graph ({} packages)",
        source.packages().len()
    )?;
    package_key(output, "root", source.root().selected().key())?;
    writeln!(output, "root-role {:?}", source.root_role())?;
    for package in source.packages() {
        package_key(output, "package", package.key())?;
        resolution(output, package.resolution())?;
    }
    writeln!(output, "edges {}", source.dependency_requests().len())?;
    for edge in source.dependency_requests() {
        writeln!(
            output,
            "edge {:?} {} -- {:?} [dependency {}] --> {:?} {}",
            edge.requester().name().as_str(),
            Hex(&edge.requester().identity().digest()),
            edge.alias().as_str(),
            edge.dependency_index(),
            edge.selected().key().name().as_str(),
            Hex(&edge.selected().key().identity().digest()),
        )?;
        // Derived Debug quotes every authored string, including locator and revision.
        writeln!(output, "  request {:?}", edge.request())?;
    }
    Ok(())
}

fn package_key(output: &mut Output, prefix: &str, key: &PackageKey) -> fmt::Result {
    write!(
        output,
        "{prefix} {:?} {} ",
        key.name().as_str(),
        Hex(&key.identity().digest())
    )?;
    // The review document's helpers are private to its decision renderer.
    match key.source_lineage() {
        SourceLineage::GitHub(source) => {
            writeln!(
                output,
                "github {:?} {:?}",
                source.owner(),
                source.repository()
            )
        }
        SourceLineage::GitLab(source) => writeln!(output, "gitlab {:?}", source.repository_path()),
        SourceLineage::Git(source) => {
            let transport = match source.transport() {
                GitTransport::Https => "https",
                GitTransport::SshUrl => "ssh",
                GitTransport::ScpLike => "scp",
            };
            writeln!(
                output,
                "git {transport} user {:?} host {:?} port {:?} path {:?}",
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

fn resolution(output: &mut Output, resolution: &ImmutableSourceResolution) -> fmt::Result {
    match resolution {
        ImmutableSourceResolution::Git {
            commit,
            tree,
            content,
        } => writeln!(
            output,
            "  source git commit {} tree {} content {}",
            commit.to_hex(),
            tree.to_hex(),
            content.to_hex()
        ),
        ImmutableSourceResolution::Workspace { content } => {
            writeln!(output, "  source workspace content {}", content.to_hex())
        }
        ImmutableSourceResolution::ExternalLocal { content } => {
            writeln!(output, "  source local content {}", content.to_hex())
        }
    }
}

fn path(
    output: &mut Output,
    prefix: &str,
    path: Option<&PackagePolicyDependencyPath>,
) -> fmt::Result {
    let Some(path) = path else {
        return output
            .fail("package inspection is missing a dependency path for a present package");
    };
    write!(output, "{prefix} {}", Hex(&path.root().digest()))?;
    for step in path.steps() {
        write!(
            output,
            " -> {:?} [dependency {}] {}",
            step.alias(),
            step.dependency_index(),
            Hex(&step.target().digest())
        )?;
    }
    writeln!(output)
}

struct Output {
    text: String,
    maximum_bytes: usize,
    error: Option<&'static str>,
}

impl Output {
    fn new(maximum_bytes: usize) -> Self {
        Self {
            text: String::new(),
            maximum_bytes,
            error: None,
        }
    }

    fn fail(&mut self, message: &'static str) -> fmt::Result {
        self.error.get_or_insert(message);
        Err(fmt::Error)
    }
}

impl Write for Output {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        if self.error.is_some() {
            return Err(fmt::Error);
        }
        if value.len() > self.maximum_bytes.saturating_sub(self.text.len()) {
            return self.fail("package inspection exceeds its byte limit");
        }
        if self.text.try_reserve(value.len()).is_err() {
            return self.fail("package inspection output allocation failed");
        }
        self.text.push_str(value);
        Ok(())
    }
}

struct Hex<'a>(&'a [u8]);

struct Name<'a> {
    source: &'a CanonicalSourceClosureSubject,
    identity: &'a PackageReviewNominalIdentity,
}

impl fmt::Display for Name<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.identity.owner() {
            PackageReviewNominalOwner::Package(owner) => {
                let package = self
                    .source
                    .packages()
                    .iter()
                    .find(|package| package.key().identity() == owner);
                if let Some(package) = package {
                    write!(formatter, "{:?}::", package.key().name().as_str())?;
                    if self
                        .source
                        .packages()
                        .iter()
                        .filter(|other| other.key().name() == package.key().name())
                        .count()
                        > 1
                    {
                        write!(formatter, "[{}]::", Hex(&owner.digest()))?;
                    }
                } else {
                    write!(formatter, "package[{}]::", Hex(&owner.digest()))?;
                }
            }
            PackageReviewNominalOwner::ToolchainSource(owner) => {
                write!(formatter, "toolchain[{}]::", Hex(&owner.digest()))?
            }
            PackageReviewNominalOwner::Unresolved => write!(formatter, "unresolved::")?,
        }
        // Nominal paths are compiler payloads, never decoded or reconstructed here.
        write!(formatter, "{:?}", self.identity.path())
    }
}

impl fmt::Display for Hex<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

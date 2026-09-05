//! Build edit planning and source-qualified update selection.

use super::model::{PackageCommand, PackageCommandError, PackageCommandKind, failure};
use crate::declarations::{
    AliasName, BuildDependencyEditPlan, BuildFileReplacement, DependencySourceRequest, PackageKey,
    PackageSelection, plan_dependency_addition_from_source,
    plan_dependency_replacement_from_source,
};
use crate::lock::PackageLock;
use crate::resolution::graph::{CanonicalDependencySourceRequest, CanonicalSourceClosureSubject};
use package_source::GitSourceRequest;
use std::path::Path;

pub(super) struct Plan {
    pub kind: PackageCommandKind,
    pub replacement: BuildFileReplacement,
    /// None refreshes all selectors; Some(empty) preserves every accepted pin.
    pub updates: Option<Vec<PackageKey>>,
}

pub(super) fn plan(
    command: PackageCommand,
    root: &Path,
    before: &str,
    accepted: Option<&PackageLock>,
) -> Result<Plan, PackageCommandError> {
    let build_path = root.join("build.omg");
    let subject = accepted.map(|lock| lock.targets()[0].source());
    match command {
        PackageCommand::Install {
            source,
            revision,
            alias,
        } => {
            let alias = alias.map(AliasName::parse).transpose().map_err(failure)?;
            let request = source_request(source, revision, alias)?;
            let edit = plan_dependency_addition_from_source(
                build_path.clone(),
                before.to_owned(),
                &request,
            )
            .map_err(failure)?;
            let proposed = proposed(edit, before)?;
            Ok(Plan {
                kind: PackageCommandKind::Install,
                replacement: BuildFileReplacement::from_sources(build_path, before, proposed)
                    .map_err(failure)?,
                updates: Some(Vec::new()),
            })
        }
        PackageCommand::Update { packages, revision } => {
            if revision.is_some() && packages.len() != 1 {
                return Err(failure(
                    "--to requires exactly one package or root dependency alias",
                ));
            }
            let updates = if packages.is_empty() {
                None
            } else {
                let subject = subject.ok_or_else(|| failure("omega.lock is missing; run omega update without package selections to review the complete graph first"))?;
                Some(select_packages(subject, &packages)?)
            };
            let mut proposed_source = before.to_owned();
            if let Some(revision) = revision {
                let subject = subject.expect("selected update has an accepted graph");
                let selected = &updates.as_ref().expect("--to has one selection")[0];
                let mut replaced = false;
                for edge in subject.dependency_requests().iter().filter(|edge| {
                    edge.requester() == subject.root().selected().key()
                        && edge.selected().key().source_lineage() == selected.source_lineage()
                }) {
                    let CanonicalDependencySourceRequest::Git {
                        explicit_alias,
                        repository,
                        revision: before_revision,
                        selection,
                    } = edge.request()
                    else {
                        continue;
                    };
                    let validated =
                        GitSourceRequest::new(repository.clone(), Some(revision.clone()))
                            .map_err(failure)?;
                    let before_request = DependencySourceRequest::Git {
                        explicit_alias: explicit_alias.clone(),
                        repository: repository.clone(),
                        revision: before_revision.clone(),
                        selection: selection.clone(),
                    };
                    let candidate = DependencySourceRequest::Git {
                        explicit_alias: explicit_alias.clone(),
                        repository: repository.clone(),
                        revision: validated.requested_revision().to_owned(),
                        selection: selection.clone(),
                    };
                    let edit = plan_dependency_replacement_from_source(
                        build_path.clone(),
                        proposed_source.clone(),
                        &before_request,
                        &candidate,
                    )
                    .map_err(failure)?;
                    proposed_source = proposed(edit, &proposed_source)?;
                    replaced = true;
                }
                if !replaced {
                    return Err(failure(
                        "--to requires a root-authored Git dependency; transitive requests belong to their declaring package and local paths have no Git revision",
                    ));
                }
            }
            Ok(Plan {
                kind: PackageCommandKind::Update,
                replacement: BuildFileReplacement::from_sources(
                    build_path,
                    before,
                    proposed_source,
                )
                .map_err(failure)?,
                updates,
            })
        }
        PackageCommand::Resume { .. } | PackageCommand::DiscardReview => {
            unreachable!("resume and discard do not create edit plans")
        }
    }
}

fn proposed(edit: BuildDependencyEditPlan, before: &str) -> Result<String, PackageCommandError> {
    match edit {
        BuildDependencyEditPlan::Unchanged => Ok(before.to_owned()),
        BuildDependencyEditPlan::Automatic(replacement) => {
            Ok(replacement.replacement_source().to_owned())
        }
        BuildDependencyEditPlan::Manual(patch) => Err(failure(format!(
            "build.omg needs a manually placed dependency edit: {}\n{}\nAccepted project files are unchanged; edit the declaration and run omega update to review it.",
            patch.reason(),
            patch.proposed_statement(),
        ))),
    }
}

fn source_request(
    source: String,
    revision: Option<String>,
    alias: Option<AliasName>,
) -> Result<DependencySourceRequest, PackageCommandError> {
    let network = source.contains("://") && !source.starts_with("file://")
        || source
            .split_once('@')
            .is_some_and(|(_, tail)| tail.contains(':'));
    if network {
        let request = GitSourceRequest::new(source, revision).map_err(failure)?;
        Ok(DependencySourceRequest::Git {
            explicit_alias: alias,
            repository: request.requested_locator().to_owned(),
            revision: request.requested_revision().to_owned(),
            selection: PackageSelection::Root,
        })
    } else {
        let request = crate::operations::PackageSourceRequest::parse(
            crate::operations::SourceAdapter::Local,
            source,
            revision,
        )
        .map_err(|error| failure(format!("invalid local package source: {error:?}")))?;
        let crate::operations::PackageSourceRequest::LocalPath(path) = request else {
            unreachable!()
        };
        let location = path
            .to_str()
            .ok_or_else(|| failure("local dependency locations must be UTF-8 Omega strings"))?
            .to_owned();
        Ok(DependencySourceRequest::Path {
            explicit_alias: alias,
            location,
        })
    }
}

fn select_packages(
    subject: &CanonicalSourceClosureSubject,
    names: &[String],
) -> Result<Vec<PackageKey>, PackageCommandError> {
    let mut selected = Vec::new();
    for name in names {
        let alias = subject.dependency_requests().iter().find(|edge| {
            edge.requester() == subject.root().selected().key() && edge.alias().as_str() == name
        });
        let package = if let Some(edge) = alias {
            edge.selected().key()
        } else {
            let mut candidates = subject.packages().iter().filter(|source| {
                source.key() != subject.root().selected().key()
                    && source.key().name().as_str() == name
            });
            let candidate = candidates
                .next()
                .ok_or_else(|| failure(format!("no accepted dependency matches {name:?}")))?;
            if candidates.next().is_some() {
                return Err(failure(format!(
                    "package name {name:?} occurs in multiple sources; use a root dependency alias"
                )));
            }
            candidate.key()
        };
        if selected.contains(package) {
            return Err(failure(format!(
                "dependency {name:?} was selected more than once"
            )));
        }
        selected.push(package.clone());
    }
    Ok(selected)
}

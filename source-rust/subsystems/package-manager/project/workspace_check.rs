//! Select local workspace check subjects from the executing catalog's final declaration.
use super::{OpenPurpose, OpenedRoot, ProjectInput, ProjectProducts, open_products_with};
use crate::resolution::source::{AcquiredBuildSource, LocalWorkspaceMembers};
use build_declarations::BuildDeclaration;
use diagnostics::Diagnostic;
use package_compilation::BuildSnapshotRequest;
use package_compilation::build_execution::BuildSnapshotCapture;
use source_acquisition::{ExternalSourceContext, LocalSourceLimits};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use target::TargetProfile;

/// Find the conventional entry without running ancestor programs to guess their role.
pub fn discover_check_root(directory: &Path) -> Result<PathBuf, String> {
    let mut selected: Option<PathBuf> = None;
    for parent in directory.ancestors() {
        let entry = parent.join("build.omg");
        match std::fs::symlink_metadata(&entry) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(format!("cannot inspect {}: {error}", entry.display())),
            Ok(metadata) if !metadata.is_file() => {
                return Err(format!("{} is not a regular Build entry", entry.display()));
            }
            Ok(_) => {}
        }
        if let Some(nearer) = selected {
            return Err(format!(
                "multiple enclosing Build entries: {} and {}; name the workspace catalog to check every member, or name the member to check it alone",
                nearer.display(),
                entry.display()
            ));
        }
        selected = Some(entry);
    }
    selected.ok_or_else(|| format!("no enclosing build.omg for {}", directory.display()))
}

pub struct CheckSubject {
    pub entry: PathBuf,
    pub products: Result<ProjectProducts, Vec<Diagnostic>>,
}

/// Open one Product at a time. Each member has its own subject, immutable source and
/// deployment decisions. The caller can finish checking it before opening the next one.
pub fn open_check_subjects<'a>(
    root: &Path,
    profiles: &'a [TargetProfile],
    offline: bool,
    timings: bool,
    directory: &'a Path,
    snapshot: Option<BuildSnapshotRequest>,
) -> Result<impl Iterator<Item = CheckSubject> + 'a, Vec<Diagnostic>> {
    let selected = snapshot
        .as_ref()
        .and_then(|request| match request.capture() {
            BuildSnapshotCapture::Scoped(selected) => Some(selected.clone()),
            _ => None,
        })
        .unwrap_or_else(source_acquisition::SourceCaptureRequest::entire_tree);
    let opened = open_products_with(
        ProjectInput::File(root),
        profiles,
        offline,
        timings,
        directory,
        snapshot,
        OpenPurpose::Check,
    )?;
    let (single, workspace) = match opened {
        OpenedRoot::Product(products, _) => (
            Some(CheckSubject {
                entry: root.to_owned(),
                products: Ok(products),
            }),
            None,
        ),
        OpenedRoot::Catalog(catalog) => {
            let BuildDeclaration::Workspace(roster) = catalog.declaration() else {
                unreachable!("settlement issued a workspace roster")
            };
            let mut members = BTreeMap::new();
            for member in &roster.members {
                let relative = Path::new(member.as_str());
                let describe = |reason: String| {
                    vec![Diagnostic::error(format!(
                        "workspace member `{}`: {reason}",
                        member.as_str()
                    ))]
                };
                let captured = catalog
                    .source_capture()
                    .workspace_member(relative, &selected)
                    .map_err(|error| describe(error.to_string()))?;
                let acquired = AcquiredBuildSource::from_local_capture(
                    captured,
                    LocalSourceLimits::default(),
                    ExternalSourceContext::derive(crate::dependencies::LOCAL_PROJECT_CONTEXT),
                )
                .map_err(|error| describe(error.to_string()))?;
                acquired
                    .require_compilation_source(Path::new("build.omg"))
                    .map_err(|error| describe(error.to_string()))?;
                if members.insert(relative.to_owned(), acquired).is_some() {
                    return Err(describe("duplicate member path".into()));
                }
            }
            let captured = Arc::new(LocalWorkspaceMembers {
                catalog: catalog.source_capture().clone(),
                members,
            });
            (None, Some((roster.members.clone(), captured)))
        }
    };
    let workspace = workspace.into_iter().flat_map(move |(roster, members)| {
        let mut names = BTreeMap::new();
        roster.into_iter().map(move |member| {
            let acquired = &members.members[Path::new(member.as_str())];
            let entry = acquired.entry_path();
            let products = open_products_with(
                ProjectInput::File(&entry),
                profiles,
                offline,
                timings,
                directory,
                None,
                OpenPurpose::Member(acquired, &members),
            )
            .and_then(|opened| match opened {
                OpenedRoot::Product(product, Some(name)) => {
                    if let Some(previous) = names.insert(name.clone(), member.clone()) {
                        Err(vec![Diagnostic::error(format!(
                            "duplicate declared name `{}` for workspace members `{}` and `{}`",
                            name.as_str(),
                            previous.as_str(),
                            member.as_str()
                        ))])
                    } else {
                        Ok(product)
                    }
                }
                _ => Err(vec![Diagnostic::error(format!(
                    "workspace member `{}` must return a named package or application",
                    member.as_str()
                ))]),
            });
            CheckSubject { entry, products }
        })
    });
    Ok(single.into_iter().chain(workspace))
}

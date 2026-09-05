//! Real repository movement through the candidate graph, with local test transport.

use super::super::cache::{GitAcquisitionCache, SourceCacheLane};
use super::*;
use crate::declarations::{DependencySourceRequest, PackageKey};
use crate::resolution::graph::reconcile::resolve_package_source_closure;
use crate::resolution::graph::{
    CanonicalSourceClosureSubject, CanonicalSourceClosureSubjectLimits, GitDependencyPins,
    ResolvedPackageSourceClosure,
};
use crate::resolution::source::{
    GitPackageSourceRequest, bind_staged_external_local_project_source,
    resolve_external_local_project_source_in_lane,
};
use package_source::git::resolution::GitExactRevisionAcquisition;
use package_source::local::staging::stage_local_source_replacement_in_lane;
use sha2::{Digest, Sha256};

struct Project {
    directory: PathBuf,
    storage: SourceResolverStorage,
    context: ExternalSourceContext,
}

impl Project {
    fn new(name: &str) -> Self {
        let directory = temp_root(name);
        let storage = SourceResolverStorage::for_hardened_base(directory.join("cache")).unwrap();
        Self {
            directory,
            storage,
            context: ExternalSourceContext::derive(b"selective-updates"),
        }
    }

    fn repository(&self, name: &str, dependencies: &str) {
        let root = self.directory.join(name);
        write_package(&root, name, None);
        std::fs::write(root.join("build.omg"), format!(
            "machine build(builder: &mut Build) {{ builder.package(\"{name}\"); {dependencies} }}\n"
        )).unwrap();
        run_test_git(&root, ["init", "--quiet"]);
        run_test_git(&root, ["config", "user.email", "omega@example.invalid"]);
        run_test_git(&root, ["config", "user.name", "Omega Tests"]);
        self.commit(name);
    }

    fn commit(&self, name: &str) {
        let root = self.directory.join(name);
        run_test_git(&root, ["add", "."]);
        run_test_git(&root, ["commit", "--quiet", "-m", "fixture source"]);
    }

    fn change(&self, name: &str) {
        std::fs::write(
            self.directory.join(name).join("main.omg"),
            "machine updated() {}\n",
        )
        .unwrap();
        self.commit(name);
    }

    fn build(dependencies: &str) -> String {
        format!(
            "machine build(builder: &mut Build) {{ builder.application(\"application\"); {dependencies} }}\n"
        )
    }

    fn root(&self, dependencies: &str) {
        let root = self.directory.join("root");
        write_application(&root, "application", None);
        std::fs::write(root.join("build.omg"), Self::build(dependencies)).unwrap();
    }

    fn resolve(
        &self,
        proposed: Option<&str>,
        pins: Option<GitDependencyPins<'_>>,
    ) -> ResolvedPackageSourceClosure {
        let root_path = self.directory.join("root");
        let root = if let Some(dependencies) = proposed {
            let original = std::fs::read(root_path.join("build.omg")).unwrap();
            let stage = stage_local_source_replacement_in_lane(
                &root_path,
                &SourceRelativePath::parse("build.omg").unwrap(),
                &Sha256::digest(&original).into(),
                Self::build(dependencies).as_bytes(),
                self.storage.external_local_sources(),
                LocalSourceLimits::default(),
            )
            .unwrap();
            bind_staged_external_local_project_source(
                &stage,
                LocalSourceLimits::default(),
                self.context.clone(),
            )
            .unwrap()
            .into_custody()
        } else {
            resolve_external_local_project_source_in_lane(
                &root_path,
                self.storage.external_local_sources(),
                LocalSourceLimits::default(),
                self.context.clone(),
            )
            .unwrap()
            .into_custody()
        };
        let mut acquisitions = pins
            .map(GitAcquisitionCache::preserving)
            .unwrap_or_default();
        resolve_package_source_closure(
            PackageRootSourceRequest::ExternalLocal {
                requested_root: root_path,
                source_context: self.context.clone(),
            },
            root,
            |_, dependency| {
                let DependencySourceRequest::Git {
                    repository,
                    revision,
                    selection,
                    ..
                } = dependency
                else {
                    panic!("fixture uses Git edges only");
                };
                let name = repository
                    .rsplit('/')
                    .next()
                    .unwrap()
                    .strip_suffix(".git")
                    .unwrap();
                // Only the transport is redirected; declarations, source acquisition,
                // pin selection, graph traversal and reconciliation are production code.
                let request = GitPackageSourceRequest::new(
                    GitSourceRequest::for_local_test_repository_with_lineage(
                        &self.directory.join(name),
                        Some(revision.clone()),
                        repository,
                    )
                    .unwrap(),
                    selection.clone(),
                );
                acquisitions
                    .resolve_selected(
                        &request,
                        SourceCacheLane::Retained(self.storage.git_sources()),
                        SourceCacheLane::Retained(self.storage.workspace_members()),
                        LocalSourceLimits::default(),
                    )
                    .map(|source| source.into_custody())
            },
        )
        .unwrap()
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        // Snapshots use read-only directories. Restore fixture-only directory
        // traversal for teardown without following any source symlinks.
        let mut pending = vec![self.directory.clone()];
        while let Some(path) = pending.pop() {
            let Ok(metadata) = std::fs::symlink_metadata(&path) else {
                continue;
            };
            if metadata.is_dir() {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700));
                }
                if let Ok(entries) = std::fs::read_dir(path) {
                    pending.extend(entries.flatten().map(|entry| entry.path()));
                }
            }
        }
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

fn dependency(name: &str) -> String {
    format!(
        "builder.depend(Source::Git {{ repository: \"https://github.com/CathedralOS/{name}.git\", revision: \"HEAD\" }});"
    )
}

fn subject(closure: &ResolvedPackageSourceClosure) -> CanonicalSourceClosureSubject {
    CanonicalSourceClosureSubject::from_resolved(
        &closure.for_exact_target(target::TargetProfile::WindowsX64),
        CanonicalSourceClosureSubjectLimits::default(),
    )
    .unwrap()
}

fn key(closure: &ResolvedPackageSourceClosure, name: &str) -> PackageKey {
    closure
        .custodies()
        .iter()
        .find(|source| source.key().name().as_str() == name)
        .unwrap()
        .key()
        .clone()
}

#[test]
fn selected_update_keeps_transitive_and_direct_unaffected_repositories_pinned() {
    let project = Project::new("selective-update-transitive");
    project.repository("leaf", "");
    project.repository("middle", &dependency("leaf"));
    project.repository("unrelated", "");
    let original = format!("{} {}", dependency("middle"), dependency("unrelated"));
    project.root(&original);
    let before = project.resolve(None, None);
    let baseline = subject(&before);
    let selected = [key(&before, "middle")];
    for name in ["leaf", "middle", "unrelated"] {
        project.change(name);
    }
    let pins =
        GitDependencyPins::new(&baseline, &selected, GitExactRevisionAcquisition::Offline).unwrap();
    let after = project.resolve(Some(&original), Some(pins));
    assert_eq!(after.custodies().len(), 4);
    for name in ["middle", "leaf", "unrelated"] {
        let package = key(&before, name);
        let old = before.custody(&package).unwrap().resolution();
        let new = after.custody(&package).unwrap().resolution();
        assert_eq!(
            old == new,
            name != "middle",
            "unexpected pin selection for {name}"
        );
    }
    crate::resolution::package_compilation_inputs(&after).unwrap();
    assert_eq!(
        std::fs::read_to_string(project.directory.join("root/build.omg")).unwrap(),
        Project::build(&original)
    );
}

#[test]
fn install_preserves_old_git_pins_while_resolving_new_dependencies() {
    let project = Project::new("selective-update-install");
    project.repository("existing", "");
    project.repository("added", "");
    let original = dependency("existing");
    project.root(&original);
    let before = project.resolve(None, None);
    let baseline = subject(&before);
    project.change("existing");
    project.change("added");
    let pins =
        GitDependencyPins::new(&baseline, &[], GitExactRevisionAcquisition::Offline).unwrap();
    // Reverse placement and change the alias: neither edits the source request.
    let renamed = original.replace("builder.depend(", "builder.depend_as(\"renamed\", ");
    let after = project.resolve(
        Some(&format!("{} {renamed}", dependency("added"))),
        Some(pins),
    );
    let existing = key(&before, "existing");
    assert_eq!(
        before.custody(&existing).unwrap().resolution(),
        after.custody(&existing).unwrap().resolution()
    );
    let added = after.custody(&key(&after, "added")).unwrap();
    assert!(added.snapshot_root().join("main.omg").is_file());
    assert_eq!(
        std::fs::read_to_string(added.snapshot_root().join("main.omg")).unwrap(),
        "machine updated() {}\n"
    );
    assert_eq!(after.custodies().len(), 3);
    assert_eq!(
        std::fs::read_to_string(project.directory.join("root/build.omg")).unwrap(),
        Project::build(&original)
    );
}

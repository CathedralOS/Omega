use super::super::super::Path;
use super::{
    ExternalSourceContext, LOCAL_PROJECT_CONTEXT, LocalSourceLimits, PackageRootSourceRequest,
    Project, TargetProfile, fs, preparation_target, prepare_local_project_in_storage,
};
use crate::declarations::dependencies::read::DependencySourceRequest;
use crate::operations::LocalProjectPreparationOptions;
use crate::resolution::graph::reconcile::resolve_package_source_closure;
use crate::resolution::source::{
    ResolvePackageSourceError, resolve_external_local_project_source, resolve_git_package_source,
};
use package_source::GitSourceRequest;

fn git(repository: &Path, arguments: &[&str]) {
    let output = std::process::Command::new("git")
        .current_dir(repository)
        .args(arguments)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}


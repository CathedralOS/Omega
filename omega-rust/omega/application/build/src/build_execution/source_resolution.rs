//! Execute acquired Build sources, reconcile their actual requests, retain work.
//!
//! Package management owns acquisition and graph validity. This application
//! owner runs the requested code and keeps each returned invocation through the
//! final source join. That work remains available to Product admission/review
//! rather than requiring Build replay. No source IR crosses this boundary.

use crate::build_execution::{ReturnedPackageBuild, execute_returned_build};
use compiler::{CheckedCompileRequest, CompilationSession, CompileFailure, PackageBuild};
use package_compilation::{CapturedLocalSource, PackageCompilationInputs};
use package_manager::operations::{
    LocalProjectPreparation, PrepareLocalProjectError, PreparedLocalProject,
};
use package_manager::resolution::source::{BuildSourceAcquisition, ExecutedSourceResolution};
use std::path::Path;
use target::TargetProfile;

#[derive(Debug)]
pub enum ProjectSourceResolutionError {
    Preparation(PrepareLocalProjectError),
    Compilation(CompileFailure),
    Source(String),
}

/// Source acquisition and returned Build work share one lifetime. The roster
/// is discovery-ordered, not dependency completion order; consumers schedule
/// its exact package identities using the resolved inputs, never source paths.
pub struct ResolvedProjectBuilds {
    pub prepared: PreparedLocalProject,
    pub builds: Vec<ResolvedPackageBuild>,
}

/// A Build whose complete request ledger has joined its acquired source graph.
/// Accepted grants and generated sources remain explicit later inputs. This is
/// not a portable compiled product and cannot be cloned or replayed.
pub struct ResolvedPackageBuild {
    build: ReturnedPackageBuild,
    inputs: PackageCompilationInputs,
}

impl ResolvedPackageBuild {
    pub fn package_inputs(&self) -> &PackageCompilationInputs {
        &self.inputs
    }

    /// Complete the original invocation after package admission has joined
    /// generated inputs and any accepted policy. Compiler completion compares
    /// the graph with its executed request receipt before Product typing.
    pub fn complete(
        mut self,
        compiler: &mut CompilationSession,
        inputs: PackageCompilationInputs,
    ) -> Result<PackageBuild, CompileFailure> {
        self.build.invocation.join_package_inputs(inputs)?;
        self.build.complete(compiler)
    }
}

/// Resolve actual local dependency requests without static Build projection.
/// The shared execution owner is also used by ordinary compilation and review.
/// This migration entry retains normal returns for subsequent admission; it
/// does not authorize effects, refresh locks, or realize native dependencies.
/// Git/workspace requests retain the package resolver's explicit refusal until
/// their pinned acquisition adapters are connected.
pub fn resolve_project_sources(
    compiler: &mut CompilationSession,
    preparation: LocalProjectPreparation,
    profile: TargetProfile,
) -> Result<ResolvedProjectBuilds, ProjectSourceResolutionError> {
    use ProjectSourceResolutionError::{Compilation, Preparation, Source};

    let acquired = preparation.acquire_root().map_err(Preparation)?;
    let root = execute_returned_build(
        compiler,
        source_request(
            acquired.source_capture(),
            &preparation.entry_path(),
            profile,
        ),
    )
    .map_err(Compilation)?;
    let declared = acquired
        .declare_build(normal_return(&root)?)
        .map_err(Source)?;
    let root_identity = declared.identity();
    let mut pending = vec![(root_identity, root)];
    let mut resolution = declared.resolve().map_err(Source)?;
    let resolved = loop {
        match resolution {
            ExecutedSourceResolution::Complete(resolved) => break resolved,
            ExecutedSourceResolution::Request(request) => {
                let acquired = match request.acquire_local_source().map_err(Source)? {
                    BuildSourceAcquisition::Execute(acquired) => acquired,
                    BuildSourceAcquisition::Reused(next) => {
                        resolution = next;
                        continue;
                    }
                };
                let capture = acquired.source_capture();
                let build = execute_returned_build(
                    compiler,
                    source_request(capture, &acquired.entry_path(), profile),
                )
                .map_err(Compilation)?;
                let (identity, next) = acquired.complete(normal_return(&build)?).map_err(Source)?;
                pending.push((identity, build));
                resolution = next;
            }
        }
    };

    let mut builds = Vec::with_capacity(pending.len());
    for (identity, mut build) in pending {
        let returned = build
            .returned
            .as_mut()
            .ok_or_else(|| Source("acquired source has no executed Build declaration".into()))?;
        let inputs = resolved.join_build(identity, returned).map_err(Source)?;
        builds.push(ResolvedPackageBuild { build, inputs });
    }
    let prepared = preparation
        .complete_source_resolution(resolved)
        .map_err(Preparation)?;
    Ok(ResolvedProjectBuilds { prepared, builds })
}

fn source_request(
    capture: &CapturedLocalSource,
    entry: &Path,
    profile: TargetProfile,
) -> CheckedCompileRequest {
    CheckedCompileRequest {
        source_capture: Some(capture.clone()),
        ..CheckedCompileRequest::new(entry, Some(profile.target_name()))
    }
}

fn normal_return(
    build: &ReturnedPackageBuild,
) -> Result<&build_evaluation::ReturnedBuild, ProjectSourceResolutionError> {
    build.returned.as_ref().ok_or_else(|| {
        ProjectSourceResolutionError::Source(
            "acquired source has no executed Build declaration".into(),
        )
    })
}

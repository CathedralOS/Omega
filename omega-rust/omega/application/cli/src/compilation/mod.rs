//! Prepare a project, compile it, and publish the requested product.

pub mod publication;

mod build_directory;

use crate::temporary_directory::TemporaryDirectory;
use artifacts::compile_timings::{CompileTimings, StageMeta, TimingCategory};
use compiler::{
    CompileOptions, CompileReport, CompileRequest, OptimizationRollback, RequestedCompileProduct,
    compile,
};
use diagnostics::Diagnostic;
use package_manager::operations as packages;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectProduct {
    Check,
    NativeArtifact,
}

/// Project policy and product selection, independent of argument spelling.
#[derive(Clone)]
pub struct CompileProjectRequest {
    pub options: CompileOptions,
    pub product: ProjectProduct,

    pub timings: bool,
    pub offline: bool,
    /// `--check --verify`: recheck the root's sources under the retained pins
    /// without the accepted lock's restricted-request grants.
    pub verify: bool,
    /// Reject standalone source after package preparation and transaction recovery.
    pub require_package_project: bool,
    pub optimization_rollback: OptimizationRollback,
    /// Caller-selected root inputs and required outputs, relative to the source root.
    pub build_snapshot: Option<compiler::BuildSnapshotRequest>,
}

impl CompileProjectRequest {
    pub fn new(options: CompileOptions) -> Self {
        Self {
            options,
            product: ProjectProduct::NativeArtifact,

            timings: false,
            offline: false,
            verify: false,
            require_package_project: false,
            optimization_rollback: OptimizationRollback::default(),
            build_snapshot: None,
        }
    }
}

pub struct CompileProjectOutcome {
    pub report: CompileReport,
    pub timings: CompileTimings,
    /// Present only after native publication succeeds.
    pub executable_path: Option<PathBuf>,
}

#[derive(Debug)]
pub enum CompileProjectError {
    Diagnostics(Vec<Diagnostic>),
    Preparation(packages::PrepareLocalProjectError),
    PackageCheck(packages::CheckPreparedLocalProjectError),
    PackageNative(packages::CompilePreparedLocalProjectNativeError),
    Publication(String),
    /// The private workspace a check stages into could not be created.
    CheckWorkspace(std::io::Error),
    /// The output directory records a different owner or a live occupant.
    BuildDirectory(String),
}

impl std::fmt::Display for CompileProjectError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Diagnostics(diagnostics) => {
                diagnostics::write_diagnostics(formatter, diagnostics)
            }
            Self::Preparation(error) => write!(formatter, "{error}"),
            Self::PackageCheck(error) => write!(formatter, "{error}"),
            Self::PackageNative(error) => write!(formatter, "{error}"),
            Self::Publication(error) => write!(formatter, "{error}"),
            Self::CheckWorkspace(error) => {
                write!(formatter, "cannot create the check workspace: {error}")
            }
            Self::BuildDirectory(conflict) => write!(formatter, "{conflict}"),
        }
    }
}

impl std::error::Error for CompileProjectError {}

impl CompileProjectError {
    /// The compiler-request outcome this refusal publishes. Compilation
    /// diagnostics — the standalone route's and accepted package realization's
    /// — are `Incomplete` when every error names an exceeded private provision;
    /// every other refusal (preparation, review, publication,
    /// workspace) is a `Reject`.
    pub fn outcome(&self) -> compiler::CompilerRequestOutcome {
        match self {
            Self::Diagnostics(diagnostics)
            | Self::PackageNative(packages::CompilePreparedLocalProjectNativeError::Native(
                diagnostics,
            )) => compiler::CompilerRequestOutcome::of_refusal(diagnostics),
            _ => compiler::CompilerRequestOutcome::Reject,
        }
    }
}

/// One target's result from [`compile_project_for_targets`].
pub struct TargetProjectOutcome {
    pub target: target::TargetProfile,
    pub result: Result<CompileProjectOutcome, CompileProjectError>,
}

/// Compile the project once for every selected target through one shared
/// preparation, in canonical profile order. The package closure resolves
/// once and every target's candidate pipeline shares one binding-independent
/// source preparation, so identical source and typed-frontier inputs assemble
/// once for the union; each target still keeps its own staging directory,
/// accepted consent, trust settlement, and product realization. One failing
/// target never stops or hides a sibling, and every target reports its own
/// outcome (wiki/spec/build/configuration.md#multi-target-compilation). Each
/// target publishes into `<build dir>/<target>/`, since several targets can
/// share an output file name. Invalid target names reject before any compile
/// runs.
pub fn compile_project_for_targets(
    request: CompileProjectRequest,
    target_names: &[String],
) -> Result<Vec<TargetProjectOutcome>, CompileProjectError> {
    let targets = compiler::ExplicitTargetSet::from_caller_names(target_names)
        .map_err(CompileProjectError::Diagnostics)?;
    compile_project_targets(request, targets.profiles(), true)
}

pub fn compile_project(
    request: CompileProjectRequest,
) -> Result<CompileProjectOutcome, CompileProjectError> {
    let target = crate::invocation_target_profile(request.options.target_name.as_deref())
        .map_err(|diagnostic| CompileProjectError::Diagnostics(vec![diagnostic]))?;
    compile_project_targets(request, &[target], false)?
        .pop()
        .expect("a one-target request produces one outcome")
        .result
}

/// The shared compilation body behind [`compile_project`] and
/// [`compile_project_for_targets`]. `per_target_directories` is the
/// multi-target shape: staging and publication live under `<base>/<target>`;
/// a single target keeps the caller's spelled directory verbatim.
fn compile_project_targets(
    request: CompileProjectRequest,
    profiles: &[target::TargetProfile],
    per_target_directories: bool,
) -> Result<Vec<TargetProjectOutcome>, CompileProjectError> {
    let CompileProjectRequest {
        mut options,
        product,
        timings: collect_timings,
        offline,
        verify,
        require_package_project,
        optimization_rollback,
        build_snapshot,
    } = request;
    let mut timings = if collect_timings {
        CompileTimings::enabled()
    } else {
        CompileTimings::default()
    };
    // Placement and policy belong to the authored project, not its resolver
    // snapshot. A check produces no product, so it owns nothing beside the
    // root: the build evaluation and package review it performs stage into a
    // private workspace that is removed on return, while a product compile
    // keeps the authored build directory for its artifacts.
    let check_workspace = match product {
        ProjectProduct::Check => Some(
            TemporaryDirectory::create("check", false)
                .map_err(CompileProjectError::CheckWorkspace)?,
        ),
        ProjectProduct::NativeArtifact => None,
    };
    let base_build_dir = match &check_workspace {
        Some(workspace) => workspace.path().to_path_buf(),
        None => options.retain_build_dir(),
    };
    let mut target_directories = profiles
        .iter()
        .map(|profile| {
            if per_target_directories {
                base_build_dir.join(profile.target_name())
            } else {
                base_build_dir.clone()
            }
        })
        .collect::<Vec<_>>();
    // A retained product directory must belong to this source root and to one
    // live compile; a check owns a private workspace and needs no record. The
    // claim is per target, so a refused sibling is refused without blocking
    // the rest. Artifact writes bind the admitted canonical directory rather
    // than the spelled name, so an alias swapped in during admission cannot
    // redirect the publish or retained product paths.
    let mut refused = profiles
        .iter()
        .map(|_| None)
        .collect::<Vec<Option<CompileProjectError>>>();
    let _occupancies = if check_workspace.is_none() {
        let mut occupancies = Vec::with_capacity(profiles.len());
        for index in 0..target_directories.len() {
            match build_directory::acquire(&target_directories[index], &options.root_path) {
                Ok(occupancy) => {
                    target_directories[index] = occupancy.directory().to_path_buf();
                    occupancies.push(occupancy);
                }
                Err(conflict) => {
                    refused[index] =
                        Some(CompileProjectError::BuildDirectory(conflict.to_string()));
                }
            }
        }
        occupancies
    } else {
        Vec::new()
    };
    let prepared = timings
        .record_result(
            StageMeta::new(
                "prepare",
                "project",
                "prepared sources",
                TimingCategory::Pipeline,
            ),
            || packages::prepare_local_project_for_targets(&options.root_path, profiles, offline),
        )
        .map_err(CompileProjectError::Preparation)?;
    // Preparation owns pending-transaction recovery and project discovery.
    // Batch callers may require a package, but must not bypass that recovery
    // with an earlier filesystem check for the declaration.
    if require_package_project && prepared.is_none() {
        return Err(CompileProjectError::Diagnostics(vec![Diagnostic::error(
            "compilation requires a package project with a sibling build.omg".to_owned(),
        )]));
    }
    // Targets whose output directory was refused take no part in the shared
    // compile; their row is the refusal itself.
    let active_profiles = profiles
        .iter()
        .copied()
        .enumerate()
        .filter(|(index, _)| refused[*index].is_none())
        .map(|(_, profile)| profile)
        .collect::<Vec<_>>();
    let reports = if active_profiles.is_empty() {
        Vec::new()
    } else {
        timings.record_result(
            StageMeta::new(
                "compile",
                "sources",
                "requested product",
                TimingCategory::Pipeline,
            ),
            || -> Result<
                Vec<(
                    target::TargetProfile,
                    Result<CompileReport, CompileProjectError>,
                )>,
                CompileProjectError,
            > {
                match (prepared, product) {
                    (Some(prepared), ProjectProduct::NativeArtifact) => {
                        let request = packages::PreparedLocalProjectNativeRequest::for_targets(
                            prepared,
                            &base_build_dir,
                            active_profiles.clone(),
                        )
                        .with_optimization_rollback(optimization_rollback.clone())
                        .with_timings(collect_timings);
                        let request = match &build_snapshot {
                            Some(snapshot) => request.with_build_snapshot(snapshot.clone()),
                            None => request,
                        };
                        Ok(
                            packages::compile_prepared_local_project_for_native_targets(
                                request,
                            )
                            .into_iter()
                            .map(|outcome| {
                                (
                                    outcome.target,
                                    outcome
                                        .result
                                        .map_err(CompileProjectError::PackageNative),
                                )
                            })
                            .collect(),
                        )
                    }
                    (Some(prepared), ProjectProduct::Check) => {
                        // A project check carries no overlay into package
                        // compilation, so the requested stop's own phase
                        // predicate refuses one here, before project
                        // preparation, rather than a hand-derived copy of it.
                        RequestedCompileProduct::Check
                            .validate_optimization_rollback(&optimization_rollback)
                            .map_err(|error| CompileProjectError::Diagnostics(vec![error]))?;
                        let request = packages::PreparedLocalProjectCheckRequest::for_targets(
                            prepared,
                            &base_build_dir,
                            active_profiles.clone(),
                        )
                        .with_verify(verify)
                        .with_timings(collect_timings);
                        let request = match &build_snapshot {
                            Some(snapshot) => request.with_build_snapshot(snapshot.clone()),
                            None => request,
                        };
                        Ok(packages::check_prepared_local_project_for_targets(request)
                            .into_iter()
                            .map(|outcome| {
                                (
                                    outcome.target,
                                    outcome.result.map_err(CompileProjectError::PackageCheck),
                                )
                            })
                            .collect())
                    }
                    (None, product) => {
                        let product = match product {
                            ProjectProduct::Check => RequestedCompileProduct::Check,
                            ProjectProduct::NativeArtifact => {
                                RequestedCompileProduct::NativeArtifact
                            }
                        };
                        // Build owns one staging directory. Product publication
                        // below places each realized artifact separately.
                        let targets = profiles
                            .iter()
                            .zip(&refused)
                            .filter(|(_, refusal)| refusal.is_none())
                            .map(|(profile, _)| {
                                (per_target_directories || options.target_name.is_some()).then_some(*profile)
                            })
                            .collect();
                        let mut request = CompileRequest::for_targets(
                            options.root_path.clone(),
                            base_build_dir.clone(),
                            targets,
                        )
                        .with_requested_product(product)
                        .with_optimization_rollback(optimization_rollback.clone())
                        .with_timings(collect_timings);
                        if let Some(snapshot) = &build_snapshot {
                            request = request.with_build_snapshot(snapshot.clone());
                        }
                        compile(request)
                            .map(|outcomes| {
                                outcomes
                                    .into_outcomes()
                                    .into_vec()
                                    .into_iter()
                                    .enumerate()
                                    .map(|(index, outcome)| {
                                        (
                                            active_profiles[index],
                                            outcome
                                                .into_result()
                                                .map_err(CompileProjectError::Diagnostics),
                                        )
                                    })
                                    .collect()
                            })
                            .map_err(CompileProjectError::Diagnostics)
                    }
                }
            },
        )?
    };
    // Settlement and publication stay per target after the shared compile.
    // The ladder measures shared work once; every target's report copies it.
    let mut reports = reports;
    let mut outcomes = Vec::with_capacity(profiles.len());
    for (index, profile) in profiles.iter().enumerate() {
        if let Some(error) = refused[index].take() {
            outcomes.push(TargetProjectOutcome {
                target: *profile,
                result: Err(error),
            });
            continue;
        }
        let position = reports
            .iter()
            .position(|(reported, _)| reported == profile)
            .expect("every active target produces a report row");
        let (_, result) = reports.remove(position);
        let result = result.and_then(|report| match product {
            ProjectProduct::Check => Ok(CompileProjectOutcome {
                report,
                timings: timings.clone(),
                executable_path: None,
            }),
            ProjectProduct::NativeArtifact => {
                let (report, path) = timings
                    .record_result(
                        StageMeta::new(
                            "publish",
                            "retained product",
                            "published product",
                            TimingCategory::Pipeline,
                        ),
                        || publication::publish_compilation(report, &target_directories[index]),
                    )
                    .map_err(CompileProjectError::Publication)?;
                Ok(CompileProjectOutcome {
                    report,
                    timings: timings.clone(),
                    executable_path: path,
                })
            }
        });
        outcomes.push(TargetProjectOutcome {
            target: *profile,
            result,
        });
    }
    Ok(outcomes)
}

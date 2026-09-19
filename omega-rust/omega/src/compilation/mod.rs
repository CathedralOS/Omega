//! Prepare a project, compile it, settle admission, and publish the requested product.

pub mod publication;

use crate::temporary_directory::TemporaryDirectory;
use artifacts::compile_timings::{CompileTimings, StageMeta, TimingCategory};
use compiler::{
    CompileOptions, CompileReport, CompileRequest, OptimizationRollback, RequestedCompileProduct,
    TrustAdmissionSettlement, compile,
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
pub struct CompileProjectRequest {
    pub options: CompileOptions,
    pub product: ProjectProduct,

    pub timings: bool,
    pub offline: bool,
    pub accept_admissions: bool,
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
            accept_admissions: false,
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
    UnsettledAdmissions(TrustAdmissionSettlement),
    Publication(String),
    /// The private workspace a check stages into could not be created.
    CheckWorkspace(std::io::Error),
}

impl std::fmt::Display for CompileProjectError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Diagnostics(diagnostics) => {
                for (index, diagnostic) in diagnostics.iter().enumerate() {
                    if index != 0 {
                        writeln!(formatter)?;
                    }
                    write!(formatter, "{diagnostic}")?;
                }
                Ok(())
            }
            Self::Preparation(error) => write!(formatter, "{error}"),
            Self::PackageCheck(error) => write!(formatter, "{error}"),
            Self::PackageNative(error) => write!(formatter, "{error}"),
            Self::UnsettledAdmissions(_) => {
                write!(formatter, "project trust admissions are not settled")
            }
            Self::Publication(error) => write!(formatter, "{error}"),
            Self::CheckWorkspace(error) => {
                write!(formatter, "cannot create the check workspace: {error}")
            }
        }
    }
}

impl std::error::Error for CompileProjectError {}

pub fn compile_project(
    request: CompileProjectRequest,
) -> Result<CompileProjectOutcome, CompileProjectError> {
    let CompileProjectRequest {
        mut options,
        product,
        timings: collect_timings,
        offline,
        accept_admissions,
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
    let build_dir = match &check_workspace {
        Some(workspace) => {
            options.build_dir = Some(workspace.path().to_path_buf());
            workspace.path().to_path_buf()
        }
        None => options.retain_build_dir(),
    };
    let policy_root_path = options.root_path.clone();
    let target = crate::invocation_target_profile(options.target_name.as_deref())
        .map_err(|diagnostic| CompileProjectError::Diagnostics(vec![diagnostic]))?;
    let prepared = timings
        .record_result(
            StageMeta::new(
                "prepare",
                "project",
                "prepared sources",
                TimingCategory::Pipeline,
            ),
            || {
                packages::prepare_local_project(
                    &options.root_path,
                    packages::LocalProjectPreparationOptions { target, offline },
                )
            },
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
    let admissions = trust_ledger::read_trust_admissions(&policy_root_path)
        .map_err(CompileProjectError::Diagnostics)?;
    let report = timings.record_result(
        StageMeta::new(
            "compile",
            "sources",
            "requested product",
            TimingCategory::Pipeline,
        ),
        || {
            Ok(match (prepared, product) {
                (Some(prepared), ProjectProduct::NativeArtifact) => {
                    let request = packages::PreparedLocalProjectNativeRequest::new(
                        prepared, &build_dir, target,
                    )
                    .with_accepted_trust_admissions(admissions)
                    .with_optimization_rollback(optimization_rollback);
                    let request = match build_snapshot {
                        Some(snapshot) => request.with_build_snapshot(snapshot),
                        None => request,
                    };
                    packages::compile_prepared_local_project_for_native(request, |_| ())
                        .map(|(report, ())| report)
                        .map_err(CompileProjectError::PackageNative)?
                }
                (Some(prepared), ProjectProduct::Check) => {
                    if !optimization_rollback.is_empty() {
                        let names = optimization_rollback
                            .requested_disabled()
                            .as_slice()
                            .iter()
                            .map(|optimization| format!("`{}`", optimization.build_case_name()))
                            .collect::<Vec<_>>()
                            .join(", ");
                        return Err(CompileProjectError::Diagnostics(vec![Diagnostic::error(
                            format!(
                                "optimization rollback {names} names stages not executed by Check"
                            ),
                        )]));
                    }
                    let request = packages::PreparedLocalProjectCheckRequest::new(
                        prepared, &build_dir, target,
                    )
                    .with_accepted_trust_admissions(admissions);
                    let request = match build_snapshot {
                        Some(snapshot) => request.with_build_snapshot(snapshot),
                        None => request,
                    };
                    packages::check_prepared_local_project(request)
                        .map_err(CompileProjectError::PackageCheck)?
                }
                (None, product) => {
                    let product = match product {
                        ProjectProduct::Check => RequestedCompileProduct::Check,
                        ProjectProduct::NativeArtifact => RequestedCompileProduct::NativeArtifact,
                    };
                    let request = CompileRequest::new(options)
                        .with_requested_product(product)
                        .with_optimization_rollback(optimization_rollback)
                        .with_accepted_trust_admissions(admissions);
                    let request = match build_snapshot {
                        Some(snapshot) => request.with_build_snapshot(snapshot),
                        None => request,
                    };
                    compile(request)
                        .and_then(compiler::CompileOutcomes::into_single_report)
                        .map_err(CompileProjectError::Diagnostics)?
                }
            })
        },
    )?;
    let settlement = report.trust_admission_settlement();
    if accept_admissions {
        trust_ledger::accept_trust_admissions(&policy_root_path, settlement.required())
            .map_err(CompileProjectError::Diagnostics)?;
    } else if !settlement.is_exactly_admitted() {
        return Err(CompileProjectError::UnsettledAdmissions(settlement.clone()));
    }
    match product {
        ProjectProduct::Check => Ok(CompileProjectOutcome {
            report,
            timings,
            executable_path: None,
        }),
        ProjectProduct::NativeArtifact => {
            let (report, path) = timings
                .record_result(
                    StageMeta::new(
                        "publish",
                        "native artifact",
                        "executable",
                        TimingCategory::Pipeline,
                    ),
                    || publication::publish_native_artifact(report, &build_dir),
                )
                .map_err(CompileProjectError::Publication)?;
            Ok(CompileProjectOutcome {
                report,
                timings,
                executable_path: Some(path),
            })
        }
    }
}

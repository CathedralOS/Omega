//! Admit accepted package inputs without compiling or reconstructing review.
//!
//! The 2026-10-03 owner ruling makes review an explicit acceptance operation.
//! Matching source and compiler identities select retained compilation inputs;
//! they do not prove generated code, supply host grants, or waive Psi checking.

mod identity;
pub(crate) use identity::compiler_input_identity;

use crate::build_session::{PackageBuildSession, dependency_first_package_order};
use crate::declarations::PackageKey;
use crate::lock::{PackageCheckedContext, PackageLockTarget, PackageOccurrenceRoster};
use crate::resolution::PackageCompilationScope;
use crate::resolution::graph::{
    CanonicalSourceClosureSubject, CanonicalSourceClosureSubjectLimits,
    ExactTargetPackageSourceClosure,
};
use crate::review::{CompileResolvedPackageReviewsError, RestrictedBuildCheckpoint};
use compiler::{
    CheckedCompileRequest, CompilationOutput, CompilationProduct, PackageCompilationRequest,
};
use diagnostics::Diagnostic;
use std::path::Path;

type Error = CompileResolvedPackageReviewsError;

/// Inputs for ordinary compilation, distinct from a review candidate. An absent
/// lock supplies no accepted bindings or restricted grants. It never triggers
/// an inline audit or creates acceptance as a side effect of compilation.
pub(super) struct AcceptedCompilationRequest<'request> {
    pub closure: ExactTargetPackageSourceClosure<'request>,
    pub accepted: Option<&'request PackageLockTarget>,
    pub entry: &'request Path,
    pub snapshot: Option<&'request build_evaluation::BuildSnapshotRequest>,
    pub verify: bool,
    pub timings: bool,
    pub product: CompilationProduct,
}

fn failure(package: &PackageKey, message: impl Into<String>) -> Error {
    Error::Compilation {
        package: package.clone(),
        diagnostics: vec![Diagnostic::error(message.into())],
    }
}

fn invalid_production_root(
    product: &CompilationProduct,
    role: crate::declarations::BuildDeclarationKind,
) -> bool {
    matches!(product, CompilationProduct::Native(_))
        && role != crate::declarations::BuildDeclarationKind::Application
}

/// Ordinary project inputs to admit, without invoking a compiler. The caller
/// schedules the returned requests together and returns their ordered outcomes.
pub struct ProjectCompilationAdmissionRequest<'request> {
    pub prepared: super::PreparedLocalProject,
    pub target_profiles: &'request [target::TargetProfile],
    pub snapshot: Option<&'request build_evaluation::BuildSnapshotRequest>,
    pub verify: bool,
    pub timings: bool,
    pub product: CompilationProduct,
}

impl ProjectCompilationAdmissionRequest<'_> {
    /// Attribute an invocation preparation failure to every selected target.
    /// No compilation or package publication has occurred at this boundary.
    pub fn refuse(self, reason: String) -> Vec<ProjectCompilationOutcome> {
        let (_, closure, targets) = self.prepared.into_target_review_parts();
        let invalid_role = invalid_production_root(&self.product, closure.root_role());
        targets
            .into_iter()
            .filter(|target| self.target_profiles.contains(&target.profile))
            .map(|target| ProjectCompilationOutcome {
                target: target.profile,
                result: Err(ProjectCompilationError::Review(if invalid_role {
                    Error::InvalidProductionRootRole {
                        package: closure.graph().root().clone(),
                        role: closure.root_role(),
                    }
                } else {
                    failure(closure.graph().root(), reason.clone())
                })),
            })
            .collect()
    }
}

#[derive(Debug)]
pub enum ProjectCompilationError {
    Prepare(super::PrepareLocalProjectError),
    Review(Error),
}

impl std::fmt::Display for ProjectCompilationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Prepare(error) => write!(formatter, "{error}"),
            Self::Review(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for ProjectCompilationError {}

pub struct ProjectCompilationOutcome {
    pub target: target::TargetProfile,
    pub result: Result<CompilationOutput, ProjectCompilationError>,
}

/// Requests and their package-owned reconciliation. The application retains
/// its package session while compiling, then transfers it into completion for
/// disposal. Neither admission nor reconciliation invokes a compiler.
pub struct AdmittedProjectCompilation {
    pub requests: Vec<PackageCompilationRequest>,
    pub completion: ProjectCompilationCompletion,
}

struct TargetCompletion {
    target: target::TargetProfile,
    receipt: Result<CompilationReceipt, ProjectCompilationError>,
}

struct CompilationReceipt {
    packages: Vec<PackageKey>,
    identity: [u8; 32],
}

pub struct ProjectCompilationCompletion {
    root: PackageKey,
    targets: Vec<TargetCompletion>,
    // Keep acquisition custody alive through execution and disposal, just as
    // the prepared project did before scheduling moved to the application.
    _source_closure: Option<crate::resolution::graph::ResolvedPackageSourceClosure>,
}

/// Enforce source/lock identity, occurrence consent and native root role before
/// the admitted occurrences execute. The caller's staging owner and sponsors
/// cover all targets and earlier source discovery; a target refusal is retained
/// alongside its siblings rather than aborting the batch.
pub fn admit_project_compilation(
    request: ProjectCompilationAdmissionRequest<'_>,
    session: &mut PackageBuildSession,
) -> AdmittedProjectCompilation {
    let (entry, closure, prepared_targets) = request.prepared.into_target_review_parts();
    let root = closure.graph().root().clone();
    let targets: Vec<_> = prepared_targets
        .into_iter()
        .filter(|target| request.target_profiles.contains(&target.profile))
        .collect();
    if invalid_production_root(&request.product, closure.root_role()) {
        return AdmittedProjectCompilation {
            requests: Vec::new(),
            completion: ProjectCompilationCompletion {
                root: root.clone(),
                targets: targets
                    .iter()
                    .map(|target| TargetCompletion {
                        target: target.profile,
                        receipt: Err(ProjectCompilationError::Review(
                            Error::InvalidProductionRootRole {
                                package: root.clone(),
                                role: closure.root_role(),
                            },
                        )),
                    })
                    .collect(),
                _source_closure: None,
            },
        };
    }
    let mut requests = Vec::new();
    let mut completions = Vec::new();
    // Inspection never exercises retained restricted-action grants, even when
    // an application constructs its admission request directly.
    let verify = request.verify || matches!(request.product, CompilationProduct::Inspection(_));
    for target in targets {
        let receipt = match target.accepted {
            Err(error) => Err(ProjectCompilationError::Prepare(error)),
            Ok(accepted) => {
                let inputs = AcceptedCompilationRequest {
                    closure: closure.for_exact_target(target.profile),
                    accepted: accepted.as_ref(),
                    entry: &entry,
                    snapshot: request.snapshot,
                    verify,
                    timings: request.timings,
                    product: request.product.clone(),
                };
                match prepare_accepted_project(inputs, session) {
                    Ok((plan, packages, identity)) => {
                        requests.push(plan);
                        Ok(CompilationReceipt { packages, identity })
                    }
                    Err(error) => Err(ProjectCompilationError::Review(error)),
                }
            }
        };
        completions.push(TargetCompletion {
            target: target.profile,
            receipt,
        });
    }
    AdmittedProjectCompilation {
        requests,
        completion: ProjectCompilationCompletion {
            root,
            targets: completions,
            _source_closure: Some(closure),
        },
    }
}

impl ProjectCompilationCompletion {
    /// Reconcile occurrence diagnostics and recheck the external compiler library
    /// once after execution. This is not an admission claim for a native image.
    /// Disposal failure is reported rather than silently dropping a failed close.
    pub fn complete(
        self,
        outputs: Vec<Result<CompilationOutput, compiler::PackageCompileFailure>>,
        mut session: PackageBuildSession,
    ) -> Vec<ProjectCompilationOutcome> {
        let expected = self
            .targets
            .iter()
            .filter(|target| target.receipt.is_ok())
            .count();
        let cardinality_matches = outputs.len() == expected;
        let current_identity = compiler_input_identity();
        let mut outputs = outputs.into_iter();
        let mut outcomes = Vec::with_capacity(self.targets.len());
        for target in self.targets {
            let result = match target.receipt {
                Err(error) => Err(error),
                Ok(_) if !cardinality_matches => Err(ProjectCompilationError::Review(failure(
                    &self.root,
                    "compiler returned the wrong number of package target outcomes",
                ))),
                Ok(receipt) => receipt.reconcile(&self.root, &current_identity, outputs.next()),
            };
            outcomes.push(ProjectCompilationOutcome {
                target: target.target,
                result,
            });
        }
        if let Err(error) = session.close() {
            for outcome in &mut outcomes {
                let message = match &outcome.result {
                    Ok(_) => error.to_string(),
                    Err(prior) => format!("{error}; prior compilation failure: {prior}"),
                };
                outcome.result = Err(ProjectCompilationError::Review(failure(
                    &self.root, message,
                )));
            }
        }
        outcomes
    }
}

impl CompilationReceipt {
    fn reconcile(
        self,
        root: &PackageKey,
        current_identity: &Result<[u8; 32], String>,
        output: Option<Result<CompilationOutput, compiler::PackageCompileFailure>>,
    ) -> Result<CompilationOutput, ProjectCompilationError> {
        let output = match output {
            None => {
                return Err(ProjectCompilationError::Review(failure(
                    root,
                    "compiler lost a package target outcome",
                )));
            }
            Some(Err(error)) => {
                return Err(ProjectCompilationError::Review(Error::Compilation {
                    package: self.packages.get(error.occurrence).unwrap_or(root).clone(),
                    diagnostics: error.failure.into_diagnostics(),
                }));
            }
            Some(Ok(output)) => output,
        };
        let current = current_identity
            .as_ref()
            .map_err(|error| ProjectCompilationError::Review(failure(root, error.clone())))?;
        if *current != self.identity {
            return Err(ProjectCompilationError::Review(failure(
                root,
                "compiler library changed during compilation; run `omega update` for fresh review",
            )));
        }
        Ok(output)
    }
}

fn prepare_accepted_project(
    request: AcceptedCompilationRequest<'_>,
    session: &mut PackageBuildSession,
) -> Result<(PackageCompilationRequest, Vec<PackageKey>, [u8; 32]), Error> {
    let root = request.closure.source_closure().graph().root();
    let compiler_identity = compiler_input_identity().map_err(|error| failure(root, error))?;
    if let Some(accepted) = request.accepted {
        identity::require_current_acceptance(&request.closure, accepted, compiler_identity)
            .map_err(|error| failure(root, error))?;
    }
    let terminal_permissions = if matches!(
        request.product,
        CompilationProduct::Native(_) | CompilationProduct::Terminal(_)
    ) {
        request.accepted.map(|accepted| {
            accepted.occurrences().iter().find(|occurrence| {
                occurrence.acceptance().package() == root.identity()
                    && occurrence.context().purpose().is_product()
                    && occurrence.context().target() == request.closure.target_profile()
            }).map(|occurrence| occurrence.acceptance().terminal_permissions())
                .ok_or_else(|| failure(root, "locked target has no accepted root Product occurrence; run omega update"))
        }).transpose()?
    } else {
        None
    };
    let (occurrences, packages) = prepare_occurrences(&request, session)?;
    let plan = PackageCompilationRequest {
        occurrences,
        product: request.product,
        terminal_permissions,
    };
    Ok((plan, packages, compiler_identity))
}

fn prepare_occurrences(
    request: &AcceptedCompilationRequest<'_>,
    session: &mut PackageBuildSession,
) -> Result<(Vec<CheckedCompileRequest>, Vec<PackageKey>), Error> {
    let closure = request.closure.source_closure();
    let graph = closure.graph();
    let root = graph.root();
    let source = CanonicalSourceClosureSubject::from_resolved(
        &request.closure,
        CanonicalSourceClosureSubjectLimits::default(),
    )
    .map_err(|error| failure(root, error.to_string()))?;
    let roster = PackageOccurrenceRoster::derive(&source)
        .map_err(|error| failure(root, error.to_string()))?;
    let execution_profile = target::TargetProfile::host_if_supported();
    let checkpoint = if request.verify {
        None
    } else {
        request.accepted.map(RestrictedBuildCheckpoint::derive)
    };
    let mut occurrences = Vec::new();
    let mut packages = Vec::new();

    for key in dependency_first_package_order(closure) {
        let custody = closure
            .custody(&key)
            .ok_or_else(|| failure(&key, "resolved package has no source custody"))?;
        let purposes = roster
            .purposes(&key)
            .ok_or_else(|| failure(&key, "resolved package has no occurrence roster"))?;
        let mut root_binding = None;
        for &purpose in purposes {
            let target = if purpose.is_product() {
                request.closure.target_profile()
            } else {
                execution_profile.ok_or_else(|| {
                    failure(
                        &key,
                        "Build dependencies require an admitted execution profile",
                    )
                })?
            };
            let context = PackageCheckedContext::new(purpose, target, execution_profile);
            let bindings = request
                .accepted
                .and_then(|accepted| {
                    accepted.occurrences().iter().find(|occurrence| {
                        occurrence.acceptance().package() == key.identity()
                            && occurrence.context() == context
                    })
                })
                .and_then(|occurrence| occurrence.compilation())
                .map(|compilation| compilation.semantic_bindings().to_vec())
                .unwrap_or_default();
            let inputs = PackageCompilationScope::new(closure, &key)
                .compilation_inputs_reusing(&mut root_binding)
                .and_then(|inputs| inputs.with_compilation_purpose(purpose))
                .and_then(|inputs| inputs.with_accepted_semantic_bindings(bindings))
                .map_err(|errors| Error::CompilationInputs {
                    package: key.clone(),
                    errors,
                })?;
            let is_root = &key == root && purpose.is_product();
            let entry = if is_root {
                request.entry.to_path_buf()
            } else {
                crate::resolution::source::package_entry(
                    custody.snapshot_root(),
                    custody.captured_source().map(std::sync::Arc::as_ref),
                )
            };
            let snapshot = if is_root {
                request.snapshot.cloned()
            } else {
                None
            }
            .unwrap_or_else(|| {
                build_evaluation::BuildSnapshotRequest::new(std::iter::empty::<Vec<u8>>())
            });
            let compile_request = CheckedCompileRequest {
                package_inputs: Some(inputs),
                build_execution_profile: execution_profile,
                build_dir: Some(session.create_occurrence_directory(
                    &key,
                    custody.resolution(),
                    purpose,
                )?),
                filesystem_sponsor: Some(session.filesystem_sponsor().clone()),
                evaluation_sponsor: Some(session.evaluation_sponsor().clone()),
                build_snapshot: Some(snapshot),
                restricted_build_grants: checkpoint.as_ref().map(|checkpoint| {
                    Box::new(checkpoint.grants(
                        key.identity(),
                        context,
                        closure.dependency_path(&key),
                    )) as Box<dyn compiler::RestrictedBuildGrants>
                }),
                collect_timings: request.timings,
                optimization_rollback: request.product.optimization_rollback(),
                ..CheckedCompileRequest::new(&entry, Some(target.target_name()))
            };
            occurrences.push(compile_request);
            packages.push(key.clone());
        }
    }
    Ok((occurrences, packages))
}

//! Build prerequisites are ordinary admitted activations. The application
//! drives each one through its existing executor; source preparation and the
//! pending caller's resources stay inside this compiler-owned continuation.

use super::PreparedBuildSourceInvocation;
use artifacts::compile_timings::CompileTimings;
use build_declarations::DependencyPurpose;
use compilation_products::{CheckedExecutionRequest, CompileFailure, PreparedCheckedSource};
use diagnostics::Diagnostic;
use package_compilation::{
    BuildExecutionInputs, BuildHandoffInventory, BuildSourceOwner, CapturedCoreSource,
    PackageBuildHandoff, PackageCompilationInputs,
};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
pub(crate) struct BuildCompilationContext {
    pure: build_evaluation::PureBuildCompilation,
    handoffs: Arc<Mutex<BuildHandoffInventory>>,
}

/// One completed activation either exposes another pending Build or returns
/// the original caller's result. No pending prerequisite is a Product result.
pub enum BuildExecutionProgress {
    More,
    Complete(Result<Option<build_evaluation::ReturnedBuild>, CompileFailure>),
}

pub(crate) struct BuildSourceInvocation {
    state: Option<BuildSourceState>,
}

enum BuildSourceState {
    Prerequisite {
        source: Box<PreparedBuildSourceInvocation>,
        input: CapturedCoreSource,
        purpose: DependencyPurpose,
        pending: PendingBuildSource,
    },
    Caller(Box<PreparedBuildSourceInvocation>),
}

struct PendingBuildSource {
    root: PathBuf,
    source: PreparedCheckedSource,
    core: CapturedCoreSource,
    target: Option<target::TargetProfile>,
    packages: Option<PackageCompilationInputs>,
    build: BuildExecutionInputs,
    rollback: optimization_core::OptimizationRollback,
    reuse: compilation_products::SettledCheckReuse,
    context: BuildCompilationContext,
}

impl BuildSourceInvocation {
    pub(in crate::checked::checking) fn prepare(
        root: &Path,
        source: PreparedCheckedSource,
        child: CheckedExecutionRequest<'_>,
        context: BuildCompilationContext,
    ) -> Result<Self, CompileFailure> {
        source.admit_root(root)?;
        child.admit_package_inputs()?;
        let core = source.captured_core_source()?;
        PendingBuildSource {
            root: root.to_path_buf(),
            source,
            core,
            target: child.selected_target_profile,
            packages: child.package_inputs.cloned(),
            build: child.build,
            rollback: child.optimization_rollback,
            reuse: child.settled_check_reuse,
            context,
        }
        .next()
    }

    fn current(&self) -> &PreparedBuildSourceInvocation {
        match self
            .state
            .as_ref()
            .expect("source Build continuation is active")
        {
            BuildSourceState::Prerequisite { source, .. } | BuildSourceState::Caller(source) => {
                source
            }
        }
    }

    pub(in crate::checked::checking) fn product_queries(&self) -> crate::ProductQueries {
        self.current().product_queries()
    }

    pub(in crate::checked::checking) fn execution(
        &mut self,
    ) -> Option<&mut build_evaluation::PreparedBuildExecution> {
        match self.state.as_mut()? {
            BuildSourceState::Prerequisite { source, .. } | BuildSourceState::Caller(source) => {
                source.execution()
            }
        }
    }

    pub(in crate::checked::checking) fn validate_return(
        &self,
        returned: &build_evaluation::ReturnedBuild,
    ) -> Result<(), CompileFailure> {
        match self.state.as_ref() {
            Some(BuildSourceState::Caller(source)) => source.validate_return(returned),
            _ => Err(vec![Diagnostic::error(
                "caller Build has unfinished source prerequisites",
            )]
            .into()),
        }
    }

    pub(in crate::checked::checking) fn advance_build(
        &mut self,
        returned: Result<Option<build_evaluation::ReturnedBuild>, CompileFailure>,
    ) -> BuildExecutionProgress {
        if matches!(self.state, Some(BuildSourceState::Caller(_))) {
            return BuildExecutionProgress::Complete(returned);
        }
        let result = (|| {
            let Some(BuildSourceState::Prerequisite {
                source,
                input,
                purpose,
                pending,
            }) = self.state.take()
            else {
                return Err(vec![Diagnostic::error(
                    "source Build continuation has already finished",
                )]
                .into());
            };
            let prepared = source.complete(returned?, None, None)?;
            let configuration = &prepared.built.computed_build_config;
            let handoff = PackageBuildHandoff::from_toolchain_core(
                input,
                purpose,
                prepared.build_sources.build_execution_profile,
            )
            .with_checked_provider_selections(
                prepared.built.source.typed(),
                &configuration.config.provider_selections,
                configuration.observation_summary.as_ref(),
            )
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
            pending
                .context
                .handoffs
                .lock()
                .map_err(|_| vec![Diagnostic::error("Build handoff owner is poisoned")])?
                .insert(purpose, handoff)
                .map_err(|reason| vec![Diagnostic::error(reason)])?;
            *self = pending.next()?;
            Ok::<_, CompileFailure>(())
        })();
        match result {
            Ok(()) => BuildExecutionProgress::More,
            Err(failure) => BuildExecutionProgress::Complete(Err(failure)),
        }
    }

    pub(in crate::checked::checking) fn complete(
        self,
        returned: Option<build_evaluation::ReturnedBuild>,
        joined: Option<&PackageCompilationInputs>,
        target: Option<target::TargetProfile>,
    ) -> Result<compilation_products::PreparedBuild, CompileFailure> {
        match self.state {
            Some(BuildSourceState::Caller(source)) => source.complete(returned, joined, target),
            _ => Err(vec![Diagnostic::error(
                "Product completion has unfinished source Build prerequisites",
            )]
            .into()),
        }
    }

    pub(in crate::checked::checking) fn settle(
        self,
        returned: Option<build_evaluation::ReturnedBuild>,
        joined: Option<&PackageCompilationInputs>,
        target: Option<target::TargetProfile>,
    ) -> Result<compilation_products::SettledBuildSource, CompileFailure> {
        match self.state {
            Some(BuildSourceState::Caller(source)) => source.settle(returned, joined, target),
            _ => Err(vec![Diagnostic::error(
                "Build settlement has unfinished source prerequisites",
            )]
            .into()),
        }
    }
}

impl PendingBuildSource {
    fn next(self) -> Result<BuildSourceInvocation, CompileFailure> {
        let product_purpose = self.packages.as_ref().map_or(
            DependencyPurpose::Product,
            PackageCompilationInputs::compilation_purpose,
        );
        let mut required = vec![DependencyPurpose::Build];
        if product_purpose != DependencyPurpose::Build {
            required.push(product_purpose);
        }
        let mut handoffs = BuildHandoffInventory::default();
        for purpose in required {
            let retained = self
                .context
                .handoffs
                .lock()
                .map_err(|_| vec![Diagnostic::error("Build handoff owner is poisoned")])?
                .find(
                    purpose,
                    BuildSourceOwner::ToolchainCore(self.core.identity()),
                    purpose,
                    None,
                    self.build.profile,
                )
                .cloned();
            let Some(retained) = retained else {
                let preparation = self.source.preparation();
                let mut timings = CompileTimings::default();
                let checkpoint = package_compilation::source_assembly::ImmutableSourceParseCheckpoint::prepare_captured_core(
                    &self.core, &mut timings, &mut *preparation.catalogue()?,
                )?;
                let core = PreparedCheckedSource::from_parsed(checkpoint, timings, preparation);
                let root = self.core.build_path();
                let execution = CheckedExecutionRequest {
                    selected_target_profile: None,
                    package_inputs: None,
                    build: BuildExecutionInputs {
                        profile: self.build.profile,
                        ..Default::default()
                    },
                    optimization_rollback: Default::default(),
                    settled_check_reuse: self.reuse.clone(),
                };
                let source = super::super::prepare_admitted_build_source(
                    &root,
                    core,
                    execution,
                    Some(&self.context.pure),
                )?;
                return Ok(BuildSourceInvocation {
                    state: Some(BuildSourceState::Prerequisite {
                        source: Box::new(source),
                        input: self.core.clone(),
                        purpose,
                        pending: self,
                    }),
                });
            };
            let consumer_scope = if purpose == DependencyPurpose::Build {
                DependencyPurpose::Build
            } else {
                DependencyPurpose::Product
            };
            handoffs
                .insert(consumer_scope, retained.clone())
                .map_err(|reason| vec![Diagnostic::error(reason)])?;
            if product_purpose == DependencyPurpose::Build {
                handoffs
                    .insert(DependencyPurpose::Product, retained)
                    .map_err(|reason| vec![Diagnostic::error(reason)])?;
            }
        }
        let source = self.source.with_build_handoffs(handoffs)?;
        let child = CheckedExecutionRequest {
            selected_target_profile: self.target,
            package_inputs: self.packages.as_ref(),
            build: self.build,
            optimization_rollback: self.rollback,
            settled_check_reuse: self.reuse,
        };
        let source = super::super::prepare_admitted_build_source(
            &self.root,
            source,
            child,
            Some(&self.context.pure),
        )?;
        Ok(BuildSourceInvocation {
            state: Some(BuildSourceState::Caller(Box::new(source))),
        })
    }
}

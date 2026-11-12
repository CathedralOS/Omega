// We sequence the two Build activations (`State::Defaults` followed by
// `State::Root`) for a single source request (`SourceBuild`). The captured
// bundled `core` companion completes its default provider inventory first in an
// isolated `SourcePreparation` before `advance` prepares the root application
// or package Build invocation with those `CompletedProviderDefaults`.
//
// Every package Build in one compilation needs the same core defaults, so the
// session keeps each completed inventory (`RetainedProviderDefaults`) and a
// later request with the identical core capture and Build execution profile
// starts at `State::Root`. That pair is the core Build's whole input: it runs
// with no package inputs, grants, staging directory, snapshot or sponsor, so
// it has no other observation or effect that a fresh run could differ in
// (wiki/spec/build/scoped_execution.md#execution-contexts-and-reuse).

use super::{
    ProductQueries, build_continuation, prepare_build_source_invocation, prepare_checked_source,
};
use compilation_products::{CheckedExecutionRequest, CompileFailure, PreparedCheckedSource};
use diagnostics::Diagnostic;
use package_compilation::{CompletedProviderDefaults, PackageCompilationInputs};
use std::path::Path;
use std::sync::{Arc, Mutex};

/*
    Completed core provider-default inventories for one compilation session,
    each with the exact core capture and Build execution profile that produced
    it. Matching compares both in full; nothing else enters the core Build.
*/
#[derive(Clone, Default)]
pub(crate) struct RetainedProviderDefaults {
    completed: Arc<Mutex<Vec<RetainedCoreDefaults>>>,
}

struct RetainedCoreDefaults {
    capture: package_compilation::CapturedLocalSource,
    profile: Option<target::TargetProfile>,
    defaults: CompletedProviderDefaults,
}

impl RetainedProviderDefaults {
    fn matching(
        &self,
        capture: &package_compilation::CapturedLocalSource,
        profile: Option<target::TargetProfile>,
    ) -> Result<Option<CompletedProviderDefaults>, CompileFailure> {
        let completed = self.lock()?;
        Ok(completed
            .iter()
            .find(|retained| retained.profile == profile && retained.capture == *capture)
            .map(|retained| retained.defaults.clone()))
    }

    fn retain(
        &self,
        capture: package_compilation::CapturedLocalSource,
        profile: Option<target::TargetProfile>,
        defaults: &CompletedProviderDefaults,
    ) -> Result<(), CompileFailure> {
        let mut completed = self.lock()?;
        if !completed
            .iter()
            .any(|retained| retained.profile == profile && retained.capture == capture)
        {
            completed.push(RetainedCoreDefaults {
                capture,
                profile,
                defaults: defaults.clone(),
            });
        }
        Ok(())
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Vec<RetainedCoreDefaults>>, CompileFailure> {
        self.completed.lock().map_err(|_| {
            vec![Diagnostic::error(
                "completed core provider defaults are unavailable",
            )]
            .into()
        })
    }
}

pub enum BuildAdvance {
    Continue,
    Complete(Result<Option<build_settings::ReturnedBuild>, CompileFailure>),
}

pub(super) enum BuildSourcePurpose<'request> {
    Defaults,
    Source(&'request CompletedProviderDefaults),
}

pub(super) struct SourceBuild {
    state: State,
    retained_defaults: RetainedProviderDefaults,
}

struct PendingRoot {
    root: std::path::PathBuf,
    source: PreparedCheckedSource,
    target: Option<target::TargetProfile>,
    inputs: Option<PackageCompilationInputs>,
    build: package_compilation::build_execution::BuildExecutionInputs,
    rollback: optimization_core::OptimizationRollback,
    reuse: compilation_products::SettledCheckReuse,
    compilation: build_settings::PureBuildCompilation,
}

impl PendingRoot {
    fn prepare(self, defaults: CompletedProviderDefaults) -> Result<State, CompileFailure> {
        let PendingRoot {
            root,
            source,
            target,
            inputs,
            build,
            rollback,
            reuse,
            compilation,
        } = self;
        let source = source.with_toolchain_defaults(&defaults)?;
        let invocation = prepare_build_source_invocation(
            &root,
            source,
            CheckedExecutionRequest {
                selected_target_profile: target,
                package_inputs: inputs.as_ref(),
                build,
                optimization_rollback: rollback,
                settled_check_reuse: reuse,
            },
            Some(&compilation),
            BuildSourcePurpose::Source(&defaults),
        )?;
        Ok(State::Root {
            invocation: Box::new(invocation),
            defaults,
        })
    }
}

enum State {
    Defaults {
        invocation: Box<build_continuation::BuildSourceInvocation>,
        capture: package_compilation::CapturedLocalSource,
        profile: Option<target::TargetProfile>,
        root: Box<PendingRoot>,
    },
    Root {
        invocation: Box<build_continuation::BuildSourceInvocation>,
        defaults: CompletedProviderDefaults,
    },
    Refused,
}

impl SourceBuild {
    pub(super) fn prepare(
        root: &Path,
        source: PreparedCheckedSource,
        child: CheckedExecutionRequest<'_>,
        compilation: &build_settings::PureBuildCompilation,
        retained_defaults: &RetainedProviderDefaults,
    ) -> Result<Self, CompileFailure> {
        child.admit_package_inputs()?;
        let core_root = package_compilation::source_imports::bundled_core_root()?;
        let capture = package_compilation::capture_anonymous_compilation_source(&core_root)?;
        let profile = child.build.profile;
        let root = PendingRoot {
            root: root.to_path_buf(),
            source,
            target: child.selected_target_profile,
            inputs: child.package_inputs.cloned(),
            build: child.build,
            rollback: child.optimization_rollback,
            reuse: child.settled_check_reuse,
            compilation: compilation.clone(),
        };
        if let Some(defaults) = retained_defaults.matching(&capture, profile)? {
            return Ok(Self {
                state: root.prepare(defaults)?,
                retained_defaults: retained_defaults.clone(),
            });
        }
        let core_source = prepare_checked_source(
            &core_root,
            None,
            Some(&capture),
            false,
            compilation_products::SourcePreparation::default(),
        )?;
        let core_child = CheckedExecutionRequest {
            selected_target_profile: None,
            package_inputs: None,
            build: package_compilation::build_execution::BuildExecutionInputs {
                profile,
                ..Default::default()
            },
            optimization_rollback: Default::default(),
            settled_check_reuse: Default::default(),
        };
        let invocation = prepare_build_source_invocation(
            &core_root,
            core_source,
            core_child,
            Some(compilation),
            BuildSourcePurpose::Defaults,
        )?;
        Ok(Self {
            state: State::Defaults {
                invocation: Box::new(invocation),
                capture,
                profile,
                root: Box::new(root),
            },
            retained_defaults: retained_defaults.clone(),
        })
    }

    pub(super) fn product_queries(&self) -> ProductQueries {
        match &self.state {
            State::Defaults { invocation, .. } | State::Root { invocation, .. } => {
                invocation.product_queries()
            }
            State::Refused => unreachable!("refused source Build cannot be resumed"),
        }
    }

    pub(super) fn execution(&mut self) -> Option<&mut build_settings::PreparedBuildExecution> {
        match &mut self.state {
            State::Defaults { invocation, .. } | State::Root { invocation, .. } => {
                invocation.execution()
            }
            State::Refused => None,
        }
    }

    pub(super) fn advance(
        &mut self,
        returned: Result<Option<build_settings::ReturnedBuild>, CompileFailure>,
    ) -> Result<BuildAdvance, CompileFailure> {
        if matches!(self.state, State::Root { .. }) {
            return Ok(BuildAdvance::Complete(returned));
        }
        let State::Defaults {
            invocation,
            capture,
            profile,
            root,
        } = std::mem::replace(&mut self.state, State::Refused)
        else {
            return Err(vec![Diagnostic::error(
                "source Build sequence has already refused",
            )]
            .into());
        };
        let original_sources = invocation.anonymous_source().cloned().ok_or_else(|| {
            vec![Diagnostic::error(
                "core Build has no original source capture",
            )]
        })?;
        let completed = invocation.complete(returned?, None, None)?;
        let product_sources = completed.built.anonymous_source.as_ref().ok_or_else(|| {
            vec![Diagnostic::error(
                "core Product declarations have no source capture",
            )]
        })?;
        let configuration = &completed.built.computed_build_config;
        let observation = configuration.observation_summary.as_ref().ok_or_else(|| {
            vec![Diagnostic::error(
                "core Build returned no completed observation",
            )]
        })?;
        let defaults = CompletedProviderDefaults::from_captured_toolchain(
            capture.clone(),
            &original_sources,
            product_sources,
            completed.built.source.typed(),
            &configuration.config.provider_selections,
            observation,
        )?;
        self.retained_defaults.retain(capture, profile, &defaults)?;
        self.state = root.prepare(defaults)?;
        Ok(BuildAdvance::Continue)
    }

    pub(super) fn source_result<'result>(
        &'result mut self,
        returned: Option<&'result mut build_settings::ReturnedBuild>,
    ) -> Result<build_settings::BuildSourceResult<'result>, CompileFailure> {
        match &mut self.state {
            State::Root { invocation, .. } => invocation.source_result(returned),
            _ => Err(vec![Diagnostic::error(
                "root Build has not completed its core companion",
            )]
            .into()),
        }
    }

    pub(super) fn settle(
        self,
        returned: Option<build_settings::ReturnedBuild>,
        joined_inputs: Option<&PackageCompilationInputs>,
        product_target: Option<target::TargetProfile>,
    ) -> Result<compilation_products::SettledBuildSource, CompileFailure> {
        let State::Root {
            invocation,
            defaults,
        } = self.state
        else {
            return Err(vec![Diagnostic::error(
                "root Build has not completed its core companion",
            )]
            .into());
        };
        let mut settled = invocation.settle(returned, joined_inputs, product_target)?;
        settled.toolchain_provider_defaults = Some(defaults);
        Ok(settled)
    }

    pub(super) fn complete(
        self,
        returned: Option<build_settings::ReturnedBuild>,
        joined_inputs: Option<&PackageCompilationInputs>,
        product_target: Option<target::TargetProfile>,
    ) -> Result<compilation_products::PreparedBuild, CompileFailure> {
        let settled = self.settle(returned, joined_inputs, product_target)?;
        let inputs = settled.package_inputs.clone();
        build_continuation::prepare_settled_product(settled, inputs.as_ref())
    }
}

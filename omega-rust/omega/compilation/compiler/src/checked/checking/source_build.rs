//! Sequential Build activations for one source request.
//!
//! The captured core companion returns its own completed default inventory before
//! the application companion is prepared. Both use the same admitted invocation
//! and application executor; supplier completion cannot replace root authority.

use super::{
    ProductQueries, build_continuation, prepare_build_source_invocation, prepare_checked_source,
};
use compilation_products::{CheckedExecutionRequest, CompileFailure, PreparedCheckedSource};
use diagnostics::Diagnostic;
use package_compilation::{CompletedProviderDefaults, PackageCompilationInputs};
use std::path::Path;

/// An actual activation return either prepares the next companion or completes
/// this source's Build sequence. The final return remains paired with its owner.
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

enum State {
    Defaults {
        invocation: Box<build_continuation::BuildSourceInvocation>,
        capture: package_compilation::CapturedLocalSource,
        root: Box<PendingRoot>,
    },
    Root {
        invocation: Box<build_continuation::BuildSourceInvocation>,
        defaults: CompletedProviderDefaults,
    },
    // A refused advancement consumes its activation; it cannot be replayed.
    Refused,
}

impl SourceBuild {
    pub(super) fn prepare(
        root: &Path,
        source: PreparedCheckedSource,
        child: CheckedExecutionRequest<'_>,
        compilation: &build_settings::PureBuildCompilation,
    ) -> Result<Self, CompileFailure> {
        child.admit_package_inputs()?;
        let core_root = package_compilation::source_imports::bundled_core_root()?;
        let capture = package_compilation::capture_anonymous_compilation_source(&core_root)?;
        // The supplier is an independent source invocation. Its coordinates and
        // retained frontends must not include the requesting application's
        // earlier declarations; completed choices rejoin by captured anchors.
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
                profile: child.build.profile,
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
                root: Box::new(PendingRoot {
                    root: root.to_path_buf(),
                    source,
                    target: child.selected_target_profile,
                    inputs: child.package_inputs.cloned(),
                    build: child.build,
                    rollback: child.optimization_rollback,
                    reuse: child.settled_check_reuse,
                    compilation: compilation.clone(),
                }),
            },
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
            capture,
            &original_sources,
            product_sources,
            completed.built.source.typed(),
            &configuration.config.provider_selections,
            observation,
        )?;
        let PendingRoot {
            root,
            source,
            target,
            inputs,
            build,
            rollback,
            reuse,
            compilation,
        } = *root;
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
        self.state = State::Root {
            invocation: Box::new(invocation),
            defaults,
        };
        Ok(BuildAdvance::Continue)
    }

    pub(super) fn validate_return(
        &self,
        returned: &build_settings::ReturnedBuild,
    ) -> Result<(), CompileFailure> {
        match &self.state {
            State::Root { invocation, .. } => invocation.validate_return(returned),
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

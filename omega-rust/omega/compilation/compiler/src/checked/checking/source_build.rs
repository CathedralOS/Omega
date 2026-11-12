// A source request prepares only its own Build activation. Canonical float
// operations come from the selected target's realization table, so core has no
// provider-default Build to execute first.
//
// Core's immutable source capture remains an input to the request independently
// of Build execution. Admitting it before typing preserves the exact toolchain
// bytes and the typed-frontier reuse key without manufacturing a Build result.

use super::{ProductQueries, build_continuation, prepare_build_source_invocation};
use compilation_products::{CheckedExecutionRequest, CompileFailure, PreparedCheckedSource};
use package_compilation::PackageCompilationInputs;
use std::path::Path;

pub(super) struct SourceBuild {
    invocation: Box<build_continuation::BuildSourceInvocation>,
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
        let source = source.with_toolchain_capture(capture)?;
        Ok(Self {
            invocation: Box::new(prepare_build_source_invocation(
                root,
                source,
                child,
                Some(compilation),
            )?),
        })
    }

    pub(super) fn product_queries(&self) -> ProductQueries {
        self.invocation.product_queries()
    }

    pub(super) fn execution(&mut self) -> Option<&mut build_settings::PreparedBuildExecution> {
        self.invocation.execution()
    }

    pub(super) fn source_result<'result>(
        &'result mut self,
        returned: Option<&'result mut build_settings::ReturnedBuild>,
    ) -> Result<build_settings::BuildSourceResult<'result>, CompileFailure> {
        self.invocation.source_result(returned)
    }

    pub(super) fn settle(
        self,
        returned: Option<build_settings::ReturnedBuild>,
        joined_inputs: Option<&PackageCompilationInputs>,
        product_target: Option<target::TargetProfile>,
    ) -> Result<compilation_products::SettledBuildSource, CompileFailure> {
        self.invocation
            .settle(returned, joined_inputs, product_target)
    }

    pub(super) fn complete(
        self,
        returned: Option<build_settings::ReturnedBuild>,
        joined_inputs: Option<&PackageCompilationInputs>,
        product_target: Option<target::TargetProfile>,
    ) -> Result<compilation_products::PreparedBuild, CompileFailure> {
        self.invocation
            .complete(returned, joined_inputs, product_target)
    }
}

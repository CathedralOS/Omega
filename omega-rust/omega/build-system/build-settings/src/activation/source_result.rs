//! Source discovery consumes the completed declaration and request ledger.
//! Pure reuse keeps these immutable values, never an old activation or grant.

use super::{BuildDependencySelection, ReturnedBuild};
use build_declarations::{BuildDeclaration, BuildRequest};
use package_compilation::{PackageCompilationInputs, PackageCompilationSourceInputs};
use std::sync::Arc;

#[derive(Clone)]
pub(crate) struct CompletedBuildSource {
    declaration: Option<BuildDeclaration>,
    requests: Vec<BuildRequest>,
    demanded_sources: Vec<BuildDependencySelection>,
    source_join: Option<Arc<PackageCompilationSourceInputs>>,
}

impl CompletedBuildSource {
    pub(crate) fn capture(returned: &ReturnedBuild) -> Self {
        Self {
            declaration: returned.declaration.clone(),
            requests: returned
                .requests
                .iter()
                .map(|issued| issued.request().clone())
                .collect(),
            demanded_sources: returned.demanded_sources.clone(),
            source_join: returned.source_join.clone(),
        }
    }

    pub(crate) fn validate_source_join(
        &self,
        joined: Option<&Arc<PackageCompilationSourceInputs>>,
        inputs: Option<&PackageCompilationInputs>,
        initially_unbound: bool,
    ) -> Result<(), String> {
        super::source_join::validate_source_join(
            self.declaration.as_ref(),
            joined.or(self.source_join.as_ref()),
            inputs,
            initially_unbound,
        )
    }
}

enum SourceResult<'result> {
    Executed(&'result mut ReturnedBuild),
    Reused {
        source: &'result CompletedBuildSource,
        joined: &'result mut Option<Arc<PackageCompilationSourceInputs>>,
    },
}

/// A source result supplied by its freshly admitted evaluation. Construction
/// stays with the evaluator; a caller's configuration cannot issue this view.
pub struct BuildSourceResult<'result> {
    state: SourceResult<'result>,
}

impl<'result> BuildSourceResult<'result> {
    pub(crate) fn executed(returned: &'result mut ReturnedBuild) -> Self {
        Self {
            state: SourceResult::Executed(returned),
        }
    }

    pub(crate) fn reused(
        source: &'result CompletedBuildSource,
        joined: &'result mut Option<Arc<PackageCompilationSourceInputs>>,
    ) -> Self {
        Self {
            state: SourceResult::Reused { source, joined },
        }
    }

    pub fn declaration(&self) -> Option<&BuildDeclaration> {
        match &self.state {
            SourceResult::Executed(returned) => returned.declaration(),
            SourceResult::Reused { source, .. } => source.declaration.as_ref(),
        }
    }

    pub fn requests(&self) -> impl Iterator<Item = &BuildRequest> {
        let (issued, retained) = match &self.state {
            SourceResult::Executed(returned) => (Some(returned.requests.as_slice()), None),
            SourceResult::Reused { source, .. } => (None, Some(source.requests.as_slice())),
        };
        issued
            .into_iter()
            .flatten()
            .map(|issued| issued.request())
            .chain(retained.into_iter().flatten())
    }

    pub fn demanded_sources(&self) -> &[BuildDependencySelection] {
        match &self.state {
            SourceResult::Executed(returned) => returned.demanded_sources(),
            SourceResult::Reused { source, .. } => &source.demanded_sources,
        }
    }

    pub fn join_dependency_sources(
        &mut self,
        inputs: &PackageCompilationInputs,
        selections: &[BuildDependencySelection],
    ) -> Result<(), String> {
        match &mut self.state {
            SourceResult::Executed(returned) => {
                returned.join_dependency_sources(inputs, selections)
            }
            SourceResult::Reused { source, joined } => {
                if joined.is_some() {
                    return Err("Build source requests were already joined".into());
                }
                super::source_join::validate_dependency_sources(
                    source.declaration.as_ref(),
                    source.requests.iter(),
                    &source.demanded_sources,
                    inputs,
                    selections,
                )?;
                source.validate_source_join(None, Some(inputs), false)?;
                **joined = Some(inputs.source_inputs());
                Ok(())
            }
        }
    }
}

//! Build and constant invocations need the same checked publication as a
//! product. Keep its frozen source beside the artifact so companion requests
//! can use exact typed occurrences instead of reconstructing source identity.

use std::sync::Arc;

use diagnostics::Diagnostic;
use lowered_discharge::RoutePublication;
use structure_elaboration::Compilation;
use typed_trees::FrozenTypedTrees;

/// The ordinary checker accepted this source and published this exact module.
/// Execution custody and source-issued companions are attached afterward;
/// neither may change the accepted module or its evidence.
#[derive(Debug)]
pub struct CheckedInvocationSource {
    typed: Arc<FrozenTypedTrees>,
    publication: RoutePublication,
}

impl PartialEq for CheckedInvocationSource {
    fn eq(&self, other: &Self) -> bool {
        // Source and accepted code define the reusable input. Each execution
        // receives fresh custody after this publication has been retained.
        self.typed == other.typed && self.publication.artifact == other.publication.artifact
    }
}

impl Eq for CheckedInvocationSource {}

impl CheckedInvocationSource {
    pub fn prepare(
        typed: Arc<FrozenTypedTrees>,
        compilation: Compilation<'_>,
    ) -> Result<Self, Vec<Diagnostic>> {
        let (judgment, publication) = Self::judge(typed, compilation);
        judgment.verdict()?;
        publication
    }

    /// Preserve the source verdict separately from executable publication.
    /// Both Build and product consumers retain the same private source owner.
    pub fn judge(
        typed: Arc<FrozenTypedTrees>,
        compilation: Compilation<'_>,
    ) -> (
        lowered_discharge::CompilationJudgment,
        Result<Self, Vec<Diagnostic>>,
    ) {
        let (judgment, publication) = lowered_discharge::judge_and_publish(&typed, compilation);
        let publication = publication.map(|publication| Self { typed, publication });
        (judgment, publication)
    }

    /// Activate this exact accepted entry. Host companions come from its typed
    /// occurrences, and this execution receives a fresh custody identity.
    pub fn produce_invocation(&self) -> Result<crate::ProducedTerminalProgram, Vec<Diagnostic>> {
        crate::ProducedTerminalProgram::from_accepted_source(self)
    }

    pub fn typed(&self) -> &Arc<FrozenTypedTrees> {
        &self.typed
    }

    pub fn publication(&self) -> &RoutePublication {
        &self.publication
    }

    pub fn into_parts(self) -> (Arc<FrozenTypedTrees>, RoutePublication) {
        (self.typed, self.publication)
    }
}

//! Drive compiler-owned Build prerequisites through the same application
//! executor as the caller. The application never assembles source stages.

use compiler::{BuildExecutionProgress, CompileFailure, ProductQueries};

pub(crate) trait BuildContinuation {
    fn queries(&self) -> Option<ProductQueries>;
    fn execution(&mut self) -> Option<&mut build_evaluation::PreparedBuildExecution>;
    fn advance(
        &mut self,
        returned: Result<Option<build_evaluation::ReturnedBuild>, CompileFailure>,
    ) -> BuildExecutionProgress;
}

macro_rules! continuation {
    ($owner:ty, $queries:expr) => {
        impl BuildContinuation for $owner {
            fn queries(&self) -> Option<ProductQueries> {
                ($queries)(self)
            }
            fn execution(&mut self) -> Option<&mut build_evaluation::PreparedBuildExecution> {
                <$owner>::execution(self)
            }
            fn advance(
                &mut self,
                returned: Result<Option<build_evaluation::ReturnedBuild>, CompileFailure>,
            ) -> BuildExecutionProgress {
                <$owner>::advance_build(self, returned)
            }
        }
    };
}
continuation!(
    compiler::SourceCompilation,
    |source: &compiler::SourceCompilation| Some(source.product_queries())
);
continuation!(
    compiler::PackageBuildInvocation,
    |source: &compiler::PackageBuildInvocation| Some(source.product_queries())
);
continuation!(
    compiler::PreparedCompilation,
    |source: &compiler::PreparedCompilation| source.product_queries()
);

pub(crate) fn execute_builds(
    continuation: &mut impl BuildContinuation,
) -> Result<Option<build_evaluation::ReturnedBuild>, CompileFailure> {
    loop {
        let mut queries = continuation.queries();
        let returned = super::execute_build(continuation.execution(), queries.as_mut());
        match continuation.advance(returned) {
            BuildExecutionProgress::More => {}
            BuildExecutionProgress::Complete(returned) => return returned,
        }
    }
}

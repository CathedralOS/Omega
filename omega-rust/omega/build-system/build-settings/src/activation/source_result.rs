//! The request ledger of an actually executed Build activation. Source joins
//! retain that activation's declaration and demanded-source authority.

use super::{BuildDependencySelection, ReturnedBuild};
use build_declarations::{BuildDeclaration, BuildRequest};
use package_compilation::PackageCompilationInputs;

pub struct BuildSourceResult<'result> {
    returned: &'result mut ReturnedBuild,
}

impl<'result> BuildSourceResult<'result> {
    pub(crate) fn executed(returned: &'result mut ReturnedBuild) -> Self {
        Self { returned }
    }

    pub fn declaration(&self) -> Option<&BuildDeclaration> {
        self.returned.declaration()
    }

    pub fn requests(&self) -> impl Iterator<Item = &BuildRequest> {
        self.returned.requests.iter().map(|issued| issued.request())
    }

    pub fn demanded_sources(&self) -> &[BuildDependencySelection] {
        self.returned.demanded_sources()
    }

    pub fn join_dependency_sources(
        &mut self,
        inputs: &PackageCompilationInputs,
        selections: &[BuildDependencySelection],
    ) -> Result<(), String> {
        self.returned.join_dependency_sources(inputs, selections)
    }
}

//! Untrusted semantic candidates for whole-review corruption controls.
//!
//! Production compilation custody stays immutable. A test detaches raw trees
//! only when deliberately supplying malformed input to independent review.

use diagnostics::Diagnostic;
use omega::compiler::{CheckedCompilation, CheckedCompileRequest};
use omega::package_evidence::PackageReviewInput;
use typed_trees_to_checked_trees::checked_trees::CheckedTrees;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ReviewFixture {
    pub(crate) custody: CheckedCompilation,
    supplied: Option<CheckedTrees>,
}

pub(crate) fn compile_review_fixture(
    request: CheckedCompileRequest<'_>,
) -> Result<ReviewFixture, Vec<Diagnostic>> {
    omega::compiler::compile_to_checked(request).map(|custody| ReviewFixture {
        custody,
        supplied: None,
    })
}

impl std::ops::Deref for ReviewFixture {
    type Target = CheckedTrees;

    fn deref(&self) -> &CheckedTrees {
        self.supplied.as_ref().unwrap_or(&self.custody)
    }
}

impl std::ops::DerefMut for ReviewFixture {
    fn deref_mut(&mut self) -> &mut CheckedTrees {
        self.supplied
            .get_or_insert_with(|| self.custody.clone().into_program())
    }
}

impl<'a> From<&'a ReviewFixture> for PackageReviewInput<'a> {
    fn from(fixture: &'a ReviewFixture) -> Self {
        Self::supplied(fixture, &fixture.custody)
    }
}

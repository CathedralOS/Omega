//! Semantic review candidates remain separate from compiler-issued custody.

use checked_trees::CheckedTrees;
use compiler::CheckedCompilation;

/// Read-only inputs for producing inert package-review evidence.
///
/// The semantic program may be supplied independently and is not presumed to
/// match the original compilation. Capture rejoins it to the borrowed source
/// and selection custody. This view cannot construct a checked compilation,
/// change compiler state, or authorize execution or package admission.
#[derive(Clone, Copy)]
pub struct PackageReviewInput<'a> {
    pub(crate) program: &'a CheckedTrees,
    pub(crate) custody: &'a CheckedCompilation,
}

impl<'a> PackageReviewInput<'a> {
    /// Review an untrusted semantic candidate against immutable compiler custody.
    /// Results are review data only, never checked compiler or execution authority.
    pub const fn supplied(program: &'a CheckedTrees, custody: &'a CheckedCompilation) -> Self {
        Self { program, custody }
    }
}

impl<'a> From<&'a CheckedCompilation> for PackageReviewInput<'a> {
    fn from(compilation: &'a CheckedCompilation) -> Self {
        Self::supplied(compilation, compilation)
    }
}

impl<'a> From<&PackageReviewInput<'a>> for PackageReviewInput<'a> {
    fn from(input: &PackageReviewInput<'a>) -> Self {
        *input
    }
}

impl std::ops::Deref for PackageReviewInput<'_> {
    type Target = CheckedTrees;

    fn deref(&self) -> &Self::Target {
        self.program
    }
}

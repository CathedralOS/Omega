//! Semantic review candidates remain separate from compiler-issued custody.

use checked_trees::CheckedTrees;
use compilation_products::CheckedCompilation;

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

    /// Authored deployment scope must agree with immutable compiler custody.
    /// A selected member's cleared marker cannot establish portability, and
    /// a supplied candidate cannot hide a member by deleting its family row.
    pub(super) fn authored_machine_target(
        &self,
        symbol: symbols::SymbolHandle,
    ) -> Result<Option<&str>, Vec<diagnostics::Diagnostic>> {
        let rejected = |reason: &str| {
            vec![diagnostics::Diagnostic::error(format!(
                "package target context rejects {reason}"
            ))]
        };
        let target = self
            .program
            .typed
            .authored_machine_target(symbol)
            .map_err(rejected)?;
        let original = self
            .custody
            .typed
            .authored_machine_target(symbol)
            .map_err(rejected)?;
        if target != original {
            return Err(rejected(
                "authored target context differs from checked source custody",
            ));
        }
        Ok(target)
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

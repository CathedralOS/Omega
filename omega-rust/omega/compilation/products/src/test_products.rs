/*
Separately selected test roots (wiki/spec/build/testing.md#products-reuse-and-native-harnesses).

A registered test group selects test roots beside the application roots over
the same checked source. Each discovered test becomes its own product whose
only `ProgramEntry` root is that test machine, so its entry, fixtures and
providers never become roots of the published application Psi or native image.
The application's pending native product carries these products to native
completion, which runs every scheduled invocation before the application
report can succeed.
*/

use crate::PendingNativeProduct;
use diagnostics::Diagnostic;

// Exact identity of one scheduled test invocation, retained in roster order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestRootIdentity {
    pub group: String, // Registered requirement's canonical path, e.g. `Tests::arithmetic`.
    pub test: String,  // Satisfying machine's canonical path.
    pub target: target::TargetProfile,
}

impl std::fmt::Display for TestRootIdentity {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "test `{}` in group `{}`", self.test, self.group)
    }
}

// One test product awaiting native completion. A production failure is kept
// beside its identity: an enabled test that cannot be produced makes the run
// incomplete rather than disappearing from the roster.
pub struct PendingTestProduct {
    pub identity: TestRootIdentity,
    pub product: Result<Box<PendingNativeProduct>, Vec<Diagnostic>>,
}

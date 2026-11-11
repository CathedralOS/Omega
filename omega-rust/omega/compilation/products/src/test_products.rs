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

// One row of the published test roster
// (wiki/spec/build/testing.md#execution-plan), in roster order. A group that is
// disabled or has no tests is reported as such and never counts as a pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestRosterEntry {
    pub group: String,
    pub test: Option<String>, // `None` for a row describing the whole group.
    pub target: target::TargetProfile,
    pub status: TestRosterStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TestRosterStatus {
    Passed {
        engines: Vec<String>,     // Engines that ran the test and saw it return.
        unavailable: Vec<String>, // Engines that could not run it, with the reason.
    },
    GroupDisabled,
    GroupWithoutTests,
}

impl std::fmt::Display for TestRosterEntry {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let target = self.target.target_name();
        match (&self.test, &self.status) {
            (
                Some(test),
                TestRosterStatus::Passed {
                    engines,
                    unavailable,
                },
            ) => {
                write!(
                    formatter,
                    "test `{test}` in group `{}` (target `{target}`): passed on {}",
                    self.group,
                    engines.join(", ")
                )?;
                if !unavailable.is_empty() {
                    write!(formatter, "; not run on {}", unavailable.join("; "))?;
                }
                Ok(())
            }
            (_, TestRosterStatus::GroupDisabled) => write!(
                formatter,
                "group `{}` (target `{target}`): disabled; its tests were not run",
                self.group
            ),
            (_, TestRosterStatus::GroupWithoutTests) => write!(
                formatter,
                "group `{}` (target `{target}`): registered with no tests; nothing ran",
                self.group
            ),
            (None, TestRosterStatus::Passed { .. }) => {
                write!(formatter, "group `{}` (target `{target}`)", self.group)
            }
        }
    }
}

// One test product awaiting native completion. A production failure is kept
// beside its identity: an enabled test that cannot be produced makes the run
// incomplete rather than disappearing from the roster.
pub struct PendingTestProduct {
    pub identity: TestRootIdentity,
    pub product: Result<Box<PendingNativeProduct>, Vec<Diagnostic>>,
}

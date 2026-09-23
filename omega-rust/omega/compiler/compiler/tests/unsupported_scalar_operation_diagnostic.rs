//! End-to-end pin for the unsupported-scalar-operation diagnostic: a
//! well-formed source program that reaches selection with an operation family
//! the stage has no legalized scalar instruction for must surface that family
//! by name — `UnsupportedScalarOperation` retaining the rejected operation —
//! instead of collapsing into the `SourceCustodyMismatch` producer-defect
//! spelling or aborting, on every bound target rather than only the host.
//!
//! The reachable witness is saturating multiplication: `SaturatingCarrier`
//! covers every native fixed width, and saturating add/subtract/divide have
//! legalized kinds, but `SaturatingIntegerMultiply` is lowered to target
//! operations with no legal scalar kind at any carrier
//! (`target-operations-to-selected-instructions` legalization unit coverage:
//! `saturating_multiply_reports_the_unsupported_family_with_its_operation`).
//! The `samples/cli/basics/number_guess` cohort observes the same route for
//! signed saturating arithmetic; this test holds the diagnostic end to end
//! through `compiler::compile` for a dedicated fixture rather than a sample.

use compiler::{
    CompileOptions, CompileRequest, RequestedCompileProduct, TargetCompileConfiguration,
};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use target::TargetProfile;

static NEXT_PROJECT: AtomicU64 = AtomicU64::new(0);

/// The fixture's `main` never reaches a service: a saturating multiply over
/// fields the optimizer cannot constant-fold across the transition boundary.
const MAIN: &str = r#"data Main {
    a: i32 in Saturating;
    b: i32 in Saturating;
    q: i32 in Saturating;
}

machine Main::main(&mut self) {
    self.a = 23;
    self.b = 5;
    transition { _ -> mul() }
    state mul(&mut self) {
        self.q = self.a * self.b;
        transition self.q == 115 { true -> yes() false -> no() }
    }
    state yes(&mut self) {}
    state no(&mut self) {}
}
"#;

/// Every hosted target binds an entry so the compile reaches selection
/// regardless of which host runs the test.
const BUILD: &str = r#"machine build(builder: &mut Build) {
    builder.application("unsupported-scalar-operation-diagnostic");
    builder.roots.bind(windows_x86_64::ProgramEntry, Main::main);
    builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);
    builder.roots.bind(linux_arm64::ProgramEntry, Main::main);
    builder.roots.bind(macos_arm64::ProgramEntry, Main::main);
}
"#;

/// Every target the fixture's build machine binds. The refusal is exercised
/// on each, so the pin does not depend on which host runs the test — and a
/// `macos_x86_64` host, which binds no root here, is not special-cased.
const BOUND_TARGETS: [&str; 4] = [
    "windows_x86_64",
    "linux_x86_64",
    "linux_arm64",
    "macos_arm64",
];

/// A unique scratch project: parallel test threads must not share a source
/// dir or a build dir while the fixture is written and compiled.
fn scratch_dir(label: &str) -> PathBuf {
    let unique = NEXT_PROJECT.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "omega-unsupported-scalar-{label}-{}-{unique}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

struct Project(PathBuf);

impl Project {
    fn write() -> Self {
        let dir = scratch_dir("project");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("main.omg"), MAIN).unwrap();
        std::fs::write(dir.join("build.omg"), BUILD).unwrap();
        Self(dir)
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn compile_all_bound_targets(
    project_root: &Path,
) -> Result<compiler::CompileOutcomes, Vec<diagnostics::Diagnostic>> {
    let build_dir = scratch_dir("build");
    let result = compiler::compile(
        CompileRequest::new(CompileOptions {
            root_path: project_root.join("main.omg"),
            build_dir: None,
            target_name: None,
        })
        .with_target_configurations(
            BOUND_TARGETS
                .iter()
                .map(|name| {
                    TargetCompileConfiguration::new(
                        TargetProfile::from_omega_target_name(Some(name))
                            .expect("bound targets are canonical profile names"),
                    )
                    .with_build_dir(build_dir.join(name))
                })
                .collect(),
        )
        .with_requested_product(RequestedCompileProduct::NativeArtifact),
    );
    let _ = std::fs::remove_dir_all(&build_dir);
    result
}

fn messages(diagnostics: &[diagnostics::Diagnostic]) -> String {
    diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn saturating_multiply_reports_its_family_instead_of_a_custody_mismatch() {
    let project = Project::write();
    // Per-target failures ride the outcome roster rather than rejecting the
    // request, so every bound target must report its own named refusal.
    let outcomes = compile_all_bound_targets(&project.0)
        .expect("per-target failures are retained on the outcomes, not the request");
    let mut covered = outcomes
        .outcomes()
        .iter()
        .map(|outcome| {
            outcome
                .target_profile()
                .expect("each configuration names its target")
                .target_name()
        })
        .collect::<Vec<_>>();
    covered.sort_unstable();
    let mut expected = BOUND_TARGETS.to_vec();
    expected.sort_unstable();
    assert_eq!(covered, expected, "one outcome per bound target");
    for outcome in outcomes.outcomes() {
        let target = outcome.target_profile().unwrap().target_name();
        let diagnostics = outcome.diagnostics().unwrap_or_else(|| {
            panic!("saturating multiply has no legalized scalar kind on {target}")
        });
        let text = messages(diagnostics);
        assert!(
            text.contains("SaturatingIntegerMultiply"),
            "the {target} diagnostic retains the rejected operation, got:\n{text}"
        );
        assert!(
            !text.contains("SourceCustodyMismatch"),
            "a well-formed unsupported family is not a producer/consumer \
             defect on {target}:\n{text}"
        );
    }
}

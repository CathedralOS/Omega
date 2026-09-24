//! End-to-end pin that saturating multiplication, wrapping division, and
//! every Trapping primitive reach native artifacts at every fixed integer
//! width on every bound target, not only on the host the run canaries
//! execute on. Selection legalizes each
//! family per carrier (x86-64 u64 multiplication sits on the fixed `MUL`
//! row, narrow signed quotients re-normalize after the i64 divide), and a
//! target whose encoder or constraint row disagreed would refuse here rather
//! than in a host-only execution test. Each Trapping primitive is one
//! selected instruction ending in an inline trap per target (x86-64 pins the
//! divisor, the u64 multiplier and the shift count to RCX), so the macOS run
//! canaries alone would not notice an x86-64 row or encoder that refused.
//!
//! Every integer arithmetic family now has a legalized kind at every native
//! width, so this replaces the earlier end-to-end pin of the
//! `UnsupportedScalarOperation` diagnostic, whose saturating-multiply witness
//! no longer exists; the named-diagnostic route itself stays pinned at the
//! stage by `admission_vocabulary_tests` in
//! `target-operations-to-selected-instructions`.

use compiler::{
    CompileOptions, CompileRequest, RequestedCompileProduct, TargetCompileConfiguration,
};
use std::fmt::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use target::TargetProfile;

static NEXT_PROJECT: AtomicU64 = AtomicU64::new(0);

const WIDTHS: [&str; 8] = ["i8", "i16", "i32", "i64", "u8", "u16", "u32", "u64"];

/// One machine multiplying saturating fields and dividing wrapping fields of
/// every width. Each state assigns its operands before using them, the shape
/// the run canaries use to keep the operations out of constant folding while
/// the divisor stays provably nonzero.
fn main_source() -> String {
    let mut fields = String::new();
    let mut multiply_inputs = String::new();
    let mut multiply = String::new();
    let mut divide_inputs = String::new();
    let mut divide = String::new();
    for width in WIDTHS {
        writeln!(fields, "    sa_{width}: {width} in Saturating;").unwrap();
        writeln!(fields, "    sb_{width}: {width} in Saturating;").unwrap();
        writeln!(fields, "    wa_{width}: {width} in Wrapping;").unwrap();
        writeln!(fields, "    wb_{width}: {width} in Wrapping;").unwrap();
        writeln!(fields, "    wq_{width}: {width} in Wrapping;").unwrap();
        writeln!(fields, "    wr_{width}: {width} in Wrapping;").unwrap();
        writeln!(multiply_inputs, "        self.sa_{width} = 100;").unwrap();
        writeln!(multiply_inputs, "        self.sb_{width} = 3;").unwrap();
        writeln!(
            multiply,
            "        self.sa_{width} = self.sa_{width} * self.sb_{width};"
        )
        .unwrap();
        writeln!(divide_inputs, "        self.wa_{width} = 100;").unwrap();
        writeln!(divide_inputs, "        self.wb_{width} = 7;").unwrap();
        writeln!(
            divide,
            "        self.wq_{width} = self.wa_{width} / self.wb_{width};"
        )
        .unwrap();
        writeln!(
            divide,
            "        self.wr_{width} = self.wa_{width} % self.wb_{width};"
        )
        .unwrap();
    }
    format!(
        r#"data Main {{
{fields}}}

machine Main::main(&mut self) {{
    transition {{ _ -> multiply() }}
    state multiply(&mut self) {{
{multiply_inputs}{multiply}        transition {{ _ -> divide() }}
    }}
    state divide(&mut self) {{
{divide_inputs}{divide}        transition self.wq_i8 == 14 {{ true -> yes() false -> no() }}
    }}
    state yes(&mut self) {{}}
    state no(&mut self) {{}}
}}
"#
    )
}

/// One machine applying every Trapping primitive at every width, then every
/// Trapping conversion between two distinct widths. The operands are small
/// enough that no operation always traps, and field operands keep each
/// operation a runtime Trapping site rather than a folded constant.
fn trapping_source() -> String {
    let mut fields = String::new();
    let mut inputs = String::new();
    let mut arithmetic = String::new();
    let mut conversions = String::new();
    for width in WIDTHS {
        for name in ["ta", "tb", "tr", "tc"] {
            writeln!(fields, "    {name}_{width}: {width} in Trapping;").unwrap();
        }
        writeln!(inputs, "        self.ta_{width} = 7;").unwrap();
        writeln!(inputs, "        self.tb_{width} = 3;").unwrap();
        for operator in ["+", "-", "*", "/", "%", "<<", ">>"] {
            writeln!(
                arithmetic,
                "        self.tr_{width} = self.ta_{width} {operator} self.tb_{width};"
            )
            .unwrap();
        }
        for source in WIDTHS.into_iter().filter(|source| *source != width) {
            writeln!(
                conversions,
                "        self.tc_{width} = self.ta_{source} as {width} in Trapping;"
            )
            .unwrap();
        }
    }
    format!(
        r#"data Main {{
{fields}}}

machine Main::main(&mut self) {{
    transition {{ _ -> arithmetic() }}
    state arithmetic(&mut self) {{
{inputs}{arithmetic}        transition {{ _ -> convert() }}
    }}
    state convert(&mut self) {{
{inputs}{conversions}        transition self.tc_i8 == 7 {{ true -> yes() false -> no() }}
    }}
    state yes(&mut self) {{}}
    state no(&mut self) {{}}
}}
"#
    )
}

/// Every hosted target binds an entry, so each compiles its own artifact
/// regardless of which host runs the test.
const BUILD: &str = r#"machine build(builder: &mut Build) {
    builder.application("policy_arithmetic_native_targets");
    builder.roots.bind(windows_x86_64::ProgramEntry, Main::main);
    builder.roots.bind(linux_x86_64::ProgramEntry, Main::main);
    builder.roots.bind(linux_arm64::ProgramEntry, Main::main);
    builder.roots.bind(macos_arm64::ProgramEntry, Main::main);
}
"#;

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
        "omega-policy-arithmetic-{label}-{}-{unique}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

struct Project(PathBuf);

impl Project {
    fn write(source: String) -> Self {
        let dir = scratch_dir("project");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("main.omg"), source).unwrap();
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

#[test]
fn saturating_multiply_and_wrapping_division_compile_on_every_bound_target() {
    assert_every_bound_target_compiles(main_source());
}

#[test]
fn trapping_arithmetic_and_conversions_compile_on_every_bound_target() {
    assert_every_bound_target_compiles(trapping_source());
}

fn assert_every_bound_target_compiles(source: String) {
    let project = Project::write(source);
    let outcomes = compile_all_bound_targets(&project.0)
        .unwrap_or_else(|diagnostics| panic!("request refused: {diagnostics:?}"));
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
        if let Some(diagnostics) = outcome.diagnostics() {
            let text = diagnostics
                .iter()
                .map(|diagnostic| diagnostic.message.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            panic!("{target} refused the native artifact:\n{text}");
        }
    }
}

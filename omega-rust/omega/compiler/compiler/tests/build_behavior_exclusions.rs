//! End-to-end coverage for `builder.exclude_crash(CrashCause::X)`
//! (wiki/spec/build/behavior_exclusions.md): the authored exclusion is a
//! product-admission requirement harvested statically from the root build
//! machine's checked call scope, retaining the exact toolchain `CrashCause`
//! case identity and the authored source span.

use compiler::CheckedCompileRequest;
use compiler::compile_to_checked;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

struct TempProject(PathBuf);

impl TempProject {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "omega-build-behavior-exclusions-{}-{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).expect("create temporary Omega project");
        Self(path)
    }

    fn write(&self, name: &str, source: &str) {
        let path = self.0.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create temporary Omega source directory");
        }
        fs::write(path, source).expect("write temporary Omega source");
    }

    fn main(&self) -> PathBuf {
        self.0.join("main.omg")
    }
}

impl Drop for TempProject {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn toolchain_crash_cause_case(
    checked: &compiler::CheckedCompilation,
    case: &str,
) -> symbols::SymbolHandle {
    let definition = checked
        .typed
        .data_definitions()
        .iter()
        .find(|definition| {
            definition.name.as_str() == "CrashCause"
                && checked
                    .typed
                    .symbols
                    .symbol_source_span(definition.symbol)
                    .and_then(|span| checked.typed.symbols.source_file(span))
                    .is_some_and(|file| {
                        file.origin == source::SourceOrigin::Toolchain
                            && file.path == std::path::Path::new("<build-prelude>")
                    })
        })
        .expect("exact toolchain CrashCause declaration");
    checked
        .typed
        .data_members(definition)
        .iter()
        .find_map(|member| match member {
            typed_trees::data::DataMember::Variant(variant) if variant.name.as_str() == case => {
                Some(variant.symbol)
            }
            _ => None,
        })
        .expect("toolchain CrashCause case")
}

#[test]
fn authored_exclude_crash_retains_exact_case_identity_and_authored_span() {
    let project = TempProject::new();
    project.write("main.omg", "const ANSWER: u32 = 42;\n");
    project.write(
        "build.omg",
        r#"machine build(builder: &mut Build) {
    builder.application("behavior-exclusion-crash");
    builder.exclude_crash(CrashCause::Trap);
}
"#,
    );

    let checked = compile_to_checked(CheckedCompileRequest::new(&project.main(), None))
        .expect("an authored crash exclusion must check");
    let build = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "build")
        .expect("checked build machine");

    let [exclusion] = checked.behavior_exclusions() else {
        panic!("exactly one authored exclusion must be retained");
    };
    let build_evaluation::AuthoredBehaviorExclusionKind::CrashCause { cause, case_symbol } =
        exclusion.kind;
    assert_eq!(cause, terminal_psi::CrashCause::Trap);
    assert_eq!(
        case_symbol,
        toolchain_crash_cause_case(&checked, "Trap"),
        "the exclusion must name the exact toolchain CrashCause case symbol"
    );
    assert_eq!(exclusion.selecting_machine, build.symbol);
    let source = checked
        .typed
        .symbols
        .source_file(exclusion.source_span)
        .expect("the exclusion must retain authored source custody");
    assert_eq!(
        source.path.file_name().and_then(|name| name.to_str()),
        Some("build.omg")
    );
    assert!(exclusion.source_span.span.start < exclusion.source_span.span.end);
}

#[test]
fn repeated_and_reordered_exclusions_collapse_to_the_canonical_union() {
    let project = TempProject::new();
    project.write("main.omg", "const ANSWER: u32 = 42;\n");
    project.write(
        "build.omg",
        r#"machine build(builder: &mut Build) {
    builder.application("behavior-exclusion-union");
    builder.exclude_crash(CrashCause::Abort);
    builder.exclude_crash(CrashCause::Trap);
    builder.exclude_crash(CrashCause::Abort);
}
"#,
    );

    let checked = compile_to_checked(CheckedCompileRequest::new(&project.main(), None))
        .expect("repeated authored exclusions must check");

    // Authored occurrences keep their provenance; the canonical admission
    // set is order- and repetition-insensitive.
    assert_eq!(checked.behavior_exclusions().len(), 3);
    let exclusions =
        build_evaluation::authored_behavior_exclusion_set(checked.behavior_exclusions());
    assert!(exclusions.excludes_crash_cause(terminal_psi::CrashCause::Trap));
    assert!(exclusions.excludes_crash_cause(terminal_psi::CrashCause::Abort));
    assert_eq!(exclusions.crash_causes().len(), 2);
}

#[test]
fn a_reachable_helper_may_spell_the_selection_for_the_root() {
    let project = TempProject::new();
    project.write("main.omg", "const ANSWER: u32 = 42;\n");
    project.write(
        "build.omg",
        r#"machine restrict(builder: &mut Build) {
    builder.exclude_crash(CrashCause::Abort);
}

machine build(builder: &mut Build) {
    builder.application("behavior-exclusion-helper");
    restrict(builder);
}
"#,
    );

    let checked = compile_to_checked(CheckedCompileRequest::new(&project.main(), None))
        .expect("a helper inside the checked call scope may spell the selection");
    let restrict = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "restrict")
        .expect("checked helper machine");

    let [exclusion] = checked.behavior_exclusions() else {
        panic!("the helper's authored exclusion must be retained");
    };
    let build_evaluation::AuthoredBehaviorExclusionKind::CrashCause { cause, case_symbol } =
        exclusion.kind;
    assert_eq!(cause, terminal_psi::CrashCause::Abort);
    assert_eq!(case_symbol, toolchain_crash_cause_case(&checked, "Abort"));
    assert_eq!(exclusion.selecting_machine, restrict.symbol);
}

#[test]
fn an_exclusion_outside_the_build_scope_rejects_rather_than_dropping() {
    let project = TempProject::new();
    project.write("main.omg", "const ANSWER: u32 = 42;\n");
    project.write(
        "build.omg",
        r#"machine unused(builder: &mut Build) {
    builder.exclude_crash(CrashCause::Trap);
}

machine build(builder: &mut Build) {
    builder.application("behavior-exclusion-out-of-scope");
}
"#,
    );

    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&project.main(), None))
        .expect_err("a selection the admission check cannot see must reject");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("outside the root build machine's checked call scope")
        }),
        "unexpected diagnostics: {diagnostics:#?}"
    );
}

#[test]
fn the_exclusion_argument_must_name_the_exact_toolchain_case() {
    let project = TempProject::new();
    project.write("main.omg", "const ANSWER: u32 = 42;\n");
    project.write(
        "build.omg",
        r#"machine build(builder: &mut Build) {
    builder.application("behavior-exclusion-indirect");
    let cause: CrashCause = CrashCause::Trap;
    builder.exclude_crash(cause);
}
"#,
    );

    let diagnostics = compile_to_checked(CheckedCompileRequest::new(&project.main(), None))
        .expect_err("an indirect value must not stand in for the exact case");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("does not name an exact compiler-owned CrashCause case")
        }),
        "unexpected diagnostics: {diagnostics:#?}"
    );
}

#[test]
fn an_authored_same_named_machine_is_not_the_toolchain_exclusion() {
    let project = TempProject::new();
    project.write(
        "main.omg",
        r#"data Marker { }
machine Marker::exclude_crash(&mut self, cause: u8) { }

data Main { marker: Marker; }
machine Main::main(&mut self) {
    self.marker.exclude_crash(0);
}
const ANSWER: u32 = 42;
"#,
    );
    project.write(
        "build.omg",
        "machine build(builder: &mut Build) { builder.application(\"same-named-exclusion\"); }\n",
    );

    let checked = compile_to_checked(CheckedCompileRequest::new(&project.main(), None))
        .expect("an authored same-named machine must not masquerade as the toolchain marker");
    assert!(checked.behavior_exclusions().is_empty());
}

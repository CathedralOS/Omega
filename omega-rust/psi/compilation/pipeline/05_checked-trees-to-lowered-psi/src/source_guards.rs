//! Source guards on what lowering may read and refuse. Lowering consumes
//! stage 04's plans and checked facts; it never re-reads the authored program,
//! re-enters checking to reconstruct a decision checking already recorded, or
//! re-validates the plan it receives. Split needles keep the guards' own
//! source from matching forbidden names.

use std::fs;
use std::path::Path;

/// Every `(file, line number, line)` of this crate's non-test sources, outside
/// line comments.
fn source_lines() -> Vec<(String, usize, String)> {
    lines_under(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src"))
}

/// Every `(file, line number, line)` of non-test sources below `source_root`,
/// outside line comments.
fn lines_under(source_root: &Path) -> Vec<(String, usize, String)> {
    let mut directories = vec![source_root.to_path_buf()];
    let mut lines = Vec::new();
    while let Some(directory) = directories.pop() {
        let entries = fs::read_dir(&directory)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", directory.display()));
        for entry in entries {
            let path = entry
                .unwrap_or_else(|error| panic!("cannot list {}: {error}", directory.display()))
                .path();
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default();
            if path.is_dir() {
                if !name.starts_with('.') && !matches!(name, "tests" | "examples" | "target") {
                    directories.push(path);
                }
                continue;
            }
            if path.extension().and_then(|extension| extension.to_str()) != Some("rs")
                || name == "tests.rs"
                || name.ends_with("_tests.rs")
            {
                continue;
            }
            let source = fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
            let file = path
                .strip_prefix(source_root)
                .expect("scanned source stays within the crate")
                .display()
                .to_string();
            for (line_number, line) in source.lines().enumerate() {
                if line.trim_start().starts_with("//") {
                    continue;
                }
                lines.push((file.clone(), line_number + 1, line.to_owned()));
            }
        }
    }
    lines
}

/// Every `file:line: name` in this crate's non-test sources, outside line
/// comments, that names one of `forbidden`.
fn violations(forbidden: &[&str]) -> Vec<String> {
    let mut violations = Vec::new();
    for (file, line_number, line) in source_lines() {
        for name in forbidden {
            if line.contains(name) {
                violations.push(format!("{file}:{line_number}: {name}"));
            }
        }
    }
    violations.sort();
    violations
}

/// The contents of each string literal that opens and closes on `line`.
fn string_literals(line: &str) -> Vec<String> {
    let mut literals = Vec::new();
    let mut characters = line.chars();
    while let Some(character) = characters.next() {
        if character != '"' {
            continue;
        }
        let mut literal = String::new();
        while let Some(character) = characters.next() {
            match character {
                '\\' => {
                    if let Some(escaped) = characters.next() {
                        literal.push(escaped);
                    }
                }
                '"' => break,
                _ => literal.push(character),
            }
        }
        literals.push(literal);
    }
    literals
}

#[test]
fn lowering_reads_checked_type_facts_without_reconstruction() {
    let violations = violations(&[
        concat!("declared_place_", "type_raw"),
        concat!("expression_result_", "type_reference"),
        concat!("unwrapped_", "type_reference"),
    ]);
    assert!(
        violations.is_empty(),
        "lowering must read checked place/result/carrier facts: {violations:?}"
    );
}

/// A value lowering needs from the authored program is recorded in a stage
/// 04 plan row or checked fact (a state's `let` locals are
/// `CheckFacts::state_locals`); reference custody is the recorded
/// `CheckFacts::reference_custody`, read through `crate::reference_custody`.
#[test]
fn lowering_reads_no_authored_statements_or_expressions() {
    let violations = violations(&[
        concat!("statement_", "table"),
        concat!("expression_", "table"),
        concat!("Statement", "Node::"),
        concat!("Expression", "Node::"),
        concat!("reference_result_", "custody"),
    ]);
    assert!(
        violations.is_empty(),
        "lowering must read plan rows and checked facts, not authored source: {violations:?}"
    );
}

/// A refusal names what lowering has no route for. A plan that breaks a stage
/// 04 invariant is a stage 04 bug the corpus finds; lowering does not compare
/// the plan against itself or against a recomputation, so no string literal
/// states such a comparison.
#[test]
fn lowering_refusals_do_not_revalidate_the_plan() {
    let phrasings = [
        concat!("disagrees with its ", "checked"),
        concat!("omits or ", "duplicates"),
        concat!("declared more ", "than once"),
        concat!("not canonical ", "source order"),
        concat!("substi", "tuted"),
        concat!("re", "join"),
        concat!("drift", "ed"),
        concat!("changed ", "its"),
        concat!("differs ", "from its"),
        concat!("exchanged ", "its"),
    ];
    let mut violations = Vec::new();
    for (file, line_number, line) in source_lines() {
        for literal in string_literals(&line) {
            for phrasing in phrasings {
                if literal.contains(phrasing) {
                    violations.push(format!("{file}:{line_number}: {literal}"));
                }
            }
        }
    }
    violations.sort();
    assert!(
        violations.is_empty(),
        "lowering refusals must name an unsupported shape, not re-validate the plan: {violations:?}"
    );
}

/// One plan carries every successor transition: a single
/// `CheckedStructuralControlSuccessorPlan` construction mints each edge —
/// scalar terminators leave the rosters empty, the composed route fills them —
/// and `successor_edge` is the only lowering of successor transfers.
#[test]
fn successor_transitions_have_one_plan_and_one_lowering() {
    let compilation_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let mut constructors = Vec::new();
    let mut lowerings = Vec::new();
    for (file, line_number, line) in lines_under(&compilation_root) {
        let trimmed = line.trim_start();
        if trimmed.contains(concat!("CheckedStructuralControl", "SuccessorPlan {"))
            && !trimmed.starts_with(concat!("struct", " "))
            && !trimmed.starts_with(concat!("pub", " struct"))
            && !trimmed.starts_with(concat!("impl", " "))
        {
            constructors.push(format!("{file}:{line_number}: {}", trimmed));
        }
        if trimmed.contains(concat!("fn ", "successor_edge(")) {
            lowerings.push(format!("{file}:{line_number}: {}", trimmed));
        }
    }
    constructors.sort();
    lowerings.sort();
    assert_eq!(
        constructors.len(),
        1,
        "exactly one successor-plan construction: {constructors:?}"
    );
    assert_eq!(
        lowerings.len(),
        1,
        "exactly one lowering of successor transfers: {lowerings:?}"
    );
}

/// One emitter mints every direct machine call: `DirectCall::new` holds the
/// single construction inside `direct_calls.rs` and `emit_direct_call` pushes
/// it, so scalar operands, graph effects, ordinary and composed bodies share
/// the operation. Call structural arguments are minted by the one
/// `structural_argument` constructor; files that resolve call operand lanes
/// never spell a bare `StructuralArgument { ... }` themselves. Boundary
/// applications keep their own operation kind — a boundary is not a machine
/// call.
fn direct_call_construction(line: &str) -> bool {
    let Some(position) = line.find(concat!("Direct", "Call {")) else {
        return false;
    };
    let prefix = &line[..position];
    let fields = &line[position + concat!("Direct", "Call").len()..];
    if fields.trim_start().starts_with("{ .. }") {
        return false;
    }
    // In a let pattern, the first assignment follows the destructuring
    // record. In a value initializer it already precedes this constructor.
    !(prefix.trim_start().starts_with("let ") && !prefix.contains('='))
}

#[test]
fn direct_call_patterns_are_not_mints() {
    assert!(direct_call_construction(concat!(
        "Direct",
        "Call { callee }"
    )));
    assert!(direct_call_construction(concat!(
        "let call = crate::direct_calls::Direct",
        "Call { callee };"
    )));
    assert!(!direct_call_construction(concat!(
        "CheckedProviderRequirementKind::Direct",
        "Call { .. }"
    )));
    assert!(!direct_call_construction(concat!(
        "let CheckedProviderRequirementKind::Direct",
        "Call { diagnostic_name } = &source.kind"
    )));
}

#[test]
fn calls_have_one_emitter_and_one_argument_constructor() {
    let stage_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    // Files that resolve call operand lanes or emit call operations.
    // Structural arguments for views, stores and successor transfers have
    // their own routes and stay outside this set.
    const CALL_FILES: &[&str] = &[
        "unit/attached_unit/parameters.rs",
        "unit/attached_unit/ordinary_calls.rs",
        "unit/attached_unit/operation_frame.rs",
        "unit/attached_unit/operation_frame/calls.rs",
        "unit/attached_unit/operation_frame/boundary_calls.rs",
        "unit/attached_unit/argument_evaluation.rs",
        "emission/operation_emission/calls.rs",
        "emission/operation_emission/boundary_calls.rs",
        "scalar_lowering/effects.rs",
        "scalar_lowering/computations/calls.rs",
        "scalar_lowering/computations/structural_arguments.rs",
        "scalar_lowering/branch_states/structural_values/calls.rs",
    ];
    let mut emitters = Vec::new();
    let mut constructors = Vec::new();
    let mut free_mints = Vec::new();
    for (file, line_number, line) in lines_under(&stage_root) {
        let trimmed = line.trim_start();
        if direct_call_construction(trimmed)
            && !trimmed.starts_with(concat!("struct", " "))
            && !trimmed.starts_with(concat!("pub", " struct"))
            && !trimmed.starts_with(concat!("pub", "(crate) struct"))
            && !trimmed.starts_with(concat!("impl", " "))
        {
            emitters.push(format!("{file}:{line_number}: {}", trimmed));
        }
        if trimmed.contains(concat!("fn ", "structural_argument(")) {
            constructors.push(format!("{file}:{line_number}: {}", trimmed));
        }
        if CALL_FILES.iter().any(|call_file| file.ends_with(call_file))
            && trimmed.contains(concat!(" StructuralArg", "ument {"))
        {
            free_mints.push(format!("{file}:{line_number}: {}", trimmed));
        }
    }
    emitters.sort();
    constructors.sort();
    free_mints.sort();
    assert_eq!(
        emitters.len(),
        1,
        "exactly one direct-call construction: {emitters:?}"
    );
    assert_eq!(
        constructors.len(),
        1,
        "exactly one call structural-argument constructor: {constructors:?}"
    );
    assert!(
        free_mints.is_empty(),
        "call files mint structural arguments through the shared constructor: {free_mints:?}"
    );
}

//! The parser reject fixtures pin the first diagnostic. A focused edit to each fixture must prepare successfully.
//! Each package includes the helper machines its fixture calls.
use omega_parser::{ParseError, ParseErrorKind, parse_file};
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

const SUPPORT: &str =
    "machine go() {}\nmachine zero() -> u32 { 0 }\nmachine other() -> u32 { 2 }\nmachine cleanup() {}\nmachine first() {}\nmachine second() {}\n";

/// Each variant once. `plan` matches without a wildcard, so a new variant cannot compile unplanned.
const KINDS: [ParseErrorKind; 10] = [
    ParseErrorKind::UnexpectedToken,
    ParseErrorKind::UnexpectedEnd,
    ParseErrorKind::IfStatement,
    ParseErrorKind::MissingArmComma,
    ParseErrorKind::SeparatorBeforeBody,
    ParseErrorKind::TransitionNotLast,
    ParseErrorKind::StatementAfterMachine,
    ParseErrorKind::ChainedComparison,
    ParseErrorKind::NestingTooDeep,
    ParseErrorKind::SourceTooLarge,
];

struct Case {
    fixture: &'static str,
    twin: fn(&str) -> Result<String, String>,
}

enum Plan {
    Measured(Case),
    Unmeasured(&'static str),
}

fn plan(kind: ParseErrorKind) -> Plan {
    use ParseErrorKind as K;
    let case = match kind {
        K::UnexpectedToken => Case {
            fixture: "const_without_value",
            twin: |s| edit(s, &[("u32;", "u32 = 1;")]),
        },
        K::UnexpectedEnd => Case {
            fixture: "unclosed_body",
            twin: |s| edit(s, &[("go();\n", "go();\n}\n")]),
        },
        K::IfStatement => Case {
            fixture: "if_statement",
            twin: |s| edit(s, &[("if ready {\n        go();\n    }", "transition ready { true -> go(), false -> go() }")]),
        },
        K::MissingArmComma => Case {
            fixture: "missing_arm_comma",
            twin: |s| edit(s, &[("zero()\n", "zero(),\n")]),
        },
        K::SeparatorBeforeBody => Case {
            fixture: "semicolon_before_body",
            twin: |s| edit(s, &[("n > 0;\n", "n > 0\n")]),
        },
        K::TransitionNotLast => Case {
            fixture: "transition_not_last",
            twin: |s| {
                edit(
                    s,
                    &[(
                        "    transition n { 0 -> zero(), _ -> other() }\n    cleanup();\n",
                        "    cleanup();\n    transition n { 0 -> zero(), _ -> other() }\n",
                    )],
                )
            },
        },
        K::StatementAfterMachine => Case {
            fixture: "statement_after_nested_machine",
            twin: |s| {
                edit(
                    s,
                    &[
                        ("    }\n\n    second();\n", "    }\n"),
                        ("first();\n", "first();\n    second();\n"),
                    ],
                )
            },
        },
        K::ChainedComparison => Case {
            fixture: "chained_equality_comparison",
            twin: |s| edit(s, &[("a == b != c", "(a == b) != c")]),
        },
        // The twin binds the free name and reduces the expression to one parenthesized group.
        K::NestingTooDeep => Case {
            fixture: "nesting_too_deep",
            twin: |s| {
                let deep = format!("{}a{}", "(".repeat(200), ")".repeat(200));
                edit(s, &[("machine f()", "machine f(a: u32)"), (deep.as_str(), "(a)")])
            },
        },
        K::SourceTooLarge => {
            return Plan::Unmeasured(
                "needs more than u32::MAX tokens in one file (over 4 GiB of source plus its token list); \
                 only the L1 unit test token_indices_report_the_first_unrepresentable_position reaches it",
            );
        }
    };
    Plan::Measured(case)
}

fn edit(
    source: &str,
    edits: &[(&str, &str)],
) -> Result<String, String> {
    let mut text = source.to_owned();
    for &(from, to) in edits {
        let count = text.matches(from).count();
        if count != 1 {
            return Err(format!("twin edit {from:?} occurs {count} times"));
        }
        text = text.replacen(from, to, 1);
    }
    Ok(text)
}

/// Render the CLI expectation from the parser's versioned diagnostic.
fn expected(
    kind: ParseErrorKind,
    actual: &ParseError,
    file: &str,
    dump: &str,
) -> Result<String, String> {
    let mut lines = dump.lines();
    let prefix = format!("error {kind:?} at ");
    let (location, span) = lines
        .next()
        .and_then(|header| header.strip_prefix(&prefix))
        .and_then(|position| position.split_once(" bytes "))
        .ok_or("invalid golden diagnostic header")?;
    let phrase = lines
        .find_map(|line| line.strip_prefix("  expected: "))
        .ok_or("missing expected phrase")?;
    let reason = lines
        .next()
        .ok_or("missing golden diagnostic reason")?
        .trim_start();
    let actual_span = actual.get_span();
    let actual_range = format!("{}..{}", actual_span.get_start_offset(), actual_span.get_end_offset());
    if actual.get_kind() != kind || actual_range != span || actual.get_expected() != phrase || actual.get_kind().get_message(actual.get_expected()) != reason {
        return Err(format!("expected {kind:?} ({phrase}) at {span}: {reason}; parser returned {actual:?}"));
    }
    Ok(format!("{file}:{location}: parser: {reason} (bytes {span})\n"))
}

fn omega(
    directory: &Path,
    action: &str,
    file: &str,
) -> Result<Output, String> {
    Command::new(env!("CARGO_BIN_EXE_omega"))
        .current_dir(directory)
        .args(["source", action, "support.omg", file])
        .output()
        .map_err(|error| format!("could not run omega source {action}: {error}"))
}

/// Splits `file:line:column: producer: reason (bytes start..end)` into its observed parts.
fn split_rendered(rendered: &str) -> Option<(&str, &str, &str, &str, &str)> {
    let (file, rest) = rendered.split_once(':')?;
    let (_line, rest) = rest.split_once(':')?;
    let (_column, rest) = rest.split_once(": ")?;
    let (producer, rest) = rest.split_once(": ")?;
    let (reason, span) = rest.rsplit_once(" (bytes ")?;
    let (start, end) = span.strip_suffix(')')?.split_once("..")?;
    Some((file, producer, reason, start, end))
}

fn measure(
    kind: ParseErrorKind,
    case: &Case,
    directory: &Path,
) -> Result<String, String> {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../layers/01_tokens-to-syntax-trees/fixtures/reject");
    let read = |extension: &str| {
        let path = fixtures.join(format!("{}.{extension}", case.fixture));
        fs::read_to_string(&path).map_err(|error| format!("could not read {}: {error}", path.display()))
    };
    let bad = read("omg")?;
    let good = (case.twin)(&bad)?;
    let file = format!("{}.omg", case.fixture);
    let tokens = omega_lexer::lex(bad.as_bytes()).map_err(|error| format!("negative twin stopped in the lexer: {error}"))?;
    let refusal = parse_file(bad.as_bytes(), &tokens).err().ok_or("negative twin parsed")?;
    let expected = expected(kind, &refusal, &file, &read("expected")?)?;
    let path = directory.join(&file);
    let write = |text: &str| fs::write(&path, text).map_err(|error| format!("could not write {}: {error}", path.display()));

    write(good.as_str())?;
    let twin = omega(directory, "prepare", &file)?;
    if !(twin.status.success() && twin.stdout.is_empty() && twin.stderr.is_empty()) {
        return Err(format!(
            "positive twin did not prepare silently ({}): {}",
            twin.status,
            String::from_utf8_lossy(&twin.stderr).trim_end()
        ));
    }

    write(bad.as_str())?;
    let prepared = omega(directory, "prepare", &file)?;
    let lowered = omega(directory, "lower", &file)?;
    let rendered = String::from_utf8_lossy(&prepared.stderr).into_owned();
    if prepared.status.code() != Some(1) || !prepared.stdout.is_empty() || rendered != expected {
        return Err(format!("source prepare gave {} with {rendered:?}, expected {expected:?}", prepared.status));
    }
    if lowered.status.code() != Some(1) || !lowered.stdout.is_empty() || lowered.stderr != prepared.stderr {
        return Err(format!(
            "source lower did not repeat the parser stop ({}): {:?}",
            lowered.status,
            String::from_utf8_lossy(&lowered.stderr)
        ));
    }
    let (file, producer, reason, start, end) = split_rendered(rendered.trim_end()).ok_or_else(|| format!("unparsable diagnostic {rendered:?}"))?;
    if producer != "parser" {
        return Err(format!("first producer is {producer:?}, not the parser"));
    }
    let observed = refusal.get_kind();
    Ok(format!("OMEGA_DIAGNOSTIC_KIND\tL1\t{observed:?}\t{file}\t{start}\t{end}\t{reason}"))
}

#[test]
fn public_diagnostics_match_the_parse_errors_inventory() {
    for (index, kind) in KINDS.iter().enumerate() {
        assert!(!KINDS[..index].contains(kind), "{kind:?} is listed twice");
    }
    let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../target/parse-error-cases")
        .join(std::process::id().to_string());
    fs::create_dir_all(&directory).unwrap();
    fs::write(directory.join("support.omg"), SUPPORT).unwrap();
    let mut failures = Vec::new();
    for kind in KINDS {
        match plan(kind) {
            Plan::Measured(case) => match measure(kind, &case, &directory) {
                Ok(evidence) => println!("{evidence}"),
                Err(failure) => failures.push(format!("{kind:?} ({}.omg): {failure}", case.fixture)),
            },
            Plan::Unmeasured(reason) => println!("OMEGA_DIAGNOSTIC_UNMEASURED\tL1\t{kind:?}\t{reason}"),
        }
    }
    fs::remove_dir_all(&directory).unwrap();
    assert!(failures.is_empty(), "{} parse-error kinds failed:\n{}", failures.len(), failures.join("\n"));
}

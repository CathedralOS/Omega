//! Exercise each diagnostic through the package command of its first stopping layer.
//!
//! L0-L4 inputs use `source prepare`: each positive twin prepares, and `source lower` must
//! repeat the earlier stop unchanged. L5 inputs use `source lower`: each positive twin lowers,
//! and the refused input must already pass `source prepare`, so lowering owns the failure.
use std::{fs, path::Path, process::Command};
#[path = "support/diagnostic_occurrence.rs"]
mod diagnostic_occurrence;

struct Case<'a> {
    stage: &'static str,
    producer: &'static str,
    good: &'a [u8],
    bad: &'a [u8],
    occurrence: &'a [u8],
    reason: &'static str,
}

fn check(case: Case<'_>) {
    let thread = std::thread::current();
    let name = thread.name().unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../target/frontend-diagnostic-cases");
    let directory = root.join(format!("{}-{name}", std::process::id()));
    fs::create_dir_all(&directory).unwrap();
    fs::write(directory.join("other.omg"), b"machine other() {}").unwrap();
    let path = directory.join("case.omg");
    let invoke = |action: &str| {
        Command::new(env!("CARGO_BIN_EXE_omega"))
            .current_dir(&directory)
            .args(["source", action, "other.omg", "case.omg"])
            .output()
            .unwrap()
    };
    let after_preparation = case.stage == "L5";
    let (action, neighbor_action) = if after_preparation { ("lower", "prepare") } else { ("prepare", "lower") };
    fs::write(&path, case.good).unwrap();
    let good = invoke(action);
    fs::write(&path, case.bad).unwrap();
    let bad = invoke(action);
    let neighbor = invoke(neighbor_action);
    fs::remove_file(path).unwrap();
    fs::remove_file(directory.join("other.omg")).unwrap();
    fs::remove_dir(directory).unwrap();
    assert!(
        good.status.success() && good.stdout.is_empty() && good.stderr.is_empty(),
        "positive twin under source {action}: {}",
        String::from_utf8_lossy(&good.stderr)
    );
    let start = if case.occurrence.is_empty() {
        case.bad.len()
    } else {
        case.bad
            .windows(case.occurrence.len())
            .rposition(|bytes| bytes == case.occurrence)
            .unwrap()
    };
    let end = start + case.occurrence.len();
    let prefix = &case.bad[..start];
    let line = prefix.iter().filter(|&&b| b == b'\n').count() + 1;
    let column = prefix.rsplit(|&b| b == b'\n').next().unwrap().len() + 1;
    let expected = format!("case.omg:{line}:{column}: {}: {} (bytes {start}..{end})", case.producer, case.reason);
    let actual = String::from_utf8(bad.stderr).unwrap();
    assert_eq!(bad.status.code(), Some(1), "source {action}: {actual}");
    assert!(bad.stdout.is_empty());
    let rendered = actual.strip_suffix('\n').expect("diagnostic must end with a newline");
    assert_eq!(rendered, expected);
    let controls = if after_preparation {
        assert!(
            neighbor.status.success() && neighbor.stdout.is_empty() && neighbor.stderr.is_empty(),
            "the refused L5 input must already prepare: {}",
            String::from_utf8_lossy(&neighbor.stderr)
        );
        "good-twin-lowers,bad-input-prepares"
    } else {
        assert_eq!(neighbor.status.code(), Some(1), "source lower must stop at the earlier layer");
        assert!(neighbor.stdout.is_empty());
        assert_eq!(
            String::from_utf8_lossy(&neighbor.stderr),
            actual,
            "source lower must repeat the earlier stop unchanged"
        );
        "good-twin-prepares,lower-repeats-stop"
    };
    println!(
        "OMEGA_FRONTEND\t{}\t{name}\t{action}\t{}\tcase.omg\t{line}\t{column}\t{start}\t{end}\t{controls}\t{}\t{}\t{}",
        case.stage,
        case.producer,
        spec_corpus::diagnostic_field(case.reason),
        spec_corpus::diagnostic_field(rendered),
        diagnostic_occurrence::fields("case.omg", case.bad, start, case.occurrence)
    );
}

#[test]
fn deeply_grouped_result_reports_the_type_mismatch_at_the_result() {
    let group = format!("{}a{}", "(".repeat(120), ")".repeat(120));
    let good = format!("machine f(a:bool)->bool{{{group}}}");
    let bad = format!("machine f(a:bool)->u32{{{group}}}");
    check(Case {
        stage: "L3",
        producer: "typer",
        good: good.as_bytes(),
        bad: bad.as_bytes(),
        occurrence: group.as_bytes(),
        reason: "expected u32, found bool",
    });
}

#[test]
fn parser_accepted_nesting_reaches_preparation() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../target/frontend-diagnostic-cases");
    let directory = root.join(format!("{}-grouped-expression", std::process::id()));
    fs::create_dir_all(&directory).unwrap();
    let file = directory.join("nested.omg");
    // The parser limits nesting to 128. These sources exercise both the reported failure
    // and a deeper accepted expression on the normal command's main thread.
    for depth in [40, 120] {
        let source = format!("machine f(a:u32) -> u32 {{{}a{}}}", "(".repeat(depth), ")".repeat(depth));
        fs::write(&file, source).unwrap();
        for action in ["resolve", "type", "prepare"] {
            let output = Command::new(env!("CARGO_BIN_EXE_omega"))
                .current_dir(&directory)
                .args(["source", action, "nested.omg"])
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "source {action} at grouping depth {depth} exited {}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
    fs::remove_file(file).unwrap();
    fs::remove_dir(directory).unwrap();
}

#[test]
fn independent_file_errors_are_reported_together_in_input_order() {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../target/frontend-diagnostic-cases")
        .join(format!("{}-independent-files", std::process::id()));
    fs::create_dir_all(&directory).unwrap();
    fs::write(directory.join("incomplete.omg"), b"machine f() {").unwrap();
    fs::write(directory.join("invalid.omg"), [0xff]).unwrap();
    let outputs = ["resolve", "type", "prepare", "lower"].map(|action| {
        Command::new(env!("CARGO_BIN_EXE_omega"))
            .current_dir(&directory)
            .args(["source", action, "incomplete.omg", "../outside.omg", "invalid.omg"])
            .output()
            .unwrap()
    });
    fs::remove_file(directory.join("incomplete.omg")).unwrap();
    fs::remove_file(directory.join("invalid.omg")).unwrap();
    fs::remove_dir(directory).unwrap();
    for output in outputs {
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert_eq!(
            String::from_utf8(output.stderr)
                .unwrap()
                .lines()
                .collect::<Vec<_>>(),
            [
                "incomplete.omg:1:14: parser: the source ended where `}` was expected (bytes 13..13)",
                "../outside.omg: expected a path within the current package",
                "invalid.omg:1:1: lexer: source is not valid UTF-8 (bytes 0..1)",
            ]
        );
    }
}

#[test]
fn invalid_byte_after_crlf() {
    check(Case {
        stage: "L0",
        producer: "lexer",
        good: b"// first\r\n// fine",
        bad: b"// first\r\n// \xff",
        occurrence: b"\xff",
        reason: "source is not valid UTF-8",
    });
}
#[test]
fn truncated_utf8_after_multibyte_comment_text() {
    // After CRLF, `//` and a complete two-byte scalar put the tail at byte column 5, not 4.
    // Completing only the final scalar repairs the entire package.
    check(Case {
        stage: "L0",
        producer: "lexer",
        good: b"machine f()->bool{true}\r\n//\xc3\xa9\xe6\x97\xa5",
        bad: b"machine f()->bool{true}\r\n//\xc3\xa9\xe6\x97",
        occurrence: b"\xe6\x97",
        reason: "source is not valid UTF-8",
    });
}
#[test]
fn missing_closing_brace_at_eof() {
    check(Case {
        stage: "L1",
        producer: "parser",
        good: b"machine ok() {\r\n}",
        bad: b"machine ok() {\r\n",
        occurrence: b"",
        reason: "the source ended where `}` was expected",
    });
}
#[test]
fn unresolved_call_in_the_second_file() {
    check(Case {
        stage: "L2",
        producer: "resolver",
        good: b"machine use_it() { other(); }",
        bad: b"machine use_it() { missing(); }",
        occurrence: b"missing",
        reason: "unresolved name missing",
    });
}
#[test]
fn wrong_return_type_during_initial_typing() {
    check(Case {
        stage: "L3",
        producer: "typer",
        good: "// π\r\nmachine f()->bool { true }".as_bytes(),
        bad: "// π\r\nmachine f()->u32 { true }".as_bytes(),
        occurrence: b"true",
        reason: "expected u32, found bool",
    });
}

const DIVIDE: &str = "machine divide(n:u64,d:u64)->u64 requires d != 0 reaches none crashes none terminates {n / d}\nconst SIZE:u64=divide(12,3);\nmachine carry(x:[u8; SIZE])->[u8; 4]{x}";
#[test]
fn wrong_array_identity_after_evaluation() {
    let bad = DIVIDE.replace("12,3", "12,2");
    // L3 owns the refusal; its producer records that L4 already evaluated SIZE.
    check(Case {
        stage: "L3",
        producer: "typer (after static evaluation)",
        good: DIVIDE.as_bytes(),
        bad: bad.as_bytes(),
        occurrence: b"x",
        reason: "expected [u8; 4], found [u8; 6]",
    });
}
#[test]
fn a_domain_alias_with_no_matching_evaluated_width_identifies_its_target() {
    let good = "machine width()->u64{8u64} machine wide()->u64{16u64}
domain u<width()>::Ready; domain u<wide()>::Ready;
domain u8::Selected=u8::Ready; machine read(x:u8 in Selected){}";
    let bad = good.replace("{8u64}", "{32u64}");
    check(Case {
        stage: "L3",
        producer: "typer (after static evaluation)",
        good: good.as_bytes(),
        bad: bad.as_bytes(),
        occurrence: b"u8::Ready",
        reason: "no visible domain on the written carrier",
    });
}
#[test]
fn failed_precondition_during_static_evaluation() {
    let bad = DIVIDE.replace("12,3", "12,0");
    check(Case {
        stage: "L4",
        producer: "preparation",
        good: DIVIDE.as_bytes(),
        bad: bad.as_bytes(),
        occurrence: b"divide(12,0)",
        reason: "static evaluation cannot admit this invocation: the selected integer precondition is false for the supplied arguments",
    });
}
#[test]
fn unavailable_target_for_a_required_extent() {
    let bad = r"use omega\language\core; const VALUE:u64=core\size_of::<u16>(); machine consume(bytes:[u8; VALUE]){}";
    let good = bad.replace(r"core\size_of::<u16>()", "2u64");
    check(Case {
        stage: "L4",
        producer: "preparation (target input)",
        good: good.as_bytes(),
        bad: bad.as_bytes(),
        occurrence: br"core\size_of::<u16>()",
        reason: "static evaluation cannot admit this invocation: the layout query requires a selected target",
    });
}

#[test]
fn a_crashing_helper_cannot_supply_a_total_predicate_application() {
    // A total predicate term needs a terminating helper with no crash route
    // (wiki/spec/language/state_contracts.md). Changing only the helper's published
    // ceiling to `Trap` moves the stop to the application inside the domain clause.
    // The occurrence is the last `predicate()`, after `&&`, not the declaration's name.
    let good = "machine predicate()->bool reaches none crashes none terminates {true}\r\ndomain i32::Guarded requires true && predicate();";
    let bad = good.replace("crashes none", "crashes Trap");
    check(Case {
        stage: "L5",
        producer: "lowering",
        good: good.as_bytes(),
        bad: bad.as_bytes(),
        occurrence: b"predicate()",
        reason: "predicate application requires a terminating helper with an empty crash ceiling",
    });
}

#[test]
fn a_transparent_alias_cannot_change_its_targets_multiplicity() {
    check(Case {
        stage: "L3",
        producer: "typer",
        good: b"// alias\r\ndata Alias [copy] = u64;",
        bad: b"// alias\r\ndata Alias [linear] = u64;",
        occurrence: b"data Alias [linear] = u64;",
        reason: "a transparent alias cannot change its target's multiplicity",
    });
}

#[test]
fn a_parameterized_alias_attribute_stops_in_the_parser() {
    check(Case {
        stage: "L1",
        producer: "parser",
        good: b"// alias\r\ndata Alias [copy] = u64;",
        bad: b"// alias\r\ndata Alias [copy,carry(suspension:allowed,cpu:any,thread:any,address:movable,)] = u64;",
        occurrence: b"(",
        reason: "expected `,` or `]`",
    });
}

//! Run the package commands on a repaired source and the source that should be refused.
use std::{fs, path::Path, process::Command};

/// Resolve the same package identity and source order that the package command receives.
pub fn resolve_root_package<'text>(files: &[(&'text str, &'text str)]) -> Result<omega_toolchain::SourcePackage<'text>, omega_resolver::NameError> {
    let parses: Vec<_> = files
        .iter()
        .map(|(file, text)| {
            let tokens = omega_lexer::lex(text.as_bytes()).unwrap_or_else(|e| panic!("{file}: {e:?}"));
            let tree = omega_parser::parse_file(text.as_bytes(), &tokens).unwrap_or_else(|e| panic!("{file}: {e:?}"));
            (tokens, tree)
        })
        .collect();
    let sources: Vec<_> = files
        .iter()
        .zip(&parses)
        .map(|(&(logical_path, text), (tokens, tree))| omega_resolver::StageSource {
            package: "root",
            source: omega_resolver::ParsedSource {
                logical_path,
                text,
                tokens,
                tree,
            },
        })
        .collect();
    omega_toolchain::resolve_source_package(&sources)
}

pub struct Case<'a> {
    pub layer: &'static str,
    pub producer: &'static str,
    pub good: &'a [(&'a str, &'a str)],
    pub bad: &'a [(&'a str, &'a str)],
    pub file: &'a str,
    pub occurrence: &'a str,
    pub reason: &'a str,
    pub rule: &'a str,
}

pub fn check(case: Case<'_>) {
    check_input(case, None);
}

pub fn check_input(
    case: Case<'_>,
    pending: Option<&str>,
) {
    let thread = std::thread::current();
    let name = thread.name().unwrap();
    let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../target/diagnostic-twins")
        .join(format!("{}-{}-{name}", std::process::id(), case.layer));
    fs::create_dir_all(&directory).unwrap();
    for (variant, files) in [("good", case.good), ("bad", case.bad)] {
        let directory = directory.join(variant);
        for (file, text) in files {
            let path = directory.join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        }
    }
    let run = |files: &[(&str, &str)], variant: &str, action: &str| {
        Command::new(env!("CARGO_BIN_EXE_omega"))
            .current_dir(directory.join(variant))
            .args(["source", action])
            .args(files.iter().map(|(file, _)| file))
            .output()
            .unwrap()
    };
    let good = run(case.good, "good", "prepare");
    let bad = run(case.bad, "bad", "prepare");
    let before = match case.layer {
        "L2" => case
            .bad
            .chunks(1)
            .map(|file| run(file, "bad", "parse"))
            .collect(),
        "L3" => vec![run(case.bad, "bad", "resolve")],
        "L4" if pending.is_some() => Vec::new(),
        "L4" => vec![run(case.bad, "bad", "type")],
        other => panic!("unknown layer {other}"),
    };
    let owner_command = match case.layer {
        "L2" => Some("resolve"),
        "L3" => Some("type"),
        _ => None,
    };
    let first = owner_command.map(|action| run(case.bad, "bad", action));
    let neighbor = run(case.bad, "bad", "lower");
    fs::remove_dir_all(directory).unwrap();
    for upstream in before {
        assert!(
            upstream.status.success() && upstream.stdout.is_empty() && upstream.stderr.is_empty(),
            "{name}: the preceding stage refused the original input: {}",
            String::from_utf8_lossy(&upstream.stderr)
        );
    }
    if let Some(first) = first {
        assert_eq!(first.status.code(), Some(1));
        assert!(first.stdout.is_empty());
        assert_eq!(first.stderr, bad.stderr, "{name}: preparation must preserve the first refusal");
    }
    assert!(
        good.status.success() && good.stdout.is_empty() && good.stderr.is_empty(),
        "{name}: repaired package failed: {}",
        String::from_utf8_lossy(&good.stderr)
    );
    let (_, source) = case
        .bad
        .iter()
        .find(|(file, _)| *file == case.file)
        .expect("refusal belongs to the supplied package");
    let start = source
        .rfind(case.occurrence)
        .expect("expected occurrence is in the refused input");
    let end = start + case.occurrence.len();
    let origin = if case.layer == "L3" {
        let package = resolve_root_package(case.bad).unwrap();
        let error = omega_typer::type_source_package(&package).expect_err("negative twin typed");
        assert!(!error.waiting_for_static_value(), "{name}: typing requested evaluation");
        assert_eq!((error.package.as_str(), error.file.as_str()), ("root", case.file), "{name}");
        assert_eq!(error.span, omega_lexer::SourceSpan::new(start, end), "{name}");
        assert_eq!(error.message, case.reason, "{name}");
        let origin = error.diagnostic_origin();
        format!(
            "\t{}\t{}\t{}",
            spec_corpus::diagnostic_field(&origin.file().replace('\\', "/")),
            origin.line(),
            origin.column()
        )
    } else {
        String::new()
    };
    // Count source bytes independently of the compiler's line index.
    let prefix = &source[..start];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let column = prefix.rsplit('\n').next().unwrap().len() + 1;
    let expected = format!("{}:{line}:{column}: {}: {} (bytes {start}..{end})", case.file, case.producer, case.reason);
    assert_eq!(bad.status.code(), Some(1), "{name}: {}", String::from_utf8_lossy(&bad.stderr));
    assert!(bad.stdout.is_empty());
    let stderr = std::str::from_utf8(&bad.stderr)
        .unwrap()
        .strip_suffix('\n')
        .expect("CLI line terminator");
    assert_eq!(stderr, expected, "{name}");
    assert_eq!(neighbor.status.code(), Some(1));
    assert!(neighbor.stdout.is_empty());
    assert_eq!(neighbor.stderr, bad.stderr, "{name}: lowering must repeat the original refusal");
    // A call chain can continue on another line of the same compiler diagnostic.
    let (_, detail) = stderr.split_once(&format!(": {}: ", case.producer)).unwrap();
    let (detail, _) = detail.rsplit_once(" (bytes ").unwrap();
    let length = detail.len();
    let reason = spec_corpus::diagnostic_field(detail);
    let input = match (case.layer, pending) {
        ("L2", None) => "parsed".to_owned(),
        ("L3", None) => "resolved".to_owned(),
        ("L4", None) => "typed".to_owned(),
        ("L4", Some(occurrence)) => format!("pending {occurrence}"),
        other => panic!("invalid upstream observation {other:?}"),
    };
    println!(
        "OMEGA_RULE_DIAGNOSTIC\t{}\t{name}\t{}\t{}\t{}\t{start}\t{end}\t{line}\t{column}\t{input}{origin}\t{length}\t{reason}",
        case.layer, case.rule, case.producer, case.file
    );
}

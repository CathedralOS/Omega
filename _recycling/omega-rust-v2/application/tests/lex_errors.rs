//! Each lexical refusal uses the offending bytes from its golden fixture and a repaired package that prepares.
//! Invalid UTF-8 uses a raw byte. Both package commands must repeat the lexer's exact location and reason.
#[path = "support/diagnostic_occurrence.rs"]
mod diagnostic_occurrence;

use omega_lexer::{LexErrorKind, lex, lex_into};
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

/// Where the offending spelling of a case comes from.
enum Golden {
    /// A reject fixture under `layers/00_bytes-to-tokens/tests/fixtures`.
    Fixture(&'static str),
    /// A byte-level lexer test whose input cannot be a readable fixture.
    ByteCase {
        test: &'static str,
        occurrence: &'static [u8],
        message: &'static str,
    },
}

impl Golden {
    fn provenance(&self) -> String {
        match self {
            Golden::Fixture(name) => format!("omega-lexer tests/fixtures/{name}"),
            Golden::ByteCase { test, .. } => format!("omega-lexer {test}"),
        }
    }
}

struct Twin {
    golden: Golden,
    good: &'static [u8],
    bad: &'static [u8],
}

const KINDS: &[LexErrorKind] = &[
    LexErrorKind::InvalidUtf8,
    LexErrorKind::NonAsciiOutsideCommentOrQuoted,
    LexErrorKind::UnknownCharacter,
    LexErrorKind::UnterminatedBlockComment,
    LexErrorKind::UnterminatedQuoted,
    LexErrorKind::NewlineInQuoted,
    LexErrorKind::CodepointEscape,
    LexErrorKind::UnknownEscape,
    LexErrorKind::InvalidHexEscape,
    LexErrorKind::RawLiteralPrefix,
    LexErrorKind::MissingRadixDigits,
    LexErrorKind::DigitOutsideRadix,
    LexErrorKind::MissingExponentDigits,
    LexErrorKind::InvalidLiteralSuffix,
];

fn twin(
    golden: Golden,
    good: &'static [u8],
    bad: &'static [u8],
) -> Twin {
    Twin { golden, good, bad }
}

fn input(kind: LexErrorKind) -> Twin {
    use Golden::{ByteCase, Fixture};
    use LexErrorKind::*;
    match kind {
        InvalidUtf8 => twin(
            ByteCase {
                test: "byte_cases::invalid_utf8_rejects",
                occurrence: b"\xff",
                message: "source is not valid UTF-8",
            },
            b"// \xc3\xa9\nmachine f()->bool{true}",
            b"// \xff\nmachine f()->bool{true}",
        ),
        NonAsciiOutsideCommentOrQuoted => twin(
            Fixture("words/reject/non_ascii_identifier"),
            "// header\r\n/* 界 */ machine cafe()->bool{true}".as_bytes(),
            "// header\r\n/* 界 */ machine café()->bool{true}".as_bytes(),
        ),
        UnknownCharacter => twin(Fixture("punctuation/reject/unknown_character_question"), b"machine f()->bool{true}", b"machine f()->bool{true?}"),
        UnterminatedBlockComment => twin(
            Fixture("whitespace_and_comments/reject/unterminated_block_comment"),
            b"machine f()->bool{true}\n/* never closed */\n",
            b"machine f()->bool{true}\n/* never closed\n",
        ),
        UnterminatedQuoted => twin(Fixture("quoted_literals/reject/unterminated_quoted"), br#"const S:[u8;3]="abc";"#, br#"const S:[u8;3]="abc;"#),
        NewlineInQuoted => {
            // The positive twin spells the line break as the escape `\n`: seven bytes either way.
            twin(Fixture("quoted_literals/reject/newline_in_quoted"), br#"const S:[u8;7]="abc\ndef";"#, b"const S:[u8;7]=\"abc\ndef\";")
        }
        CodepointEscape => twin(Fixture("quoted_literals/reject/codepoint_escape"), br#"const S:[u8;1]="\x41";"#, br#"const S:[u8;1]="\u{41}";"#),
        UnknownEscape => twin(Fixture("quoted_literals/reject/unknown_escape"), br#"const S:[u8;3]="a\tb";"#, br#"const S:[u8;3]="a\qb";"#),
        InvalidHexEscape => twin(Fixture("quoted_literals/reject/invalid_hex_escape"), br#"const S:[u8;1]="\x21";"#, br#"const S:[u8;1]="\xZ1";"#),
        RawLiteralPrefix => twin(Fixture("quoted_literals/reject/raw_literal_prefix"), br#"const S:[u8;3]="abc";"#, br#"const S:[u8;3]=r"abc";"#),
        MissingRadixDigits => twin(Fixture("numbers/reject/missing_radix_digits"), b"machine f()->u8{0x1u8}", b"machine f()->u8{0xu8}"),
        DigitOutsideRadix => twin(Fixture("numbers/reject/digit_outside_radix_binary"), b"machine f()->u8{0b101u8}", b"machine f()->u8{0b102u8}"),
        MissingExponentDigits => twin(Fixture("numbers/reject/missing_exponent_digits"), b"const A:f64=1e+5;", b"const A:f64=1e+;"),
        InvalidLiteralSuffix => twin(Fixture("numbers/reject/invalid_suffix_usize"), b"machine f()->u64{7u64}", b"machine f()->u64{7usize}"),
    }
}

/// Reads the fixture's expected kind, byte range and message, checks them against the enum and
/// returns the offending bytes.
fn fixture_occurrence(
    name: &str,
    kind: LexErrorKind,
) -> Result<Vec<u8>, String> {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("../layers/00_bytes-to-tokens/tests/fixtures");
    let source = fs::read(directory.join(format!("{name}.omg"))).map_err(|e| format!("{name}.omg: {e}"))?;
    let expected = fs::read_to_string(directory.join(format!("{name}.expected"))).map_err(|e| format!("{name}.expected: {e}"))?;
    let lines: Vec<_> = expected.lines().collect();
    let header = format!("error {kind:?} at ");
    let range = lines
        .first()
        .and_then(|line| line.strip_prefix(&header))
        .and_then(|rest| rest.split_once(" bytes "))
        .and_then(|(_, range)| range.split_once(".."))
        .and_then(|(start, end)| Some((start.parse::<usize>().ok()?, end.parse::<usize>().ok()?)));
    let Some((start, end)) = range else {
        return Err(format!("{name}.expected does not name {kind:?} with a byte range: {:?}", lines.first()));
    };
    if lines.get(2).map(|line| line.trim()) != Some(kind.get_message()) {
        return Err(format!("{name}.expected message differs from {kind:?}: {:?}", lines.get(2)));
    }
    source
        .get(start..end)
        .map(<[u8]>::to_vec)
        .ok_or_else(|| format!("{name}.expected range {start}..{end} is outside the fixture"))
}

fn invoke(
    directory: &Path,
    action: &str,
) -> Output {
    Command::new(env!("CARGO_BIN_EXE_omega"))
        .current_dir(directory)
        .args(["source", action, "case.omg"])
        .output()
        .expect("the omega binary must start")
}

/// Measures one variant and returns its evidence line.
fn measure(
    kind: LexErrorKind,
    twin: &Twin,
    directory: &Path,
) -> Result<String, String> {
    let occurrence = match twin.golden {
        Golden::Fixture(name) => fixture_occurrence(name, kind)?,
        Golden::ByteCase { occurrence, message, .. } => {
            if kind.get_message() != message {
                return Err(format!("expected {message:?}, got {:?}", kind.get_message()));
            }
            occurrence.to_vec()
        }
    };
    if occurrence.is_empty() {
        return Err("expected a nonempty offending spelling".into());
    }
    let found: Vec<_> = twin
        .bad
        .windows(occurrence.len())
        .enumerate()
        .filter(|(_, bytes)| *bytes == occurrence.as_slice())
        .map(|(start, _)| start)
        .collect();
    let [start] = found[..] else {
        return Err(format!("the offending spelling {occurrence:?} must occur exactly once, found {found:?}"));
    };
    let end = start + occurrence.len();

    // The first producer: the production lexer itself refuses with this variant and range.
    let refusal = match lex(twin.bad) {
        Err(error) if error.get_kind() == kind && (error.get_span().get_start_offset(), error.get_span().get_end_offset()) == (start, end) => error,
        Err(error) => return Err(format!("the lexer refused with {:?} at {:?}", error.get_kind(), error.get_span())),
        Ok(_) => return Err("the negative source lexes".into()),
    };

    // The reusable-buffer API must report the same first error without leaking partial tokens.
    let mut reused = lex(twin.good).map_err(|e| format!("positive twin did not lex: {e}"))?;
    let reused_result = lex_into(twin.bad, &mut reused);
    if reused_result != Err(refusal) || !reused.is_empty() {
        return Err(format!("lex_into returned {reused_result:?} with {} tokens left, expected {refusal} and none", reused.len()));
    }
    let observed_kind = refusal.get_kind();
    let api = refusal.to_string();
    let expected_api = format!("{} (bytes {start}..{end})", kind.get_message());
    if api != expected_api {
        return Err(format!("lexer API rendered {api:?}, expected {expected_api:?}"));
    }

    let path = directory.join("case.omg");
    fs::write(&path, twin.good).map_err(|e| e.to_string())?;
    let good = invoke(directory, "prepare");
    fs::write(&path, twin.bad).map_err(|e| e.to_string())?;
    let bad = invoke(directory, "prepare");
    let lowered = invoke(directory, "lower");
    let _ = fs::remove_file(&path);

    if !(good.status.success() && good.stdout.is_empty() && good.stderr.is_empty()) {
        return Err(format!("positive twin did not prepare: {}", String::from_utf8_lossy(&good.stderr)));
    }
    let stderr = String::from_utf8(bad.stderr).map_err(|e| format!("non-UTF-8 diagnostic: {e}"))?;
    if bad.status.code() != Some(1) || !bad.stdout.is_empty() {
        return Err(format!("source prepare exited {:?}: {stderr}", bad.status.code()));
    }
    let prefix = &twin.bad[..start];
    let line = prefix.iter().filter(|&&b| b == b'\n').count() + 1;
    let column = prefix.rsplit(|&b| b == b'\n').next().unwrap().len() + 1;
    let expected = format!("case.omg:{line}:{column}: lexer: {expected_api}");
    let observed = stderr
        .strip_suffix('\n')
        .ok_or("lexer diagnostic has no line terminator")?;
    if observed != expected {
        return Err(format!("source prepare rendered {stderr:?}, expected {expected:?}"));
    }
    if lowered.status.code() != Some(1) || !lowered.stdout.is_empty() || lowered.stderr != stderr.as_bytes() {
        return Err(format!(
            "source lower did not repeat the stop: {:?} {}",
            lowered.status.code(),
            String::from_utf8_lossy(&lowered.stderr)
        ));
    }

    let field = spec_corpus::diagnostic_field;
    let witness = diagnostic_occurrence::fields("case.omg", twin.bad, start, &occurrence);
    Ok(format!(
        "OMEGA_DIAGNOSTIC_KIND\tL0\t{kind:?}\t{observed_kind:?}\t{}\t{}\t{}\t{witness}",
        field(&expected),
        field(observed),
        field(&api)
    ))
}

#[test]
fn public_diagnostics_match_the_lex_errors_inventory() {
    for (index, kind) in KINDS.iter().enumerate() {
        assert!(!KINDS[..index].contains(kind), "{kind:?} is listed twice");
    }
    let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../target/lex-error-cases")
        .join(std::process::id().to_string());
    fs::create_dir_all(&directory).unwrap();

    let mut failures = Vec::new();
    for &kind in KINDS {
        let twin = input(kind);
        match measure(kind, &twin, &directory) {
            Ok(evidence) => println!("{evidence}"),
            Err(failure) => failures.push(format!("{kind:?} ({}): {failure}", twin.golden.provenance())),
        }
    }
    let _ = fs::remove_dir(&directory);

    assert!(
        failures.is_empty(),
        "{} lexical diagnostic case(s) failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

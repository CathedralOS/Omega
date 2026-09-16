//! Regression coverage for the numeric evidence indexed text writes consume:
//! effectful call arguments, receiver-call field reads resolved against the
//! caller's storage, and conversion policies the selected scalar plan has no
//! node for (today `Saturating`). A case that cannot carry evidence must keep
//! rejecting — nothing here invents a value or a tighter bound.
use checked_trees::CheckedTrees;
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use tokens_to_syntax_trees::parse_syntax_trees;

fn check(source: &str) -> Result<CheckedTrees, Vec<String>> {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    crate::lower_typed_trees(typed).map_err(|diagnostics| {
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.clone())
            .collect()
    })
}

fn write_machine(signature: &str, statement: &str) -> String {
    format!(
        "domain [u8; 4]::Ascii requires ascii_only(self); {signature} \
         requires output in Ascii ensures output in Ascii {{ {statement} }}"
    )
}

const DIGIT: &str =
    "machine digit(value: u64) -> u8 { ((value % 10 + 48) as u8 in Wrapping) as u8 }";

/// An indexed read is effectful — its element bound comes from the byte
/// carrier when no literal snapshot is live — and it still feeds the call
/// argument evidence the captured callee consumes.
#[test]
fn indexed_read_arguments_bound_the_call_result() {
    let source = format!(
        "data Input {{ bytes: [u8; 4]; }} {DIGIT} {}",
        write_machine(
            "machine write(output: &mut [u8; 4], input: Input, position: u64 [0..=3], unknown: u64)",
            "output[position] = digit((input.bytes[position] as u64));",
        )
    );
    check(&source)
        .map(|_| ())
        .unwrap_or_else(|messages| panic!("indexed read argument keeps its bound: {messages:?}"));
}

/// A second call occurrence in the same statement — an index selector on the
/// target — must not disturb the captured source call, and proving the index
/// itself in range stays with the ranges checker, which this slice does not
/// feed.
#[test]
fn call_in_the_index_position_is_a_separate_occurrence() {
    for (statement, expected_fragment) in [
        ("output[idx()] = digit(unknown);", "cannot prove index"),
        ("output[idx()] = 65;", "cannot prove index"),
    ] {
        let source = format!(
            "machine idx() -> u64 {{ 1 }} {DIGIT} {}",
            write_machine(
                "machine write(output: &mut [u8; 4], position: u64 [0..=3], unknown: u64)",
                statement,
            )
        );
        let Err(messages) = check(&source) else {
            panic!("{statement}: index bound proof is not this slice's job");
        };
        assert!(
            messages
                .iter()
                .any(|message| message.contains(expected_fragment)),
            "{statement}: {messages:?}"
        );
    }
}

/// A nested machine call as a call argument is an authored-shape boundary the
/// checker rejects explicitly; the capture must not pretend otherwise.
#[test]
fn a_nested_call_argument_still_rejects() {
    let source = format!(
        "machine forty_two() -> u64 {{ 42 }} {DIGIT} {}",
        write_machine(
            "machine write(output: &mut [u8; 4], position: u64 [0..=3], unknown: u64)",
            "output[position] = digit(forty_two());",
        )
    );
    let Err(messages) = check(&source) else {
        panic!("a nested value-call argument must not be captured");
    };
    assert!(
        messages
            .iter()
            .any(|message| message.contains("cannot itself be a machine call")),
        "{messages:?}"
    );
}

/// Cast domains nested inside call arguments ride the callee's declared
/// parameter domain: `Saturating` lowers to a plan the captured evaluation
/// can replay, while `Trapping` binary arithmetic has no selected scalar
/// form and fails closed at the ensures contract.
#[test]
fn argument_cast_policies_follow_the_callee_parameter_domain() {
    for (domain, expected) in [("Saturating", true), ("Trapping", false)] {
        let source = format!(
            "machine digit(value: u64 in {domain}) -> u8 {{ ((value % 10 + 48) as u8 in Wrapping) as u8 }} {}",
            write_machine(
                "machine write(output: &mut [u8; 4], position: u64 [0..=3], unknown: u64)",
                &format!("output[position] = digit((unknown as u64 in {domain}));"),
            )
        );
        match (check(&source), expected) {
            (Ok(_), true) => {}
            (Err(messages), false) => assert!(
                messages
                    .iter()
                    .any(|message| message.contains("cannot prove ensures")),
                "{domain}: {messages:?}"
            ),
            (result, _) => panic!("{domain}: {result:?}"),
        }
    }
}

/// A receiver call's `self` field reads resolve against the caller's receiver
/// place: live snapshot first, then declared/carrier bounds. The returned
/// digit then satisfies the element predicate.
#[test]
fn receiver_calls_resolve_self_fields_against_the_caller_place() {
    let source = format!(
        "data Pad {{ cell: u64 [0..=9]; }} \
         machine Pad::digit(&mut self) -> u8 {{ ((self.cell % 10 + 48) as u8 in Wrapping) as u8 }} {}",
        write_machine(
            "machine write(output: &mut [u8; 4], pad: &mut Pad, position: u64 [0..=3])",
            "output[position] = pad.digit();",
        )
    );
    check(&source).map(|_| ()).unwrap_or_else(|messages| {
        panic!("receiver self-field bounds feed the call result: {messages:?}")
    });
}

/// A `&mut self` callee still cannot run on non-writable storage; capture must
/// never launder the receiver writability check.
#[test]
fn a_mutable_receiver_callee_still_requires_writable_storage() {
    let source = format!(
        "data Pad {{ cell: u64 [0..=9]; }} \
         machine Pad::digit(&mut self) -> u8 {{ ((self.cell % 10 + 48) as u8 in Wrapping) as u8 }} {}",
        write_machine(
            "machine write(output: &mut [u8; 4], pad: Pad, position: u64 [0..=3])",
            "output[position] = pad.digit();",
        )
    );
    let Err(messages) = check(&source) else {
        panic!("an immutable receiver must still reject a `&mut self` call");
    };
    assert!(
        messages
            .iter()
            .any(|message| message.contains("requires a mutable receiver")),
        "{messages:?}"
    );
}

/// Non-call operands under authored casts resolve their own evidence when the
/// selected plan cannot express the policy: a bounded operand under
/// `Saturating` clamps inside the proved range and satisfies the element
/// predicate, while an unbounded operand saturates to the full carrier and
/// cannot re-prove `Ascii` — the capture widens, never guesses.
#[test]
fn non_call_operands_under_remaining_cast_policies_carry_converted_bounds() {
    for (parameter, cast, expected) in [
        (
            "unknown: u64 [0..=9]",
            "((unknown as u8 in Saturating) as u8)",
            true,
        ),
        (
            "unknown: u64 [0..=9]",
            "((unknown as u8 in Wrapping) as u8)",
            true,
        ),
        (
            "unknown: u64 [0..=9]",
            "((unknown as u8 in Trapping) as u8)",
            true,
        ),
        ("unknown: u64 [0..=9]", "((unknown as u8) as u8)", true),
        (
            "unknown: u64",
            "((unknown as u8 in Saturating) as u8)",
            false,
        ),
    ] {
        let source = write_machine(
            &format!("machine write(output: &mut [u8; 4], position: u64 [0..=3], {parameter})"),
            &format!("output[position] = {cast};"),
        );
        match (check(&source), expected) {
            (Ok(_), true) => {}
            (Err(messages), false) => assert!(
                messages
                    .iter()
                    .any(|message| message.contains("cannot prove ensures")),
                "{parameter} {cast}: {messages:?}"
            ),
            (result, _) => panic!("{parameter} {cast}: {result:?}"),
        }
    }
}

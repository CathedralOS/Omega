//! A value whose type carries a domain, such as `u8 in Mark`, keeps that
//! domain wherever it is placed: as an array element and as the result of a
//! value `match`. Reading the element or the match back yields a `u8 in Mark`
//! the checker already knows satisfies `Mark`'s predicate, and a plain `u8`
//! supplied there is rejected, because "a type gains or loses a domain only
//! through a cast that names the domain".
//!
//! The spec rules: a domain qualification is part of a value's type and
//! "a parameter `value: T in D` accepts only an argument whose static type
//! carries the exact named domain `D`, and gives the body the fact
//! `value in D`" (`wiki/spec/language/domains.md`, "Carrier-exact
//! membership"); "a result-producing context still requires a compatible
//! result" from every `match` arm (`wiki/spec/language/patterns.md`,
//! "Coverage and domain tests").
//!
//! Each twin changes one formal from `u8 in D` to plain `u8`.

#[path = "../../06_lowered-program-to-checked-program/tests/support/source_handoff.rs"]
mod program;

use program::Program;

/// `need` requires its operand above zero, so a call `need(v as -Mark)` is
/// accepted only when `v`'s membership in `Mark` supplies `v > 0`.
const NEED: &str = "domain u8::Mark requires self > 0u8;
machine need(x: u8) -> u8 requires x > 0u8 { x }
";

/// An array of `u8 in Mark` built from a member, passed, returned, stored
/// in a field and written by index: every element read is a member whose
/// predicate needs no further proof.
#[test]
fn an_array_element_keeps_its_domain_wherever_the_array_goes() {
    let program = Program::of(&format!(
        "{NEED}
data Box [copy] {{ marks: [u8 in Mark; 2]; }}
machine pack(x: u8 in Mark) -> [u8 in Mark; 1] {{ [x] }}
machine pick(a: [u8 in Mark; 4], i: u64) -> u8 in Mark requires i < 4 {{ a[i] }}
machine read(a: [u8 in Mark; 4], i: u64) -> u8 requires i < 4 {{ need(a[i] as -Mark) }}
machine store(b: @mutable Box, v: u8 in Mark) {{ b.marks[1] = v; }}
machine field(b: Box) -> u8 {{ need(b.marks[0] as -Mark) }}"
    ));
    program.judged(|_| {}).accepted();
}

/// The twin of `pack`: a plain `u8` cannot initialize an element of
/// `[u8 in Mark; 1]`.
#[test]
fn a_plain_value_is_not_an_element_of_a_qualified_array() {
    let refusal = Program::try_of(&[(
        "product/main.omg",
        "domain u8::Mark requires self > 0u8;
machine pack(x: u8) -> [u8 in Mark; 1] { [x] }",
    )])
    .err()
    .expect("a plain u8 is not a member of Mark");
    assert_eq!(refusal.stage, "lowering");
    assert!(
        refusal.message.starts_with(
            "Violation: this value's type carries no domain and the array element it \
             initializes declares `u8::Mark`"
        ),
        "{}",
        refusal.message
    );
}

/// The twin of `store`: a plain `u8` cannot be written into an element of
/// `[u8 in Mark; 2]`.
#[test]
fn a_plain_value_cannot_be_written_into_a_qualified_element() {
    let refusal = Program::try_of(&[(
        "product/main.omg",
        "domain u8::Mark requires self > 0u8;
data Box [copy] { marks: [u8 in Mark; 2]; }
machine store(b: @mutable Box, v: u8) { b.marks[1] = v; }",
    )])
    .err()
    .expect("a plain u8 is not a member of Mark");
    assert_eq!(refusal.stage, "lowering");
    assert!(
        refusal.message.starts_with(
            "Violation: this value's type carries no domain and the place it is assigned to \
             declares `u8::Mark`"
        ),
        "{}",
        refusal.message
    );
}

/// Both arms deliver a `u8 in Mark`, so the match does too: it is passed to
/// a `u8 in Mark` formal, and its predicate supplies `need`'s requirement.
#[test]
fn a_match_whose_arms_are_qualified_delivers_their_domain() {
    let program = Program::of(&format!(
        "{NEED}
machine echo(x: u8 in Mark) -> u8 in Mark {{ x }}
machine choose(flag: bool, a: u8 in Mark, b: u8 in Mark) -> u8 {{
    let chosen = match flag {{ true -> a, false -> b, }};
    let echoed: u8 = need(echo(chosen) as -Mark);
    need(chosen as -Mark)
}}"
    ));
    program.judged(|_| {}).accepted();
}

/// The twin of `read`: a domain whose predicate says nothing about zero
/// gives the element read no `> 0` fact, so `need`'s requirement is
/// unproved there.
#[test]
fn an_element_read_knows_only_its_own_domain_predicate() {
    let rejection = Program::of(
        "domain u8::Mark requires self < 200u8;
machine need(x: u8) -> u8 requires x > 0u8 { x }
machine read(a: [u8 in Mark; 4], i: u64) -> u8 requires i < 4 { need(a[i] as -Mark) }",
    )
    .judged(|_| {})
    .rejected();
    assert_eq!(rejection.unproved.len(), 1, "{rejection:?}");
    assert!(rejection.violations.is_empty(), "{rejection:?}");
}

/// The twin: one arm delivers a plain `u8`, so the arms do not share a type.
#[test]
fn a_match_cannot_join_a_qualified_arm_with_a_plain_one() {
    let refusal = Program::try_of(&[(
        "product/main.omg",
        "domain u8::Mark requires self > 0u8;
machine echo(x: u8 in Mark) -> u8 in Mark { x }
machine choose(flag: bool, a: u8 in Mark, b: u8) -> u8 in Mark {
    let chosen = match flag { true -> a, false -> b, };
    echo(chosen)
}",
    )])
    .err()
    .expect("the arms deliver different types");
    assert_eq!(refusal.stage, "lowering");
    assert!(
        refusal.message.starts_with(
            "Violation: this value's type carries no domain and the match's common result \
             type declares `u8::Mark`"
        ),
        "{}",
        refusal.message
    );
}

#[test]
fn a_cast_reports_the_selected_array_carrier_in_its_domain_predicate() {
    use terminal_check_tables::program::EstablishedCondition;

    let program = Program::of("
        public data Wrapper<T> { value: T; }
        domain [Wrapper<u8>; 2]::Okay requires false;
        domain [Wrapper<u16>; 3]::Okay requires false;
        public machine first(value: [Wrapper<u8>; 2]) -> [Wrapper<u8>; 2] in Okay { value as +Okay }
        public machine second(value: [Wrapper<u16>; 3]) -> [Wrapper<u16>; 3] in Okay { value as +Okay }
    ");
    for (machine, carrier) in [
        ("first", "[Wrapper<u8>; 2]"),
        ("second", "[Wrapper<u16>; 3]"),
    ] {
        let metadata = program
            .elaboration()
            .sources
            .get(program.machine(machine).id)
            .unwrap();
        let predicates = metadata
            .predicate_establishments
            .iter()
            .filter_map(|(_, condition)| match condition {
                EstablishedCondition::DomainMembership(stated) => Some(stated.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            predicates,
            [format!("domain `{carrier}::Okay` requires `false`")]
        );
    }
}

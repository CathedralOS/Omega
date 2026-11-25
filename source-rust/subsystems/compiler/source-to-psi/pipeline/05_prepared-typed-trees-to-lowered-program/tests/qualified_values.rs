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

#[test]
fn a_domain_declared_on_a_qualified_alias_keeps_its_carrier_membership() {
    let program = Program::of(
        "domain u16::Positive requires self > 0u16;
         data Count = u16 in Positive;
         domain Count::Selected;
         data Segment<P> [linear] { value: P; }
         domain<P> Segment<P>::Stored<P>;
         data Kept = Segment<Count> in Stored<Count>;
         domain<P> Kept::Side<P>;
         machine scalar(x: Count in Selected) -> Count in Selected { x }
         machine structural(x: Kept in Side<Count>) -> Kept in Side<Count> { x }
         machine explicit_scalar(x: u16 in Positive & Selected) -> Count in Selected { x }",
    );
    program
        .judged(|lowered| {
            let types = &lowered.module.types;
            let name = |id| {
                let instance = lowered.domain_sources.get(id).unwrap().application;
                program
                    .trees()
                    .domain_definition(instance.declaration)
                    .unwrap()
                    .name
                    .as_str()
            };
            let structural = types
                .structural_domains
                .iter()
                .map(|row| (row.semantic_domain, row.required_memberships.as_deref()));
            let scalar = types
                .scalar_qualifications
                .domains
                .iter()
                .map(|row| (row.semantic_domain, row.required_memberships.as_deref()));
            let requirements: std::collections::BTreeMap<_, _> = structural
                .chain(scalar)
                .map(|(id, required)| {
                    (
                        name(id),
                        required
                            .unwrap()
                            .iter()
                            .copied()
                            .map(name)
                            .collect::<Vec<_>>(),
                    )
                })
                .collect();
            assert_eq!(
                requirements,
                std::collections::BTreeMap::from([
                    ("u16::Positive", vec![]),
                    ("Count::Selected", vec!["u16::Positive"]),
                    ("Segment::Stored", vec![]),
                    ("Kept::Side", vec!["Segment::Stored"]),
                ])
            );
        })
        .accepted();
}

#[test]
fn an_indexed_refinement_checks_each_familys_declared_predicate() {
    const DOMAINS: &str = "domain<const M: u8> u32::At<M> requires self > 0u32;
        domain<const N: u8> u32::At<N>::Child<N> requires self < 100u32;";
    for value in [7, 0, 150] {
        let source = format!(
            "{DOMAINS}
            machine run() -> u32 in At::Child<3> {{ {value}u32 as +At::Child<3> }}"
        );
        let program = Program::of(&source);
        let judged = program.judged(|_| {});
        if value == 7 {
            judged.accepted();
        } else {
            assert_eq!(judged.rejected().unproved.len(), 1);
        }
    }
}

#[test]
fn an_unused_refinement_publishes_its_exact_parent() {
    Program::of(
        "domain<const N: u8> u32::At<N>;
         domain u32::At<3>::Child;
         domain u32::At<3>::Child::Leaf;
         machine run() -> u32 { 7u32 }",
    )
    .judged(|_| {})
    .accepted();
}

#[test]
fn a_refined_scalar_formal_carries_its_complete_parent_chain() {
    let program = Program::of(
        "domain i32::Positive requires self > 0;
         domain i32::Positive::Small requires self < 100;
         domain i32::Positive::Small::NotTwo requires self != 2;
         machine relay(x: i32 in Positive::Small::NotTwo) -> i32 in Positive::Small::NotTwo { x }",
    );
    let formal = program.machine("relay").parameters[0].qualifications;
    program
        .judged(|lowered| {
            let catalog = &lowered.module.types.scalar_qualifications;
            let domains = &catalog
                .sets
                .iter()
                .find(|set| set.id == formal)
                .unwrap()
                .domains;
            assert_eq!(domains.len(), 3);
            let declared = |name: &str| {
                catalog
                    .domains
                    .iter()
                    .find(|row| {
                        let application = lowered
                            .domain_sources
                            .get(row.semantic_domain)
                            .unwrap()
                            .application;
                        program
                            .trees()
                            .domain_definition(application.declaration)
                            .unwrap()
                            .name
                            .as_str()
                            == name
                    })
                    .unwrap()
            };
            let positive = declared("i32::Positive");
            let small = declared("i32::Positive::Small");
            let not_two = declared("i32::Positive::Small::NotTwo");
            assert!(
                domains.contains(&positive.id)
                    && domains.contains(&small.id)
                    && domains.contains(&not_two.id)
            );
            assert_eq!(
                small.required_memberships.as_deref(),
                Some([positive.semantic_domain].as_slice())
            );
            assert_eq!(
                not_two.required_memberships.as_deref(),
                Some([small.semantic_domain].as_slice())
            );
        })
        .accepted();
}

#[test]
fn adding_a_refinement_accounts_for_its_ancestors_on_the_same_value() {
    const DOMAINS: &str = "domain i32::Positive requires self > 0;
        domain i32::Positive::Small requires self < 100;
        domain i32::Positive::Seven requires self == 7;";
    Program::of(&format!("{DOMAINS}
        machine short() -> i32 in Positive::Small {{ 7 as +Positive::Small }}
        machine parent_first() -> i32 in Positive::Small {{ 7 as +Positive +Positive::Small }}
        machine child_first() -> i32 in Positive::Small {{ 7 as +Positive::Small +Positive }}
        machine siblings() -> i32 in Positive::Small & Positive::Seven {{
            7 as +Positive::Small +Positive::Seven
        }}
        machine forget(x: i32 in Positive::Small) -> i32 in Positive {{ x as =Positive -Positive::Small }}
        machine restore(x: i32 in Positive) -> i32 in Positive::Small
            requires (x as -Positive) < 100 {{ x as +Positive::Small }}"))
        .judged(|_| {}).accepted();
    for value in [0, 150] {
        let rejection = Program::of(&format!(
            "{DOMAINS}
            machine wrong() -> i32 in Positive::Small {{ {value} as +Positive::Small }}"
        ))
        .judged(|_| {})
        .rejected();
        assert_eq!(rejection.unproved.len(), 1, "{rejection:?}");
    }
    let source = format!("{DOMAINS}
        machine orphan(x: i32 in Positive) -> i32 in Positive::Small {{ x as -Positive +Positive::Small }}");
    let refusal = Program::try_of(&[("product/main.omg", &source)])
        .err()
        .expect("removing the parent conflicts with adding its child");
    assert!(
        refusal.message.contains("without its parent `Positive`"),
        "{refusal:?}"
    );
}

#[test]
fn erasing_a_refinement_in_a_contract_preserves_the_operand_in_either_order() {
    const DOMAINS: &str = "domain i32::Positive requires self > 0;
        domain i32::Positive::Small requires self < 100;";
    for entries in ["-Positive -Positive::Small", "-Positive::Small -Positive"] {
        Program::of(&format!(
            "{DOMAINS}
            machine echo(x: i32 in Positive::Small) -> i32
                ensures result == (x as {entries}) {{ x as -Positive::Small -Positive }}"
        ))
        .judged(|_| {})
        .accepted();
    }
    let wrong = Program::of(&format!(
        "{DOMAINS}
        machine wrong(x: i32 in Positive::Small) -> i32
            ensures result == (x as -Positive -Positive::Small) {{ 0i32 }}"
    ))
    .judged(|_| {})
    .rejected();
    assert_eq!(wrong.unproved.len(), 1, "{wrong:?}");
}

#[test]
fn scalar_domain_predicates_bind_each_const_index_in_the_declaration() {
    const DOMAIN: &str = "domain<const Lo: u8, const Hi: u8> u8::Between<Lo, Hi>
        requires self >= Lo && self < Hi;";
    for value in [3, 4] {
        let source = format!(
            "{DOMAIN}
            machine f() -> u8 in Between<2,4> {{ {value}u8 as +Between<2,4> }}"
        );
        let program = Program::of(&source);
        let [family] = program.lowered.forms.as_slice() else {
            panic!("the reached domain must have its own check form")
        };
        let [form] = family.forms.as_ref().unwrap().as_slice() else {
            panic!("the domain must form once under arbitrary inputs")
        };
        let [predicate] = form.module.scalars.scalar_predicate_definitions.as_slice() else {
            panic!("expected the declaration's predicate")
        };
        assert_eq!(predicate.parameters.len(), 3);
        assert!(
            program
                .elaboration()
                .module
                .scalars
                .scalar_predicate_definitions
                .is_empty()
        );
        let verdict = program.judged(|_| {});
        if value == 3 {
            verdict.accepted();
        } else {
            assert_eq!(verdict.rejected().unproved.len(), 1);
        }
    }
}

#[test]
fn an_unused_domain_predicate_must_form_for_every_const_index() {
    let domain = "domain<const N: u8> u8::Bound<N> requires self / N == 1u8;";
    for machine in [
        "machine f() {}",
        "machine f() -> u8 in Bound<3> { 3u8 as +Bound<3> }",
    ] {
        let rejected = Program::of(&format!("{domain} {machine}"))
            .judged(|_| {})
            .rejected();
        assert!(
            rejected
                .violations
                .iter()
                .any(|violation| { violation.contains("contract arithmetic is not proved total") }),
            "{rejected:?}"
        );
    }
    Program::of(
        "domain<const N: u8> u8::Empty<N> requires self < N && self > N;
        machine f() {}",
    )
    .judged(|_| {})
    .accepted();
}

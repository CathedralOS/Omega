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
fn a_domain_cast_checks_every_member_of_a_transparent_alias() {
    const DOMAINS: &str = "domain i32::Positive requires self > 0;
        domain i32::Positive::Small requires self < 100;
        domain i32::NotTwo requires self != 2;
        domain i32::Limited = i32::Positive::Small & i32::NotTwo;
        domain i32::Together = i32::Limited & i32::NotTwo;";

    Program::of(&format!(
        "{DOMAINS}
        machine make() -> i32 in Together {{ 7i32 as +Together }}
        machine keep(x: i32 in Together) -> i32 in Positive::Small & NotTwo {{
            x as =Together =Positive
        }}
        machine erase(x: i32 in Together) -> i32
            ensures result == (x as -Together -Positive) {{ x as -Positive -Together }}"
    ))
    .judged(|_| {})
    .accepted();

    for value in [0, 2, 150] {
        let rejected = Program::of(&format!(
            "{DOMAINS}
            machine wrong() -> i32 in Together {{ {value}i32 as +Together }}"
        ))
        .judged(|_| {})
        .rejected();
        assert_eq!(rejected.unproved.len(), 1, "{rejected:?}");
    }
    let repeated = format!(
        "{DOMAINS}
        machine repeated() -> i32 in Together {{ 7i32 as +Together +NotTwo }}"
    );
    let refused = Program::try_of(&[("product/main.omg", &repeated)])
        .err()
        .expect("a separate entry named the same constituent");
    assert!(refused.message.contains("a second time"), "{refused:?}");
}

#[test]
fn indexed_membership_evaluates_with_its_written_application() {
    let family = "domain<const Bound: u64> u64::AtMost<Bound> requires self <= Bound;";
    let program = Program::of(&format!(
        "{family}
        domain u64::Positive requires self > 0u64;
        machine tagged(candidate: u64 in Positive) -> bool {{ candidate in AtMost::<8> }}
        machine under(candidate: u64) -> bool ensures result == (candidate <= 8u64) {{ candidate in AtMost::<8> }}
        machine above() -> bool ensures !result {{ 8u64 in u64::AtMost::<7> }}
        machine exact() -> bool ensures result {{ 8u64 in AtMost::<8> }}"
    ));
    program.judged(|_| {}).accepted();

    for (operand, application, refusal) in [
        ("u32", "AtMost::<8>", "domain classifies `u64`"),
        ("u64", "AtMost::<7, 8>", "incomplete index telescope"),
        ("u64", "AtMost", "incomplete index telescope"),
        (
            "u64",
            "AtMost::<bool>",
            "must name a constant or a const parameter",
        ),
    ] {
        let source = format!("{family} machine f(x: {operand}) -> bool {{ x in {application} }}");
        let error = Program::try_of(&[("product/main.omg", &source)])
            .err()
            .expect("ill-formed membership");
        assert!(error.message.contains(refusal), "{}", error.message);
    }
}

#[test]
fn indexed_membership_keeps_the_generic_machines_const_in_scope() {
    let source = "domain<const N: u64> u64::AtMost<N> requires self <= N;
        machine test<const K: u64>(x: u64) -> bool
            ensures result == (x <= K) { x in AtMost::<K> }
        machine exact() -> bool ensures result { test::<8>(8u64) }
        machine above() -> bool ensures !result { test::<7>(8u64) }";
    Program::of(source).judged(|_| {}).accepted();

    let wrong = source.replace("const K: u64", "const K: u8");
    let error = Program::try_of(&[("product/main.omg", &wrong)])
        .err()
        .expect("K cannot supply the family's u64 const index");
    assert!(
        error.message.contains("has type `u8`, expected `u64`"),
        "{}",
        error.message
    );
}

#[test]
fn a_machine_contract_uses_its_lexical_const_as_the_domain_index() {
    let domain = "domain<const N: u64> u64::AtMost<N> requires self <= N;";
    for application in ["AtMost::<K>", "AtMost<K>"] {
        let source = format!(
            "{domain}
            machine checked<const K: u64>(x: u64) -> u64
                requires x in {application} ensures result <= K {{ x }}
            machine call() -> u64 {{ checked::<8>(8u64) }}"
        );
        Program::of(&source).judged(|_| {}).accepted();
        let wrong = source.replace("checked::<8>", "checked::<7>");
        let rejection = Program::of(&wrong).judged(|_| {}).rejected();
        assert_eq!(rejection.unproved.len(), 1);
    }
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

#[test]
fn a_transparent_domain_alias_keeps_the_predicates_of_its_leaves() {
    let declarations = "domain i32::Positive requires self > 0;
        domain i32::Positive::Small requires self < 100;
        domain i32::Odd requires self != 2;
        domain i32::Limited = i32::Positive::Small & i32::Odd;
        domain i32::Again = i32::Limited;
        machine keep(x: i32 in Again) -> i32 in Positive::Small & Odd { x }";
    let check = |value| {
        Program::of(&format!(
            "{declarations}
            machine f() -> i32 in Positive::Small & Odd {{ {value}i32 as +Positive::Small +Odd }}"
        ))
        .judged(|_| {})
    };
    check(7).accepted();
    assert_eq!(check(2).rejected().unproved.len(), 1);
}

#[test]
fn an_unused_domain_alias_checks_its_constituents() {
    for (declarations, reason) in [
        ("domain i32::Bad = i32::Left & i32::Absent;", "i32::Absent"),
        (
            "domain u32::Other; domain i32::Bad = u32::Other;",
            "incompatible carriers",
        ),
        (
            "domain i32::Bad = i32::Other; domain i32::Other = i32::Bad;",
            "domain alias cycle",
        ),
        ("public domain i32::Bad = i32::Left;", "private domain"),
        (
            "domain<T, const N: u8> T::Marker<N>; domain i32::Bad = i32::Marker;",
            "requiring a type or index application",
        ),
    ] {
        let source = format!("domain i32::Left; {declarations} machine ok() -> i32 {{ 0i32 }}");
        let Err(refusal) = Program::try_of(&[("product/main.omg", &source)]) else {
            panic!("accepted {declarations}");
        };
        assert!(refusal.message.contains(reason), "{refusal:?}");
    }
}

#[test]
fn contract_membership_uses_the_record_formals_carrier_qualifications() {
    let domains = "data Word { bits: u32; }
        domain Word::Placed;
        domain Word::Other;
        data Placed = Word in Word::Placed;
        data Other = Word in Word::Other;
        domain<T> Placed::Op<T>;
        domain<T> Other::Op<T>;";
    let program = Program::of(&format!(
        "{domains}
        machine left(x: Word in Placed) requires x in Placed::Op<u32> {{}}
        machine right(x: Word in Other) requires x in Other::Op<u32> {{}}"
    ));
    program.judged(|_| {}).accepted();

    for (carrier, family) in [
        ("Word", "Placed"),
        ("Word in Other", "Placed"),
        ("Word in Placed", "Other"),
    ] {
        let source =
            format!("{domains} machine f(x: {carrier}) requires x in {family}::Op<u32> {{}}");
        let refusal = Program::try_of(&[("product/main.omg", &source)])
            .err()
            .expect("the record formal lacks the family's carrier qualification");
        assert_eq!(refusal.stage, "lowering");
        assert!(
            refusal
                .message
                .contains(&format!("requires membership in `Word::{family}`")),
            "{}",
            refusal.message
        );
    }
}

#[test]
fn a_fixed_alias_applies_a_domain_to_its_own_carrier() {
    let domains = "domain<T [copy]> T::Label;
        domain i32::IntegerLabel = i32::Label;
        domain u32::UnsignedLabel = u32::Label;
        domain i32::Twice = i32::IntegerLabel & i32::Label;";
    let checked = Program::of(&format!(
        "{domains}
        machine keep(x: i32 in Twice) -> i32 in Label {{ x }}
        machine gain(x: u32) -> u32 in UnsignedLabel {{ x as +UnsignedLabel }}
        machine erase(x: i32 in Label) -> i32 {{ x as -Twice }}"
    ));
    checked.judged(|_| {}).accepted();
    let trees = checked.trees();
    let member = |name| {
        let alias = trees
            .domain_definitions()
            .iter()
            .find(|domain| domain.name.as_str() == name)
            .unwrap();
        let edge = &alias.alias.as_ref().unwrap().constituents[0];
        let application = trees
            .domain_application(edge.domain_symbol, edge.semantic_domain)
            .unwrap();
        assert_eq!(application.arguments.len(), 1);
        assert_eq!(
            trees.package_qualified_type_identity(application.arguments[0]),
            trees.package_qualified_type_identity(alias.target_type)
        );
        edge.semantic_domain
    };
    assert_ne!(member("i32::IntegerLabel"), member("u32::UnsignedLabel"));

    for (source, expected) in [
        (
            "data Ticket [linear] { bits: u8; } domain Ticket::Bad = Ticket::Label; machine f() {}",
            "only a `copy` carrier",
        ),
        (
            "machine f(x: u32 in Label) requires x in i32::IntegerLabel {}",
            "carrier",
        ),
        (
            "machine f(x: u32 in Label) requires (x in i32::IntegerLabel) == true {}",
            "carrier",
        ),
        (
            "machine f(x: u32 in Label) -> bool { x in i32::IntegerLabel }",
            "carrier",
        ),
    ] {
        let input = format!("{domains} {source}");
        let Err(refusal) = Program::try_of(&[("product/main.omg", &input)]) else {
            panic!("accepted {source}");
        };
        assert!(refusal.message.contains(expected), "{refusal:?}");
    }
}

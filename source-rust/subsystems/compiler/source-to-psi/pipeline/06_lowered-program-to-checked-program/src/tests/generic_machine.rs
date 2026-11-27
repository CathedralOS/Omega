//! Mathematical machines that are generic over other machines.
//!
//! Such a machine is a definition or a law about every machine with the
//! signature its parameter declares. The signature gives it one more formal
//! per machine parameter, of that machine's function type, and the kernel
//! checks it once.

use super::*;
use crate::laws::{SignatureJudgment, UnprovedLaw};

const FIXED: &str =
    include_str!("../../tests/fixtures/generic_machine/pass/twice_a_fixed_point/main.omg");
const UNFOUNDED: &str = include_str!(
    "../../tests/fixtures/generic_machine/fail/twice_without_the_fixed_point/main.omg"
);
const CONCRETE: &str =
    include_str!("../../tests/fixtures/generic_machine/pass/twice_at_a_concrete_machine/main.omg");
const CONCRETE_ADDS_ONE: &str = include_str!(
    "../../tests/fixtures/generic_machine/fail/twice_at_a_concrete_machine_adds_one/main.omg"
);
const CITED: &str =
    include_str!("../../tests/fixtures/generic_machine/pass/cited_at_its_own_machine/main.omg");
const CITED_ELSEWHERE: &str =
    include_str!("../../tests/fixtures/generic_machine/fail/cited_at_another_machine/main.omg");
const ITERATE: &str =
    include_str!("../../tests/fixtures/generic_machine/pass/iterate_keeps_a_fixed_point/main.omg");
const ITERATE_UNFOUNDED: &str = include_str!(
    "../../tests/fixtures/generic_machine/fail/iterate_without_the_fixed_point/main.omg"
);
const CITED_BY_ORDINARY: &str =
    include_str!("../../tests/fixtures/generic_machine/pass/cited_by_an_ordinary_law/main.omg");
const CITED_BY_ORDINARY_AT_SUCC: &str = include_str!(
    "../../tests/fixtures/generic_machine/fail/cited_by_an_ordinary_law_at_succ/main.omg"
);
const CALLED_BY_ORDINARY: &str = include_str!(
    "../../tests/fixtures/generic_machine/pass/called_by_an_ordinary_definition/main.omg"
);
const CALLED_BY_ORDINARY_ADDS_NOTHING: &str = include_str!(
    "../../tests/fixtures/generic_machine/fail/called_by_an_ordinary_definition_adds_nothing/main.omg"
);

fn proofs(source: &str) -> String {
    let start = source.find("data Peano").expect("the fixture's data");
    let end = source.find("data Main").expect("the fixture's entry state");
    format!(
        "use {{omega}}/language/core/foundation.omg as proof;\n{}",
        &source[start..end]
    )
}

fn judged(source: &str) -> Vec<RouteVerdict> {
    let typed = super::build_purpose::frontier_with_core(
        source,
        "machine build_entry() {}",
        source_provenance::DependencyScope::Product,
        &[],
    );
    let prepared = crate::prepare_fixture(typed);
    let scope = source_provenance::DependencyScope::Product;
    let compilation = crate::lowering::prepare_compilation(
        &prepared,
        super::build_purpose::roster(&[], &[Some(super::build_purpose::package())], scope),
        terminal_codec::TerminalPublication::Invocation,
    )
    .unwrap();
    crate::judge_compilation(compilation.program)
        .into_iter()
        .map(|verdict| verdict.expect("a judged part"))
        .collect()
}

/// The one refusal of a program whose signature cannot be read.
fn refusal(verdicts: &[RouteVerdict]) -> &str {
    let mut refusals = verdicts.iter().filter_map(|verdict| match verdict {
        RouteVerdict::FallBack(refusal) => Some(refusal.detail.as_str()),
        RouteVerdict::Laws(judgment) => panic!("the program must be refused: {judgment:?}"),
        _ => None,
    });
    let (Some(refusal), None) = (refusals.next(), refusals.next()) else {
        panic!("expected exactly one refusal: {verdicts:?}")
    };
    refusal
}

fn judged_law<'v>(verdicts: &'v [RouteVerdict], machine: &str) -> &'v crate::laws::LawJudgment {
    signature(verdicts)
        .laws
        .iter()
        .find(|law| law.machine == machine)
        .unwrap_or_else(|| panic!("the law `{machine}` must be judged: {verdicts:?}"))
}

fn signature(verdicts: &[RouteVerdict]) -> &SignatureJudgment {
    let mut laws = None;
    for verdict in verdicts {
        match verdict {
            RouteVerdict::Accepted(_) => {}
            RouteVerdict::Laws(judgment) => {
                assert!(laws.replace(&**judgment).is_none(), "one signature");
            }
            other => panic!("every part must be accepted: {other:?}"),
        }
    }
    laws.expect("the signature's judgment")
}

fn unproved(verdicts: &[RouteVerdict]) -> &UnprovedLaw {
    let mut laws = verdicts.iter().filter_map(|verdict| match verdict {
        RouteVerdict::UnprovedLaws(laws) => Some(laws),
        RouteVerdict::Laws(judgment) => panic!("the law must not be accepted: {judgment:?}"),
        _ => None,
    });
    let (Some([law]), None) = (laws.next().map(Vec::as_slice), laws.next()) else {
        panic!("expected exactly one unproved law: {verdicts:?}")
    };
    law
}

#[test]
fn a_law_about_every_machine_of_a_signature_is_judged() {
    use proof_admission::Term;
    let verdicts = judged(&proofs(FIXED));
    let judgment = signature(&verdicts);
    let law = judgment
        .laws
        .iter()
        .find(|law| law.machine == "twice_keeps_a_fixed_point")
        .expect("the law");
    let MathematicalCoreDecision::Judged(receipt) = law.decision;
    assert_eq!((receipt.context_depth, receipt.assumption_closure), (0, 0));
    // The statement the kernel judged is for every function from the data
    // to itself: its first binder is that function type.
    let mut checked = terminal_codec::decode_mathematical_certificate(&law.certificate).unwrap();
    proof_admission::verify_mathematical_certificate(
        &mut checked.arena,
        &checked.certificate,
        &mut Default::default(),
    )
    .unwrap();
    let arena = &checked.arena;
    let Term::Pi { domain, .. } = arena.get(checked.certificate.expected) else {
        panic!("the machine parameter")
    };
    let Term::Pi {
        domain: from,
        codomain: to,
    } = arena.get(domain)
    else {
        panic!("a machine parameter is a function: {:?}", arena.get(domain))
    };
    assert!(arena.structurally_equal(from, to));
    assert!(matches!(arena.get(from), Term::Constant { .. }));
    eprintln!(
        "twice_keeps_a_fixed_point: {} conversion steps",
        law.conversion_steps
    );
}

#[test]
fn without_its_premise_the_generic_law_stays_unproved() {
    let source = proofs(UNFOUNDED);
    assert_eq!(
        source,
        proofs(FIXED).replace(
            "requires\n    |proof|::Identity::<Peano>(Step(a), a)\nensures\n",
            "ensures\n"
        )
    );
    let verdicts = judged(&source);
    let law = unproved(&verdicts);
    assert_eq!(law.machine, "twice_keeps_a_fixed_point");
    assert_eq!(law.cause, UnprovedCause::NoDerivation);
    assert_eq!(
        law.statement,
        "cannot prove ensures contract for `twice_keeps_a_fixed_point`: \
         core\\Identity::<Peano>(twice(Step, a), a)"
    );
}

#[test]
fn a_clause_passes_a_concrete_machine_to_a_generic_one() {
    let verdicts = judged(&proofs(CONCRETE));
    let law = judged_law(&verdicts, "twice_succ_adds_two");
    let MathematicalCoreDecision::Judged(receipt) = law.decision;
    assert_eq!((receipt.context_depth, receipt.assumption_closure), (0, 0));
}

#[test]
fn a_concrete_machine_is_unfolded_not_assumed() {
    let source = proofs(CONCRETE_ADDS_ONE);
    assert_eq!(
        source,
        proofs(CONCRETE).replace(
            "Peano::Succ { pred: Peano::Succ { pred: a } })",
            "Peano::Succ { pred: a })"
        )
    );
    let verdicts = judged(&source);
    let law = unproved(&verdicts);
    assert_eq!(law.machine, "twice_succ_adds_two");
    assert_eq!(law.cause, UnprovedCause::NoDerivation);
    assert_eq!(
        law.statement,
        "cannot prove ensures contract for `twice_succ_adds_two`: \
         core\\Identity::<Peano>(twice(succ, a), Peano::Succ(a))"
    );
}

#[test]
fn a_generic_machine_cites_another_at_its_own_machine() {
    let verdicts = judged(&proofs(CITED));
    judged_law(&verdicts, "twice_keeps_a_fixed_point");
    judged_law(&verdicts, "twice_keeps_one_fixed_point");
}

#[test]
fn a_citation_owes_its_premise_at_the_machine_it_names() {
    let source = proofs(CITED_ELSEWHERE);
    assert_eq!(
        source,
        proofs(CITED).replace(
            "twice_keeps_a_fixed_point::<Step>(a);",
            "twice_keeps_a_fixed_point::<Other>(a);"
        )
    );
    let verdicts = judged(&source);
    let law = unproved(&verdicts);
    assert_eq!(law.machine, "twice_keeps_one_fixed_point");
    assert_eq!(law.cause, UnprovedCause::NoDerivation);
}

/// `pass/cited_at_its_own_machine` with two concrete machines, and a law
/// generic over `Other` that cites the generic law at `concrete`.
fn cited_at(concrete: &str) -> String {
    proofs(CITED).replace(
        "machine twice_keeps_one_fixed_point",
        &format!(
            "machine id(a: Peano) -> Peano
{{
    transition {{ _ -> (a) }}
}}

machine succ(a: Peano) -> Peano
{{
    transition {{ _ -> Peano::Succ {{ pred: a }} }}
}}

machine twice_keeps_at_a_concrete_machine<machine Other>(a: Peano)
requires machine Other(a: Peano) -> Peano terminates;
ensures
    |proof|::Identity::<Peano>(twice::<id>(a), a)
{{
    twice_keeps_a_fixed_point::<{concrete}>(a);
}}

machine twice_keeps_one_fixed_point"
        ),
    )
}

#[test]
fn a_generic_machine_cites_another_at_a_concrete_machine() {
    let verdicts = judged(&cited_at("id"));
    judged_law(&verdicts, "twice_keeps_at_a_concrete_machine");
    // The citation's premise is owed at the machine it names: `succ` does
    // not leave `a` as it is.
    let verdicts = judged(&cited_at("succ"));
    let law = unproved(&verdicts);
    assert_eq!(law.machine, "twice_keeps_at_a_concrete_machine");
}

#[test]
fn a_concrete_machine_with_a_premise_is_refused_as_a_machine_argument() {
    // Such a machine is not a total function of its formals, so the
    // signature has no definition to pass for the parameter.
    let source = proofs(CONCRETE).replace(
        "machine succ(a: Peano) -> Peano\n",
        "machine succ(a: Peano) -> Peano\nrequires |proof|::Identity::<Peano>(a, a)\n",
    );
    assert_ne!(source, proofs(CONCRETE));
    let verdicts = judged(&source);
    assert!(
        refusal(&verdicts).starts_with(
            "contract machine argument of a generic mathematical machine with a premise"
        ),
        "{verdicts:?}"
    );
}

#[test]
fn a_generic_machine_defined_by_cases_is_proved_by_induction() {
    let verdicts = judged(&proofs(ITERATE));
    let law = judged_law(&verdicts, "iterate_keeps_a_fixed_point");
    let MathematicalCoreDecision::Judged(receipt) = law.decision;
    assert_eq!((receipt.context_depth, receipt.assumption_closure), (0, 0));
}

#[test]
fn without_its_premise_the_induction_stays_unproved() {
    let verdicts = judged(&proofs(ITERATE_UNFOUNDED));
    let law = unproved(&verdicts);
    assert_eq!(law.machine, "iterate_keeps_a_fixed_point");
    assert_eq!(law.cause, UnprovedCause::NoDerivation);
    assert!(
        law.statement.contains("in the case `Peano::Succ`"),
        "{}",
        law.statement
    );
}

#[test]
fn an_ordinary_law_cites_a_generic_law_at_a_concrete_machine() {
    let verdicts = judged(&proofs(CITED_BY_ORDINARY));
    judged_law(&verdicts, "iterate_id_keeps");
}

#[test]
fn an_ordinary_citation_owes_the_premise_at_the_machine_it_names() {
    let verdicts = judged(&proofs(CITED_BY_ORDINARY_AT_SUCC));
    let law = unproved(&verdicts);
    assert_eq!(law.machine, "iterate_succ_keeps");
}

#[test]
fn an_ordinary_definition_calls_a_generic_machine_at_a_concrete_machine() {
    let verdicts = judged(&proofs(CALLED_BY_ORDINARY));
    judged_law(&verdicts, "plus_one");
}

#[test]
fn an_ordinary_call_unfolds_the_concrete_machine() {
    let verdicts = judged(&proofs(CALLED_BY_ORDINARY_ADDS_NOTHING));
    let law = unproved(&verdicts);
    assert_eq!(law.machine, "plus_one");
}

macro_rules! fixture {
    ($path:literal) => {
        include_str!(concat!(
            "../../tests/fixtures/generic_machine/",
            $path,
            "/main.omg"
        ))
    };
}

/// A positive fixture's law is judged and its negative twin's stays
/// unproved.
fn twins(pass: &str, judged_machine: &str, fail: &str, unproved_machine: &str) {
    let verdicts = judged(&proofs(pass));
    judged_law(&verdicts, judged_machine);
    let verdicts = judged(&proofs(fail));
    assert_eq!(unproved(&verdicts).machine, unproved_machine);
}

#[test]
fn a_generic_machine_calls_itself_at_other_machines() {
    // The induction's hypothesis is the law at the swapped machines, so it
    // owes both premises.
    let verdicts = judged(&proofs(fixture!("pass/alternate_keeps_two_fixed_points")));
    judged_law(&verdicts, "alternate_twice");
    twins(
        fixture!("pass/alternate_keeps_two_fixed_points"),
        "alternate_keeps",
        fixture!("fail/alternate_keeps_one_fixed_point"),
        "alternate_keeps",
    );
}

/// Euclid's `gcd` by remainders, as `core/nat.omg` wrote it before it
/// became structural, with `Step` applied to its result, and a law about
/// it at `succ` claiming `claimed`; with them, the repeated subtraction
/// `mod` that `computed_descent/pass` keeps, generic over a `Step` it
/// does not use, whose done arm cites the bound only when `cite_bound`.
/// All judged with the whole of `core/nat.omg`.
fn gcd_then(claimed: &str, cite_bound: bool) -> Vec<RouteVerdict> {
    let nat = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../../../source/library/core/nat.omg"
    ));
    let first = nat.find("public data Nat {").expect("the data declaration");
    let nat = format!(
        "use {{omega}}/language/core/algebra.omg;\nuse {{omega}}/language/core/foundation.omg as proof;\n{}
machine gcd_then<machine Step>(a: Nat, b: Nat) -> Nat
requires machine Step(a: Nat) -> Nat terminates;
terminates by b
{{
    transition b {{
        Nat::Zero -> (Step(a)),
        Nat::Succ {{ prev as pb }} -> gcd_rec(a, b, pb)
    }}

    machine gcd_rec(a: Nat, b: Nat, pb: Nat) -> Nat {{
        _ = le_one_succ(pb);
        let next: Nat = mod(a, Nat::Succ {{ prev: pb }});
        transition {{ _ -> (gcd_then::<Step>(Nat::Succ {{ prev: pb }}, next)) }}
    }}
}}

machine mod_then<machine Step>(a: Nat, b: Nat) -> Nat
requires machine Step(a: Nat) -> Nat terminates;
terminates by a
requires
    |proof|::Identity::<Nat>(saturating_sub(Nat::Succ {{ prev: Nat::Zero }}, b), Nat::Zero)
ensures
    |proof|::Identity::<Nat>(saturating_sub(Nat::Succ {{ prev: result }}, b), Nat::Zero)
{{
    transition (saturating_sub(b, a)) {{
        Nat::Zero -> mod_rec(a, b),
        Nat::Succ {{ prev as w }} -> mod_done(a, b, w)
    }}

    machine mod_rec(a: Nat, b: Nat) -> Nat {{
        _ = sub_lt(a, b);
        let next: Nat = saturating_sub(a, b);
        transition {{ _ -> (mod_then::<Step>(next, b)) }}
    }}

    machine mod_done(a: Nat, b: Nat, w: Nat) -> Nat {{
        {cited}
        transition {{ _ -> (a) }}
    }}
}}

machine nat_succ(a: Nat) -> Nat
{{
    transition {{ _ -> Nat::Succ {{ prev: a }} }}
}}

machine gcd_then_succ()
ensures
    |proof|::Identity::<Nat>(
        gcd_then::<nat_succ>(Nat::Succ {{ prev: Nat::Succ {{ prev: Nat::Zero }} }}, Nat::Succ {{ prev: Nat::Zero }}),
        {claimed}
    )
{{
}}
",
        &nat[first..],
        cited = if cite_bound {
            "_ = lt_of_sub_pos(a, b, w);"
        } else {
            ""
        },
    );
    super::fuel_recursion::judged_library_files(&nat, &[])
}

#[test]
fn a_generic_machine_recurses_on_fuel() {
    // `gcd_then` calls itself at the computed `mod(a, b)`, so it is defined
    // on fuel, with `Step` among the definition's arguments, and owes each
    // call's descent. `gcd(2, 1)` is one, so the result at `succ` is two;
    // its twin claims one.
    let two = "Nat::Succ { prev: Nat::Succ { prev: Nat::Zero } }";
    let verdicts = gcd_then(two, true);
    judged_law(&verdicts, "gcd_then_succ");
    // A generic machine on fuel with a law of its own: the law is an
    // induction on the fuel, over the machine parameter too.
    judged_law(&verdicts, "mod_then");
    for law in &signature(&verdicts).laws {
        assert!(
            matches!(law.decision, MathematicalCoreDecision::Judged(_)),
            "{law:?}"
        );
    }
    let verdicts = gcd_then("Nat::Succ { prev: Nat::Zero }", true);
    assert_eq!(unproved(&verdicts).machine, "gcd_then_succ");
    // Without the bound in its done arm, the law on fuel stays unproved.
    let verdicts = gcd_then(two, false);
    assert_eq!(unproved(&verdicts).machine, "mod_then");
}

#[test]
fn a_generic_machine_dispatches_on_a_computed_value() {
    twins(
        fixture!("pass/pick_by_a_computed_value"),
        "pick_by_succ",
        fixture!("fail/pick_by_a_computed_value_wrong_case"),
        "pick_by_succ",
    );
}

#[test]
fn a_call_of_a_generic_machine_owes_its_premise() {
    let verdicts = judged(&proofs(fixture!("pass/call_owes_its_premise")));
    signature(&verdicts);
    let verdicts = judged(&proofs(fixture!("fail/call_without_its_premise")));
    let law = unproved(&verdicts);
    assert_eq!(law.machine, "fixed_by_succ");
}

#[test]
fn a_law_cites_through_a_generic_helper() {
    twins(
        fixture!("pass/cited_through_a_generic_helper"),
        "twice_id_keeps",
        fixture!("fail/cited_through_a_generic_helper_at_succ"),
        "twice_succ_keeps",
    );
}

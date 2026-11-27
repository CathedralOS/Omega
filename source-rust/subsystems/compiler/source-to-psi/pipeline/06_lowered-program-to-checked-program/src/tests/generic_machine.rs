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

#[test]
fn a_generic_machine_that_calls_itself_at_another_machine_is_refused() {
    // Its machine parameters are fixed for the whole definition, so the
    // kernel's recursion has no way to change them.
    let source = proofs(ITERATE).replace(
        "(Step(iterate::<Step>(pred, a)))",
        "(Step(iterate::<succ>(pred, a)))",
    );
    let source = source.replace(
        "machine iterate<machine Step>",
        "machine succ(a: Peano) -> Peano
{
    transition { _ -> Peano::Succ { pred: a } }
}

machine iterate<machine Step>",
    );
    assert!(source.contains("iterate::<succ>(pred, a)"));
    let verdicts = judged(&source);
    assert!(
        verdicts.iter().any(|verdict| matches!(
            verdict,
            RouteVerdict::FallBack(refusal) if refusal.detail.starts_with(
                "machine generic mathematical machine that calls itself at other machines"
            )
        )),
        "{verdicts:?}"
    );
    assert!(
        !verdicts
            .iter()
            .any(|verdict| matches!(verdict, RouteVerdict::Laws(_))),
        "{verdicts:?}"
    );
}

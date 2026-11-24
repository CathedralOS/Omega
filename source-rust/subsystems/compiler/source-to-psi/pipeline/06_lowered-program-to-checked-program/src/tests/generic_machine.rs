//! Mathematical machines that are generic over other machines.
//!
//! Such a machine is a definition or a law about every machine with the
//! signature its parameter declares. The signature gives it one more formal
//! per machine parameter, of that machine's function type, and the kernel
//! checks it once.

use super::*;
use crate::checking::laws::{SignatureJudgment, UnprovedLaw};

const FIXED: &str =
    include_str!("../../tests/fixtures/generic_machine/pass/twice_a_fixed_point/main.omg");
const UNFOUNDED: &str = include_str!(
    "../../tests/fixtures/generic_machine/fail/twice_without_the_fixed_point/main.omg"
);

fn proofs(source: &str) -> String {
    let start = source.find("data Peano").expect("the fixture's data");
    let end = source.find("data Main").expect("the fixture's entry state");
    source[start..end].to_owned()
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
    crate::checking::judge_compilation(compilation.program)
        .into_iter()
        .map(|verdict| verdict.expect("a judged part"))
        .collect()
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
    let MathematicalCoreDecision::Judged(receipt) = law.decision else {
        panic!("the kernel must judge the law: {law:?}")
    };
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
            "requires\n    core\\Identity::<Peano>(Step(a), a)\nensures\n",
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

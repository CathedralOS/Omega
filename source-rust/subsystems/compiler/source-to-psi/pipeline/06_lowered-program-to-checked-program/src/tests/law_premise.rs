//! Laws that require an equality.
//!
//! A premise is a hypothesis of the law's own proof and an obligation where
//! the law is cited. The cancellation law's successor case holds
//! `Succ(x) = Succ(y)` and cites the law at the field, which requires
//! `x = y`: the checker takes the constructor off both sides. A premise that
//! equates two different cases proves whatever the law states.

use super::*;
use crate::checking::laws::{SignatureJudgment, UnprovedLaw};

const CANCELS: &str =
    include_str!("../../tests/fixtures/law_premise/pass/peano_cancellation/main.omg");
const UNESTABLISHED: &str =
    include_str!("../../tests/fixtures/law_premise/fail/citation_without_its_premise/main.omg");

fn proofs(source: &str) -> String {
    let end = source.find("data Main").expect("the fixture's entry state");
    source[..end].replace("use omega\\language\\core\\ProcessExit;\n", "")
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
fn a_law_with_a_premise_is_a_statement_that_takes_its_proof() {
    use proof_admission::Term;
    let verdicts = judged(&proofs(CANCELS));
    let judgment = signature(&verdicts);
    for machine in ["plus_cancel", "never_zero"] {
        let law = judgment
            .laws
            .iter()
            .find(|law| law.machine == machine)
            .unwrap_or_else(|| panic!("the law `{machine}`: {judgment:?}"));
        let MathematicalCoreDecision::Judged(receipt) = law.decision else {
            panic!("the kernel must judge `{machine}`: {law:?}")
        };
        // Closed, and resting on no assumption: the premise is bound in the
        // statement, not assumed in the signature.
        assert_eq!((receipt.context_depth, receipt.assumption_closure), (0, 0));
        let mut checked =
            terminal_codec::decode_mathematical_certificate(&law.certificate).unwrap();
        proof_admission::verify_mathematical_certificate(
            &mut checked.arena,
            &checked.certificate,
            &mut Default::default(),
        )
        .unwrap();
        // The statement binds each formal, then a proof of the premise, an
        // equality, and concludes an equality.
        let arena = &checked.arena;
        let mut statement = checked.certificate.expected;
        let mut binders = Vec::new();
        while let Term::Pi { domain, codomain } = arena.get(statement) {
            binders.push(domain);
            statement = codomain;
        }
        let (premise, formals) = binders.split_last().expect("a premise");
        assert!(matches!(arena.get(*premise), Term::Id { .. }), "{machine}");
        for formal in formals {
            assert!(matches!(arena.get(*formal), Term::Constant { .. }));
        }
        assert!(matches!(arena.get(statement), Term::Id { .. }));
    }
}

#[test]
fn a_citation_whose_premise_is_not_established_is_refused() {
    let source = proofs(UNESTABLISHED);
    // The passing program with one more law.
    let passing = proofs(CANCELS);
    assert!(
        source[source.find("data Peano").unwrap()..]
            .starts_with(&passing[passing.find("data Peano").unwrap()..])
    );
    let verdicts = judged(&source);
    let law = unproved(&verdicts);
    assert_eq!(law.machine, "cancels_without_its_premise");
    assert_eq!(law.cause, UnprovedCause::NoDerivation);
    assert_eq!(
        law.detail,
        "a cited law's premise is not established at its citation"
    );
}

#[test]
fn without_the_premise_the_law_it_states_is_not_proved() {
    // The cancellation law with its `requires` removed claims `a = b` for
    // every `a` and `b`.
    let passing = proofs(CANCELS);
    let source = passing.replacen(
        "requires\n    core\\Identity::<Peano>(plus(c, a), plus(c, b))\n",
        "",
        1,
    );
    assert_ne!(source, passing);
    let verdicts = judged(&source);
    let law = unproved(&verdicts);
    assert_eq!(law.machine, "plus_cancel");
    assert_eq!(law.cause, UnprovedCause::NoDerivation);
}

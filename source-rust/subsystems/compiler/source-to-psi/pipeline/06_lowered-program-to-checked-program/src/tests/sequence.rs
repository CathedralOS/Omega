//! Proof data that pairs a state with a static sampling machine.
//!
//! `data Sequence<S, const At: Sampler<S>> { state: S; }` is the core
//! rational sequence's shape (quotients.md, "Cauchy sequences and the Real
//! quotient"): each value supplies its state, and the type's machine
//! argument says how a sample is drawn from it. A machine binder gives the
//! data no layout, so every application is proof-only, and the machine an
//! application selects must have exactly its parameter's formals and result.

use super::*;

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

/// The machines whose laws were judged, when every part is accepted.
fn judged_laws(verdicts: &[RouteVerdict]) -> Vec<String> {
    let mut machines = Vec::new();
    for verdict in verdicts {
        match verdict {
            RouteVerdict::Accepted(_) => {}
            RouteVerdict::Laws(judgment) => {
                for law in &judgment.laws {
                    assert!(
                        matches!(law.decision, MathematicalCoreDecision::Judged(_)),
                        "{law:?}"
                    );
                    machines.push(law.machine.clone());
                }
            }
            other => panic!("every part must be accepted: {other:?}"),
        }
    }
    machines
}

fn violations(verdicts: &[RouteVerdict]) -> Vec<String> {
    verdicts
        .iter()
        .filter_map(|verdict| match verdict {
            RouteVerdict::Rejected { violations, .. } => Some(violations.iter()),
            _ => None,
        })
        .flatten()
        .map(|violation| violation.statement.clone())
        .collect()
}

/// A sequence of `N` whose sampler is `sampler`, with the program's
/// `claims` about it.
fn sequences(sampler: &str, claims: &str) -> String {
    format!(
        r"use {{omega}}/language/core/foundation.omg as proof;

data N {{
    case Z;
    case S(p: N);
}}

data M {{
    case Z;
    case S(p: M);
}}

data Sampler<S> = machine(state: S, n: N) -> N
    reaches none crashes none terminates;

data Sequence<S, const At: Sampler<S>> {{
    state: S;
}}

{sampler}

{claims}
"
    )
}

const REPEAT: &str = "machine repeat(state: N, n: N) -> N {
    state
}";

const CONSTANT: &str = "machine constant(q: N) -> Sequence<N, repeat> {
    Sequence { state: q }
}

machine constant_sample(q: N, n: N)
ensures |proof|::Identity::<N>(repeat(constant(q).state, n), q)
{
}

machine any_sample(s: Sequence<N, repeat>, n: N)
ensures |proof|::Identity::<N>(repeat(s.state, n), s.state)
{
}";

#[test]
fn a_sequence_is_sampled_by_its_machine_at_its_state() {
    // `constant(q)` holds `q`, and `repeat` returns its state at every
    // index, so each sample of it is `q`, as is each sample of any sequence
    // with that sampler at its own state.
    let laws = judged_laws(&judged(&sequences(REPEAT, CONSTANT)));
    assert!(laws.iter().any(|law| law == "constant_sample"), "{laws:?}");
    assert!(laws.iter().any(|law| law == "any_sample"), "{laws:?}");
}

#[test]
fn a_sample_is_only_what_the_sampler_computes() {
    let verdicts = judged(&sequences(
        REPEAT,
        "machine first_sample(s: Sequence<N, repeat>, n: N)
ensures |proof|::Identity::<N>(repeat(s.state, n), N::Z)
{
}",
    ));
    assert!(
        verdicts
            .iter()
            .any(|verdict| matches!(verdict, RouteVerdict::UnprovedLaws(_))),
        "{verdicts:?}"
    );
}

const SAMPLER_RULE: &str = "a static machine argument must have its parameter's formals and result";

#[test]
fn a_sampler_must_take_the_state_and_an_index() {
    // `Sampler<N>` takes a state and an index; a machine of the state
    // alone selects no sampler, whatever it computes.
    let verdicts = judged(&sequences(
        "machine repeat(state: N) -> N {
    state
}",
        "machine any_sample(s: Sequence<N, repeat>, n: N)
ensures |proof|::Identity::<N>(repeat(s.state), s.state)
{
}",
    ));
    let violations = violations(&verdicts);
    assert!(
        violations.iter().any(|rule| rule.contains(SAMPLER_RULE)),
        "{verdicts:?}"
    );
}

#[test]
fn a_sampler_must_read_the_sequence_s_own_state() {
    // `Sequence<N, repeat>` holds an `N`, so its sampler is a
    // `Sampler<N>`; one over `M` samples no state this sequence holds.
    let verdicts = judged(&sequences(
        "machine repeat(state: M, n: N) -> N {
    N::Z
}",
        "machine any_sample(s: Sequence<N, repeat>, n: N)
ensures |proof|::Identity::<N>(N::Z, N::Z)
{
}",
    ));
    let violations = violations(&verdicts);
    assert!(
        violations.iter().any(|rule| rule.contains(SAMPLER_RULE)),
        "{verdicts:?}"
    );
}

#[test]
fn a_conditional_sampler_needs_evidence_even_to_form_its_sequence() {
    let verdicts = judged(&sequences(
        "machine only_zero(state: N, n: N) -> N
requires |proof|::Identity::<N>(n, N::Z)
{ state }",
        "machine holds_state(s: Sequence<N, only_zero>)
ensures |proof|::Identity::<N>(s.state, s.state)
{}",
    ));
    assert!(
        verdicts.iter().any(|verdict| matches!(
            verdict,
            RouteVerdict::FallBack(refusal)
                if refusal.detail.contains("record machine argument with an unproved premise")
        )),
        "{verdicts:?}"
    );
    assert!(
        !verdicts
            .iter()
            .any(|verdict| matches!(verdict, RouteVerdict::Laws(_)))
    );
}

const FUNCTION_STATE: &str = "data Fn = machine(n: N) -> N reaches none crashes none terminates;
machine apply(f: Fn, n: N) -> N { f(n) }
machine pack(f: Fn) -> Sequence<Fn, apply> { Sequence { state: f } }
machine sample(s: Sequence<Fn, apply>, n: N) -> N { apply(s.state, n) }";

#[test]
fn the_named_sampler_applies_the_projected_function_state() {
    let accepted = sequences(
        FUNCTION_STATE,
        "machine law(f: Fn, n: N)
ensures |proof|::Identity::<N>(sample(pack(f), n), apply(f, n)) {}",
    );
    use proof_admission::Term;
    let verdicts = judged(&accepted);
    assert!(judged_laws(&verdicts).iter().any(|law| law == "law"));
    let law = verdicts
        .iter()
        .find_map(|part| match part {
            RouteVerdict::Laws(judgment) => judgment.laws.iter().find(|law| law.machine == "law"),
            _ => None,
        })
        .unwrap();
    let MathematicalCoreDecision::Judged(receipt) = law.decision;
    assert_eq!((receipt.context_depth, receipt.assumption_closure), (0, 0));
    let mut proof = terminal_codec::decode_mathematical_certificate(&law.certificate).unwrap();
    let check = |arena: &mut _, certificate: &_| {
        proof_admission::verify_mathematical_certificate(
            arena,
            certificate,
            &mut Default::default(),
        )
        .unwrap();
    };
    check(&mut proof.arena, &proof.certificate);
    // Replay the same proof at the explicit statement about every function
    // and index. Both sampler applications must compute to f(n).
    let arena = &mut proof.arena;
    let Term::Pi {
        domain: function,
        codomain,
    } = arena.get(proof.certificate.expected)
    else {
        panic!("the function formal")
    };
    let Term::Pi {
        domain: index,
        codomain: goal,
    } = arena.get(codomain)
    else {
        panic!("the index formal")
    };
    let Term::Id { ty, .. } = arena.get(goal) else {
        panic!("the sample equality")
    };
    assert!(matches!(arena.get(function), Term::Pi { .. }));
    let f = arena.insert(Term::Variable(1));
    let n = arena.insert(Term::Variable(0));
    let sample = arena.apply_arguments(f, &[n]);
    let goal = arena.insert(Term::Id {
        ty,
        left: sample,
        right: sample,
    });
    let body = arena.insert(Term::Pi {
        domain: index,
        codomain: goal,
    });
    proof.certificate.expected = arena.insert(Term::Pi {
        domain: function,
        codomain: body,
    });
    check(arena, &proof.certificate);
    let wrong = accepted.replace("apply(f, n)) {}", "N::Z) {}");
    assert!(
        judged(&wrong)
            .iter()
            .any(|v| matches!(v, RouteVerdict::UnprovedLaws(_)))
    );
}

#[test]
fn a_function_state_requires_unconditional_termination() {
    let partial = FUNCTION_STATE.replace("crashes none terminates", "crashes none");
    let verdicts = judged(&sequences(
        &partial,
        "machine law(f: Fn, n: N)
ensures |proof|::Identity::<N>(sample(pack(f), n), apply(f, n)) {}",
    ));
    assert!(
        verdicts.iter().any(|verdict| matches!(
            verdict,
            RouteVerdict::FallBack(refusal)
                if refusal.detail.contains("without an unconditional mathematical denotation")
        )),
        "{verdicts:?}"
    );
}

#[test]
fn quantified_sample_evidence_belongs_to_its_exact_sequence() {
    let source = sequences(
        REPEAT,
        "machine observation(s: Sequence<N, repeat>, n: N) -> N { repeat(s.state, n) }
let zero_samples(s: Sequence<N, repeat>): |proof|::Type<0> =
    (i: N) -> (j: N) -> |proof|::Identity::<N>(observation(s, j), N::Z);
machine keep(s: Sequence<N, repeat>, t: Sequence<N, repeat>)
requires zero_samples(s)
ensures zero_samples(s) {}
machine observe(s: Sequence<N, repeat>, n: N)
ensures |proof|::Identity::<N>(observation(s,n), s.state) {}",
    );
    let verdicts = judged(&source);
    assert!(judged_laws(&verdicts).iter().any(|law| law == "keep"));
    let wrong = source.replace("ensures zero_samples(s)", "ensures zero_samples(t)");
    assert!(judged(&wrong).iter().any(|v| matches!(
        v, RouteVerdict::UnprovedLaws(laws) if laws.iter().any(|law| law.machine == "keep")
    )));
}

#[test]
fn sequence_record_construction_checks_the_exact_sample_domain() {
    let domain = "machine observe(s: Sequence<N, repeat>, n: N) -> N { repeat(s.state,n) }
let positive(s: Sequence<N, repeat>): |proof|::Type<0> =
    (i: N) -> |proof|::Identity::<N>(observe(s,i), N::S { p: N::Z });
data Admitted requires positive(samples) { samples: Sequence<N, repeat>; }
machine pack(s: Sequence<N, repeat>, t: Sequence<N, repeat>) -> Admitted
requires positive(s)
{ Admitted { samples:s } }
machine keeps(s: Sequence<N, repeat>, t: Sequence<N, repeat>, n: N)
requires positive(s)
ensures |proof|::Identity::<N>(observe(pack(s,t).samples,n), observe(s,n)) {}";
    let source = sequences(REPEAT, domain);
    let verdicts = judged(&source);
    assert!(judged_laws(&verdicts).iter().any(|law| law == "keeps"));
    for absent in [
        source.replace("requires positive(s)", ""),
        source.replace("requires positive(s)", "requires positive(t)"),
    ] {
        let verdicts = judged(&absent);
        assert!(
            verdicts.iter().any(|v| matches!(
                v, RouteVerdict::UnprovedLaws(laws)
                if laws.iter().any(|law| law.statement.contains("domain of `Admitted`"))
            )),
            "{verdicts:?}"
        );
    }
}

#[test]
fn a_received_record_carries_its_strict_domain() {
    let source = sequences(
        REPEAT,
        "
data OnlyZero requires |proof|::Squash::<|proof|::Identity::<N>(value, N::Z)> { value: N; }
machine wrap(n: N) -> OnlyZero requires |proof|::Squash::<|proof|::Identity::<N>(n, N::Z)> {
    OnlyZero { value: n }
}
machine lawful(n: N) requires |proof|::Squash::<|proof|::Identity::<N>(n, N::Z)>
ensures |proof|::Identity::<N>(wrap(n).value, n) {}
machine stored(p: OnlyZero) ensures |proof|::Squash::<|proof|::Identity::<N>(p.value, N::Z)> {}",
    );
    let verdicts = judged(&source);
    let machines = judged_laws(&verdicts);
    for name in ["lawful", "stored"] {
        assert!(machines.iter().any(|proved| proved == name));
        let law = verdicts
            .iter()
            .find_map(|verdict| match verdict {
                RouteVerdict::Laws(judgment) => {
                    judgment.laws.iter().find(|law| law.machine == name)
                }
                _ => None,
            })
            .unwrap();
        let mut proof = terminal_codec::decode_mathematical_certificate(&law.certificate).unwrap();
        proof_admission::verify_mathematical_certificate(
            &mut proof.arena,
            &proof.certificate,
            &mut Default::default(),
        )
        .expect("the receiver checks the record and its projected condition");
        if name == "stored" {
            use mathematical_kernel::Term;
            let Term::Pi { domain, .. } = proof.arena.get(proof.certificate.expected) else {
                panic!("the theorem takes a record");
            };
            let Term::Constant { declaration, .. } = proof.arena.get(domain) else {
                panic!("the input names its declared record type");
            };
            let record = proof.certificate.signature[declaration as usize]
                .body
                .expect("defined record");
            let Term::Sigma { codomain, .. } = proof.arena.get(record) else {
                panic!("the record retains both its value and its evidence");
            };
            assert!(matches!(proof.arena.get(codomain), Term::Box { .. }));
        }
        assert!(proof.certificate.context.is_empty());
        let MathematicalCoreDecision::Judged(receipt) = law.decision;
        assert_eq!(receipt.assumption_closure, 0);
    }
    for (invalid, machine) in [
        (
            source.replace(
                "-> OnlyZero requires |proof|::Squash::<|proof|::Identity::<N>(n, N::Z)>",
                "-> OnlyZero",
            ),
            "wrap",
        ),
        (
            source
                .replace("stored(p: OnlyZero)", "stored(p: OnlyZero, other: N)")
                .replace("::<N>(p.value, N::Z)", "::<N>(other, N::Z)"),
            "stored",
        ),
        (
            source.replace("::<N>(p.value, N::Z)", "::<N>(p.value, N::S { p: N::Z })"),
            "stored",
        ),
    ] {
        let verdicts = judged(&invalid);
        assert!(
            verdicts.iter().any(|v| matches!(v,
                RouteVerdict::UnprovedLaws(laws) if laws.iter().any(|law| law.machine == machine)
            )),
            "{verdicts:?}"
        );
    }
}

#[path = "support/known_gap.rs"]
mod known_gap;
#[path = "support/source.rs"]
mod sources;

use MathematicalStep::{Apply as ApplyTerm, Declaration, FormProposition, Parameter};
use PredicateOperator::{GreaterThan, NotEqual};
use PredicateStep::{Apply, Formal, HypothesisGuarantee, HypothesisParameter, Integer, MathematicalTerm};
use omega_lowering::{MathematicalStep, PredicateOperator, PredicateStep, ScalarType, TheoremBinder, TypeExpression, TypeExpressionStep};
use omega_typed::{ObligationKind, ValueType};

const TRUST: &str = "boundary let trusted(value:u8):core\\Strict<0>;";
const THEOREM: &str = "machine theorem<machine Step>(x:u8,z:u8) requires machine Step(y:u8) requires y != 0 ensures y > z; requires trusted(x) requires x != 0 ensures Step::ensures(x) {}";
const LEMMA: &str = "machine lemma(y:u8) requires y != 0 ensures y > 0 {}";

fn byte() -> TypeExpression {
    TypeExpression(vec![TypeExpressionStep::Scalar(ScalarType::Integer {
        signed: false,
        width: 8,
    })])
}

#[test]
fn a_machine_shaped_hypothesis_lowers_as_erased_evidence_with_exact_binders_goal_and_assumptions() {
    let text = format!("{TRUST} {THEOREM} {LEMMA}");
    let package = sources::lower(&[("proof.omg", &text)]).unwrap();
    // Only the ordinary lemma keeps an executable identity; the theorem is proof-only.
    let [lemma] = package.machines.as_slice() else {
        panic!("a logical theorem must not publish a runtime machine")
    };
    let [trusted] = package.mathematics.as_slice() else {
        panic!("expected the boundary assumption")
    };
    let [theorem] = package.theorems.as_slice() else {
        panic!("expected one logical theorem")
    };
    assert_ne!(lemma.declaration, theorem.declaration);
    assert!(package.machine_templates.is_empty());
    assert_eq!(theorem.binders, [TheoremBinder::Hypothesis(0)]);
    assert_eq!(theorem.parameters, [byte(), byte()]);

    // The hypothesis binds its own subject; the outer formal z stays a theorem formal.
    let [step] = theorem.hypotheses.as_slice() else {
        panic!("expected the Step hypothesis")
    };
    assert_eq!(step.parameters, [byte()]);
    let y = HypothesisParameter { hypothesis: 0, index: 0 };
    let [premise] = step.premises.as_slice() else {
        panic!("expected one premise")
    };
    let [conclusion] = step.conclusions.as_slice() else {
        panic!("expected one conclusion")
    };
    assert_eq!(premise.predicate, [y.clone(), Integer(0.into()), Apply(NotEqual)]);
    assert_eq!(conclusion.predicate, [y, Formal(1), Apply(GreaterThan)]);

    // The strict premise keeps its exact boundary assumption; it is not a Boolean.
    let strict = TypeExpression(vec![
        TypeExpressionStep::Level(0u8.into()),
        TypeExpressionStep::Universe { strict: true },
    ]);
    let application = ApplyTerm {
        generic_arguments: vec![],
        argument_count: 1,
        assumptions: vec![trusted.declaration.clone()],
    };
    assert_eq!(trusted.assumptions.as_slice(), std::slice::from_ref(&trusted.declaration));
    let [assumed, nonzero] = theorem.requires.as_slice() else {
        panic!("expected both authored requirements")
    };
    assert_eq!(
        assumed.predicate,
        [MathematicalTerm(vec![
            Declaration(trusted.declaration.clone()),
            Parameter(0),
            application,
            FormProposition(strict)
        ])]
    );
    assert_eq!(nonzero.predicate, [Formal(0), Integer(0.into()), Apply(NotEqual)]);

    // The goal applies the hypothesis to the exact formal subject.
    let goal = [
        Formal(0),
        HypothesisGuarantee {
            hypothesis: 0,
            operand_count: 1,
        },
    ];
    let [ensures] = theorem.ensures.as_slice() else { panic!("expected one goal") };
    assert_eq!(ensures.predicate, goal);
    let other = format!("{TRUST} {}", THEOREM.replace("Step::ensures(x)", "Step::ensures(z)"));
    let package = sources::lower(&[("proof.omg", &other)]).unwrap();
    assert_eq!(
        package.theorems[0].ensures[0].predicate,
        [
            Formal(1),
            HypothesisGuarantee {
                hypothesis: 0,
                operand_count: 1
            }
        ]
    );

    // A mathematical premise over the outer formal keeps that formal.
    let outer = format!("{TRUST} {}", THEOREM.replace("requires y != 0", "requires trusted(z)"));
    let package = sources::lower(&[("proof.omg", &outer)]).unwrap();
    let [MathematicalTerm(term)] = package.theorems[0].hypotheses[0].premises[0]
        .predicate
        .as_slice()
    else {
        panic!("expected a strict premise")
    };
    assert_eq!(term[1], Parameter(1));
}

#[test]
fn hypothesis_evidence_never_becomes_executable_or_rebinds_its_subject() {
    // Proof-use twin: the same premise over the hypothesis's own subject must
    // not be rebound to a theorem formal.
    let rebound = format!("{TRUST} {}", THEOREM.replace("requires y != 0", "requires trusted(y)"));
    let package = sources::lower(&[("proof.omg", &rebound)]).unwrap();
    let [MathematicalTerm(term)] = package.theorems[0].hypotheses[0].premises[0]
        .predicate
        .as_slice()
    else {
        panic!("hypothesis premise")
    };
    assert_eq!(term[1], MathematicalStep::HypothesisParameter { hypothesis: 0, index: 0 });

    // Citing the theorem from executable code acquires no runtime target.
    let theorem = THEOREM.replace("requires trusted(x) ", "");
    let cited = format!("{theorem} {LEMMA} machine run(x:u8){{theorem::<lemma>(x,x)}}");
    let files = [("proof.omg", cited.as_str())];
    let resolved = sources::resolve(&files);
    let prepared = omega_preparation::prepare_source_package(&resolved).expect("the theorem citation reaches lowering");
    // The supplied evidence reaches L5 at the citation. Psi has no erased proof application to receive it.
    let supplied: Vec<_> = prepared
        .typed()
        .obligations
        .iter()
        .filter(|o| matches!(o.kind, ObligationKind::SupplyHypothesis { .. }))
        .collect();
    let [supply] = supplied.as_slice() else {
        panic!("expected one supplied hypothesis, found {supplied:?}")
    };
    assert!(matches!(&supply.kind, ObligationKind::SupplyHypothesis { evidence: ValueType::ProofCitation(lemma), .. } if lemma.path == "lemma"));
    let error = sources::refuse_prepared(
        &prepared,
        &files,
        "proof.omg",
        sources::unique_span(&cited, "theorem::<lemma>(x,x)"),
        "a logical theorem citation requires erased proof-application lowering",
    );
    assert_eq!(error.site, supply.site, "the refusal stops at the supplied evidence");
    known_gap::report(
        "cite a theorem from executable code",
        "The call site needs an erased proof application",
        &cited,
        &error,
    );

    let declaration = THEOREM.replace("ensures Step::ensures(x) {}", "ensures Step::ensures(x) { let w = x; }");
    let worked = format!("{TRUST} {declaration}");
    let error = sources::refuse(
        &[("proof.omg", &worked)],
        "proof.omg",
        sources::unique_span(&worked, &declaration),
        "a logical theorem body requires checked proof-term lowering",
    );
    known_gap::report("proof steps in a theorem body", "The theorem needs its checked proof term", &worked, &error);
}

#[test]
fn a_result_bearing_hypothesis_stops_at_its_result_annotation() {
    // Squashed existence does not promise to produce the underlying byte. It
    // is a valid typed result even though Psi cannot publish it yet.
    let source = "machine theorem<machine Step>(x:u8) requires machine Step(y:u8)->core\\Squash<u8> requires y != 0; requires x != 0 {}";
    let files = [("proof.omg", source)];
    let resolved = sources::resolve(&files);
    let prepared = omega_preparation::prepare_source_package(&resolved).unwrap();
    let [step] = prepared.typed().logical_hypotheses.as_slice() else { panic!("expected Step") };
    assert_eq!(step.result, ValueType::Squash(Box::new(ValueType::Integer { signed: false, width: Some(8) })));
    sources::refuse_prepared(
        &prepared,
        &files,
        "proof.omg",
        sources::unique_span(source, "->core\\Squash<u8>"),
        "Psi cannot represent result `core\\Squash<u8>` on logical hypothesis `Step`",
    );

    let twin = source.replace("->core\\Squash<u8>", "");
    let package = sources::lower(&[("proof.omg", &twin)]).unwrap();
    assert!(package.machines.is_empty());
    let [theorem] = package.theorems.as_slice() else { panic!("expected one theorem") };
    assert_eq!(theorem.binders, [TheoremBinder::Hypothesis(0)]);
    assert_eq!(
        theorem.hypotheses[0].premises[0].predicate,
        [HypothesisParameter { hypothesis: 0, index: 0 }, Integer(0.into()), Apply(NotEqual)]
    );
}

#[test]
fn a_hypothesis_without_a_matching_theorem_binder_stops_at_its_name() {
    let source = "machine theorem(x:u8) requires machine Step(y:u8) requires y != 0 ensures y > 0; requires x != 0 {}";
    let files = [("proof.omg", source)];
    let resolved = sources::resolve(&files);
    let prepared = omega_preparation::prepare_source_package(&resolved).unwrap();
    let typed = prepared.typed();
    let [step] = typed.logical_hypotheses.as_slice() else { panic!("expected Step") };
    assert_eq!(step.name, "Step");
    assert!(typed.generic_parameters.get(&step.owner).is_none_or(|t| t.parameters.is_empty()));
    sources::refuse_prepared(
        &prepared,
        &files,
        "proof.omg",
        sources::unique_span(source, "Step"),
        "lowering has no matching `machine Step` binder for logical hypothesis `Step`",
    );

    let twin = source.replace("theorem(", "theorem<machine Step>(");
    let package = sources::lower(&[("proof.omg", &twin)]).unwrap();
    assert!(package.machines.is_empty());
    let [theorem] = package.theorems.as_slice() else { panic!("expected one theorem") };
    assert_eq!(theorem.binders, [TheoremBinder::Hypothesis(0)]);
    assert_eq!(
        theorem.hypotheses[0].conclusions[0].predicate,
        [HypothesisParameter { hypothesis: 0, index: 0 }, Integer(0.into()), Apply(GreaterThan)]
    );
}

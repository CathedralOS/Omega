#[path = "support/source.rs"]
mod sources;

use MathematicalStep::{Apply as ApplyTerm, Declaration, FormProposition, Parameter};
use PredicateOperator::{GreaterThan, NotEqual};
use PredicateStep::{Apply, Formal, HypothesisGuarantee, HypothesisParameter, Integer, MathematicalTerm};
use omega_lowering::{
    CallRequirement, CallSite, MathematicalStep, OperationKind, OperationResult, Parameter as BodyParameter, PredicateOperator, PredicateProgress,
    PredicateStep, ProofInput, ProofStep, ProofSubject, ScalarLiteral, ScalarType, TheoremApplication, TheoremArgument, TheoremBinder, TypeExpression,
    TypeExpressionStep,
};
use omega_typed::{CallUse, ObligationKind, ValueType};

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

    // The original body binds an erased proof local. The binding names the
    // theorem's formal x and proves nothing; Step::ensures(x) stays the goal.
    let declaration = THEOREM.replace("ensures Step::ensures(x) {}", "ensures Step::ensures(x) { let w = x; }");
    let worked = format!("{TRUST} {declaration}");
    let package = sources::lower(&[("proof.omg", &worked)]).unwrap();
    assert!(package.machines.is_empty() && package.machine_templates.is_empty() && package.callable_contracts.is_empty());
    let [theorem] = package.theorems.as_slice() else { panic!("expected one logical theorem") };
    let [ProofStep::Bind { site, .. }] = theorem.proof.as_slice() else { panic!("expected the binding step") };
    assert_eq!(site.span.get_start_offset()..site.span.get_end_offset(), sources::unique_span(&worked, "let w = x;"));
    spec_corpus::observe_handoff!(psi "psi-output/theorem-proof-binding", assert_eq!(
        theorem.proof,
        [ProofStep::Bind { subject: ProofSubject::Input(ProofInput::Formal(0)), site: site.clone() }]
    ));
    assert_eq!(theorem.ensures[0].predicate, [Formal(0), HypothesisGuarantee { hypothesis: 0, operand_count: 1 }]);
    // Consumer: a binding owes and imports nothing. Binding z instead names formal 1.
    assert!(psi_handoff_consumer::theorem_proof(&package, theorem).expect("well-formed proof").is_empty());
    let other = format!("{TRUST} {}", declaration.replace("let w = x;", "let w = z;"));
    let package = sources::lower(&[("proof.omg", &other)]).unwrap();
    let [ProofStep::Bind { subject, .. }] = package.theorems[0].proof.as_slice() else { panic!("expected the binding step") };
    assert_eq!(subject, &ProofSubject::Input(ProofInput::Formal(1)));
}

#[test]
fn a_theorem_body_cites_a_checked_machine_at_erased_subjects() {
    let theorem = |body: &str| THEOREM.replace("requires trusted(x) ", "").replace("{}", body);
    let source = |body: &str| format!("{} {LEMMA}", theorem(body));
    let cited = source("{ let w = x; lemma(w); lemma(7); }");
    let package = sources::lower(&[("proof.omg", &cited)]).unwrap();
    let [theorem_row] = package.theorems.as_slice() else { panic!("expected the theorem") };
    // Only lemma is executable, and it has no operation that calls anything.
    let [lemma] = package.machines.as_slice() else { panic!("expected only lemma") };
    assert!(lemma.body.operations.is_empty());
    let [ProofStep::Bind { .. }, ProofStep::CiteMachine { site: first, .. }, ProofStep::CiteMachine { site: second, .. }] = theorem_row.proof.as_slice()
    else {
        panic!("expected a binding and two citations, found {:?}", theorem_row.proof)
    };
    let cite = |subject, site: &omega_lowering::Site| ProofStep::CiteMachine {
        machine: lemma.declaration.clone(),
        subjects: vec![subject],
        requirements: 0..1,
        progress: PredicateProgress::CheckPrivateBody,
        site: site.clone(),
    };
    assert_eq!(first.span.get_start_offset()..first.span.get_end_offset(), sources::unique_span(&cited, "lemma(w)"));
    spec_corpus::observe_handoff!(psi "psi-output/theorem-proof-citation", assert_eq!(
        theorem_row.proof[1..],
        [
            cite(ProofSubject::Input(ProofInput::Local(0)), first),
            cite(ProofSubject::Literal(ScalarLiteral::Integer(7.into())), second),
        ]
    ));

    // Consumer: each citation owes lemma's premise and imports its guarantee,
    // with w resolved to formal x and the literal kept as itself.
    let citations = psi_handoff_consumer::theorem_proof(&package, theorem_row).expect("well-formed proof");
    let [at_w, at_seven] = citations.as_slice() else { panic!("expected two citations") };
    assert_eq!((at_w.step, at_w.machine.declaration.clone()), (1, lemma.declaration.clone()));
    assert_eq!(at_w.subjects, [ProofSubject::Input(0)]);
    assert_eq!(at_seven.subjects, [ProofSubject::Literal(ScalarLiteral::Integer(7.into()))]);
    assert_eq!(at_w.owed[0].predicate, [Formal(0), Integer(0.into()), Apply(NotEqual)]);
    assert_eq!(at_w.imported[0].predicate, [Formal(0), Integer(0.into()), Apply(GreaterThan)]);
    assert_eq!(at_w.progress, PredicateProgress::CheckPrivateBody);

    // Twins: citing z directly names formal 1; a folded constant is the same
    // literal, and a parenthesized literal keeps its landing.
    let package = sources::lower(&[("proof.omg", &source("{ lemma(z); lemma(3+4); lemma((5)); }"))]).unwrap();
    let subjects: Vec<_> = package.theorems[0]
        .proof
        .iter()
        .map(|step| match step {
            ProofStep::CiteMachine { subjects, .. } => subjects.clone(),
            _ => panic!("expected a machine citation"),
        })
        .collect();
    assert_eq!(
        subjects,
        [
            vec![ProofSubject::Input(ProofInput::Formal(1))],
            vec![ProofSubject::Literal(ScalarLiteral::Integer(7.into()))],
            vec![ProofSubject::Literal(ScalarLiteral::Integer(5.into()))]
        ]
    );

    // A helper with a Trap route supplies no total proof: the theorem has no
    // crash route to cover it. Publishing `crashes none` lowers the citation.
    let trapping = format!("{} machine lemma(y:u8) crashes Trap requires y != 0 ensures y > 0 {{}}", theorem("{ lemma(x); }"));
    sources::refuse(
        &[("proof.omg", &trapping)],
        "proof.omg",
        sources::unique_span(&trapping, "lemma(x)"),
        "proof citation requires a terminating helper with an empty crash ceiling",
    );
}

#[test]
fn citing_a_theorem_from_executable_code_applies_it_without_a_runtime_call() {
    let theorem = THEOREM.replace("requires trusted(x) ", "");
    let other = "machine other(y:u8) requires y != 0 ensures y > 1 {}";
    let source = |citation: &str| format!("{theorem} {LEMMA} {other} machine run(x:u8) requires x != 0 {{let zero:u8 = 0; {citation};}}");
    let cited = source("theorem::<lemma>(x,zero)");
    let files = [("proof.omg", cited.as_str())];
    let resolved = sources::resolve(&files);
    let prepared = omega_preparation::prepare_source_package(&resolved).expect("the theorem citation reaches lowering");

    // Producer: L3 types the citation as an executable statement whose
    // evidence is the named lemma and whose theorem precondition is owed here.
    let typed = prepared.typed();
    let span = sources::unique_span(&cited, "theorem::<lemma>(x,zero)");
    let [call] = typed.calls.iter().filter(|c| c.machine.path == "theorem").collect::<Vec<_>>()[..] else {
        panic!("expected one theorem citation")
    };
    assert_eq!(call.use_site, CallUse::Execution);
    assert_eq!(call.site.span.get_start_offset()..call.site.span.get_end_offset(), span);
    let at_call: Vec<_> = typed.obligations.iter().filter(|o| o.site == call.site).map(|o| &o.kind).collect();
    let ([ObligationKind::SupplyHypothesis { evidence: ValueType::ProofCitation(lemma), .. }, ObligationKind::CallPrecondition(precondition)]
    | [ObligationKind::CallPrecondition(precondition), ObligationKind::SupplyHypothesis { evidence: ValueType::ProofCitation(lemma), .. }]) = at_call[..]
    else {
        panic!("expected the supplied lemma and the theorem precondition, found {at_call:?}")
    };
    assert_eq!((lemma.path.as_str(), precondition), ("lemma", &call.machine));

    let package = sources::lower_prepared(&prepared, &files).unwrap();
    let [theorem_row] = package.theorems.as_slice() else { panic!("expected the theorem") };
    let machine = |name: &str| package.machines.iter().find(|m| m.declaration.path == name).unwrap_or_else(|| panic!("no machine {name}"));
    // Only lemma, other and run are executable. No operation calls a machine.
    assert_eq!(package.machines.len(), 3);
    let (lemma, run) = (machine("lemma"), machine("run"));
    let [BodyParameter::Scalar(x)] = *run.body.parameters.as_slice() else { panic!("expected x") };
    let [constant, op] = run.body.operations.as_slice() else { panic!("expected the zero binding and the citation") };
    let OperationResult::Scalar(zero) = constant.result else { panic!("zero is a scalar") };
    spec_corpus::observe_handoff!(psi "psi-output/theorem-citation", assert_eq!(
        op.kind,
        OperationKind::ApplyTheorem(Box::new(TheoremApplication {
            theorem: theorem_row.declaration.clone(),
            arguments: vec![TheoremArgument::Citation(lemma.declaration.clone())],
            subjects: vec![ProofSubject::Input(x), ProofSubject::Input(zero)],
            requirements: vec![CallRequirement { site: CallSite::Operation(op.id), clause: 0 }],
        }))
    ));
    assert_eq!(op.result, OperationResult::Unit);
    assert_eq!(run.body.diagnostics.operations[1], call.site);

    // Consumer: from Psi alone the citation owes x != 0 and the lemma's
    // refinement of Step at z = zero, and then imports Step::ensures(x).
    let citations = psi_handoff_consumer::theorem_citations(&package, &run.body).expect("well-formed citation");
    let [citation] = citations.as_slice() else { panic!("expected one citation") };
    assert_eq!(citation.subjects, [ProofSubject::Input(x), ProofSubject::Input(zero)]);
    let [owed] = citation.owed.as_slice() else { panic!("expected one owed premise") };
    assert_eq!(owed.predicate, [Formal(0), Integer(0.into()), Apply(NotEqual)]);
    assert_eq!(citation.evidence.iter().map(|(h, m)| (*h, &m.declaration)).collect::<Vec<_>>(), [(0, &lemma.declaration)]);
    assert_eq!(citation.imported[0].predicate, [Formal(0), HypothesisGuarantee { hypothesis: 0, operand_count: 1 }]);

    // Twins: swapped subjects swap the substitution; other evidence names other.
    let application = |citation: &str| {
        let text = source(citation);
        let package = sources::lower(&[("proof.omg", &text)]).unwrap();
        let run = package.machines.iter().find(|m| m.declaration.path == "run").unwrap();
        let OperationKind::ApplyTheorem(application) = &run.body.operations[1].kind else { panic!("expected the citation") };
        let other = package.machines.iter().find(|m| m.declaration.path == "other").unwrap().declaration.clone();
        (application.subjects.clone(), application.arguments.clone(), other)
    };
    let (subjects, arguments, _) = application("theorem::<lemma>(zero,x)");
    let input = ProofSubject::Input;
    assert_eq!((subjects, arguments), (vec![input(zero), input(x)], vec![TheoremArgument::Citation(lemma.declaration.clone())]));
    let (subjects, arguments, other) = application("theorem::<other>(x,zero)");
    assert_eq!((subjects, arguments), (vec![input(x), input(zero)], vec![TheoremArgument::Citation(other)]));

    // The original literal subject is published in the citation. No operation
    // computes 7, so run keeps only the zero binding and the citation. This
    // substitution is an L6 negative: lemma's y > 0 does not refine y > 7.
    let computed = source("theorem::<lemma>(x,7)");
    let package = sources::lower(&[("proof.omg", &computed)]).unwrap();
    let run = package.machines.iter().find(|m| m.declaration.path == "run").unwrap();
    let [constant, op] = run.body.operations.as_slice() else { panic!("expected the zero binding and the citation only") };
    assert_eq!(constant.kind, OperationKind::Constant(ScalarLiteral::Integer(0.into())));
    let OperationKind::ApplyTheorem(application) = &op.kind else { panic!("expected the citation") };
    let BodyParameter::Scalar(x) = run.body.parameters[0] else { panic!("expected x") };
    spec_corpus::observe_handoff!(psi "psi-output/theorem-literal-subject", assert_eq!(
        application.subjects,
        [ProofSubject::Input(x), ProofSubject::Literal(ScalarLiteral::Integer(7.into()))]
    ));
    let citations = psi_handoff_consumer::theorem_citations(&package, &run.body).expect("well-formed citation");
    assert_eq!(citations[0].subjects, application.subjects.as_slice());
    assert_eq!(citations[0].owed.len(), 1);

    // Twins: the L6 positive 0, a folded 3+4 and a frozen constant N publish
    // their literal; a subject computed from x still refuses at its operand.
    let literal = |citation: &str, prefix: &str| {
        let text = format!("{prefix}{}", source(citation));
        let package = sources::lower(&[("proof.omg", &text)]).unwrap();
        let run = package.machines.iter().find(|m| m.declaration.path == "run").unwrap();
        let OperationKind::ApplyTheorem(application) = &run.body.operations.last().unwrap().kind else { panic!("expected the citation") };
        (run.body.operations.len(), application.subjects[1].clone())
    };
    let seven = ProofSubject::Literal(ScalarLiteral::Integer(7.into()));
    assert_eq!(literal("theorem::<lemma>(x,0)", ""), (2, ProofSubject::Literal(ScalarLiteral::Integer(0.into()))));
    assert_eq!(literal("theorem::<lemma>(x,3+4)", ""), (2, seven.clone()));
    assert_eq!(literal("theorem::<lemma>(x,N)", "const N:u8 = 7; "), (2, seven));
}

#[test]
fn computed_subjects_retain_erased_arithmetic_and_its_formation_obligation() {
    let theorem = THEOREM.replace("requires trusted(x) ", "");
    let executable = format!("{theorem} {LEMMA} machine run(x:u8) requires x != 0 {{theorem::<lemma>(x,x+1);}}");
    let cited = format!("{} {LEMMA}", theorem.replace("{}", "{ lemma(x+1); }"));
    let bound = format!("{} {LEMMA}", theorem.replace("{}", "{ let w = x + 1; }"));
    let wrapping = executable.replace("x+1", "x+%1");
    let observe = |text: &str, source: &str| {
        let pkg = sources::lower(&[("proof.omg", text)]).unwrap();
        use omega_psi::{OperationKind, Parameter, ProofInput, ProofStep, ProofSubject};
        let decide = |subject: &ProofSubject<ProofInput>, expected| [1,255].map(|v| psi_handoff_consumer::byte_subject_addition(subject, expected, v));
        if let Some(run) = pkg.machines.iter().find(|m| m.declaration.path == "run") {
            let [op] = &run.body.operations[..] else { panic!("erased operand emitted an executable operation") };
            let OperationKind::ApplyTheorem(app) = &op.kind else { panic!("expected citation") };
            let Parameter::Scalar(input) = run.body.parameters[0] else { panic!("expected scalar") };
            let [_, subject] = &app.subjects[..] else { panic!("expected both subjects") };
            let ProofSubject::IntegerTerm(steps) = subject else { panic!("missing erased term") };
            assert!(matches!(steps.last(), Some(omega_psi::ProofIntegerStep::Operation {site, ..}) if site.span.slice(text.as_bytes()) == Some(source.as_bytes())));
            [1,255].map(|v| psi_handoff_consumer::byte_subject_addition(subject, input, v))
        } else {
            let [step] = &pkg.theorems[0].proof[..] else { panic!("expected single proof step") };
            let subject = match step {
                ProofStep::Bind { subject, .. } => subject,
                ProofStep::CiteMachine { subjects, .. } => &subjects[0],
                _ => panic!("expected bind or machine citation"),
            };
            decide(subject, ProofInput::Formal(0))
        }
    };
    spec_corpus::observe_handoff!(psi "psi-output/erased-arithmetic-subject", assert_eq!(
        [observe(&executable,"x+1"), observe(&cited,"x+1"), observe(&bound,"x + 1"), observe(&wrapping,"x+%1")],
        [[Some(2),None], [Some(2),None], [Some(2),None], [Some(2),Some(0)]]
    ));
}

#[test]
fn a_theorem_body_cites_a_theorem_with_its_evidence_and_erased_subjects() {
    let inner = "machine inner<machine H>(a:u8) requires machine H(y:u8) requires y != 0 ensures y > 0; requires a != 0 ensures H::ensures(a) {}";
    let outer = THEOREM.replace("requires trusted(x) ", "").replace("{}", "{ inner::<lemma>(x); }");
    let original = format!("{inner} {outer} {LEMMA}");
    let twin = format!("{} {}", original.replace("inner::<lemma>(x)", "inner::<other>(z)"), LEMMA.replace("lemma", "other"));
    let observe = |text: &str| {
        let package = sources::lower(&[("proof.omg", text)]).unwrap();
        let outer = package.theorems.iter().find(|t| t.declaration.path == "theorem").unwrap();
        let [citation] = &psi_handoff_consumer::nested_theorem_citations(&package, outer).unwrap()[..] else { panic!("expected one nested theorem application") };
        let [(0, evidence)] = &citation.evidence[..] else { panic!("missing binder evidence") };
        let [ProofSubject::Input(subject)] = &citation.subjects[..] else { panic!("missing formal substitution") };
        assert_eq!(citation.owed[0].predicate, [Formal(0), Integer(0.into()), Apply(NotEqual)]);
        assert_eq!(citation.imported[0].predicate, [Formal(0), HypothesisGuarantee { hypothesis: 0, operand_count: 1 }]);
        assert_eq!(citation.theorem.hypotheses[0].conclusions[0].predicate,
            [HypothesisParameter { hypothesis: 0, index: 0 }, Integer(0.into()), Apply(GreaterThan)]);
        assert!(!package.machines.iter().any(|m| m.declaration == outer.declaration || m.declaration == citation.theorem.declaration));
        (citation.theorem.declaration.path.clone(), evidence.declaration.path.clone(), *subject)
    };
    spec_corpus::observe_handoff!(psi "psi-output/nested-theorem-application", assert_eq!(
        [observe(&original), observe(&twin)], [("inner".into(), "lemma".into(), 0), ("inner".into(), "other".into(), 1)]
    ));
}

#[test]
fn a_logical_hypothesis_preserves_its_optional_mathematical_result() {
    for (annotation, witness) in [
        ("->core\\Squash<u8>", Some((ValueType::Integer { signed: false, width: Some(8) }, ScalarType::Integer { signed: false, width: 8 }))),
        ("->core\\Squash<bool>", Some((ValueType::Bool, ScalarType::Boolean))),
        ("", None),
    ] {
        let source = format!("machine theorem<machine Step>(x:u8) requires machine Step(y:u8){annotation} requires y != 0; requires x != 0 {{}}");
        let files = [("proof.omg", source.as_str())];
        let resolved = sources::resolve(&files);
        let prepared = omega_preparation::prepare_source_package(&resolved).unwrap();
        let [step] = prepared.typed().logical_hypotheses.as_slice() else { panic!("expected Step") };
        assert_eq!(step.result, witness.as_ref().map_or(ValueType::NoResult, |(ty, _)| ValueType::Squash(Box::new(ty.clone()))));

        let package = sources::lower_prepared(&prepared, &files).unwrap();
        assert!(package.machines.is_empty() && package.machine_templates.is_empty() && package.callable_contracts.is_empty());
        let [theorem] = package.theorems.as_slice() else { panic!("expected one proof-only theorem") };
        assert_eq!(theorem.binders, [TheoremBinder::Hypothesis(0)]);
        let [hypothesis] = theorem.hypotheses.as_slice() else { panic!("expected only Step") };
        assert_eq!(hypothesis.site, step.site);
        assert_eq!(hypothesis.parameters, [byte()]);
        let [premise] = hypothesis.premises.as_slice() else { panic!("expected one premise") };
        assert_eq!(premise.predicate, [HypothesisParameter { hypothesis: 0, index: 0 }, Integer(0.into()), Apply(NotEqual)]);
        spec_corpus::observe_handoff!(psi "psi-output/logical-hypothesis-result", assert_eq!(
            hypothesis.result,
            witness.map(|(_, carrier)| TypeExpression(vec![TypeExpressionStep::Scalar(carrier), TypeExpressionStep::Squash]))
        ));
    }
}

#[test]
fn projecting_a_result_bearing_hypothesis_supplies_the_result_as_a_proof_subject() {
    let source = "boundary let accepted(p:core\\Squash<u8>):core\\Strict<0>;
        machine theorem<machine Other,machine Step>(x:u8,p:core\\Squash<u8>)
        requires machine Other(z:u8)->core\\Squash<u8> ensures accepted(result);
        requires machine Step(y:u8)->core\\Squash<u8> ensures Other::ensures(y,result);
        ensures Step::ensures(x,p) {}";
    let package = sources::lower(&[("proof.omg", source)]).unwrap();
    let [theorem] = package.theorems.as_slice() else { panic!("expected the theorem") };
    let [other, step] = theorem.hypotheses.as_slice() else { panic!("expected Other and Step") };
    let [MathematicalTerm(term)] = other.conclusions[0].predicate.as_slice() else { panic!("expected accepted(result)") };
    assert_eq!(term[1], MathematicalStep::HypothesisResult(0));
    assert_eq!(
        step.conclusions[0].predicate,
        [HypothesisParameter { hypothesis: 1, index: 0 }, MathematicalTerm(vec![MathematicalStep::HypothesisResult(1)]),
         HypothesisGuarantee { hypothesis: 0, operand_count: 2 }]
    );
    let [goal] = theorem.ensures.as_slice() else { panic!("expected the projection") };
    assert!(package.machines.is_empty() && package.machine_templates.is_empty() && package.callable_contracts.is_empty());
    spec_corpus::observe_handoff!(psi "psi-output/logical-hypothesis-result-subject", assert_eq!(
        goal.predicate, [Formal(0), Formal(1), HypothesisGuarantee { hypothesis: 1, operand_count: 2 }]
    ));
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

    // L3 types a citation of this theorem with no evidence for Step. Lowering
    // still stops at the hypothesis name and emits no erased application.
    let cited = format!("{source} machine run(x:u8) requires x != 0 {{ theorem(x); }}");
    let files = [("proof.omg", cited.as_str())];
    let resolved = sources::resolve(&files);
    let prepared = omega_preparation::prepare_source_package(&resolved).unwrap();
    assert!(prepared.typed().calls.iter().any(|c| c.machine.path == "theorem" && c.use_site == CallUse::Execution));
    assert!(!prepared.typed().obligations.iter().any(|o| matches!(o.kind, ObligationKind::SupplyHypothesis { .. })));
    sources::refuse_prepared(
        &prepared,
        &files,
        "proof.omg",
        sources::unique_span(&cited, "Step"),
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

#[test]
fn nested_hypotheses_bind_lexical_subjects_and_take_evidence_only_at_the_roots() {
    let source = include_str!("fixtures/proof_hypotheses/nested.omg");
    let files = [("proof.omg", source)];
    let resolved = sources::resolve(&files);
    let prepared = omega_preparation::prepare_source_package(&resolved).unwrap();
    let mut package = sources::lower_prepared(&prepared, &files).unwrap();
    let [theorem] = package.theorems.as_slice() else { panic!("expected one proof-only theorem") };
    assert_eq!(theorem.hypotheses.len(), 4);
    // Locate each published row by its typed declaration. Its position is the
    // identity used by other Psi rows; generic binders have a separate order.
    let position = |name: &str| {
        let declaration = prepared.typed().logical_hypotheses.iter().find(|row| row.name == name).unwrap();
        theorem.hypotheses.iter().position(|row| row.site == declaration.site).unwrap()
    };
    let (top, middle, leaf, peer) = (position("Top"), position("Middle"), position("Leaf"), position("Peer"));
    let row = |index: usize| &theorem.hypotheses[index];
    assert_eq!((row(top).parent, row(middle).parent, row(leaf).parent, row(peer).parent), (None, Some(top), Some(middle), None));
    assert_eq!(theorem.binders, [TheoremBinder::Hypothesis(peer), TheoremBinder::Hypothesis(top)]);
    // Middle.x shadows theorem.x. Leaf captures Middle.x and Top.floor,
    // while Top's own clause still refers to the formal of the theorem.
    let parameter = |hypothesis| HypothesisParameter { hypothesis, index: 0 };
    assert_eq!(row(leaf).conclusions[0].predicate, [parameter(leaf), parameter(middle), Apply(GreaterThan)]);
    assert_eq!(row(middle).conclusions[0].predicate, [parameter(middle), parameter(top), Apply(GreaterThan)]);
    assert_eq!(row(middle).conclusions[1].predicate, [parameter(middle), HypothesisGuarantee { hypothesis: leaf, operand_count: 1 }]);
    assert_eq!(row(top).conclusions[0].predicate, [parameter(top), Formal(0), Apply(GreaterThan)]);
    let [MathematicalTerm(term)] = row(leaf).premises[0].predicate.as_slice() else { panic!("expected a strict premise") };
    assert_eq!(term[1], MathematicalStep::HypothesisParameter { hypothesis: top, index: 0 });

    let run = package.machines.iter().position(|m| m.declaration.path == "run").unwrap();
    let citations = psi_handoff_consumer::theorem_citations(&package, &package.machines[run].body).unwrap();
    let [citation] = citations.as_slice() else { panic!("expected one erased theorem citation") };
    let evidence: Vec<_> = citation.evidence.iter().map(|(index, machine)| (*index, machine.declaration.path.as_str())).collect();
    assert_eq!(evidence, [(peer, "lemma"), (top, "other")]);
    assert!(!package.machines.iter().any(|m| m.declaration == theorem.declaration));

    // The nested premise belongs to its parent. Even a valid machine citation
    // at this argument cannot turn it into a separately supplied theorem binder.
    package.theorems[0].binders[1] = TheoremBinder::Hypothesis(leaf);
    assert!(psi_handoff_consumer::theorem_citations(&package, &package.machines[run].body).is_none());
}

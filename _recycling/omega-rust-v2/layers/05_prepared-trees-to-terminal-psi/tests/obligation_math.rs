//! Mathematical and proof obligations, from the prepared input to the Psi term
//! or clause that carries their meaning.
//!
//! Each case lowers one source package through the shared obligation pipeline,
//! locates the prepared obligation by kind and source span, then reads only
//! Psi. Psi terms carry no site per step, so a term belongs to an obligation
//! through the declaration or clause whose retained site encloses that span.
//! Twins change one authored choice and the expected Psi is written from the
//! source. L6 judges every proposition; these tests judge none.
#[allow(dead_code)]
#[path = "obligation_outputs/pipeline.rs"]
mod pipeline;
#[allow(dead_code)]
#[path = "support/source.rs"]
mod sources;

use omega_lowering::{
    ContractClause, LogicalTheorem, MathematicalBody, MathematicalDeclaration, MathematicalStep as Term,
    PredicateOperator::{Equal, GreaterThan, NotEqual},
    PredicateStep as Step, Site, TypeExpression, TypeExpressionStep,
};
use omega_typed::ObligationKind;

const TRUST: &str = "boundary let trusted(value:u8):core\\Strict<0>;";
const THEOREM: &str = "machine theorem<machine Step>(x:u8,z:u8) requires machine Step(y:u8) requires y != 0 ensures y > z; requires trusted(x) requires x != 0 ensures Step::ensures(x) {}";
const LEMMA: &str = "machine lemma(y:u8) requires y != 0 ensures y > 0 {}";

/// Receipt for the L5 readiness report, printed after the checks above it pass.
fn observed(label: &str) {
    if std::env::var_os("OMEGA_LAYER_REPORT").is_some() {
        println!("OMEGA_L5_ASSERT\t{label}");
    }
}

/// The theorem package of `tests/hypotheses.rs` with its goal replaced.
fn proof(goal: &str) -> String {
    format!("{TRUST} {} {LEMMA}", THEOREM.replace("Step::ensures(x)", goal))
}

fn within(
    inner: &Site,
    outer: &Site,
) -> bool {
    inner.file == outer.file && outer.span.get_start_offset() <= inner.span.get_start_offset() && inner.span.get_end_offset() <= outer.span.get_end_offset()
}

fn mathematics<'a>(
    lowered: &'a pipeline::Lowered<'_>,
    path: &str,
) -> &'a MathematicalDeclaration {
    let found: Vec<_> = lowered
        .package
        .mathematics
        .iter()
        .filter(|d| d.declaration.path == path)
        .collect();
    let [declaration] = found.as_slice() else {
        panic!("expected one mathematical declaration {path}, found {found:?}")
    };
    declaration
}

fn theorem<'a>(lowered: &'a pipeline::Lowered<'_>) -> &'a LogicalTheorem {
    let [theorem] = lowered.package.theorems.as_slice() else {
        panic!("expected one logical theorem")
    };
    theorem
}

/// The one clause whose retained site encloses `site`.
fn clause_at<'a>(
    clauses: &'a [ContractClause],
    site: &Site,
) -> &'a ContractClause {
    let found: Vec<_> = clauses
        .iter()
        .filter(|clause| within(site, &clause.site))
        .collect();
    let [clause] = found.as_slice() else {
        panic!("expected one clause at {site:?}, found {found:?}")
    };
    clause
}

fn strict_zero() -> TypeExpression {
    TypeExpression(vec![
        TypeExpressionStep::Level(0u8.into()),
        TypeExpressionStep::Universe { strict: true },
    ])
}

/// A `MathematicalApplication` names the selected head, its generic arguments
/// and argument count at the call. The Psi definition enclosing that call
/// applies the same head to the parameters in authored order. Twins change
/// only the head or exchange the arguments.
#[test]
fn mathematical_application_applies_the_selected_head_to_ordered_arguments_at_its_site() {
    // (applied head, authored arguments, parameter supplying each argument)
    for (head, arguments, order) in [
        ("first", "x,y", [0, 1]),
        ("first", "y,x", [1, 0]),
        ("second", "x,y", [0, 1]),
    ] {
        let application = format!("{head}({arguments})");
        let source = format!("boundary let first(x:u8,y:u8):u8; boundary let second(x:u8,y:u8):u8; let combined(x:u8,y:u8):u8={application};");
        let lowered = pipeline::lower(&source);

        let applications = lowered.obligations("MathematicalApplication");
        let [obligation] = applications.as_slice() else {
            panic!("{application}: expected one application, found {applications:?}")
        };
        assert_eq!(lowered.spelling(&obligation.site), application);
        let ObligationKind::MathematicalApplication {
            definition: Some(definition),
            generic_arguments,
            argument_count,
        } = &obligation.kind
        else {
            panic!("{application}: expected a selected definition, found {:?}", obligation.kind)
        };
        assert_eq!(
            (definition.package.as_str(), definition.path.as_str(), generic_arguments.len(), *argument_count),
            (pipeline::PACKAGE, head, 0, 2)
        );

        let selected = mathematics(&lowered, head);
        let combined = mathematics(&lowered, "combined");
        assert!(
            within(&obligation.site, &combined.site),
            "{application}: the application lies in the lowered definition"
        );
        let applied = Term::Apply {
            generic_arguments: vec![],
            argument_count: 2,
            assumptions: vec![selected.declaration.clone()],
        };
        let expected = vec![
            Term::Declaration(selected.declaration.clone()),
            Term::Parameter(order[0]),
            Term::Parameter(order[1]),
            applied,
        ];
        assert_eq!(combined.body, MathematicalBody::Definition(expected), "{application}");
    }
    observed("obligation-output/mathematical-application");
}

/// Each `AdmitMathematicalAssumption` names the boundary an application depends
/// on, both where the definition applies it and where a guarantee applies that
/// definition. The Psi application at each site lists exactly that boundary.
/// The twin builds the same definition over the other boundary.
#[test]
fn an_admitted_assumption_is_the_exact_boundary_of_the_application_at_its_site() {
    for inner in ["trusted", "other"] {
        let direct = format!("{inner}(x)");
        let source =
            format!("boundary let trusted(x:u8):u8; boundary let other(x:u8):u8; let wrap(x:u8):u8={direct}; machine claim(x:u8) ensures wrap(x)==x {{}}");
        let lowered = pipeline::lower(&source);

        let boundary = mathematics(&lowered, inner);
        let wrap = mathematics(&lowered, "wrap");
        let [ensures] = lowered.machine("claim").contract.ensures.as_slice() else {
            panic!("{inner}: expected one guarantee")
        };
        let admitted = lowered.obligations("AdmitMathematicalAssumption");
        let mut sites: Vec<_> = admitted.iter().map(|o| lowered.spelling(&o.site)).collect();
        sites.sort_unstable();
        let mut expected_sites = [direct.as_str(), "wrap(x)"];
        expected_sites.sort_unstable();
        assert_eq!(sites, expected_sites, "{inner}: one admission in the definition and one at its proof use");
        for obligation in &admitted {
            let ObligationKind::AdmitMathematicalAssumption(id) = &obligation.kind else {
                unreachable!()
            };
            assert_eq!((id.package.as_str(), id.path.as_str()), (pipeline::PACKAGE, inner));
            let enclosing = if lowered.spelling(&obligation.site) == "wrap(x)" {
                &ensures.site
            } else {
                &wrap.site
            };
            assert!(within(&obligation.site, enclosing), "{inner}: admission at {obligation:?}");
        }

        assert_eq!(boundary.body, MathematicalBody::BoundaryAssumption);
        let applied = Term::Apply {
            generic_arguments: vec![],
            argument_count: 1,
            assumptions: vec![boundary.declaration.clone()],
        };
        assert_eq!(
            wrap.body,
            MathematicalBody::Definition(vec![
                Term::Declaration(boundary.declaration.clone()),
                Term::Parameter(0),
                applied.clone()
            ]),
            "{inner}"
        );
        assert_eq!(wrap.assumptions, std::slice::from_ref(&boundary.declaration), "{inner}");
        assert_eq!(
            ensures.predicate,
            [
                Step::MathematicalTerm(vec![
                    Term::Declaration(wrap.declaration.clone()),
                    Term::Parameter(0),
                    applied
                ]),
                Step::Formal(0),
                Step::Apply(Equal)
            ],
            "{inner}"
        );
    }
    observed("obligation-output/admit-mathematical-assumption");
}

/// A `MathematicalFact` names the strict sort of a Boolean-valued term. The
/// Psi term at that site ends by forming a proposition at that sort. In a
/// definition the twins exchange the compared parameters. In a theorem the
/// strict requirement carries the fact and the Boolean requirement does not.
#[test]
fn a_mathematical_fact_forms_its_strict_proposition_at_its_site() {
    for (fact, order) in [("value > limit", [1, 0]), ("limit > value", [0, 1])] {
        let source = format!("let greater_than(limit:i32,value:i32):core\\Strict<0> = {fact};");
        let lowered = pipeline::lower(&source);
        let facts = lowered.obligations("MathematicalFact");
        let [obligation] = facts.as_slice() else {
            panic!("{fact}: expected one fact, found {facts:?}")
        };
        assert_eq!(lowered.spelling(&obligation.site), fact);
        let ObligationKind::MathematicalFact { sort } = &obligation.kind else {
            unreachable!()
        };
        assert_eq!(sort.to_string(), "core\\Strict<0>");
        let greater = mathematics(&lowered, "greater_than");
        assert!(within(&obligation.site, &greater.site), "{fact}");
        assert_eq!(
            greater.body,
            MathematicalBody::Definition(vec![
                Term::Parameter(order[0]),
                Term::Parameter(order[1]),
                Term::Predicate(GreaterThan),
                Term::FormProposition(strict_zero())
            ]),
            "{fact}"
        );
    }

    let source = proof("Step::ensures(x)");
    let lowered = pipeline::lower(&source);
    let facts = lowered.obligations("MathematicalFact");
    let [obligation] = facts.as_slice() else {
        panic!("expected one theorem fact, found {facts:?}")
    };
    assert_eq!(lowered.spelling(&obligation.site), "trusted(x)");
    assert!(matches!(&obligation.kind, ObligationKind::MathematicalFact { sort } if sort.to_string() == "core\\Strict<0>"));
    let [assumed, nonzero] = theorem(&lowered).requires.as_slice() else {
        panic!("expected both theorem requirements")
    };
    assert!(within(&obligation.site, &assumed.site) && !within(&obligation.site, &nonzero.site));
    let trusted = mathematics(&lowered, "trusted");
    let applied = Term::Apply {
        generic_arguments: vec![],
        argument_count: 1,
        assumptions: vec![trusted.declaration.clone()],
    };
    assert_eq!(
        assumed.predicate,
        [Step::MathematicalTerm(vec![
            Term::Declaration(trusted.declaration.clone()),
            Term::Parameter(0),
            applied,
            Term::FormProposition(strict_zero())
        ])]
    );
    assert_eq!(nonzero.predicate, [Step::Formal(0), Step::Integer(0.into()), Step::Apply(NotEqual)]);
    observed("obligation-output/mathematical-fact");
}

/// An `ApplyHypothesis` names the hypothesis requirement and the ordered
/// subject sites of `Step::ensures(..)`. The theorem's Psi goal at that site
/// applies the conclusions of the hypothesis whose Psi site is that
/// requirement to the formal spelled by the subject. Twins select `x` or `z`.
#[test]
fn an_applied_hypothesis_concludes_at_its_requirement_and_exact_subject() {
    // (goal, subject, theorem formal of the subject)
    for (goal, subject, formal) in [("Step::ensures(x)", "x", 0), ("Step::ensures(z)", "z", 1)] {
        let source = proof(goal);
        let lowered = pipeline::lower(&source);
        let applied = lowered.obligations("ApplyHypothesis");
        let [obligation] = applied.as_slice() else {
            panic!("{goal}: expected one hypothesis application, found {applied:?}")
        };
        assert_eq!(lowered.spelling(&obligation.site), goal);
        let ObligationKind::ApplyHypothesis { requirement, operands } = &obligation.kind else {
            unreachable!()
        };
        let subjects: Vec<_> = operands.iter().map(|site| lowered.spelling(site)).collect();
        assert_eq!(subjects, [subject], "{goal}");

        let theorem = theorem(&lowered);
        let hypothesis = theorem
            .hypotheses
            .iter()
            .position(|h| &h.site == requirement)
            .unwrap_or_else(|| panic!("{goal}: the requirement is a lowered hypothesis"));
        let [ensures] = theorem.ensures.as_slice() else {
            panic!("{goal}: expected one theorem goal")
        };
        assert!(within(&obligation.site, &ensures.site), "{goal}");
        assert_eq!(
            ensures.predicate,
            [
                Step::Formal(formal),
                Step::HypothesisGuarantee {
                    hypothesis,
                    operand_count: operands.len()
                }
            ],
            "{goal}"
        );
    }
    observed("obligation-output/apply-hypothesis");
}

/// A `ProveGuarantee` names the owning machine and its predicate site. Psi
/// places that predicate in the owner's `ensures`, never in its requirements.
/// Scalar twins change only the compared formal. In the theorem package the
/// theorem and the lemma each owe their own goal, while the hypothesis
/// conclusion is assumed evidence and owes none.
#[test]
fn a_proved_guarantee_is_an_ensures_clause_of_its_owner_at_its_predicate() {
    for (guarantee, formal) in [("result == second", 1), ("result == first", 0)] {
        let source =
            format!("machine checked(first:u8, second:u8, flag:bool)->u8 requires first > 0 requires flag ensures {guarantee} ensures flag {{second}}");
        let lowered = pipeline::lower(&source);
        let contract = &lowered.machine("checked").contract;
        let proved = lowered.obligations("ProveGuarantee");
        assert_eq!(proved.len(), contract.ensures.len(), "{guarantee}: one obligation per guarantee");
        let mut spelled = Vec::new();
        for obligation in &proved {
            let ObligationKind::ProveGuarantee { machine, predicate } = &obligation.kind else {
                unreachable!()
            };
            assert_eq!((machine.package.as_str(), machine.path.as_str()), (pipeline::PACKAGE, "checked"));
            assert_eq!(predicate, &obligation.site, "{guarantee}: the obligation sits on its predicate");
            assert!(
                !contract
                    .requires
                    .iter()
                    .any(|clause| within(predicate, &clause.site)),
                "{guarantee}: a requirement owes no guarantee"
            );
            let expected = match lowered.spelling(predicate) {
                text if text == guarantee => vec![Step::Result, Step::Formal(formal), Step::Apply(Equal)],
                "flag" => vec![Step::Formal(2)],
                other => panic!("{guarantee}: unexpected guarantee {other}"),
            };
            assert_eq!(clause_at(&contract.ensures, predicate).predicate, expected, "{guarantee}");
            spelled.push(lowered.spelling(predicate));
        }
        spelled.sort_unstable();
        let mut expected = [guarantee, "flag"];
        expected.sort_unstable();
        assert_eq!(spelled, expected);
    }

    let source = proof("Step::ensures(x)");
    let lowered = pipeline::lower(&source);
    let theorem = theorem(&lowered);
    let lemma = lowered.machine("lemma");
    let proved = lowered.obligations("ProveGuarantee");
    let mut owners = Vec::new();
    for obligation in &proved {
        let ObligationKind::ProveGuarantee { machine, predicate } = &obligation.kind else {
            unreachable!()
        };
        assert_eq!(predicate, &obligation.site);
        let (ensures, expected) = match machine.path.as_str() {
            "theorem" => (
                &theorem.ensures,
                vec![
                    Step::Formal(0),
                    Step::HypothesisGuarantee {
                        hypothesis: 0,
                        operand_count: 1,
                    },
                ],
            ),
            "lemma" => (
                &lemma.contract.ensures,
                vec![
                    Step::Formal(0),
                    Step::Integer(0.into()),
                    Step::Apply(GreaterThan),
                ],
            ),
            other => panic!("unexpected guarantee owner {other}"),
        };
        assert_eq!(clause_at(ensures, predicate).predicate, expected, "{}", machine.path);
        owners.push(machine.path.as_str());
    }
    owners.sort_unstable();
    assert_eq!(owners, ["lemma", "theorem"]);
    let [conclusion] = theorem.hypotheses[0].conclusions.as_slice() else {
        panic!("expected one hypothesis conclusion")
    };
    assert_eq!(
        conclusion.predicate,
        [
            Step::HypothesisParameter { hypothesis: 0, index: 0 },
            Step::Formal(1),
            Step::Apply(GreaterThan)
        ]
    );
    assert!(
        !proved.iter().any(|o| within(&o.site, &conclusion.site)),
        "an assumed conclusion owes no guarantee"
    );
    observed("obligation-output/prove-guarantee");
}

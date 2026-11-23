//! Preparation hands three mathematical obligations to lowering. The tests
//! identify each one at its authored occurrence, then inspect the Psi from
//! lowering that same package.
//!
//! - `FormSquash`: L3 records it where `core\Squash::<T>` is used as a
//!   proposition (`expressions/type_expression.rs`). A theorem goal reaches the
//!   mathematical term reader with the checked source type and its level.
//! - `RefineMathematicalCitation`: L3 records it where a machine stands for
//!   evidence of a function interface (`expressions/mathematical_application.rs`).
//!   A mathematical term retains the cited machine and the expected type for the checker.
//! - `UniverseMembership`: L3 records it for every argument of a binder bounded
//!   by a universe. At a mathematical application (`call_arguments.rs`,
//!   `validate_call_bindings`) the term reader matches it to the applied
//!   argument and the instantiated bound on the selected declaration.
//!   At a type application in a signature (`type_system/formation.rs`) no L5
//!   reader looks at the row, and Psi carries the applied argument and the
//!   template bound that decide it. That route is compared, not pinned.
//!
//! Each case has a nearby source without that obligation as its control.
#[allow(dead_code)]
#[path = "obligation_outputs/pipeline.rs"]
mod pipeline;
#[allow(dead_code)]
#[path = "support/source.rs"]
mod sources;

use omega_lowering::{
    MathematicalBody, MathematicalDeclaration, MathematicalStep as Term, PredicateStep as Step, ScalarType, Site, TypeExpression,
    TypeExpressionStep as Ty,
};
use omega_psi::TemplateBinder;
use omega_typed::{ObligationKind, ValueType};

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

fn spelled<'s>(
    site: &Site,
    source: &'s str,
) -> &'s str {
    assert_eq!((site.package.as_str(), site.file.as_str()), (pipeline::PACKAGE, pipeline::FILE));
    std::str::from_utf8(
        site.span
            .slice(source.as_bytes())
            .expect("site lies in the source"),
    )
    .unwrap()
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

/// A theorem goal `core\Squash::<T>` owes `FormSquash { T, level(T) }` at the
/// goal, which is also the site of its strict fact and of the theorem's own
/// guarantee. L5 emits the proposition and strict sort, not a proof.
/// Twins change only `T`. The control keeps the theorem and replaces only the
/// goal by `Step::ensures(x)`: it owes no squash and lowers to its hypothesis
/// guarantee.
#[test]
fn a_squash_goal_preserves_its_source_and_sort_on_the_theorem() {
    for (inhabited, carrier) in [("u8", ScalarType::Integer { signed: false, width: 8 }), ("bool", ScalarType::Boolean)] {
        let goal = format!("core\\Squash::<{inhabited}>");
        let source = proof(&goal);
        let lowered = pipeline::lower(&source);
        let squashes = lowered.obligations("FormSquash");
        let [squash] = squashes.as_slice() else {
            panic!("{goal}: expected one squash, found {squashes:?}")
        };
        assert_eq!(spelled(&squash.site, &source), goal);
        let ObligationKind::FormSquash { source: squashed, level } = &squash.kind else {
            unreachable!()
        };
        assert_eq!(squashed.to_string(), inhabited, "{goal}: the squashed source");
        assert_eq!(level, &ValueType::LevelOf(Box::new(squashed.clone())), "{goal}: the level of that source");
        let facts = lowered.obligations("MathematicalFact");
        assert!(
            facts.iter().any(|fact| fact.site == squash.site
                && matches!(&fact.kind, ObligationKind::MathematicalFact { sort: ValueType::Universe { strict: true, level: at } } if at.as_ref() == level)),
            "{goal}: the goal is a strict proposition at the squash level: {facts:?}"
        );
        let guarantees = lowered.obligations("ProveGuarantee");
        assert!(
            guarantees.iter().any(|g| matches!(&g.kind,
                ObligationKind::ProveGuarantee { machine, predicate } if machine.path == "theorem" && predicate == &squash.site)),
            "{goal}: the theorem owes the squash goal: {guarantees:?}"
        );

        let [theorem] = lowered.package.theorems.as_slice() else { panic!("expected the proof-only theorem") };
        let [clause] = theorem.ensures.as_slice() else { panic!("expected its one guarantee") };
        assert_eq!(theorem.declaration.path, "theorem");
        assert_eq!(theorem.requires.len(), 2);
        assert_eq!(lowered.spelling(&clause.site), format!("ensures {goal}"));
        let source_type = Ty::Scalar(carrier);
        spec_corpus::observe_handoff!(psi "psi-output/squash-theorem-goal", assert_eq!(
            clause.predicate, [Step::MathematicalTerm(vec![
                Term::Squash(TypeExpression(vec![source_type.clone()])),
                Term::FormProposition(TypeExpression(vec![source_type, Ty::LevelOf, Ty::Universe { strict: true }]))
            ])]
        ));
    }

    let control = proof("Step::ensures(x)");
    let lowered = pipeline::lower(&control);
    assert!(lowered.obligations("FormSquash").is_empty(), "the hypothesis goal owes no squash");
    let [theorem] = lowered.package.theorems.as_slice() else {
        panic!("expected one logical theorem")
    };
    assert_eq!(theorem.requires.len(), 2, "both authored requirements lower before the goal");
    let [goal] = theorem.ensures.as_slice() else {
        panic!("expected one theorem goal")
    };
    spec_corpus::observe_handoff!(psi "psi-output/squash-projection-control", assert_eq!(
        goal.predicate, [Step::Formal(0), Step::HypothesisGuarantee { hypothesis: 0, operand_count: 1 }]
    ));
}

/// Equal universe bounds do not make the two theorem parameters interchangeable.
#[test]
fn a_squash_goal_keeps_the_selected_type_binder_even_under_equal_bounds() {
    for (name, index) in [("A", 1), ("B", 2)] {
        let source = proof(&format!("core\\Squash::<{name}>"))
            .replace("<machine Step>", "<u:core\\Level,A:core\\Type<u>,B:core\\Type<u>,machine Step>");
        let lowered = pipeline::lower(&source);
        let [theorem] = lowered.package.theorems.as_slice() else { panic!("expected the proof-only theorem") };
        let subject = Ty::GenericParameter { owner: theorem.declaration.clone(), index };
        assert_eq!(theorem.ensures[0].predicate, [Step::MathematicalTerm(vec![
            Term::Squash(TypeExpression(vec![subject.clone()])),
            Term::FormProposition(TypeExpression(vec![subject, Ty::LevelOf, Ty::Universe { strict: true }]))
        ])]);
    }
}

/// A machine named where a mathematical parameter expects `u8 -> core\Squash<u8>`
/// owes `RefineMathematicalCitation { lemma, that interface }` at the name
/// inside the application. This is a checker input, not a proof: in particular
/// the expected arrow does not restrict its input to `y>0`.
#[test]
fn a_machine_citation_retains_its_selected_contract_and_expected_evidence_type() {
    const CITED: &str = "machine lemma(y:u8) requires y>0 ensures y==y {}";
    const CONSUME: &str = "boundary let consume(evidence:u8 -> core\\Squash<u8>):bool;";
    let citing = |argument: &str| format!("{CITED} {CONSUME} let checked(e:u8 -> core\\Squash<u8>):bool = consume({argument});");
    let byte = Ty::Scalar(ScalarType::Integer { signed: false, width: 8 });
    let sibling = " machine other(y:u8) requires y>0 ensures y==y {}";
    for (source, selected, inner) in [
        (citing("lemma"), "lemma", byte.clone()),
        (citing("lemma") + sibling, "lemma", byte.clone()),
        (citing("other") + sibling, "other", byte.clone()),
        (citing("lemma").replace("Squash<u8>", "Squash<bool>"), "lemma", Ty::Scalar(ScalarType::Boolean)),
    ] {
        let lowered = pipeline::lower(&source);
        let applications = lowered.obligations("MathematicalApplication");
        let [application] = applications.as_slice() else { panic!("expected one application") };
        assert_eq!(spelled(&application.site, &source), format!("consume({selected})"));
        assert!(matches!(&application.kind, ObligationKind::MathematicalApplication { definition: Some(id), generic_arguments, argument_count: 1 }
            if id.path == "consume" && generic_arguments.is_empty()));
        let citations = lowered.obligations("RefineMathematicalCitation");
        let [citation] = citations.as_slice() else { panic!("expected one citation") };
        assert_eq!(spelled(&citation.site, &source), selected);
        assert!(within(&citation.site, &application.site));
        let ObligationKind::RefineMathematicalCitation { machine, expected } = &citation.kind else { unreachable!() };
        assert_eq!((machine.package.as_str(), machine.path.as_str()), (pipeline::PACKAGE, selected));
        let typed_byte = ValueType::Integer { signed: false, width: Some(8) };
        let carrier = if inner == byte { typed_byte.clone() } else { ValueType::Bool };
        assert_eq!(*expected, ValueType::Function(Box::new(typed_byte), Box::new(ValueType::Squash(Box::new(carrier)))));

        let machine = lowered.machine(selected);
        let [premise] = machine.contract.requires.as_slice() else { panic!("missing machine premise") };
        assert_eq!(premise.predicate, [Step::Formal(0), Step::Integer(0.into()), Step::Apply(omega_psi::PredicateOperator::GreaterThan)]);
        let [conclusion] = machine.contract.ensures.as_slice() else { panic!("missing machine guarantee") };
        assert_eq!(conclusion.predicate, [Step::Formal(0), Step::Formal(0), Step::Apply(omega_psi::PredicateOperator::Equal)]);
        let consume = mathematics(&lowered, "consume");
        let checked = mathematics(&lowered, "checked");
        assert_eq!(checked.assumptions, std::slice::from_ref(&consume.declaration));
        spec_corpus::observe_handoff!(psi "psi-output/mathematical-citation", assert_eq!(checked.body,
            MathematicalBody::Definition(vec![
                Term::Declaration(consume.declaration.clone()),
                Term::RefineCitation { machine: machine.declaration.clone(), expected: TypeExpression(vec![byte.clone(), inner, Ty::Squash, Ty::Function]) },
                Term::Apply { generic_arguments: vec![], argument_count: 1, assumptions: vec![consume.declaration.clone()] },
            ])
        ));
    }

    let control = citing("e");
    let lowered = pipeline::lower(&control);
    assert!(lowered.obligations("RefineMathematicalCitation").is_empty(), "a parameter is no citation");
    let applications = lowered.obligations("MathematicalApplication");
    let [application] = applications.as_slice() else {
        panic!("expected one application, found {applications:?}")
    };
    assert_eq!(lowered.spelling(&application.site), "consume(e)");
    let consume = mathematics(&lowered, "consume");
    let checked = mathematics(&lowered, "checked");
    assert_eq!(consume.body, MathematicalBody::BoundaryAssumption);
    spec_corpus::observe_handoff!(psi "psi-output/mathematical-citation-control", assert_eq!(
        checked.body,
        MathematicalBody::Definition(vec![
            Term::Declaration(consume.declaration.clone()),
            Term::Parameter(0),
            Term::Apply {
                generic_arguments: vec![],
                argument_count: 1,
                assumptions: vec![consume.declaration.clone()],
            },
        ])
    ));
}

/// `id::<core\Type<n>>(x)` with `A:core\Type<n+1>` owes
/// `UniverseMembership { core\Type<n>, core\Type<n+1> }` at the application,
/// beside the application that selects the same argument. Both levels must
/// survive in Psi; neither the bound nor the argument alone is the question.
#[test]
fn a_universe_bounded_application_retains_the_member_and_its_binders_sort() {
    let universe_type = |n: u8| TypeExpression(vec![Ty::Level(n.into()), Ty::Universe { strict: false }]);
    for (bound, subject) in [(1u8, 0u8), (2, 1)] {
        let universe = format!("core\\Type<{subject}>");
        let application = format!("id::<{universe}>(x)");
        let source = format!("let id<A:core\\Type<{bound}>>(x:A):A=x; let f(x:{universe}):{universe} = {application};");
        let lowered = pipeline::lower(&source);
        let memberships = lowered.obligations("UniverseMembership");
        let [membership] = memberships.as_slice() else {
            panic!("{application}: expected one membership, found {memberships:?}")
        };
        assert_eq!(spelled(&membership.site, &source), application);
        let ObligationKind::UniverseMembership { subject: member, sort } = &membership.kind else {
            unreachable!()
        };
        assert_eq!((member.to_string(), sort.to_string()), (universe.clone(), format!("core\\Type<{bound}>")));
        let applications = lowered.obligations("MathematicalApplication");
        let [applied] = applications.as_slice() else {
            panic!("{application}: expected one application, found {applications:?}")
        };
        assert_eq!(applied.site, membership.site);
        assert!(
            matches!(&applied.kind,
                ObligationKind::MathematicalApplication { definition: Some(id), generic_arguments, argument_count: 1 }
                    if id.path == "id" && generic_arguments.as_slice() == std::slice::from_ref(member)),
            "{application}: the application selects the member: {:?}",
            applied.kind
        );

        let id = mathematics(&lowered, "id");
        let f = mathematics(&lowered, "f");
        assert!(within(&membership.site, &f.site));
        spec_corpus::observe_handoff!(psi "psi-output/universe-bounded-application", assert_eq!(
            (&id.generic_bounds, &f.body),
            (&vec![universe_type(bound)], &MathematicalBody::Definition(vec![
                Term::Declaration(id.declaration.clone()),
                Term::Parameter(0),
                Term::Apply { generic_arguments: vec![universe_type(subject)], argument_count: 1, assumptions: vec![] },
            ]))
        ));
    }

    let control = "let id(x:core\\Type<0>):core\\Type<0> = x; let f(x:core\\Type<0>):core\\Type<0> = id(x);";
    let lowered = pipeline::lower(control);
    assert!(lowered.obligations("UniverseMembership").is_empty(), "an unbounded head owes no membership");
    let id = mathematics(&lowered, "id");
    let f = mathematics(&lowered, "f");
    assert!(id.generic_bounds.is_empty());
    assert_eq!(
        f.body,
        MathematicalBody::Definition(vec![
            Term::Declaration(id.declaration.clone()),
            Term::Parameter(0),
            Term::Apply {
                generic_arguments: vec![],
                argument_count: 1,
                assumptions: vec![],
            },
        ])
    );
}

/// A level supplied at slot zero instantiates the universe bound at slot one.
/// Keep the declared reference, not a bound baked in from either call.
#[test]
fn a_mathematical_universe_bound_retains_its_level_binder_at_two_applications() {
    let source = r"let id<L:core\Level,A:core\Type<L>>(x:A):A=x; let f(x:core\Type<1>):core\Type<1> = id::<2,core\Type<1>>(x); let g(x:core\Type<0>):core\Type<0> = id::<1,core\Type<0>>(x);";
    let lowered = pipeline::lower(source);
    let id = mathematics(&lowered, "id");
    let memberships = lowered.obligations("UniverseMembership");
    assert_eq!(memberships.len(), 2);
    for (name, bound, subject) in [("f", 2u8, 1u8), ("g", 1, 0)] {
        let caller = mathematics(&lowered, name);
        let row = memberships.iter().find(|r| within(&r.site, &caller.site)).unwrap();
        let ObligationKind::UniverseMembership { subject: member, sort } = &row.kind else { unreachable!() };
        assert_eq!((member.to_string(), sort.to_string()), (format!(r"core\Type<{subject}>"), format!(r"core\Type<{bound}>")));
        let bounds = vec![TypeExpression(vec![Ty::LevelKind]), TypeExpression(vec![
            Ty::GenericParameter { owner: id.declaration.clone(), index: 0 }, Ty::Universe { strict: false },
        ])];
        let arguments = vec![TypeExpression(vec![Ty::Level(bound.into())]), TypeExpression(vec![Ty::Level(subject.into()), Ty::Universe { strict: false }])];
        spec_corpus::observe_handoff!(psi "psi-output/instantiated-mathematical-universe", assert_eq!(
            (&id.generic_bounds, &caller.body),
            (&bounds, &MathematicalBody::Definition(vec![
                Term::Declaration(id.declaration.clone()), Term::Parameter(0),
                Term::Apply { generic_arguments: arguments, argument_count: 1, assumptions: vec![] },
            ]))
        ));
    }
}

/// `Box<core\Type<n>>` in a boundary signature, with `A:core\Type<n+1>` on
/// `Box`, owes `UniverseMembership { core\Type<n>, core\Type<n+1> }` at that
/// type. No L5 reader takes the row. Psi carries both sides of the question:
/// the signature applies `Box` to exactly the obligation's subject, and the
/// template's only binder is bounded by exactly its sort. Twins change both
/// levels and both Psi terms follow.
#[test]
fn a_universe_membership_in_a_signature_is_the_applied_argument_and_the_template_bound() {
    let universe = |level: u8| vec![Ty::Level(level.into()), Ty::Universe { strict: false }];
    for (bound, subject) in [(1u8, 0u8), (2, 1)] {
        let applied = format!("Box<core\\Type<{subject}>>");
        let source = format!("data Box<A:core\\Type<{bound}>>{{}} boundary let pick(x:{applied}):u8;");
        let lowered = pipeline::lower(&source);

        let memberships = lowered.obligations("UniverseMembership");
        let [membership] = memberships.as_slice() else {
            panic!("{applied}: expected one membership, found {memberships:?}")
        };
        assert_eq!(lowered.spelling(&membership.site), applied);
        let ObligationKind::UniverseMembership { subject: member, sort } = &membership.kind else {
            unreachable!()
        };
        assert_eq!(
            (member.to_string(), sort.to_string()),
            (format!("core\\Type<{subject}>"), format!("core\\Type<{bound}>"))
        );

        let pick = mathematics(&lowered, "pick");
        assert!(within(&membership.site, &pick.site), "{applied}: the membership lies in the signature");
        let [template] = lowered.package.data_templates.as_slice() else {
            panic!("{applied}: expected the Box template")
        };
        assert_eq!(template.declaration.path, "Box");
        assert_eq!(
            template.parameters,
            [TemplateBinder::Kinded(TypeExpression(universe(bound)))],
            "{applied}: the sort"
        );
        let mut signature = universe(subject);
        signature.extend([
            Ty::Named {
                declaration: template.declaration.clone(),
                argument_count: 1,
            },
            Ty::Scalar(ScalarType::Integer { signed: false, width: 8 }),
            Ty::Function,
        ]);
        assert_eq!(pick.body, MathematicalBody::BoundaryAssumption);
        assert_eq!(pick.signature.0, signature, "{applied}: the subject applied to Box");
    }
    observed("obligation-output/universe-membership");
}

/// The signature supplies the closed actuals. The field selects the next
/// application by referring to the two binders in declaration order or in reverse.
///
/// The stored reference names its lifetime, so `Link` declares `'r` before its
/// two type binders and every application writes a lifetime first. That
/// lifetime selects nothing and keeps its position: binder 0 of the template,
/// argument 0 of the field's referent and of the formal's application, and
/// the one lifetime binder of `pick`, which has no bound.
#[test]
fn recursive_field_substitution_preserves_the_next_closed_application() {
    let byte = ScalarType::Integer { signed: false, width: 8 };
    let typed_byte = ValueType::Integer { signed: false, width: Some(8) };
    for (field_args, use_args, actuals, slots, successor) in [
        ("B,A", "u8,bool", [byte, ScalarType::Boolean], [2, 1], Some([ValueType::Bool, typed_byte.clone()])),
        ("B,A", "bool,u8", [ScalarType::Boolean, byte], [2, 1], Some([typed_byte, ValueType::Bool])),
        ("A,B", "u8,bool", [byte, ScalarType::Boolean], [1, 2], None),
    ] {
        let applied = format!("Link<'r,{use_args}>");
        let source = format!(r"data Link<'r,A:core\Type<0>,B:core\Type<0>>{{next:@'r Link<'r,{field_args}>;}} boundary let pick<'r>(x:{applied}):u8;");
        let lowered = pipeline::lower(&source);
        let pick = mathematics(&lowered, "pick");
        let [template] = lowered.package.data_templates.as_slice() else { panic!("expected Link") };
        assert_eq!(template.declaration.path, "Link");
        assert!(
            matches!(template.parameters.as_slice(), [TemplateBinder::Lifetime, TemplateBinder::Kinded(_), TemplateBinder::Kinded(_)]),
            "{:?}",
            template.parameters
        );
        let head = Ty::Named { declaration: template.declaration.clone(), argument_count: 3 };
        let [first, second] = actuals.map(Ty::Scalar);
        // `pick<'r>` declares the lifetime its formal applies `Link` at. It is
        // the telescope's binder 0 and has no bound.
        let applied_at = Ty::GenericParameter { owner: pick.declaration.clone(), index: 0 };
        assert_eq!((pick.lifetime_count, pick.generic_bounds.len()), (1, 0));
        assert_eq!(pick.signature.0, [applied_at, first, second, head.clone(), Ty::Scalar(byte), Ty::Function]);

        let [field] = template.fields.as_slice() else { panic!("expected the recursive field") };
        assert_eq!(field.declaration.path, "Link::next");
        let [first, second] = slots.map(|index| Ty::GenericParameter { owner: template.declaration.clone(), index });
        let held_at = Ty::GenericParameter { owner: template.declaration.clone(), index: 0 };
        assert_eq!(
            field.ty.0,
            [held_at.clone(), first, second, head, Ty::Reference(omega_psi::TemplateAccess::Shared), held_at, Ty::AtLifetime]
        );

        let rows = lowered.obligations("FormRecursiveApplication");
        let Some(expected) = successor else {
            assert!(rows.is_empty(), "the identity field leaves the application unchanged");
            continue;
        };
        let at_signature: Vec<_> = rows.into_iter().filter(|row| within(&row.site, &pick.site)).collect();
        let [row] = at_signature.as_slice() else { panic!("expected one recursive obligation in pick") };
        assert_eq!(lowered.spelling(&row.site), applied);
        let ObligationKind::FormRecursiveApplication { declaration, arguments, .. } = &row.kind else { unreachable!() };
        assert_eq!((&declaration.package, &declaration.path), (&template.declaration.package, &template.declaration.path));
        // The next application keeps the lifetime `pick` applied, then the two actuals in the field's order.
        let [lifetime, successor @ ..] = arguments.as_slice() else { panic!("expected the lifetime argument first") };
        assert!(matches!(lifetime, ValueType::Parameter { owner, index: 0 } if owner.path == "pick"), "{lifetime:?}");
        assert_eq!(successor, expected);
    }
    observed("obligation-output/recursive-field-substitution");
}

//! Preparation hands three mathematical obligations to lowering. The tests
//! identify each one at its authored occurrence, then inspect the Psi or the
//! first refusal from lowering that same package.
//!
//! - `FormSquash`: L3 records it where `core\Squash::<T>` is used as a
//!   proposition (`expressions/type_expression.rs`). A theorem goal reaches the
//!   contract predicate reader, which refuses the row at that goal
//!   (`src/declarations/contracts.rs`, `records`).
//! - `RefineMathematicalCitation`: L3 records it where a machine stands for
//!   evidence of a function interface (`expressions/mathematical_application.rs`).
//!   As an argument of a mathematical application it reaches the term reader,
//!   which refuses the row at the cited name (`src/declarations/mathematics.rs`).
//! - `UniverseMembership`: L3 records it for every argument of a binder bounded
//!   by a universe. At a mathematical application (`call_arguments.rs`,
//!   `validate_call_bindings`) the term reader refuses it at the application.
//!   At a type application in a signature (`type_system/formation.rs`) no L5
//!   reader looks at the row, and Psi carries the applied argument and the
//!   template bound that decide it. That route is compared, not pinned.
//!
//! For each refusal, a nearby source without that obligation lowers successfully.
#[path = "support/known_gap.rs"]
mod known_gap;
#[allow(dead_code)]
#[path = "obligation_outputs/pipeline.rs"]
mod pipeline;
#[path = "support/preparation.rs"]
mod preparation;
#[allow(dead_code)]
#[path = "support/source.rs"]
mod sources;

use omega_lowering::{
    LoweringError, MathematicalBody, MathematicalDeclaration, MathematicalStep as Term, PredicateStep as Step, ScalarType, Site, TypeExpression,
    TypeExpressionStep as Ty,
};
use omega_psi::TemplateBinder;
use omega_typed::{ObligationKind, TypingObligation, ValueType};
use tree_handoff_consumer::obligation_report;

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

/// Prepare one source, copy its obligations, lower the same trees and return the refusal.
fn refusal(source: &str, span: std::ops::Range<usize>, message: &str) -> (Vec<TypingObligation>, LoweringError) {
    let files = [(pipeline::FILE, source)];
    let resolved = sources::resolve(&files);
    let prepared = preparation::prepare(&resolved).unwrap_or_else(|(stage, error)| panic!("{stage} stopped before L5: {error:?}\n{source}"));
    let obligations = prepared.typed().obligations.clone();
    let error = sources::refuse_prepared(&prepared, &files, pipeline::FILE, span, message);
    (obligations, error)
}

fn of_kind<'a>(
    obligations: &'a [TypingObligation],
    kind: &str,
) -> Vec<&'a TypingObligation> {
    assert!(obligation_report::KINDS.contains(&kind), "unknown obligation kind {kind}");
    obligations
        .iter()
        .filter(|obligation| obligation_report::name(&obligation.kind) == kind)
        .collect()
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
/// guarantee. L5 lowers the theorem's binders, hypothesis and both requirements,
/// then refuses at that goal because no predicate record accepts the squash.
/// Twins change only `T`. The control keeps the theorem and replaces only the
/// goal by `Step::ensures(x)`: it owes no squash and lowers to its hypothesis
/// guarantee.
#[test]
fn a_squash_goal_reaches_its_theorem_and_stops_at_that_goal() {
    let mut first = None;
    for inhabited in ["u8", "bool"] {
        let goal = format!("core\\Squash::<{inhabited}>");
        let source = proof(&goal);
        let (obligations, error) = refusal(
            &source, sources::unique_span(&source, &goal),
            "Psi cannot form a squash proposition in a contract predicate",
        );

        let squashes = of_kind(&obligations, "FormSquash");
        let [squash] = squashes.as_slice() else {
            panic!("{goal}: expected one squash, found {squashes:?}")
        };
        assert_eq!(spelled(&squash.site, &source), goal);
        let ObligationKind::FormSquash { source: squashed, level } = &squash.kind else {
            unreachable!()
        };
        assert_eq!(squashed.to_string(), inhabited, "{goal}: the squashed source");
        assert_eq!(level, &ValueType::LevelOf(Box::new(squashed.clone())), "{goal}: the level of that source");
        let facts = of_kind(&obligations, "MathematicalFact");
        assert!(
            facts.iter().any(|fact| fact.site == squash.site
                && matches!(&fact.kind, ObligationKind::MathematicalFact { sort: ValueType::Universe { strict: true, level: at } } if at.as_ref() == level)),
            "{goal}: the goal is a strict proposition at the squash level: {facts:?}"
        );
        let guarantees = of_kind(&obligations, "ProveGuarantee");
        assert!(
            guarantees.iter().any(|g| matches!(&g.kind,
                ObligationKind::ProveGuarantee { machine, predicate } if machine.path == "theorem" && predicate == &squash.site)),
            "{goal}: the theorem owes the squash goal: {guarantees:?}"
        );

        assert_eq!(error.message, "Psi cannot form a squash proposition in a contract predicate", "{goal}");
        assert_eq!(error.site, squash.site, "{goal}: L5 stops at the squash goal itself");
        if first.is_none() {
            first = Some((source, error));
        }
    }
    let (source, error) = first.expect("one refused squash goal");
    known_gap::report(
        "squash proposition as a theorem goal",
        "The theorem goal needs a Psi proposition that squashes the source FormSquash names at its level",
        &source,
        &error,
    );

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
    assert_eq!(
        goal.predicate,
        [
            Step::Formal(0),
            Step::HypothesisGuarantee {
                hypothesis: 0,
                operand_count: 1
            }
        ]
    );
}

/// A machine named where a mathematical parameter expects `u8 -> core\Squash<u8>`
/// owes `RefineMathematicalCitation { lemma, that interface }` at the name
/// inside the application. L5 lowers the boundary assumption and then refuses
/// the citing definition at that name. The control passes the definition's
/// own parameter of the same interface instead: it owes no citation and lowers
/// to the application of the assumption.
#[test]
fn a_machine_cited_as_evidence_stops_at_the_citation_inside_its_definition() {
    const CITED: &str = "machine lemma(y:u8) requires y>0 ensures y==y {}";
    const CONSUME: &str = "boundary let consume(evidence:u8 -> core\\Squash<u8>):bool;";
    let citing = |argument: &str| format!("{CITED} {CONSUME} let checked(e:u8 -> core\\Squash<u8>):bool = consume({argument});");
    let byte = ValueType::Integer { signed: false, width: Some(8) };

    let source = citing("lemma");
    let (obligations, error) = refusal(
        &source, sources::span_within(&source, "consume(lemma)", "lemma"),
        r"Psi cannot cite machine `lemma` as evidence of `(u8 -> core\Squash<u8>)` in a mathematical term",
    );
    let applications = of_kind(&obligations, "MathematicalApplication");
    let [application] = applications.as_slice() else {
        panic!("expected one application, found {applications:?}")
    };
    assert_eq!(spelled(&application.site, &source), "consume(lemma)");
    assert!(matches!(&application.kind,
        ObligationKind::MathematicalApplication { definition: Some(id), generic_arguments, argument_count: 1 }
            if id.path == "consume" && generic_arguments.is_empty()));
    let citations = of_kind(&obligations, "RefineMathematicalCitation");
    let [citation] = citations.as_slice() else {
        panic!("expected one citation, found {citations:?}")
    };
    assert_eq!(spelled(&citation.site, &source), "lemma");
    assert!(within(&citation.site, &application.site), "the citation is the argument, not the declaration");
    let ObligationKind::RefineMathematicalCitation { machine, expected } = &citation.kind else {
        unreachable!()
    };
    assert_eq!((machine.package.as_str(), machine.path.as_str()), (pipeline::PACKAGE, "lemma"));
    assert!(
        matches!(expected, ValueType::Function(input, output)
            if input.as_ref() == &byte && matches!(output.as_ref(), ValueType::Squash(inner) if inner.as_ref() == &byte)),
        "the citation names the parameter interface: {expected:?}"
    );

    assert_eq!(error.message, r"Psi cannot cite machine `lemma` as evidence of `(u8 -> core\Squash<u8>)` in a mathematical term");
    assert_eq!(error.site, citation.site, "L5 stops at the cited machine");
    known_gap::report(
        "machine cited as mathematical evidence",
        "The definition needs a Psi step that cites the machine against the evidence interface RefineMathematicalCitation names",
        &source,
        &error,
    );

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
    assert_eq!(
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
    );
}

/// `id::<core\Type<n>>(x)` with `A:core\Type<n+1>` owes
/// `UniverseMembership { core\Type<n>, core\Type<n+1> }` at the application,
/// beside the application that selects the same argument. L5 lowers `id` with
/// its universe bound, then refuses `f` at that application. Twins change both
/// levels. The control removes the universe binder from `id`: the application
/// owes no membership and lowers to the plain head and argument.
#[test]
fn a_universe_bounded_application_stops_at_its_membership_in_the_definition() {
    let mut first = None;
    for (bound, subject) in [(1u8, 0u8), (2, 1)] {
        let universe = format!("core\\Type<{subject}>");
        let application = format!("id::<{universe}>(x)");
        let source = format!("let id<A:core\\Type<{bound}>>(x:A):A=x; let f(x:{universe}):{universe} = {application};");
        let (obligations, error) = refusal(
            &source, sources::unique_span(&source, &application),
            "Psi cannot carry a universe membership check on a mathematical application",
        );

        let memberships = of_kind(&obligations, "UniverseMembership");
        let [membership] = memberships.as_slice() else {
            panic!("{application}: expected one membership, found {memberships:?}")
        };
        assert_eq!(spelled(&membership.site, &source), application);
        let ObligationKind::UniverseMembership { subject: member, sort } = &membership.kind else {
            unreachable!()
        };
        assert_eq!((member.to_string(), sort.to_string()), (universe.clone(), format!("core\\Type<{bound}>")));
        let applications = of_kind(&obligations, "MathematicalApplication");
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

        assert_eq!(error.message, "Psi cannot carry a universe membership check on a mathematical application", "{application}");
        assert_eq!(error.site, membership.site, "{application}: L5 stops at the membership");
        if first.is_none() {
            first = Some((source, error));
        }
    }
    let (source, error) = first.expect("one refused application");
    known_gap::report(
        "universe-bounded mathematical application",
        "The application needs the UniverseMembership of its explicit argument in Psi beside the selected head",
        &source,
        &error,
    );

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

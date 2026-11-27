use super::*;

#[test]
fn cited_kernel_terms_rewrite_beneath_an_application_and_chain_in_reverse() {
    use crate::equality::{SearchError, prove};
    use proof_admission::{
        Budget, Context, Declaration, Level, Sort, Term, TermArena, check_signature, check_type,
    };
    let mut arena = TermArena::new();
    let universe = arena.insert(Term::Sort(Sort::Type(Level::Constant(0))));
    let [carrier, operation, x, y, z, xy, zy] = std::array::from_fn(|slot| {
        arena.insert(Term::Constant {
            declaration: slot as u32,
            levels: vec![],
        })
    });
    let unary = arena.insert(Term::Pi {
        domain: carrier,
        codomain: carrier,
    });
    let x_equals_y = arena.insert(Term::Id {
        ty: carrier,
        left: x,
        right: y,
    });
    let z_equals_y = arena.insert(Term::Id {
        ty: carrier,
        left: z,
        right: y,
    });
    let declarations = [
        universe, unary, carrier, carrier, carrier, x_equals_y, z_equals_y,
    ]
    .map(|ty| Declaration::assumption(0, ty));
    let mut budget = Budget::default();
    let signature = check_signature(&mut arena, &declarations, &mut budget).unwrap();
    let context = Context::empty().with_signature(signature);
    let left = arena.apply_arguments(operation, &[x]);
    let right = arena.apply_arguments(operation, &[z]);
    let goal = arena.insert(Term::Id {
        ty: carrier,
        left,
        right,
    });
    let proof = prove(&mut arena, &context, goal, &[xy, zy], 256, &mut budget).unwrap();
    check_type(&mut arena, &context, proof, goal, &mut budget).unwrap();
    assert_eq!(
        prove(&mut arena, &context, goal, &[xy], 256, &mut budget),
        Err(SearchError::NoProof)
    );
    assert_eq!(
        prove(&mut arena, &context, goal, &[xy, zy], 1, &mut budget),
        Err(SearchError::Budget(
            crate::equality::BudgetLimit::SearchSteps
        ))
    );
}

const CITED: &str = include_str!("../../tests/fixtures/semiring_identity/pass/cited/main.omg");
const UNCITED: &str = include_str!("../../tests/fixtures/semiring_identity/fail/uncited/main.omg");

fn generic(source: &str) -> RouteVerdict {
    generic_with_tables(source, |_| {})
}

/// `generic`, after `change` has edited the Identity tables stage 05 wrote.
fn generic_with_tables(
    source: &str,
    change: impl FnOnce(&mut Vec<terminal_check_tables::mathematics::IdentityTable>),
) -> RouteVerdict {
    let typed = super::build_purpose::frontier_with_core(
        source,
        "machine build_entry() {}",
        source_provenance::DependencyScope::Product,
        &[
            (
                "algebra.omg",
                "CommutativeSemiring",
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../../../../../source/library/core/algebra.omg"
                )),
            ),
            (
                "proof_clause_operation.omg",
                "f",
                "public boundary machine f(x: u64) -> u64 reaches none crashes none terminates;",
            ),
        ],
    );
    let prepared = crate::prepare_fixture(typed);
    let scope = source_provenance::DependencyScope::Product;
    let compilation = crate::lowering::prepare_compilation(
        &prepared,
        super::build_purpose::roster(&[], &[Some(super::build_purpose::package())], scope),
        terminal_codec::TerminalPublication::Invocation,
    )
    .unwrap();
    let mut program = compilation.program;
    if let terminal_check_tables::compilation::TerminalCompilation::Prepared(prepared) =
        &mut program
    {
        change(&mut prepared.mathematics.identities);
    }
    let mut parts = crate::judge_compilation(program).into_iter();
    let closure = parts.next().expect("package judgment").unwrap();
    assert!(matches!(closure, RouteVerdict::Accepted(_)), "{closure:?}");
    let form = parts.next().expect("generic machine judgment").unwrap();
    for other in parts {
        assert!(matches!(other, Ok(RouteVerdict::Accepted(_))), "{other:?}");
    }
    form
}

#[test]
fn semiring_identity_uses_the_cited_law() {
    assert_cited_semiring_identity(generic(CITED));
}

/// The generic machine's carrier `T`, the operation `Ring::add` and the law
/// `Ring::add_comm` are one table of the mathematical signature, which the
/// kernel checks once. The judged Identity starts from that table and adds
/// only the machine's two inputs.
#[test]
fn semiring_identity_starts_from_the_forms_identity_table() {
    let mut table = None;
    let verdict = generic_with_tables(CITED, |tables| {
        let [only] = tables.as_slice() else {
            panic!("one generic check form has one Identity table: {tables:?}");
        };
        table = Some(only.clone());
    });
    let table = table.unwrap();
    assert_eq!(table.suppliers.len(), 1, "{table:?}");
    assert!(
        table
            .carriers
            .contains(&semantic_vocabulary::ClauseCarrier::Parameter(0)),
        "{table:?}"
    );
    let add = table
        .operations
        .iter()
        .find(|operation| operation.domains.len() == 2)
        .unwrap_or_else(|| panic!("the table declares `Ring::add`: {table:?}"));
    assert_eq!(
        add.declaration.supplier,
        Some((
            table.suppliers[0].arena_index(),
            table.suppliers[0].generation()
        ))
    );
    assert!(!table.laws.is_empty(), "{table:?}");
    let RouteVerdict::Accepted(closure) = verdict else {
        panic!("{verdict:?}")
    };
    let guarantees = verdicts(closure.elaboration(), closure.discharge(), 1, guarantee);
    let [(_, Ok(Discharged::Identity(judgment)))] = guarantees.as_slice() else {
        panic!("{guarantees:?}")
    };
    let checked = terminal_codec::decode_mathematical_certificate(&judgment.certificate).unwrap();
    // Every declaration past the table is one of the machine's two inputs.
    let declared = checked.certificate.signature.len();
    let table_declarations = declared - 2;
    assert!(
        checked.certificate.signature[table_declarations..]
            .iter()
            .all(proof_admission::Declaration::is_assumption)
    );
    assert!(table_declarations > table.carriers.len() + table.operations.len());
}

/// The twin: the same program, with the law it cites missing from the
/// table. The obligation does not declare the law itself; it is refused.
#[test]
fn a_cited_law_missing_from_the_identity_table_is_refused() {
    let verdict = generic_with_tables(CITED, |tables| tables[0].laws.clear());
    assert_refused_for(
        verdict,
        "a cited law that is not in the form's Identity table",
    );
}

/// The twin for an operation: with `Ring::add` missing from the table, the
/// goal that applies it is refused rather than given a declaration of its own.
#[test]
fn a_trait_operation_missing_from_the_identity_table_is_refused() {
    let verdict = generic_with_tables(CITED, |tables| {
        tables[0].operations.clear();
        tables[0].laws.clear();
    });
    assert_refused_for(
        verdict,
        "a trait operation that is not in the form's Identity table",
    );
}

/// The twin for a table the kernel does not accept: an operation declared at
/// a carrier the table never declared is refused for every Identity of the
/// form, with the reason.
#[test]
fn an_identity_table_the_kernel_refuses_refuses_the_forms_identities() {
    let verdict = generic_with_tables(CITED, |tables| {
        let table = &mut tables[0];
        let mut twice = table.operations[0].clone();
        twice.result =
            semantic_vocabulary::ClauseCarrier::Scalar(semantic_vocabulary::ScalarType::Boolean);
        table.operations.push(twice);
    });
    assert_refused_for(verdict, "the machine's Identity table cannot be declared");
}

fn assert_refused_for(verdict: RouteVerdict, reason: &str) {
    let text = format!("{verdict:?}");
    assert!(
        !matches!(verdict, RouteVerdict::Accepted(_)),
        "the guarantee must not be judged: {text}"
    );
    assert!(text.contains(reason), "expected `{reason}`: {text}");
}

fn assert_cited_semiring_identity(verdict: RouteVerdict) {
    let RouteVerdict::Accepted(closure) = verdict else {
        panic!("expected an accepted generic contract: {verdict:?}")
    };
    let guarantees = verdicts(closure.elaboration(), closure.discharge(), 1, guarantee);
    let [(_, Ok(Discharged::Identity(judgment)))] = guarantees.as_slice() else {
        panic!("expected one judged Identity, got {guarantees:?}")
    };
    assert!(matches!(
        judgment.decision,
        MathematicalCoreDecision::Judged(_)
    ));
    assert_eq!(judgment.citations.len(), 1);
    let mut checked =
        terminal_codec::decode_mathematical_certificate(&judgment.certificate).unwrap();
    proof_admission::verify_mathematical_certificate(
        &mut checked.arena,
        &checked.certificate,
        &mut Default::default(),
    )
    .unwrap();
    use proof_admission::Term;
    let Term::Id { ty, left, right } = checked.arena.get(checked.certificate.expected) else {
        panic!("the contract must denote kernel Id")
    };
    let binary = |term| {
        let Term::Apply {
            function,
            argument: second,
        } = checked.arena.get(term)
        else {
            panic!("second add operand")
        };
        let Term::Apply {
            function: add,
            argument: first,
        } = checked.arena.get(function)
        else {
            panic!("first add operand")
        };
        (add, first, second)
    };
    let (add, b, a) = binary(left);
    let (other_add, other_a, other_b) = binary(right);
    assert!(checked.arena.structurally_equal(add, other_add));
    assert!(checked.arena.structurally_equal(a, other_a));
    assert!(checked.arena.structurally_equal(b, other_b));
    assert!(!checked.arena.structurally_equal(a, b));
    assert!(matches!(checked.arena.get(ty), Term::Constant { .. }));
    assert!(matches!(
        checked.arena.get(checked.certificate.term),
        Term::IdElim { .. }
    ));
}

#[test]
fn a_law_over_an_integer_keeps_its_operation_and_inputs() {
    let source = CITED
        .replace("T [copy], ", "")
        .replace("<T>", "<u64>")
        .replace(": T", ": u64");
    let verdict = generic(&source);
    let RouteVerdict::Accepted(closure) = verdict else {
        panic!("{verdict:?}")
    };
    let guarantees = verdicts(closure.elaboration(), closure.discharge(), 1, guarantee);
    assert!(
        matches!(guarantees.as_slice(), [(_, Ok(Discharged::Identity(judgment)))]
        if matches!(judgment.decision, MathematicalCoreDecision::Judged(_))),
        "{guarantees:?}"
    );
}

#[test]
fn an_entry_equality_is_a_kernel_hypothesis() {
    let equality = UNCITED
        .lines()
        .find_map(|line| line.strip_prefix("ensures "))
        .unwrap();
    let source = UNCITED.replace("ensures ", &format!("requires {equality}\nensures "));
    let verdict = generic(&source);
    let RouteVerdict::Accepted(closure) = verdict else {
        panic!("{verdict:?}")
    };
    let guarantees = verdicts(closure.elaboration(), closure.discharge(), 1, guarantee);
    let [(_, Ok(Discharged::Identity(judgment)))] = guarantees.as_slice() else {
        panic!("{guarantees:?}")
    };
    assert!(
        matches!(judgment.decision, MathematicalCoreDecision::Judged(receipt) if receipt.context_depth == 1)
    );
}

#[test]
fn semiring_identity_without_citation_names_the_unproved_equality() {
    let statement = unproved_statement_at_ensures(UNCITED, generic(UNCITED));
    assert!(statement.contains(ensures_text(UNCITED)), "{statement}");
}

fn ensures_text(source: &str) -> &str {
    source
        .lines()
        .find_map(|line| line.strip_prefix("ensures "))
        .unwrap()
}

fn unproved_statement_at_ensures(source: &str, verdict: RouteVerdict) -> String {
    let RouteVerdict::Rejected { unproved, .. } = verdict else {
        panic!("expected an unproved return guarantee: {verdict:?}")
    };
    let [unproved] = unproved.as_slice() else {
        panic!("one unproved guarantee")
    };
    let equality = ensures_text(source);
    assert_eq!(unproved.cause, UnprovedCause::NoDerivation);
    let terminal_check_tables::program::SourceOrigin::Authored(site) = unproved.site else {
        panic!("expected authored source position")
    };
    assert!(
        source[site.span.start..site.span.end].contains(equality),
        "{site:?}"
    );
    unproved.statement.clone()
}

const ERASED: &str = r#"
use {omega}/language/core/algebra.omg;
use {omega}/language/core/foundation.omg as proof;

machine entry<const Ring: |algebra|::CommutativeSemiring<bool>>(a: bool, b: bool) {
    Ring::add_comm(a, b);
    consume(Ring::add(a, b), Ring::add(b, a));
}

machine consume(left [erased]: bool, right [erased]: bool)
requires |proof|::Identity::<bool>(left, right)
{}
"#;

#[test]
fn an_erased_declaration_application_keeps_the_cited_equality() {
    let verdict = generic(ERASED);
    let RouteVerdict::Accepted(closure) = verdict else {
        panic!("{verdict:?}");
    };
    let requirements = verdicts(closure.elaboration(), closure.discharge(), 1, requirement);
    let [(_, Ok(Discharged::Identity(judgment)))] = requirements.as_slice() else {
        panic!("expected the consuming call's kernel judgment, got {requirements:?}");
    };
    assert!(matches!(
        judgment.decision,
        MathematicalCoreDecision::Judged(_)
    ));
    assert_eq!(judgment.citations.len(), 1);
    let mut checked =
        terminal_codec::decode_mathematical_certificate(&judgment.certificate).unwrap();
    proof_admission::verify_mathematical_certificate(
        &mut checked.arena,
        &checked.certificate,
        &mut Default::default(),
    )
    .unwrap();
    let proof_admission::Term::Id { left, right, .. } =
        checked.arena.get(checked.certificate.expected)
    else {
        panic!("the erased requirement must denote kernel Identity");
    };
    for endpoint in [left, right] {
        assert!(matches!(
            checked.arena.get(endpoint),
            proof_admission::Term::Apply { .. }
        ));
    }
    assert!(!checked.arena.structurally_equal(left, right));

    let entry = &closure.elaboration().module.machines[0];
    let operations = entry
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .collect::<Vec<_>>();
    assert_eq!(
        operations.len(),
        2,
        "only the law citation and the consuming call"
    );
    let OperationKind::CallUnit {
        arguments,
        erased_arguments,
        ..
    } = &operations[1].kind
    else {
        panic!("the second operation calls the consumer");
    };
    assert!(arguments.is_empty());
    assert!(matches!(erased_arguments.as_slice(), [
        ScalarTerm::DeclarationApplication { declaration: left, .. },
        ScalarTerm::DeclarationApplication { declaration: right, .. },
    ] if left == right));
}

#[test]
fn an_erased_declaration_application_cannot_supply_its_own_equality() {
    let source = ERASED.replace("    Ring::add_comm(a, b);\n", "");
    let verdict = generic(&source);
    let RouteVerdict::Rejected { unproved, .. } = verdict else {
        panic!("{verdict:?}");
    };
    let [failure] = unproved.as_slice() else {
        panic!("expected the erased argument's unproved equality, got {unproved:?}");
    };
    assert_eq!(failure.cause, UnprovedCause::NoDerivation);
    assert!(requirement(&failure.kind));
}

#[test]
fn a_domain_cast_proves_its_typed_identity_in_the_kernel() {
    let verdict = generic(
        r#"
use {omega}/language/core/proof_clause_operation.omg;
use {omega}/language/core/foundation.omg as proof;
domain u64::Reflexive requires |proof|::Identity::<u64>(|proof_clause_operation|::f(self), |proof_clause_operation|::f(self));
machine entry<T [copy]>(x: u64, _fallback: T) {
    let held: u64 in Reflexive = x as +Reflexive;
}
"#,
    );
    let RouteVerdict::Accepted(closure) = verdict else {
        panic!("{verdict:?}");
    };
    let arrivals = verdicts(closure.elaboration(), closure.discharge(), 1, |kind| {
        matches!(kind, ObligationKind::ArrivalInvariant { .. })
    });
    assert!(arrivals.iter().any(|(_, result)| matches!(result,
        Ok(Discharged::Identity(judgment)) if matches!(judgment.decision, MathematicalCoreDecision::Judged(_))
    )), "{arrivals:?}");
}

#[test]
fn a_definition_body_retains_a_bodyless_application() {
    let verdict = generic(
        r#"
use {omega}/language/core/proof_clause_operation.omg;
use {omega}/language/core/foundation.omg as proof;
let same(x: u64): |proof|::Strict<0> = |proof_clause_operation|::f(x) == |proof_clause_operation|::f(x);
machine entry<T [copy]>(x: u64, _fallback: T) ensures same(x) {}
"#,
    );
    let RouteVerdict::Accepted(closure) = verdict else {
        panic!("{verdict:?}");
    };
    assert!(matches!(
        verdicts(closure.elaboration(), closure.discharge(), 1, guarantee).as_slice(),
        [(_, Ok(Discharged::Identity(judgment)))]
            if matches!(judgment.decision, MathematicalCoreDecision::Judged(_))
    ));
}

#[test]
fn an_erased_integer_application_uses_the_typed_kernel_route() {
    let verdict = generic(&ERASED.replace("bool", "u64"));
    let RouteVerdict::Accepted(closure) = verdict else {
        panic!("{verdict:?}");
    };
    assert!(matches!(
        verdicts(closure.elaboration(), closure.discharge(), 1, requirement).as_slice(),
        [(_, Ok(Discharged::Identity(judgment)))]
            if matches!(judgment.decision, MathematicalCoreDecision::Judged(_))
    ));
}

#[test]
fn an_erased_application_keeps_its_selected_semiring() {
    let source = ERASED
        .replace(
            "const Ring: |algebra|::CommutativeSemiring<bool>",
            "const Ring: |algebra|::CommutativeSemiring<bool>, const Other: |algebra|::CommutativeSemiring<bool>",
        )
        .replace(
            "consume(Ring::add(a, b), Ring::add(b, a))",
            "consume(Other::add(a, b), Other::add(b, a))",
        );
    let RouteVerdict::Rejected { unproved, .. } = generic(&source) else {
        panic!("a law from Ring must not prove a goal over Other");
    };
    assert!(matches!(unproved.as_slice(), [failure]
        if requirement(&failure.kind) && failure.cause == UnprovedCause::NoDerivation));
}

#[test]
fn a_type_definition_body_forms_identity_through_the_clause_reader() {
    let verdict = generic(
        r#"
use {omega}/language/core/proof_clause_operation.omg;
use {omega}/language/core/foundation.omg as proof;
let same(x: u64): |proof|::Type<0> = |proof|::Identity::<u64>(|proof_clause_operation|::f(x), |proof_clause_operation|::f(x));
machine entry<T [copy]>(x: u64, _fallback: T) ensures same(x) {}
"#,
    );
    let RouteVerdict::Accepted(closure) = verdict else {
        panic!("{verdict:?}");
    };
    assert!(matches!(
        verdicts(closure.elaboration(), closure.discharge(), 1, guarantee).as_slice(),
        [(_, Ok(Discharged::Identity(judgment)))]
            if matches!(judgment.decision, MathematicalCoreDecision::Judged(_))
    ));
}

#[test]
fn an_erased_type_argument_retains_the_callers_identity() {
    let through_formal = |source: &str| {
        let statement = ensures_text(source);
        source
            .replace(
                "machine entry",
                "let stated(p [erased]: |proof|::Type<0>): |proof|::Type<0> = p;\nmachine entry",
            )
            .replace(
                &format!("ensures {statement}"),
                &format!("ensures stated({statement})"),
            )
    };
    let cited = through_formal(CITED);
    let verdict = generic(&cited);
    if let RouteVerdict::Accepted(closure) = &verdict {
        let RouteVerdict::Accepted(direct) = generic(CITED) else {
            panic!("direct Identity refused")
        };
        let actual = &closure.elaboration().module.machines[0];
        let original = &direct.elaboration().module.machines[0];
        assert!(
            actual
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .eq(original.blocks.iter().flat_map(|block| &block.operations)),
            "passing a proposition changed the executable operations"
        );
        let definitions = &closure
            .elaboration()
            .module
            .scalars
            .scalar_predicate_definitions;
        assert!(
            definitions
                .iter()
                .any(|definition| definition.type_parameters == [0]
                    && definition.parameters.is_empty()
                    && definition.body
                        == terminal_psi::MathematicalPredicateBody::TypeParameter(0))
        );
    }
    assert_cited_semiring_identity(verdict);
    let uncited = through_formal(UNCITED);
    let statement = unproved_statement_at_ensures(&uncited, generic(&uncited));
    assert!(
        statement.ends_with("stated(|proof|::Identity::<T>(Ring.add(b, a), Ring.add(a, b)))"),
        "{statement}"
    );
}

#[test]
fn an_unused_erased_type_argument_does_not_require_an_identity_witness() {
    let verdict = generic(
        r#"
use {omega}/language/core/foundation.omg as proof;

let ignore(p [erased]: |proof|::Type<0>): |proof|::Strict<0> = 0u64 == 0;
machine probe<T [copy]>(x: u64, y: u64, _unused: T)
ensures ignore(|proof|::Identity::<u64>(x, y))
{}
"#,
    );
    assert!(matches!(verdict, RouteVerdict::Accepted(_)), "{verdict:?}");
}

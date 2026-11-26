//! Products expanded over sums under a conformance's distributivity law,
//! judged by the mathematical kernel.
//!
//! The recovered `ring_full_polynomial_compile` program states
//! `mul(add(a, b), add(c, d))` equal to its four-term expansion over the
//! library's `Nat` and cites nothing. `core/nat.omg` declares
//! `CommutativeSemiring<Nat>`, whose `mul_distributes` law is left
//! distributivity, so the proof may expand both sides into sums of products
//! and then regroup and reorder them under the conformance's commutativity
//! and associativity laws. Every expansion is an application of the law in
//! the proof term, so the certificate's assumptions name it.
//!
//! Expansion doubles the terms per factor, so each product may expand to at
//! most `MAX_SUMMANDS` summands, counted before anything is built; a larger
//! one stops the proof as out of budget, never as wrong.

use super::*;
use crate::laws::{SignatureJudgment, UnprovedLaw};
use terminal_check_tables::mathematics::{OperandLaw, operand_law};

const POLYNOMIAL: &str =
    include_str!("../../tests/fixtures/polynomial/pass/ring_full_polynomial_compile/main.omg");
const UNLICENSED: &str = include_str!(
    "../../tests/fixtures/polynomial/fail/ring_rearrange_unlicensed_rejected/main.omg"
);
/// A Peano semiring whose conformance declares left distributivity.
const PEANO: &str =
    include_str!("../../tests/fixtures/ring_law/pass/peano_left_distributivity/main.omg");
const NAT: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../../source/library/core/nat.omg"
));
const ALGEBRA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../../source/library/core/algebra.omg"
));

/// The fixture's proofs, without the entry that needs the standard library.
fn proofs(source: &str) -> String {
    let end = source.find("data Main").expect("the fixture's entry state");
    source[..end].replace("use {omega}/language/core/process_exit.omg;\n", "")
}

/// The recovered program without its entry state, its exit machine and its
/// entry, which need the standard library. Its laws stay where they are,
/// including those declared after the entry state.
fn without_entry(source: &str) -> String {
    let start = source.find("data Main").expect("the fixture's entry state");
    let exit = start
        + source[start..]
            .find("machine exit_with_status")
            .expect("the exit machine");
    let end = exit + source[exit..].find('}').expect("the exit machine's body") + 1;
    let rest = &source[end..];
    let rest = rest
        .find("// The entry owns")
        .map_or(rest, |entry| &rest[..entry]);
    format!("{}{rest}", &source[..start])
        .replace("use {omega}/language/core/process_exit.omg;\n", "")
}

fn compile(source: &str, core: &[(&str, &str, &str)]) -> Vec<RouteVerdict> {
    let typed = super::build_purpose::frontier_with_core(
        source,
        "machine build_entry() {}",
        source_provenance::DependencyScope::Product,
        core,
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

fn over_nat(source: &str) -> Vec<RouteVerdict> {
    compile(
        source,
        &[
            ("nat.omg", "Nat", NAT),
            ("nat.omg", "add", NAT),
            ("nat.omg", "mul", NAT),
            ("algebra.omg", "CommutativeSemiring", ALGEBRA),
        ],
    )
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

/// The law the kernel judged for `machine`, with its certificate decoded and
/// checked again, and the laws its proof assumed, read by the one reader of
/// operand laws.
fn assumed_laws(
    judgment: &SignatureJudgment,
    machine: &str,
) -> (
    proof_admission::MathematicalJudgmentReceipt,
    u32,
    Vec<(u32, OperandLaw)>,
) {
    let law = judgment
        .laws
        .iter()
        .find(|law| law.machine == machine)
        .unwrap_or_else(|| panic!("the law `{machine}`: {judgment:?}"));
    let MathematicalCoreDecision::Judged(receipt) = law.decision;
    let mut checked = terminal_codec::decode_mathematical_certificate(&law.certificate).unwrap();
    proof_admission::verify_mathematical_certificate(
        &mut checked.arena,
        &checked.certificate,
        &mut Default::default(),
    )
    .unwrap();
    let closure =
        proof_admission::certificate_assumption_closure(&checked.arena, &checked.certificate);
    let laws = closure
        .iter()
        .map(|&assumed| {
            operand_law(
                &checked.arena,
                checked.certificate.signature[assumed as usize].ty,
            )
            .unwrap_or_else(|| panic!("assumption {assumed} states no law of operands"))
        })
        .collect();
    (receipt, law.conversion_steps, laws)
}

#[test]
fn a_product_of_two_sums_expands_under_the_library_distributivity_law() {
    let verdicts = over_nat(&without_entry(POLYNOMIAL));
    let judgment = signature(&verdicts);
    let (receipt, steps, laws) = assumed_laws(judgment, "four_term_expansion");
    assert_eq!(receipt.context_depth, 0);
    eprintln!(
        "four_term_expansion: {} declarations, {} arena slots, {} conversion steps, assumes {laws:?}",
        receipt.declarations, receipt.arena_slots, steps
    );
    // The body cites nothing. The proof rests on exactly four laws of the
    // library's conformance, and the certificate names each one: left
    // distributivity of `mul` over `add` (`mul_distributes`), commutativity
    // of `mul` to bring the sum of `mul(add(a, b), c)` to the side that law
    // expands, and commutativity and associativity of `add` to bring the
    // four products into one order.
    let Some(&(mul, OperandLaw::Distributes { over: add, .. })) = laws
        .iter()
        .find(|(_, law)| matches!(law, OperandLaw::Distributes { .. }))
    else {
        panic!("a distributivity law: {laws:?}")
    };
    assert_ne!(mul, add);
    let mut expected = vec![
        (add, OperandLaw::Commutes),
        (
            add,
            OperandLaw::Associates {
                left_nested_first: true,
            },
        ),
        (mul, OperandLaw::Commutes),
        (
            mul,
            OperandLaw::Distributes {
                over: add,
                from_left: true,
                product_first: true,
            },
        ),
    ];
    let mut laws = laws;
    let key = |law: &(u32, OperandLaw)| format!("{law:?}");
    laws.sort_by_key(key);
    expected.sort_by_key(key);
    assert_eq!(laws, expected);
}

/// The Peano fixture with one more law, stated after its entry state is cut.
fn peano_with(law: &str) -> String {
    format!("{}\n{law}\n", proofs(PEANO))
}

const EXPANDS: &str = "machine expands(a: Peano, b: Peano, c: Peano, d: Peano)
ensures |proof|::Identity::<Peano>(times(a, plus(b, plus(c, d))), plus(plus(times(a, d), times(a, b)), times(a, c)))
{
}";

#[test]
fn a_distributivity_law_licenses_expansion_only_through_its_conformance() {
    // Peano's conformance declares left distributivity but no commutativity
    // of `times`, so only a sum on the right of a product expands. The new
    // law needs two expansions, and the three products reordered.
    let licensed = peano_with(EXPANDS);
    let verdicts = compile(&licensed, &[]);
    let judgment = signature(&verdicts);
    let (_, steps, laws) = assumed_laws(judgment, "expands");
    eprintln!("expands: {steps} conversion steps, assumes {laws:?}");
    assert!(
        laws.iter().any(|(_, law)| matches!(
            law,
            OperandLaw::Distributes {
                from_left: true,
                ..
            }
        )),
        "{laws:?}"
    );

    // The same proved law, no longer declared as the trait's
    // `mul_distributes`: a law of the right shape in scope licenses nothing.
    let source = licensed.replace("satisfies Semiring3<Peano>::mul_distributes\n", "");
    assert_ne!(source, licensed);
    let verdicts = compile(&source, &[]);
    let law = unproved(&verdicts);
    assert_eq!(law.machine, "expands");
    assert_eq!(law.cause, UnprovedCause::NoDerivation);
}

/// `T` nesting `levels` products, each of `b` and a sum of two copies of the
/// one below it, down to `plus(c, d)`. Under left distributivity alone it
/// expands to `2^(levels + 1)` summands.
fn doubling(levels: u32) -> String {
    let mut term = "plus(c, d)".to_owned();
    for _ in 0..levels {
        term = format!("times(b, plus({term}, {term}))");
    }
    term
}

#[test]
fn a_product_past_the_summand_limit_runs_out_of_proof_budget() {
    // `times(a, plus(T, T))` with five levels in `T` expands to 128
    // summands, past the limit of 64. The law is true (its sides differ by
    // one distribution step), but the checker stops before building the
    // expansion and does not call it wrong.
    let nested = doubling(5);
    let law = format!(
        "machine too_wide(a: Peano, b: Peano, c: Peano, d: Peano)
ensures |proof|::Identity::<Peano>(plus(times(a, {nested}), times(a, {nested})), times(a, plus({nested}, {nested})))
{{
}}"
    );
    let verdicts = compile(&peano_with(&law), &[]);
    let law = unproved(&verdicts);
    assert_eq!(law.machine, "too_wide");
    assert_eq!(law.cause, UnprovedCause::BudgetExhausted);
    let message = law.reject_diagnostic().message;
    assert!(
        message.contains("ran out of proof budget")
            && message.contains("no claim that it is wrong"),
        "{message}"
    );
    // One level less is 64 summands, within the limit, and proved.
    let nested = doubling(4);
    let law = format!(
        "machine wide(a: Peano, b: Peano, c: Peano, d: Peano)
ensures |proof|::Identity::<Peano>(plus(times(a, {nested}), times(a, {nested})), times(a, plus({nested}, {nested})))
{{
}}"
    );
    let verdicts = compile(&peano_with(&law), &[]);
    let judgment = signature(&verdicts);
    let (_, steps, _) = assumed_laws(judgment, "wide");
    eprintln!("wide: {steps} conversion steps");
}

#[test]
fn expansion_is_counted_before_it_is_built_and_each_mutual_law_still_applies() {
    use crate::equality::{
        Definitions, Distribution, License, Licenses, MAX_SUMMANDS, SearchError, prove_licensed,
    };
    use proof_admission::{
        Budget, Context, Declaration, Level, Sort, Term, TermArena, check_signature, check_type,
    };
    // A bare signature: a type, two operations, eight values, and each
    // operation distributing over the other from the left.
    let mut arena = TermArena::new();
    let universe = arena.insert(Term::Sort(Sort::Type(Level::Constant(0))));
    let constants: [_; 13] = std::array::from_fn(|slot| {
        arena.insert(Term::Constant {
            declaration: slot as u32,
            levels: vec![],
        })
    });
    let [
        carrier,
        meet,
        join,
        values @ ..,
        meet_over_join,
        join_over_meet,
    ] = constants;
    let binary = {
        let unary = arena.insert(Term::Pi {
            domain: carrier,
            codomain: carrier,
        });
        arena.insert(Term::Pi {
            domain: carrier,
            codomain: unary,
        })
    };
    let apply = |arena: &mut TermArena, op, x, y| arena.apply_arguments(op, &[x, y]);
    let left_distributes = |arena: &mut TermArena, outer, inner| {
        let (x, y, z) = (
            arena.insert(Term::Variable(2)),
            arena.insert(Term::Variable(1)),
            arena.insert(Term::Variable(0)),
        );
        let sum = apply(arena, inner, y, z);
        let product = apply(arena, outer, x, sum);
        let (xy, xz) = (apply(arena, outer, x, y), apply(arena, outer, x, z));
        let expanded = apply(arena, inner, xy, xz);
        let mut statement = arena.insert(Term::Id {
            ty: carrier,
            left: product,
            right: expanded,
        });
        for _ in 0..3 {
            statement = arena.insert(Term::Pi {
                domain: carrier,
                codomain: statement,
            });
        }
        statement
    };
    let meet_law = left_distributes(&mut arena, meet, join);
    let join_law = left_distributes(&mut arena, join, meet);
    let mut declarations = vec![universe, binary, binary];
    declarations.extend([carrier; 8]);
    declarations.extend([meet_law, join_law]);
    let declarations: Vec<_> = declarations
        .into_iter()
        .map(|ty| Declaration::assumption(0, ty))
        .collect();
    let checked = check_signature(&mut arena, &declarations, &mut Budget::default()).unwrap();
    let context = Context::empty().with_signature(checked);
    let law = |outer, inner, proof| Distribution {
        outer,
        inner,
        carrier,
        law: proof,
        from_left: true,
        product_first: true,
    };
    let distributions = [
        law(meet, join, meet_over_join),
        law(join, meet, join_over_meet),
    ];
    let attempt = |arena: &mut TermArena, left, right, search_steps| {
        let goal = arena.insert(Term::Id {
            ty: carrier,
            left,
            right,
        });
        let mut budget = Budget::default();
        let proof = prove_licensed(
            arena,
            &context,
            &Definitions::plain(&context),
            goal,
            &[],
            Licenses {
                operations: &[] as &[License],
                distributions: &distributions,
            },
            search_steps,
            &mut budget,
        )?;
        check_type(arena, &context, proof, goal, &mut budget).unwrap();
        Ok::<_, SearchError>(())
    };
    let [a, b, c, ..] = values;
    // Each law proves its own expansion, though expanding both ways at once
    // would undo itself.
    for (outer, inner) in [(meet, join), (join, meet)] {
        let sum = apply(&mut arena, inner, b, c);
        let product = apply(&mut arena, outer, a, sum);
        let (ab, ac) = (
            apply(&mut arena, outer, a, b),
            apply(&mut arena, outer, a, c),
        );
        let expanded = apply(&mut arena, inner, ab, ac);
        attempt(&mut arena, expanded, product, 4096).unwrap();
    }
    // meet(v0, join(meet(v1, join(...)), ...)): a product whose expansion
    // has 2^7 summands, one more doubling than the limit allows.
    let mut wide = apply(&mut arena, join, values[6], values[7]);
    for &value in values[..6].iter().rev() {
        let pair = apply(&mut arena, join, wide, wide);
        wide = apply(&mut arena, meet, value, pair);
    }
    const { assert!(1 << 7 > MAX_SUMMANDS) };
    let other = apply(&mut arena, join, wide, values[0]);
    // The goal is false, so without the limit the search would expand both
    // sides and find no proof. With no step limit, only the count can stop
    // it as out of budget.
    assert_eq!(
        attempt(&mut arena, other, wide, usize::MAX),
        Err(SearchError::Budget)
    );
}

#[test]
fn the_recovered_unlicensed_program_is_refused_for_its_three_reasons() {
    // The recovered program claims three things, and each is one law left
    // unproved while every other law in it is judged. Laws merely in scope
    // license nothing: `needs_rearrange` has `plus`'s associativity beside
    // it and no conformance. One trait application cannot borrow another's
    // law: `mixed_application_associativity` is about the operation of
    // `ApplicationJoin<Peano, FirstTag>`, whose associativity only
    // `ApplicationJoin<Peano, SecondTag>` proves. And one satisfier group
    // cannot borrow another's: `mixed_family_associativity` is about
    // `combine as CommutativeNand`, and the associativity proved for
    // `FamilyJoin<Peano>` is of `combine as AssociativeAnd`.
    let verdicts = compile(&without_entry(UNLICENSED), &[]);
    let mut laws = verdicts.iter().filter_map(|verdict| match verdict {
        RouteVerdict::UnprovedLaws(laws) => Some(laws),
        RouteVerdict::Laws(judgment) => panic!("the program must not be accepted: {judgment:?}"),
        RouteVerdict::Accepted(_) => None,
        other => panic!("every part must be read: {other:?}"),
    });
    let (Some(laws), None) = (laws.next(), laws.next()) else {
        panic!("one list of unproved laws")
    };
    let left: Vec<_> = laws
        .iter()
        .map(|law| (law.machine.as_str(), law.cause))
        .collect();
    assert_eq!(
        left,
        [
            ("needs_rearrange", UnprovedCause::NoDerivation),
            (
                "mixed_application_associativity",
                UnprovedCause::NoDerivation
            ),
            ("mixed_family_associativity", UnprovedCause::NoDerivation),
        ]
    );
    assert_eq!(
        laws[2].statement,
        "cannot prove ensures contract for `mixed_family_associativity`: \
         core\\Identity::<Peano>(family_nand(family_nand(a, b), c), \
         family_nand(a, family_nand(b, c)))"
    );

    // The program's first part, everything before its entry state, needs
    // no satisfier group. Nothing there declares a conformance, so nothing
    // licenses rearranging `plus`, and the shuffle is refused at its
    // successor arm. That arm also needs `plus(b, Succ(x)) = Succ(plus(b,
    // x))`, which nothing here proves, so this refusal alone does not show
    // that the missing license is what stops it; the Peano twin above does.
    let verdicts = compile(&proofs(UNLICENSED), &[]);
    let law = unproved(&verdicts);
    assert_eq!(law.machine, "needs_rearrange");
    assert_eq!(law.cause, UnprovedCause::NoDerivation);
    assert_eq!(
        law.statement,
        "cannot prove ensures contract for `needs_rearrange` in the case `Peano::Succ`: \
         core\\Identity::<Peano>(plus(b, plus(Peano::Succ(pred), c)), \
         plus(Peano::Succ(pred), plus(b, c)))"
    );
}

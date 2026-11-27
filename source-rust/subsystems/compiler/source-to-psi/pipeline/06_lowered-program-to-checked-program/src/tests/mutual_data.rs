//! A mutual block of data from source, judged by the mathematical kernel.
//!
//! `Even` and `Odd` contain each other, so stage 05 declares them as one
//! kernel block with a recursor per member, and reads the mutually recursive
//! `half` and `half_odd` through the recursors of that block.

use super::seq_laws::{judged, signature, unproved};
use super::*;

const MUTUAL: &str = "
data Even {
    case Zero;
    case Next(odd: Odd);
}

data Odd {
    case Next(even: Even);
}

machine half(e: Even) -> Nat
terminates by e
{
    transition e {
        Even::Zero -> Nat::Zero,
        Even::Next { odd } -> Nat::Succ { prev: half_odd(odd) }
    }
}

machine half_odd(o: Odd) -> Nat
terminates by o
{
    transition o {
        Odd::Next { even } -> half(even)
    }
}

machine half_two()
ensures
    |proof|::Identity::<Nat>(
        half(Even::Next { odd: Odd::Next { even: Even::Zero } }),
        Nat::Succ { prev: Nat::Zero }
    )
{
}
";

fn program(declarations: &str) -> String {
    format!(
        "{}{declarations}",
        super::nat_semiring::library_declarations()
    )
}

#[test]
fn a_law_over_a_mutual_block_is_judged() {
    let verdicts = judged(&program(MUTUAL));
    let judgment = signature(&verdicts);
    let law = judgment
        .laws
        .iter()
        .find(|law| law.machine == "half_two")
        .unwrap_or_else(|| panic!("`half_two` is judged: {judgment:?}"));
    assert!(
        matches!(law.decision, MathematicalCoreDecision::Judged(_)),
        "{law:?}"
    );
    let carriers: Vec<_> = judgment
        .carriers
        .iter()
        .map(|carrier| carrier.type_identity.as_str())
        .collect();
    for member in ["Even", "Odd"] {
        assert!(
            carriers.iter().any(|carrier| carrier.contains(member)),
            "{carriers:?}"
        );
    }
}

#[test]
fn a_false_law_over_a_mutual_block_is_refused() {
    let source =
        program(MUTUAL).replace("Nat::Succ { prev: Nat::Zero }\n    )", "Nat::Zero\n    )");
    assert_ne!(source, program(MUTUAL), "the law is changed");
    let verdicts = judged(&source);
    let law = unproved(&verdicts);
    assert_eq!(law.machine, "half_two", "{law:?}");
}

/// The non-accepted verdicts' refusal details; every one must be a refusal.
fn refusals(verdicts: &[RouteVerdict]) -> Vec<String> {
    verdicts
        .iter()
        .filter(|verdict| !matches!(verdict, RouteVerdict::Accepted(_)))
        .map(|verdict| match verdict {
            RouteVerdict::FallBack(refusal) => refusal.detail.clone(),
            other => panic!("the part must be refused: {other:?}"),
        })
        .collect()
}

/// Two laws proved together by the block's induction: rebuilding a value of
/// either member gives it back, each citing the other at its field.
const REBUILD: &str = "
machine rebuild(e: Even) -> Even
terminates by e
{
    transition e {
        Even::Zero -> Even::Zero,
        Even::Next { odd } -> Even::Next { odd: rebuild_odd(odd) }
    }
}

machine rebuild_odd(o: Odd) -> Odd
terminates by o
{
    transition o {
        Odd::Next { even } -> Odd::Next { even: rebuild(even) }
    }
}

machine rebuild_id(e: Even) -> Nat
terminates by e
ensures
    |proof|::Identity::<Even>(rebuild(e), e)
{
    transition e {
        Even::Zero -> Nat::Zero,
        Even::Next { odd } -> rebuild_odd_id(odd)
    }
}

machine rebuild_odd_id(o: Odd) -> Nat
terminates by o
ensures
    |proof|::Identity::<Odd>(rebuild_odd(o), o)
{
    transition o {
        Odd::Next { even } -> rebuild_id(even)
    }
}
";

#[test]
fn laws_by_induction_over_a_mutual_block_are_judged_together() {
    let verdicts = judged(&program(&format!("{MUTUAL}{REBUILD}")));
    let judgment = signature(&verdicts);
    for machine in ["rebuild_id", "rebuild_odd_id"] {
        let law = judgment
            .laws
            .iter()
            .find(|law| law.machine == machine)
            .unwrap_or_else(|| panic!("`{machine}` is judged: {judgment:?}"));
        assert!(
            matches!(law.decision, MathematicalCoreDecision::Judged(_)),
            "{law:?}"
        );
    }
}

#[test]
fn a_false_law_by_induction_over_a_mutual_block_is_refused() {
    let source = program(&format!("{MUTUAL}{REBUILD}")).replace(
        "|proof|::Identity::<Odd>(rebuild_odd(o), o)",
        "|proof|::Identity::<Odd>(rebuild_odd(o), Odd::Next { even: Even::Zero })",
    );
    // The two are proved by one induction, so neither stands.
    let verdicts = judged(&source);
    let mut refused: Vec<_> = verdicts
        .iter()
        .flat_map(|verdict| match verdict {
            RouteVerdict::UnprovedLaws(laws) => {
                laws.iter().map(|law| law.machine.as_str()).collect()
            }
            RouteVerdict::Laws(judgment) => panic!("the laws must not be accepted: {judgment:?}"),
            _ => Vec::new(),
        })
        .collect();
    refused.sort_unstable();
    assert_eq!(refused, ["rebuild_id", "rebuild_odd_id"], "{verdicts:?}");
}

#[test]
fn a_mutual_law_without_its_companion_hypothesis_is_refused() {
    // Citing nothing at the odd field leaves the step's goal open: the
    // hypothesis comes only from the other law of the group.
    let source = program(&format!("{MUTUAL}{REBUILD}")).replace(
        "Even::Next { odd } -> rebuild_odd_id(odd)",
        "Even::Next { odd } -> Nat::Zero",
    );
    let verdicts = judged(&source);
    let law = unproved(&verdicts);
    assert_eq!(law.machine, "rebuild_id", "{law:?}");
}

/// Case analyses of members of the block: a machine that dispatches without
/// recursion, a law by cases with no other law of its group, and a law whose
/// case goes on by the cases of a field of another member.
const ANALYSES: &str = "
machine starts(e: Even) -> Nat
{
    transition e {
        Even::Zero -> Nat::Zero,
        Even::Next { odd } -> Nat::Succ { prev: Nat::Zero }
    }
}

machine same(e: Even) -> Nat
{
    transition e {
        Even::Zero -> Nat::Zero,
        Even::Next { odd } -> Nat::Succ { prev: Nat::Zero }
    }
}

machine inner(o: Odd) -> Even
{
    transition o {
        Odd::Next { even } -> even
    }
}

machine starts_two()
ensures
    |proof|::Identity::<Nat>(
        starts(Even::Next { odd: Odd::Next { even: Even::Zero } }),
        Nat::Succ { prev: Nat::Zero }
    )
{
}

machine starts_same(e: Even) -> Nat
terminates by e
ensures
    |proof|::Identity::<Nat>(starts(e), same(e))
{
    transition e {
        Even::Zero -> Nat::Zero,
        Even::Next { odd } -> Nat::Zero
    }
}

machine starts_same_inside(o: Odd) -> Nat
terminates by o
ensures
    |proof|::Identity::<Nat>(starts(inner(o)), same(inner(o)))
{
    transition o {
        Odd::Next { even } -> by_even(even)
    }

    machine by_even(even: Even) -> Nat {
        transition even {
            Even::Zero -> Nat::Zero,
            Even::Next { odd } -> Nat::Zero
        }
    }
}
";

const ANALYSIS_LAWS: [&str; 3] = ["starts_two", "starts_same", "starts_same_inside"];

#[test]
fn case_analyses_of_a_member_of_a_mutual_block_are_judged() {
    let verdicts = judged(&program(&format!("{MUTUAL}{ANALYSES}")));
    let judgment = signature(&verdicts);
    for machine in ANALYSIS_LAWS {
        let law = judgment
            .laws
            .iter()
            .find(|law| law.machine == machine)
            .unwrap_or_else(|| panic!("`{machine}` is judged: {judgment:?}"));
        assert!(
            matches!(law.decision, MathematicalCoreDecision::Judged(_)),
            "{law:?}"
        );
    }
}

#[test]
fn false_case_analyses_of_a_member_of_a_mutual_block_are_refused() {
    for (machine, from, to) in [
        (
            "starts_two",
            "starts(Even::Next { odd: Odd::Next { even: Even::Zero } }),\n        Nat::Succ { prev: Nat::Zero }",
            "starts(Even::Next { odd: Odd::Next { even: Even::Zero } }),\n        Nat::Zero",
        ),
        (
            "starts_same",
            "|proof|::Identity::<Nat>(starts(e), same(e))",
            "|proof|::Identity::<Nat>(starts(e), Nat::Zero)",
        ),
        (
            "starts_same_inside",
            "|proof|::Identity::<Nat>(starts(inner(o)), same(inner(o)))",
            "|proof|::Identity::<Nat>(starts(inner(o)), Nat::Zero)",
        ),
    ] {
        let source = program(&format!("{MUTUAL}{ANALYSES}"));
        let changed = source.replacen(from, to, 1);
        assert_ne!(changed, source, "{machine} is changed");
        let verdicts = judged(&changed);
        let law = unproved(&verdicts);
        assert_eq!(law.machine, machine, "{law:?}");
    }
}

#[test]
fn data_containing_itself_through_a_callable_is_refused_at_its_declaration() {
    // `Loop` reaches itself through the parameter of the callable `Test`: a
    // negative occurrence, which no inductive declares. Its twin is
    // `BRANCHING`, which reaches itself through a callable's result.
    let source = program(
        "
data Test = machine(value: Loop) -> bool reaches none crashes none terminates;

data Loop {
    case End;
    case Wrap(test: Test, next: Loop);
}

machine depth(l: Loop) -> Nat
terminates by l
{
    transition l {
        Loop::End -> Nat::Zero,
        Loop::Wrap { test, next } -> Nat::Succ { prev: depth(next) }
    }
}

machine depth_end()
ensures
    |proof|::Identity::<Nat>(depth(Loop::End), Nat::Zero)
{
}
",
    );
    let refused = refusals(&judged(&source));
    let declaration = "while reading the mathematical declaration of `Loop`";
    assert!(
        refused.iter().any(|detail| detail.ends_with(declaration)),
        "{refused:#?}"
    );
    // Every machine over `Loop` repeats the declaration's refusal.
    let reason =
        "data that contains itself as the input of a stored callable: a negative occurrence";
    for machine in ["depth", "depth_end"] {
        let at = format!("while reading the mathematical declaration of `{machine}`");
        assert!(
            refused
                .iter()
                .any(|detail| detail.contains(reason) && detail.ends_with(&at)),
            "{refused:#?}"
        );
    }
}

/// Data with a child for every argument: a field holding a callable whose
/// result is the data, a strictly positive occurrence.
const BRANCHING: &str = "
data Branch = machine(index: u64) -> Tree reaches none crashes none terminates;

data Tree {
    case Leaf;
    case Node(children: Branch);
}

machine leftmost(t: Tree) -> Nat
terminates by t
{
    transition t {
        Tree::Leaf -> Nat::Zero,
        Tree::Node { children } -> Nat::Succ { prev: leftmost(children(0)) }
    }
}

machine leftmost_again(t: Tree) -> Nat
terminates by t
{
    transition t {
        Tree::Leaf -> Nat::Zero,
        Tree::Node { children } -> add(leftmost_again(children(0)), Nat::Succ { prev: Nat::Zero })
    }
}

machine only_leaves(index: u64) -> Tree
{
    Tree::Leaf
}

machine node_of_leaves() -> Tree
{
    Tree::Node { children: only_leaves }
}

machine leftmost_leaf()
ensures
    |proof|::Identity::<Nat>(leftmost(Tree::Leaf), Nat::Zero)
{
}

machine leftmost_node()
ensures
    |proof|::Identity::<Nat>(
        leftmost(node_of_leaves()),
        Nat::Succ { prev: Nat::Zero }
    )
{
}

machine leftmost_agrees(t: Tree) -> Nat
terminates by t
ensures
    |proof|::Identity::<Nat>(leftmost(t), leftmost_again(t))
{
    transition t {
        Tree::Leaf -> Nat::Zero,
        Tree::Node { children } -> agrees_step(children)
    }

    machine agrees_step(children: Branch) -> Nat {
        _ = add_succ_law(leftmost_again(children(0)), Nat::Zero);
        add_zero_right(leftmost_again(children(0)));
        transition { _ -> leftmost_agrees(children(0)) }
    }
}
";

const BRANCHING_LAWS: [&str; 3] = ["leftmost_leaf", "leftmost_node", "leftmost_agrees"];

#[test]
fn laws_over_data_with_a_callable_to_itself_are_judged() {
    let verdicts = judged(&program(BRANCHING));
    let judgment = signature(&verdicts);
    for machine in BRANCHING_LAWS {
        let law = judgment
            .laws
            .iter()
            .find(|law| law.machine == machine)
            .unwrap_or_else(|| panic!("`{machine}` is judged: {judgment:?}"));
        assert!(
            matches!(law.decision, MathematicalCoreDecision::Judged(_)),
            "{law:?}"
        );
    }
}

#[test]
fn false_laws_over_data_with_a_callable_to_itself_are_refused() {
    for (machine, from, to) in [
        (
            "leftmost_node",
            "leftmost(node_of_leaves()),\n        Nat::Succ { prev: Nat::Zero }",
            "leftmost(node_of_leaves()),\n        Nat::Zero",
        ),
        (
            "leftmost_agrees",
            "transition { _ -> leftmost_agrees(children(0)) }",
            "transition { _ -> leftmost_agrees(children(1)) }",
        ),
        (
            "leftmost_agrees",
            "transition { _ -> leftmost_agrees(children(0)) }",
            "transition { _ -> leftmost(children(0)) }",
        ),
    ] {
        let changed = program(BRANCHING).replacen(from, to, 1);
        assert_ne!(changed, program(BRANCHING), "{machine} is changed");
        let verdicts = judged(&changed);
        let law = unproved(&verdicts);
        assert_eq!(law.machine, machine, "{law:?}");
    }
}

/// Mutual machines with another formal, which each recursive call passes a
/// different value, at different positions in the two machines, and laws
/// about them proved together, each citing the other at another value of
/// that formal.
const SHIFTED: &str = "
machine plus_even(e: Even, n: Nat) -> Nat
terminates by e
{
    transition e {
        Even::Zero -> (n),
        Even::Next { odd } -> plus_odd(Nat::Succ { prev: n }, odd)
    }
}

machine plus_odd(n: Nat, o: Odd) -> Nat
terminates by o
{
    transition o {
        Odd::Next { even } -> plus_even(even, Nat::Succ { prev: n })
    }
}

machine plus_two()
ensures
    |proof|::Identity::<Nat>(
        plus_even(Even::Next { odd: Odd::Next { even: Even::Zero } }, Nat::Zero),
        Nat::Succ { prev: Nat::Succ { prev: Nat::Zero } }
    )
{
}

machine plus_even_succ(e: Even, n: Nat) -> Nat
terminates by e
ensures
    |proof|::Identity::<Nat>(
        plus_even(e, Nat::Succ { prev: n }),
        Nat::Succ { prev: plus_even(e, n) }
    )
{
    transition e {
        Even::Zero -> Nat::Zero,
        Even::Next { odd } -> plus_odd_succ(Nat::Succ { prev: n }, odd)
    }
}

machine plus_odd_succ(n: Nat, o: Odd) -> Nat
terminates by o
ensures
    |proof|::Identity::<Nat>(
        plus_odd(Nat::Succ { prev: n }, o),
        Nat::Succ { prev: plus_odd(n, o) }
    )
{
    transition o {
        Odd::Next { even } -> plus_even_succ(even, Nat::Succ { prev: n })
    }
}
";

/// Laws about `plus_even` and `plus_odd` that require how their two `Nat`
/// formals relate, cited at values that meet that premise.
const PREMISED: &str = "
machine plus_even_at(e: Even, n: Nat, m: Nat) -> Nat
terminates by e
requires
    |proof|::Identity::<Nat>(m, Nat::Succ { prev: n })
ensures
    |proof|::Identity::<Nat>(plus_even(e, m), Nat::Succ { prev: plus_even(e, n) })
{
    transition e {
        Even::Zero -> Nat::Zero,
        Even::Next { odd } -> plus_odd_at(Nat::Succ { prev: n }, Nat::Succ { prev: m }, odd)
    }
}

machine plus_odd_at(n: Nat, m: Nat, o: Odd) -> Nat
terminates by o
requires
    |proof|::Identity::<Nat>(m, Nat::Succ { prev: n })
ensures
    |proof|::Identity::<Nat>(plus_odd(m, o), Nat::Succ { prev: plus_odd(n, o) })
{
    transition o {
        Odd::Next { even } -> plus_even_at(even, Nat::Succ { prev: n }, Nat::Succ { prev: m })
    }
}
";

/// Every one of `machines` is judged.
fn all_judged(verdicts: &[RouteVerdict], machines: &[&str]) {
    let judgment = signature(verdicts);
    for machine in machines {
        let law = judgment
            .laws
            .iter()
            .find(|law| law.machine == *machine)
            .unwrap_or_else(|| panic!("`{machine}` is judged: {judgment:?}"));
        assert!(
            matches!(law.decision, MathematicalCoreDecision::Judged(_)),
            "{law:?}"
        );
    }
}

/// The machines of the unproved laws, sorted; none may be accepted.
fn unproved_machines(verdicts: &[RouteVerdict]) -> Vec<&str> {
    let mut refused: Vec<_> = verdicts
        .iter()
        .flat_map(|verdict| match verdict {
            RouteVerdict::UnprovedLaws(laws) => {
                laws.iter().map(|law| law.machine.as_str()).collect()
            }
            RouteVerdict::Laws(judgment) => panic!("the laws must not be accepted: {judgment:?}"),
            _ => Vec::new(),
        })
        .collect();
    refused.sort_unstable();
    refused
}

#[test]
fn mutual_machines_and_laws_with_other_formals_are_judged() {
    let verdicts = judged(&program(&format!("{MUTUAL}{SHIFTED}")));
    all_judged(&verdicts, &["plus_two", "plus_even_succ", "plus_odd_succ"]);
}

#[test]
fn false_mutual_laws_with_other_formals_are_refused() {
    let source = program(&format!("{MUTUAL}{SHIFTED}"));
    // A closed law with the wrong value.
    let wrong = source.replace(
        "Nat::Succ { prev: Nat::Succ { prev: Nat::Zero } }\n    )",
        "Nat::Succ { prev: Nat::Zero }\n    )",
    );
    assert_ne!(wrong, source, "the closed law is changed");
    assert_eq!(unproved_machines(&judged(&wrong)), ["plus_two"]);
    // The companion cited at the formal's own value, not the one the
    // recursion passes: its hypothesis is about another value.
    let unshifted = source.replace(
        "Even::Next { odd } -> plus_odd_succ(Nat::Succ { prev: n }, odd)",
        "Even::Next { odd } -> plus_odd_succ(n, odd)",
    );
    assert_ne!(unshifted, source, "the citation is changed");
    assert_eq!(
        unproved_machines(&judged(&unshifted)),
        ["plus_even_succ", "plus_odd_succ"]
    );
}

#[test]
fn mutual_laws_with_premises_are_judged() {
    let verdicts = judged(&program(&format!("{MUTUAL}{SHIFTED}{PREMISED}")));
    all_judged(&verdicts, &["plus_even_at", "plus_odd_at"]);
}

#[test]
fn a_mutual_law_cited_where_its_premise_fails_is_refused() {
    let source = program(&format!("{MUTUAL}{SHIFTED}{PREMISED}"));
    // At `m` instead of `Succ(m)` the companion's premise would be
    // `m = Succ(Succ(n))`, which `m = Succ(n)` refutes.
    let unmet = source.replace(
        "plus_odd_at(Nat::Succ { prev: n }, Nat::Succ { prev: m }, odd)",
        "plus_odd_at(Nat::Succ { prev: n }, m, odd)",
    );
    assert_ne!(unmet, source, "the citation is changed");
    assert_eq!(
        unproved_machines(&judged(&unmet)),
        ["plus_even_at", "plus_odd_at"]
    );
}

/// Mutual machines with a premise, which each recursive call meets, and
/// laws about them under the same premise.
const DROPPED: &str = "
machine drop_even(e: Even, n: Nat) -> Nat
terminates by e
requires
    |proof|::Identity::<Nat>(n, Nat::Zero)
{
    transition e {
        Even::Zero -> (n),
        Even::Next { odd } -> drop_odd(odd, n)
    }
}

machine drop_odd(o: Odd, n: Nat) -> Nat
terminates by o
requires
    |proof|::Identity::<Nat>(n, Nat::Zero)
{
    transition o {
        Odd::Next { even } -> drop_even(even, n)
    }
}

machine drop_zero(e: Even, n: Nat) -> Nat
terminates by e
requires
    |proof|::Identity::<Nat>(n, Nat::Zero)
ensures
    |proof|::Identity::<Nat>(drop_even(e, n), Nat::Zero)
{
    transition e {
        Even::Zero -> Nat::Zero,
        Even::Next { odd } -> drop_odd_zero(odd, n)
    }
}

machine drop_odd_zero(o: Odd, n: Nat) -> Nat
terminates by o
requires
    |proof|::Identity::<Nat>(n, Nat::Zero)
ensures
    |proof|::Identity::<Nat>(drop_odd(o, n), Nat::Zero)
{
    transition o {
        Odd::Next { even } -> drop_zero(even, n)
    }
}
";

#[test]
fn mutual_machines_with_premises_are_judged() {
    let verdicts = judged(&program(&format!("{MUTUAL}{DROPPED}")));
    all_judged(&verdicts, &["drop_zero", "drop_odd_zero"]);
}

#[test]
fn a_mutual_machine_calling_where_its_premise_fails_is_refused() {
    let source = program(&format!("{MUTUAL}{DROPPED}")).replace(
        "Even::Next { odd } -> drop_odd(odd, n)",
        "Even::Next { odd } -> drop_odd(odd, Nat::Succ { prev: n })",
    );
    // The call owes `Succ(n) = Zero`, which the branch's premise `n = Zero`
    // does not give.
    let verdicts = judged(&source);
    let owed = verdicts
        .iter()
        .flat_map(|verdict| match verdict {
            RouteVerdict::UnprovedLaws(laws) => laws.iter().collect(),
            RouteVerdict::Laws(judgment) => panic!("the laws must not be accepted: {judgment:?}"),
            _ => Vec::new(),
        })
        .find(|law| law.machine == "drop_even")
        .unwrap_or_else(|| panic!("`drop_even` owes its call's premise: {verdicts:?}"));
    assert!(
        owed.statement
            .contains("the premise of `drop_odd` at its call"),
        "{owed:?}"
    );
}

/// The laws of `REBUILD` with no result, each case a machine declared inside
/// the law, read in place, that cites the other law.
const REBUILT: &str = "
machine rebuild(e: Even) -> Even
terminates by e
{
    transition e {
        Even::Zero -> Even::Zero,
        Even::Next { odd } -> Even::Next { odd: rebuild_odd(odd) }
    }
}

machine rebuild_odd(o: Odd) -> Odd
terminates by o
{
    transition o {
        Odd::Next { even } -> Odd::Next { even: rebuild(even) }
    }
}

machine rebuild_id(e: Even)
terminates by e
ensures
    |proof|::Identity::<Even>(rebuild(e), e)
{
    transition e {
        Even::Zero -> zero(),
        Even::Next { odd } -> next(odd)
    }
    machine zero() {
    }
    machine next(odd: Odd) {
        rebuild_odd_id(odd);
    }
}

machine rebuild_odd_id(o: Odd)
terminates by o
ensures
    |proof|::Identity::<Odd>(rebuild_odd(o), o)
{
    transition o {
        Odd::Next { even } -> next(even)
    }
    machine next(even: Even) {
        rebuild_id(even);
    }
}
";

#[test]
fn mutual_laws_citing_each_other_inside_a_case_machine_are_judged() {
    // The group is found through the machines the cases call.
    let verdicts = judged(&program(&format!("{MUTUAL}{REBUILT}")));
    all_judged(&verdicts, &["rebuild_id", "rebuild_odd_id"]);
}

#[test]
fn a_case_machine_citing_a_law_of_the_group_elsewhere_is_refused() {
    let source = program(&format!("{MUTUAL}{REBUILT}")).replace(
        "        rebuild_odd_id(odd);",
        "        rebuild_odd_id(Odd::Next { even: Even::Zero });",
    );
    assert_ne!(
        source,
        program(&format!("{MUTUAL}{REBUILT}")),
        "the case is changed"
    );
    // Only the law at the field is the induction's hypothesis.
    let refused = refusals(&judged(&source));
    assert!(
        refused.iter().any(|detail| detail.contains(
            "law of a mutual induction cited at an argument that is not a field of the dispatched case"
        )),
        "{refused:?}"
    );
}

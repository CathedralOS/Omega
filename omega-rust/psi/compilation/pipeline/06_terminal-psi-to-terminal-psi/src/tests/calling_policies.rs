//! The standard library's calling vocabulary and a hosted target's calling
//! policy, read from their own source files. A calling policy is compile-time
//! code: the compiler runs its `plan` machine over a boundary's signature to
//! get the placement of each argument, so the checker has to judge that
//! machine and everything it calls before any program can be entered.

use super::boundary_calls::typed;
use super::*;

const VOCABULARY: &str = include_str!("../../../../../../../source/library/std/calling.omg");
const SHAPES: &str = include_str!("../../../../../../../source/library/std/entry_shapes.omg");
/// Each hosted target's entry source and the data its policy hangs on.
const TARGETS: [(&str, &str); 4] = [
    (
        "WindowsX86_64",
        include_str!("../../../../../../../source/library/std/targets/windows_x86_64/entry.omg"),
    ),
    (
        "LinuxX86_64",
        include_str!("../../../../../../../source/library/std/targets/linux_x86_64/entry.omg"),
    ),
    (
        "LinuxArm64",
        include_str!("../../../../../../../source/library/std/targets/linux_arm64/entry.omg"),
    ),
    (
        "MacosArm64",
        include_str!("../../../../../../../source/library/std/targets/macos_arm64/entry.omg"),
    ),
];

/// The verdict on `text` rooted at the machines named `roots`, judged as a
/// compile-time invocation is: `accepted`, or the refusal's text.
fn verdict(text: &str, roots: &[&str]) -> String {
    let typed = typed(text);
    let entries: Vec<_> = typed
        .machines()
        .iter()
        .filter(|machine| {
            let path = typed.symbols.display_path(machine.symbol, "::");
            roots.iter().any(|root| path.ends_with(root))
        })
        .map(|machine| machine.symbol)
        .collect();
    assert!(!entries.is_empty());
    let frozen: typed_trees::FrozenTypedTrees = typed.into();
    let verdicts = crate::judge_compilation(
        typed_trees_to_terminal_psi::prepare_compilation(
            &frozen,
            typed_trees_to_terminal_psi::Compilation {
                entries: &entries,
                subjects: &[],
                purpose: source::DependencyScope::Product,
                selected: &[],
                bindings: &[],
                supplies: &[],
            },
            terminal_codec::TerminalPublication::Invocation,
        )
        .program,
    );
    let parts: Vec<String> = verdicts
        .into_iter()
        .map(|verdict| match verdict {
            Err(crate::CompilationRefusal::Elaboration(refusal))
            | Ok(RouteVerdict::FallBack(refusal)) => refusal.detail,
            Ok(RouteVerdict::Accepted(accepted)) => match accepted.publish_invocation() {
                Ok(_) => "accepted".to_owned(),
                Err(error) => format!("accepted and not published: {error:?}"),
            },
            Ok(RouteVerdict::Rejected { unproved, .. }) => unproved
                .iter()
                .map(|entry| format!("{}: {}", entry.statement, entry.detail))
                .collect::<Vec<_>>()
                .join(" | "),
            other => format!("{other:?}"),
        })
        .collect();
    parts.join("; ")
}

/// Each hosted target's policy as the library declares it, less the lines
/// that tie it to the core package's entry trait: from the policy's data to
/// its `...Application` boundary trait. The entry closure is accepted and
/// published as an invocation, which is how the compiler will run it; the
/// second part is the witness that `<Target>::plan` gives
/// `CallingPolicy::plan`, which is judged and is no invocation.
#[test]
fn each_hosted_calling_policy_is_judged() {
    let mut verdicts = Vec::new();
    for (policy, source) in TARGETS {
        let start = source.find(&format!("data {policy} {{}}")).unwrap();
        let end = source[start..]
            .find(
                "
boundary trait ",
            )
            .unwrap()
            + start;
        let text = format!(
            "{VOCABULARY}
{SHAPES}
{}",
            &source[start..end]
        );
        verdicts.push((policy, verdict(&text, &[&format!("{policy}::plan")])));
    }
    for (policy, verdict) in &verdicts {
        let (closure, witness) = verdict.split_once("; ").unwrap_or((verdict, ""));
        assert_eq!(closure, "accepted", "{policy}: {verdicts:#?}");
        assert!(
            witness.starts_with("accepted and not published"),
            "{policy}: {witness}"
        );
    }
}

/// Two table cells read at `first` and `second` under `guard`.
fn guarded_index(guard: &str) -> String {
    verdict(
        &format!(
            r#"
data Table [copy] {{ cells: [u32; 256]; count: u64; }}
machine pick(table: Table, first: u64, second: u64) -> u32 {{
    transition {guard} {{
        true -> sum(table.cells[first], table.cells[second])
        _ -> none()
    }}
    machine sum(left: u32, right: u32) -> u32 {{ left }}
    machine none() -> u32 {{ 0 }}
}}
"#
        ),
        &["pick"],
    )
}

const UNSAFE_INDEX: &str = "cannot prove operation safety in machine `pick`";

/// `a && b && c` is two joins: the first's result is the second's left
/// operand. On the edge where that operand is true the first join's
/// predicate has one alternative left, and the second join states it, so
/// the arm a guard of any width selects knows every conjunct. The policy's
/// `semantic_indices` indexes under four.
#[test]
fn an_arm_knows_every_conjunct_of_its_guard() {
    for guard in [
        "first < 256 && second < 256",
        "first < 256 && second < 256 && first < table.count",
        "first < 256 && second < 256 && first < table.count && second < table.count",
        "first < table.count && second < table.count && first < 256 && second < 256",
    ] {
        assert_eq!(guarded_index(guard), "accepted", "{guard}");
    }
    // The failing twins differ in the guard only: one index is left
    // unbounded, one bound is too wide, one conjunct is only an alternative.
    for guard in [
        "first < 256 && first < table.count",
        "first < 300 && second < 256 && first < table.count",
        "(first < 256 || first < table.count) && second < 256 && first < table.count",
    ] {
        assert!(guarded_index(guard).starts_with(UNSAFE_INDEX), "{guard}");
    }
}

/// A table cell read at a field of `key`, declared as `formal`.
fn keyed_index(formal: &str, body: &str) -> String {
    verdict(
        &format!(
            r#"
data Table [copy] {{ cells: [u32; 256]; count: u64; }}
data Key [copy] {{ slot: u64; other: u64; }}
machine pick(table: &Table, {formal}) -> u32 {{
    {body}
    machine sum(left: u32) -> u32 {{ left }}
    machine none() -> u32 {{ 0 }}
}}
"#
        ),
        &["pick"],
    )
}

/// A guard over fields of a formal crosses its joins: what a formal's
/// storage held at entry is one value for the whole invocation, so a join's
/// predicate may name it as it names a formal. The policy's `fields`
/// indexes by `base.shape` under six conjuncts over fields of references.
#[test]
fn a_guard_over_the_fields_of_a_formal_reaches_its_arm() {
    let arms = |guard: &str, index: &str| {
        format!("transition {guard} {{ true -> sum(table.cells[{index}]) _ -> none() }}")
    };
    for formal in ["key: Key", "key: &Key"] {
        for guard in [
            "key.slot < 256 && key.other < 256",
            "key.slot < 256 && key.other < 256 && key.slot < table.count",
        ] {
            assert_eq!(
                keyed_index(formal, &arms(guard, "key.slot")),
                "accepted",
                "{formal} {guard}"
            );
        }
        // The guard bounds another field than the one that indexes.
        let other = arms("key.other < 256 && key.other < table.count", "key.slot");
        assert!(
            keyed_index(formal, &other).starts_with(UNSAFE_INDEX),
            "{formal}"
        );
    }
    // The entry content is not the field's current content: a store between
    // the guard and the read leaves the read unbounded.
    let stored = |store: &str| {
        keyed_index(
            "mut key: Key",
            &format!(
                "let within: bool = key.slot < 256 && key.other < 256;\n    {store}\n    \
                 transition within {{ true -> sum(table.cells[key.slot]) _ -> none() }}"
            ),
        )
    };
    assert_eq!(stored(""), "accepted");
    assert!(stored("key.slot = 300;").starts_with(UNSAFE_INDEX));
}

const PAIR: &str = "data Pair [copy] requires right <= 3, left <= 2 { left: u64; right: u64; }\n";

/// A shared loan of data that states `requires` is read by its callee: a
/// shared loan admits no store, so the callee cannot leave the value outside
/// its domain. The policy's helpers take `&BoundarySignature`. An exclusive
/// loan stays refused, because the domain is owed again where it returns.
#[test]
fn data_with_requires_is_lent_shared_and_not_exclusively() {
    let root =
        |access: &str| format!("{PAIR}machine root(pair: {access}Pair) -> u64 {{ pair.left }}");
    assert_eq!(verdict(&root("&"), &["root"]), "accepted");
    assert_eq!(
        verdict(&root("&mut "), &["root"]),
        "parameter reference to data with requires"
    );
}

const PLACE: &str = r#"
data Place [copy]
    requires (self in Place::One) || ((self in Place::Two) && (self.count >= 1) && (self.count <= 16))
{
    case One(a: u64);
    case Two(b: u64, count: u64);
}
"#;

/// A data `requires` is published with its type, so its conjunctions and
/// disjunctions take Terminal Psi's canonical order, wherever the case test
/// that guards a payload's conjuncts then stands. `NativePlace` in the
/// calling vocabulary states this clause.
#[test]
fn a_data_clause_that_tests_a_case_is_published() {
    let root =
        |place: &str| format!("{PLACE}machine root() -> u64 {{ let place: Place = {place}; 1 }}");
    assert_eq!(verdict(&root("Place::One { a: 1 }"), &["root"]), "accepted");
    assert_eq!(
        verdict(&root("Place::Two { b: 1, count: 16 }"), &["root"]),
        "accepted"
    );
    let outside = verdict(&root("Place::Two { b: 1, count: 17 }"), &["root"]);
    assert!(outside.starts_with("cannot prove"), "{outside}");
}

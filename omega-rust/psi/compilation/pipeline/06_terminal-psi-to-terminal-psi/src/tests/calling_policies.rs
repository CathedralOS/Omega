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

/// Judge the actual source roots as the normal compile-time invocation does.
/// Keep the full verdict so rejection controls can pin its real obligation.
fn prepared(
    typed: typed_trees::TypedTrees,
    roots: &[&str],
    publication: terminal_codec::TerminalPublication,
) -> terminal_check_tables::compilation::TerminalCompilation {
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
    typed_trees_to_terminal_psi::prepare_compilation(
        &frozen,
        typed_trees_to_terminal_psi::Compilation {
            declaration_metadata: &[],
            entries: &entries,
            subjects: &[],
            purpose: source::DependencyScope::Product,
            selected: &[],
            bindings: &[],
            supplies: &[],
        },
        publication,
    )
    .program
}

fn judged(text: &str, roots: &[&str]) -> Vec<Result<RouteVerdict, crate::CompilationRefusal>> {
    crate::judge_compilation(prepared(
        typed(text),
        roots,
        terminal_codec::TerminalPublication::Invocation,
    ))
}

fn verdict(text: &str, roots: &[&str]) -> String {
    let parts: Vec<String> = judged(text, roots)
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

/// A valid incoming value keeps its default-domain obligations when lent.
/// Shared reads and exclusive writes use the same storage predicates; a
/// temporary invalid state must be restored before normal loan handback.
#[test]
fn data_with_requires_is_lent_and_restored_at_loan_handback() {
    let root = |access: &str, body: &str| {
        format!("{PAIR}machine root(pair: {access}Pair) -> u64 {{ {body} }}")
    };
    for access in ["&", "&mut "] {
        assert_eq!(verdict(&root(access, "pair.left"), &["root"]), "accepted");
    }
    assert_eq!(
        verdict(
            &root("&mut ", "pair.left = 3; pair.left = 2; pair.left"),
            &["root"]
        ),
        "accepted",
        "the exact field's invariant window closes after a restoring write"
    );
    assert_eq!(
        verdict(&root("&write ", "0u64"), &["root"]),
        "parameter write-only reference parameter",
        "a readable established input is not an uninitialized write-only loan"
    );
    let crash = format!(
        "{PAIR}machine root(pair: &mut Pair) -> u64 crashes Trap {{ pair.left = 3; crash Trap; }}"
    );
    assert_eq!(
        verdict(&crash, &["root"]),
        "accepted",
        "the existing crash contract may abandon a window without promising survivor safety"
    );
    let gated = judged(
        "data Positive [copy] requires value > 0 { value: u64; }\n\
         machine consume(value: &mut Positive) -> u64 { 1u64 }\n\
         machine root() -> u64 { let mut value: Positive; consume(&mut value) }",
        &["root"],
    );
    let [Ok(RouteVerdict::Rejected { unestablished, .. })] = gated.as_slice() else {
        panic!("a zeroed gated value cannot be supplied as an initialized loan: {gated:?}")
    };
    assert!(!unestablished.is_empty(), "{gated:?}");
    let nested = |body: &str| {
        format!(
            "{PAIR}data Wrapper {{ pair: Pair; sibling: u64; }}\n\
         machine root(value: &mut Wrapper) -> u64 {{ {body} }}"
        )
    };
    assert_eq!(
        verdict(&nested("value.sibling = 9; value.pair.left"), &["root"]),
        "accepted"
    );
    for text in [
        root("&mut ", "pair.left = 3; 0u64"),
        nested("value.pair.left = 3; value.sibling"),
    ] {
        let verdicts = judged(&text, &["root"]);
        let [
            Ok(RouteVerdict::Rejected {
                formations,
                violations,
                unproved,
                unestablished,
                conflicts,
                exceeded,
            }),
        ] = verdicts.as_slice()
        else {
            panic!("only the actual default-domain handback must reject: {verdicts:?}")
        };
        assert!(
            formations.is_empty()
                && violations.is_empty()
                && unestablished.is_empty()
                && conflicts.is_empty()
                && exceeded.is_empty(),
            "{verdicts:?}"
        );
        let [failure] = unproved.as_slice() else {
            panic!("one changed clause: {unproved:?}")
        };
        assert!(matches!(
            failure.kind,
            terminal_check_tables::obligations::ObligationKind::DataDomain { .. }
        ));
        assert_eq!(failure.cause, UnprovedCause::NoDerivation);
        assert!(
            failure.consumed.is_some(),
            "an actual open window at loan handback"
        );
    }
}

// A public helper's written fields are operational contract information.
// The caller keeps the other field's entry identity without an authored
// guarantee for that untouched field or a private result-summary premise.
#[test]
fn a_call_preserves_only_storage_outside_its_published_write_frame() {
    let source = r#"
data Pair [copy] { left: u64; right: u64; }
public machine update(pair: &mut Pair, value: u64) reaches none crashes none terminates {
    pair.left = value;
}
machine root(mut pair: Pair) -> u64
requires pair.right == 9
ensures result == 9
{
    update(&mut pair, 7);
    pair.right
}
"#;
    // A portable artifact needs actual package-owned source. Use the same
    // retained test package loader as the existing provider controls; do not
    // give an anonymous invocation a portable identity after preparation.
    let input = prepared(
        super::build_purpose::frontier_of(source, "machine build_entry() { }"),
        &["root"],
        terminal_codec::TerminalPublication::Portable,
    );
    let terminal_check_tables::compilation::TerminalCompilation::Prepared(compilation) = &input
    else {
        panic!("the ordinary source preparation must retain its complete compilation")
    };
    assert!(compilation.forms.is_empty() && compilation.witnesses.is_empty());
    let program = compilation.closure.as_ref().unwrap().clone();
    let root = &program.module.machines[0];
    let calls: Vec<_> = root
        .operations()
        .filter_map(|operation| terminal_psi::direct_call_parts(&operation.kind))
        .collect();
    let [call] = calls.as_slice() else {
        panic!("the source contains one actual update call")
    };
    let callee = program.module.machine(call.callee).unwrap();
    let stores: Vec<_> = callee
        .operations()
        .filter_map(|operation| match operation.kind {
            terminal_psi::OperationKind::StructuralScalarFieldStore { field, .. } => Some(field),
            _ => None,
        })
        .collect();
    let [left] = stores.as_slice() else {
        panic!("the helper writes one actual field")
    };
    let declaration = program
        .module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == callee.structural_parameters[0].structural_type)
        .unwrap();
    let fields = declaration.shape.record_fields().unwrap();
    assert_eq!(
        program.module.identities.text(
            terminal_psi::live_field_by_id(fields, *left)
                .unwrap()
                .identity
        ),
        Some("left")
    );
    assert_eq!(
        callee.contract.published_writes,
        Some(vec![(0, vec![*left])])
    );

    let verdicts = crate::judge_compilation(input);
    let [Ok(RouteVerdict::Accepted(accepted))] = verdicts.as_slice() else {
        panic!("the untouched right field retains its actual entry value: {verdicts:?}")
    };
    accepted.publish().unwrap();
    let encoded = terminal_codec::encode_module(&accepted.elaboration.module).unwrap();
    let received = terminal_codec::decode_module(&encoded).unwrap();
    assert_eq!(
        received
            .machine(call.callee)
            .unwrap()
            .contract
            .published_writes,
        Some(vec![(0, vec![*left])])
    );

    let mut omitted = program.clone();
    omitted
        .module
        .machines
        .iter_mut()
        .find(|machine| machine.id == call.callee)
        .unwrap()
        .contract
        .published_writes = Some(Vec::new());
    let refused = judge(Ok(omitted)).unwrap_err();
    assert_eq!(refused.reason, RefusalReason::ForeignIdentity);
    assert!(
        refused.detail.contains("published write frame"),
        "{refused:?}"
    );
    let mut forged = received.clone();
    forged
        .machines
        .iter_mut()
        .find(|machine| machine.id == call.callee)
        .unwrap()
        .contract
        .published_writes = Some(Vec::new());
    assert!(
        matches!(terminal_codec::decode_module(&terminal_codec::encode_module(&forged).unwrap()),
        Err(terminal_codec::CodecError::MalformedWriteFrame(detail)) if detail.contains("differs from its body")),
        "receiving reconstructs actual writes instead of trusting an empty supplied frame"
    );
    let mut unknown = received.clone();
    unknown
        .machines
        .iter_mut()
        .find(|machine| machine.id == call.callee)
        .unwrap()
        .contract
        .published_writes = None;
    let unknown_bytes = terminal_codec::encode_module(&unknown).unwrap();
    let unknown = terminal_codec::decode_module(&unknown_bytes).unwrap();
    assert_eq!(
        unknown
            .machine(call.callee)
            .unwrap()
            .contract
            .published_writes,
        None,
        "an unknown ceiling does not become a complete empty frame"
    );
    let mut old = encoded.clone();
    old[8..10].copy_from_slice(&260u16.to_le_bytes());
    assert!(matches!(
        terminal_codec::decode_module(&old),
        Err(terminal_codec::CodecError::UnsupportedFormatMarker(260))
    ));

    let changed = source.replace("pair.left = value;", "pair.right = value;");
    let rejected = judged(&changed, &["root"]);
    let [
        Ok(RouteVerdict::Rejected {
            unproved,
            formations,
            violations,
            unestablished,
            conflicts,
            exceeded,
        }),
    ] = rejected.as_slice()
    else {
        panic!("an overlapping write cannot preserve the original right value: {rejected:?}")
    };
    assert!(
        formations.is_empty()
            && violations.is_empty()
            && unestablished.is_empty()
            && conflicts.is_empty()
            && exceeded.is_empty(),
        "{rejected:?}"
    );
    assert_eq!(unproved.len(), 1, "{rejected:?}");
    assert!(
        matches!(unproved[0].kind, ObligationKind::NormalGuarantee { .. })
            && unproved[0].cause == UnprovedCause::NoDerivation,
        "{rejected:?}"
    );

    let restored = source.replace(
        "pair.left = value;",
        "let saved: u64 = pair.right; pair.right = value; pair.right = saved;",
    );
    let restored = prepared(
        super::build_purpose::frontier_of(&restored, "machine build_entry() { }"),
        &["update"],
        terminal_codec::TerminalPublication::Portable,
    );
    let terminal_check_tables::compilation::TerminalCompilation::Prepared(restored) = restored
    else {
        panic!("the restoring helper must retain its complete compilation")
    };
    assert!(restored.forms.is_empty() && restored.witnesses.is_empty());
    let restored = restored.closure.unwrap();
    let restoring = &restored.module.machines[0];
    let writes: Vec<_> = restoring
        .operations()
        .filter_map(|operation| match operation.kind {
            terminal_psi::OperationKind::StructuralScalarFieldStore { field, .. } => Some(field),
            _ => None,
        })
        .collect();
    assert_eq!(writes.len(), 2);
    assert_eq!(writes[0], writes[1]);
    assert_eq!(
        restoring.contract.published_writes,
        Some(vec![(0, vec![writes[0]])]),
        "restoring the original value does not erase the two actual write events"
    );
}

// An indexed replacement preserves an intact array's availability even
// after a disjoint sibling moved. It neither reconstructs an unknown hole
// nor supplies the separate bound needed by the actual runtime selector.
#[test]
fn runtime_record_replacement_needs_an_intact_array_and_its_actual_bound() {
    let source = |prefix: &str, bound: u64| {
        format!(
            "data Item {{ value: u64; }}\n\
         data Table {{ items: [Item; 2]; side: Item; }}\n\
         machine root(mut table: Table, index: u64) -> u64 requires index < {bound} {{\n\
             {prefix}\n\
             table.items[index] = Item {{ value: 7 }};\n\
             table.items[index].value\n\
         }}"
        )
    };
    for prefix in ["", "let detached: Item = table.side;"] {
        let verdicts = judged(&source(prefix, 2), &["root"]);
        let [Ok(RouteVerdict::Accepted(accepted))] = verdicts.as_slice() else {
            panic!("an intact array stays available: {prefix}: {verdicts:?}")
        };
        accepted.publish_invocation().unwrap();
        let module = &accepted.elaboration.module;
        let mut forged = module.machines[0].clone();
        terminal_semantics::plain_affine::verify(&forged, &module.structural_types).unwrap();
        let store = forged.blocks.iter_mut().flat_map(|block| &mut block.operations).find(|operation| {
            matches!(&operation.kind, terminal_psi::OperationKind::StoreStructuralField { path, .. }
                if path.iter().any(|segment| matches!(segment, terminal_psi::StructuralPathSegment::RuntimeIndex { .. })))
        }).expect("the actual source emits a runtime-indexed record store");
        let stored_at = store.id;
        let terminal_psi::OperationKind::StoreStructuralField { disposition, .. } = &mut store.kind
        else {
            unreachable!()
        };
        assert_eq!(
            *disposition,
            terminal_psi::StructuralStoreDisposition::ReplaceDiscardable
        );
        *disposition = terminal_psi::StructuralStoreDisposition::Vacant;
        assert!(
            matches!(
                terminal_semantics::plain_affine::verify(&forged, &module.structural_types),
                Err(terminal_semantics::plain_affine::Error::Malformed {
                    detail: "a vacant store displaces established affine contents",
                    site: Some(terminal_semantics::plain_affine::Site::Operation(at)),
                }) if at == stored_at
            ),
            "source-free receiving reconstructs the disposition instead of trusting the producer tag"
        );
    }
    let missing = judged(
        &source("let detached: Item = table.items[0];", 2),
        &["root"],
    );
    let [Ok(RouteVerdict::FallBack(refusal))] = missing.as_slice() else {
        panic!("the inexact store cannot reconstruct a moved element: {missing:?}")
    };
    assert_eq!(refusal.reason, RefusalReason::Unsupported);
    assert!(
        matches!(refusal.concerns, Construct::Operation(_)),
        "{refusal:?}"
    );
    assert!(
        refusal
            .detail
            .ends_with("runtime-indexed replacement in an incomplete affine array"),
        "{refusal:?}"
    );
    let unsafe_index = judged(&source("", 3), &["root"]);
    let [
        Ok(RouteVerdict::Rejected {
            formations,
            violations,
            unproved,
            unestablished,
            conflicts,
            exceeded,
        }),
    ] = unsafe_index.as_slice()
    else {
        panic!("store and read still owe the actual array bound: {unsafe_index:?}")
    };
    assert!(
        formations.is_empty()
            && violations.is_empty()
            && unestablished.is_empty()
            && conflicts.is_empty()
            && exceeded.is_empty(),
        "{unsafe_index:?}"
    );
    assert_eq!(unproved.len(), 2, "the actual store and read bounds");
    assert!(
        unproved.iter().all(|failure| matches!(
            failure.kind,
            terminal_check_tables::obligations::ObligationKind::OperationSafety { .. }
        ) && failure.cause == UnprovedCause::NoDerivation),
        "{unproved:?}"
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

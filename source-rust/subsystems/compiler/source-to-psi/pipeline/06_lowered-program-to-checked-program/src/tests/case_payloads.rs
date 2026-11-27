//! Loans of a case payload. Read from source, as a Build program: the
//! toolchain's `Build::request_dependency` dispatches on a `Source` and hands
//! the selected case's views to the request it makes.
//!
//! A payload field is a place like any other projected place, one that
//! exists only while its case is selected. patterns.md, "A sum held in a
//! field": "The case fact belongs to the place", and "Payload mutation and
//! case replacement retain ordinary subject invalidation and loan rules."
//! So a loan of a payload is a loan of that exact place, formed where the
//! sum is known to hold the case, and it conflicts with whatever overlaps
//! it, as a loan of a field does.

use super::build_purpose::judge_build;
use super::*;

/// `accepted` for a program the checker accepts and can publish. For one it
/// judges wrong, the rule it breaks, what it could not establish or the loan
/// that conflicts. Else the refusal's text.
fn verdict(build: &str) -> String {
    match judge_build(build) {
        Err(crate::CompilationRefusal::Elaboration(refusal))
        | Ok(RouteVerdict::FallBack(refusal)) => refusal.detail,
        Ok(RouteVerdict::Accepted(accepted)) => {
            accepted.publish_invocation().unwrap();
            "accepted".to_owned()
        }
        Ok(RouteVerdict::Rejected {
            violations,
            unestablished,
            conflicts,
            ..
        }) => match (&violations[..], &unestablished[..], &conflicts[..]) {
            ([], [one], []) => one.detail.to_owned(),
            ([], [], [one]) => one.statement.clone(),
            ([one], [], []) => one.statement.clone(),
            _ => panic!("{violations:?} {unestablished:?} {conflicts:?}"),
        },
        other => panic!("{other:?}"),
    }
}

const NO_CASE: &str = "cannot prove selected case for payload access: a path to this access does not establish the case";

const VIEWS: &str = r#"
data Source<'storage> {
    case Path(location: @'storage [u8]);
    case Git(repository: @'storage [u8], revision: u32);
}
machine take(view: @[u8]) { }
machine measure(view: @[u8]) -> u32 { 1 }
"#;

const SCALARS: &str = r#"
data Source [copy] {
    case Path(location: u32);
    case Git(repository: u32, revision: u32);
}
machine look(value: @u32) -> u32 { 1 }
machine bump(value: @mutable u32) -> u32 { 1 }
machine two(value: @mutable u32, other: @u32) -> u32 { 1 }
machine both(held: @mutable Source, value: @u32) -> u32 { 1 }
machine replace(held: @mutable Source) { }
"#;

#[test]
fn an_indexed_case_payload_proves_a_recursive_arrival() {
    const SOURCE: &str = r#"
domain i64::FileHandle requires self != -1;
data Frame [copy] {
    case Empty;
    case Directory(handle: i64 in FileHandle);
}
machine entry(frames: @mutable [Frame; 2], depth: u64, restart: u8) -> i64
requires depth > 0 && depth <= 2 && restart <= 1 {
    transition { _ -> block next(frames, depth, restart) }
    machine next(frames: @mutable [Frame; 2], depth: u64, restart: u8) -> i64
    requires depth > 0 && depth <= 2 && restart <= 1 {
        transition frames[depth - 1] {
            Frame::Directory { handle } -> block directory(frames, depth, restart, handle),
            Frame::Empty -> 0i64
        }
    }
    machine directory(frames: @mutable [Frame; 2], depth: u64, restart: u8,
        handle: i64 in FileHandle) -> i64
    requires depth > 0 && depth <= 2 && restart <= 1 {
        transition restart == 1 {
            true -> block next(frames, depth, 0u8),
            false -> (handle as -FileHandle)
        }
    }
}
"#;

    let negative = SOURCE.replacen(
        "    requires depth > 0 && depth <= 2 && restart <= 1 {\n        transition restart",
        "    requires depth > 0 && depth < 2 && restart <= 1 {\n        transition restart",
        1,
    );
    assert_ne!(negative, SOURCE);

    for (source, accepts) in [(negative.as_str(), false), (SOURCE, true)] {
        let typed = boundary_calls::typed(source);
        let entry = boundary_calls::entry(&typed);
        let elaboration = crate::lowering::elaborate_for(
            &typed,
            super::build_purpose::roster(
                &[entry],
                &[],
                source_provenance::DependencyScope::Product,
            ),
            terminal_codec::TerminalPublication::Invocation,
        )
        .expect("the indexed frame cycle lowers");

        // Each lowered copy of the nested cycle may contain this authored
        // call. Bind its source position to the destination's actual arrival.
        let callee_start = source.find("block directory(").unwrap() + "block ".len();
        let mut arrivals = Vec::new();
        for (machine, sources) in elaboration.sources.iter() {
            let tables = elaboration.tables.machines.get(machine).unwrap();
            for (edge, origin) in sources.edges.iter() {
                if origin
                    .span()
                    .is_none_or(|span| span.span.start != callee_start)
                {
                    continue;
                }
                let edge_row = tables.edges.get(edge).unwrap();
                let terminal_check_tables::edges::Destination::Block(head) = edge_row.destination
                else {
                    panic!("the authored call must transfer to a block");
                };
                let (_, member) = sources
                    .heads
                    .iter()
                    .find(|(block, _)| *block == head)
                    .unwrap();
                assert_eq!(elaboration.names.get(*member).unwrap().machine, "directory");
                let lowered = elaboration.module.machine(machine).unwrap();
                let from = lowered
                    .blocks
                    .iter()
                    .find(|block| block.id == edge_row.source)
                    .unwrap();
                assert!(matches!(&from.terminator,
                    terminal_psi::Terminator::Jump { edge: jump, target, arguments, structural_arguments, .. }
                    if *jump == edge && *target == head
                        && arguments.len() == 3 && structural_arguments.len() == 1));
                let (obligation, row) = tables
                    .obligations
                    .iter()
                    .find(|(_, row)| row.kind == (ObligationKind::ArrivalInvariant { edge }))
                    .expect("the terminal directory edge owes an arrival invariant");
                assert_eq!(row.created_at, ProgramPoint::Edge(edge));
                assert_eq!(sources.obligations.get(obligation), Some(origin));
                assert!(
                    elaboration
                        .module
                        .proofs.scalar_block_invariants
                        .iter()
                        .any(|invariant| invariant.machine == machine
                            && invariant.header == head
                            && invariant
                                .arrivals
                                .iter()
                                .any(|arrival| arrival.edge == edge
                                    && arrival.obligation == obligation))
                );
                arrivals.push((machine, obligation, edge, *origin));
            }
        }
        assert!(
            !arrivals.is_empty(),
            "lowering must retain the directory arrival"
        );

        let verdict = judge(Ok(elaboration)).unwrap();
        let directory_refused = matches!(&verdict, RouteVerdict::Rejected {
            formations, violations, unproved, unestablished, conflicts, exceeded
        } if formations.is_empty() && violations.is_empty() && unestablished.is_empty()
            && conflicts.is_empty() && exceeded.is_empty() && !unproved.is_empty()
            && unproved.iter().all(|row| {
                row.cause == UnprovedCause::NoDerivation
                    && row.statement.contains("call directory from next")
                    && arrivals.iter().any(|&(machine, obligation, edge, origin)| {
                        row.machine == machine && row.source == crate::ObligationSource::Lowered(obligation)
                            && row.kind == (ObligationKind::ArrivalInvariant { edge })
                            && row.site == origin
                    })
            }));
        if accepts {
            match verdict {
                RouteVerdict::Accepted(accepted) => {
                    accepted.publish_invocation().unwrap();
                }
                refused if directory_refused => {
                    panic!(
                        "the indexed case payload did not prove its recursive arrival: {refused:?}"
                    );
                }
                other => {
                    panic!("unexpected outcome for the unchanged recursive arrival: {other:?}")
                }
            }
        } else {
            assert!(
                directory_refused,
                "tightening directory's depth bound must refuse its authored recursive arrival: {verdict:?}"
            );
        }
    }
}

/// A vacated slot from an earlier iteration cannot supply the selected
/// payload of the frame copied before this iteration's slot replacement.
#[test]
fn a_copied_case_payload_keeps_its_domain_across_recursive_vacation() {
    const SOURCE: &str = r#"
domain i64::FileHandle requires self != -1;
data Frame [copy] {
    case Empty;
    case Directory(handle: i64 in FileHandle);
}
machine entry(frames: @mutable [Frame; 2], count: u64, original: u32) -> u32
requires count <= 2 {
    transition { _ -> block next(frames, count, original) }
    machine next(frames: @mutable [Frame; 2], count: u64, original: u32) -> u32
    requires count <= 2 {
        transition count > 0 {
            true -> block pop(frames, count, original),
            false -> (original)
        }
    }
    machine pop(frames: @mutable [Frame; 2], count: u64, original: u32) -> u32
    requires count > 0 && count <= 2 {
        let frame: Frame = frames[count - 1];
        frames[count - 1] = Frame::Empty;
        transition frame {
            Frame::Empty -> block next(frames, count - 1, original),
            Frame::Directory { handle } -> block close(frames, count - 1, original, handle)
        }
    }
    machine close(frames: @mutable [Frame; 2], count: u64, original: u32,
        handle: i64 in FileHandle) -> u32
    requires count < 2 {
        transition (handle as -FileHandle) != 0 {
            true -> block release(frames, count, original, handle),
            false -> block next(frames, count, original)
        }
    }
    machine release(frames: @mutable [Frame; 2], count: u64, original: u32,
        handle: i64 in FileHandle) -> u32
    requires count < 2 && (handle as -FileHandle) != 0 {
        transition { _ -> block next(frames, count, original) }
    }
}
"#;
    let negative = SOURCE.replacen(
        "    requires count < 2 {\n",
        "    requires count < 1 {\n",
        1,
    );
    assert_ne!(negative, SOURCE);
    for (source, accepts) in [(negative.as_str(), false), (SOURCE, true)] {
        let typed = boundary_calls::typed(source);
        let entry = boundary_calls::entry(&typed);
        let elaboration = crate::lowering::elaborate_for(
            &typed,
            super::build_purpose::roster(
                &[entry],
                &[],
                source_provenance::DependencyScope::Product,
            ),
            terminal_codec::TerminalPublication::Invocation,
        )
        .expect("the copied frame cycle lowers");
        let callee_start = source.find("block close(").unwrap() + "block ".len();
        let mut arrivals = Vec::new();
        for (machine, sources) in elaboration.sources.iter() {
            let tables = elaboration.tables.machines.get(machine).unwrap();
            for (edge, origin) in sources.edges.iter() {
                if origin
                    .span()
                    .is_none_or(|span| span.span.start != callee_start)
                {
                    continue;
                }
                let (obligation, row) = tables
                    .obligations
                    .iter()
                    .find(|(_, row)| row.kind == (ObligationKind::ArrivalInvariant { edge }))
                    .expect("the actual close edge owes its arrival invariant");
                assert_eq!(row.created_at, ProgramPoint::Edge(edge));
                assert_eq!(sources.obligations.get(obligation), Some(origin));
                arrivals.push((machine, obligation, edge, *origin));
            }
        }
        assert!(
            !arrivals.is_empty(),
            "the authored close arrival must survive lowering"
        );
        let verdict = judge(Ok(elaboration)).unwrap();
        if accepts {
            match verdict {
                RouteVerdict::Accepted(accepted) => {
                    accepted.publish_invocation().unwrap();
                }
                other => panic!("the selected copied payload must retain its domain: {other:?}"),
            }
        } else {
            assert!(
                matches!(&verdict, RouteVerdict::Rejected {
                    formations, violations, unproved, unestablished, conflicts, exceeded
                } if formations.is_empty() && violations.is_empty() && unestablished.is_empty()
                    && conflicts.is_empty() && exceeded.is_empty() && !unproved.is_empty()
                    && unproved.iter().all(|row| {
                        row.cause == UnprovedCause::NoDerivation
                            && row.statement.contains("call close from pop")
                            && arrivals.iter().any(|&(machine, obligation, edge, origin)| {
                                row.machine == machine && row.source == crate::ObligationSource::Lowered(obligation)
                                    && row.kind == (ObligationKind::ArrivalInvariant { edge })
                                    && row.site == origin
                            })
                    })),
                "the stricter count must refuse its exact authored close arrival: {verdict:?}"
            );
        }
    }
}

/// Build's Source dispatch forwards a shared builder and the selected payload.
/// Its Git case contains another sum, passed to the helper that selects it.
#[test]
fn a_dispatch_arm_forwards_its_shared_borrow_and_nested_payload() {
    assert_eq!(
        verdict(
            r#"
data Builder { count: u32; }
data Selection<'storage> {
    case Root;
    case Named(package: @'storage [u8]);
}
data Source<'storage> {
    case Path(location: @'storage [u8]);
    case Git(repository: @'storage [u8], revision: @'storage [u8], selection: Selection<'storage>);
}
machine build_entry(builder: @Builder, source: Source) { route(builder, source); }
machine route(builder: @Builder, source: Source) {
    transition source {
        Source::Path { location } -> request(builder, location, "", ""),
        Source::Git { repository, revision, selection } -> git(builder, repository, revision, selection)
    }
}
machine git(builder: @Builder, repository: @[u8], revision: @[u8], selection: Selection) {
    transition selection {
        Selection::Root -> request(builder, repository, revision, ""),
        Selection::Named { package } -> request(builder, repository, revision, package)
    }
}
machine request(builder: @Builder, location: @[u8], revision: @[u8], package: @[u8]) {
    let observed: u32 = builder.count;
}
"#
        ),
        "accepted"
    );
}

/// The toolchain's shape: each arm of a dispatch over an owned sum passes
/// a view its case holds. The binding is the payload place of the subject
/// the dispatch saved, and the arm is where that case is selected. A
/// binding of another case is no name in the arm.
#[test]
fn a_dispatch_arm_passes_the_view_its_case_holds() {
    let arms = |git: &str| {
        format!(
            r#"{VIEWS}
machine build_entry(source: Source) {{ route(source); }}
machine route(source: Source) {{
    transition source {{
        Source::Path {{ location }} -> take(location),
        Source::Git {{ repository, revision }} -> take({git})
    }}
}}
"#
        )
    };
    assert_eq!(verdict(&arms("repository")), "accepted");
    assert_eq!(
        verdict(&arms("location")),
        "this expression has no visible value declaration in its source scope: an unknown name is \
         an error wherever it appears while reading the signature of `route`"
    );
}

/// The same from a value-position `match`, and for a scalar payload lent
/// shared.
#[test]
fn a_match_arm_lends_the_payload_its_case_holds() {
    assert_eq!(
        verdict(&format!(
            r#"{VIEWS}
machine build_entry(source: Source) {{ let n: u32 = route(source); }}
machine route(source: Source) -> u32 {{
    match source {{
        Source::Path {{ location }} -> measure(location),
        Source::Git {{ repository, revision }} -> measure(repository)
    }}
}}
"#
        )),
        "accepted"
    );
    let scalar = |lent: &str| {
        format!(
            r#"{SCALARS}
machine build_entry(source: Source) {{ let n: u32 = route(source); }}
machine route(source: Source) -> u32 {{
    let mutable held: Source = source;
    match held {{
        Source::Path {{ location }} -> {lent},
        Source::Git {{ repository, revision }} -> 2
    }}
}}
"#
        )
    };
    assert_eq!(verdict(&scalar("look(@location)")), "accepted");
    // The payload lies inside `held`, so lending both is lending one
    // place twice.
    assert_eq!(
        verdict(&scalar("both(@mutable held, @location)")),
        "state `both` receives `@<saved subject #1:1>.location` as both mutable and read-only"
    );
}

/// A transition holds its subject in a binding of its own, which nothing
/// may write, so an arm's payload is not lent exclusively.
#[test]
fn a_transition_arm_does_not_lend_its_payload_exclusively() {
    assert_eq!(
        verdict(&format!(
            r#"{SCALARS}
machine build_entry(source: Source) {{ route(source); }}
machine route(source: Source) {{
    let mutable held: Source = source;
    transition held {{
        Source::Path {{ location }} -> replace_scalar(@mutable location),
        Source::Git {{ repository, revision }} -> replace_scalar(@mutable repository)
    }}
}}
machine replace_scalar(value: @mutable u32) {{ }}
"#
        )),
        "call exclusive loan of an immutable binding while reading the body of `route`"
    );
}

/// A payload named directly is lent where the sum is known to hold its
/// case, here from the construction that is its only reaching write, and
/// nowhere else: not a payload of another case, and not after a write that
/// may have changed the case.
#[test]
fn a_payload_is_lent_only_where_its_case_is_selected() {
    let view = |lent: &str| {
        format!(
            r#"{VIEWS}
machine build_entry() {{ let n: u32 = route(); }}
machine route() -> u32 {{
    let held: Source = Source::Path {{ location: "here" }};
    measure(held.{lent})
}}
"#
        )
    };
    assert_eq!(verdict(&view("location")), "accepted");
    assert_eq!(verdict(&view("repository")), NO_CASE);

    let scalar = |before: &str| {
        format!(
            r#"{SCALARS}
machine build_entry() {{ let n: u32 = route(); }}
machine route() -> u32 {{
    let mutable held: Source = Source::Path {{ location: 3 }};
    {before}
    bump(@mutable held.location)
}}
"#
        )
    };
    assert_eq!(verdict(&scalar("")), "accepted");
    assert_eq!(
        verdict(&scalar(
            "held = Source::Git { repository: 1, revision: 2 };"
        )),
        NO_CASE
    );
    // An exclusive loan of the sum lets its holder replace the case, and
    // what a helper may have written is the write frame it publishes
    // (dependent_values.md, "Mutation frames"). This helper writes nothing,
    // so the case it was lent still stands after the call.
    assert_eq!(verdict(&scalar("replace(@mutable held);")), "accepted");
}

/// A sum never known to hold the case lends no payload of it.
#[test]
fn a_sum_of_unknown_case_lends_no_payload() {
    assert_eq!(
        verdict(&format!(
            r#"{VIEWS}
machine build_entry(source: Source) {{ let n: u32 = route(source); }}
machine route(source: Source) -> u32 {{ measure(source.location) }}
"#
        )),
        NO_CASE
    );
}

/// Two payload fields of one case are disjoint places: one lent
/// exclusively beside the other. The same field is one place.
#[test]
fn payload_fields_of_one_case_are_lent_as_sibling_places() {
    let lent = |other: &str| {
        format!(
            r#"{SCALARS}
machine build_entry() {{ let n: u32 = route(); }}
machine route() -> u32 {{
    let mutable held: Source = Source::Git {{ repository: 1, revision: 2 }};
    two(@mutable held.repository, @held.{other})
}}
"#
        )
    };
    assert_eq!(verdict(&lent("revision")), "accepted");
    assert_eq!(
        verdict(&lent("repository")),
        "state `two` receives `@held.repository` as both mutable and read-only"
    );
}

// An owned payload is replaced at its captured case path, while its sum keeps
// its case and neighboring storage. The false arm makes the authored value seven an
// executable oracle rather than accepting any completing store.
const OWNED_STORE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../../source-rust/subsystems/compiler/source-to-psi/pipeline/06_lowered-program-to-checked-program/tests/fixtures/ownership/run/known_case_payload_store/main.omg"
));

// The inline source harness selects build_entry; the kept executable binds
// the same body as main through each target's ordinary ProgramEntry.
fn owned_store_source() -> String {
    OWNED_STORE.replace("machine main()", "machine build_entry()")
}

fn execute_owned_store(source: &str) {
    let verdict = judge_build(source);
    let Ok(RouteVerdict::Accepted(accepted)) = verdict else {
        panic!("the exact selected payload store must be checked: {verdict:?}");
    };
    let publication = accepted.publish_invocation().unwrap();
    let program =
        terminal_interpreter::TerminalProgram::borrow(publication.artifact.view()).unwrap();
    let mut instance = program.instantiate(Default::default()).unwrap();
    let status = instance
        .invoke(&mut terminal_work::WorkMeter::with_budget(10_000))
        .unwrap();
    assert!(
        matches!(
            status,
            terminal_interpreter::TerminalExecutionStatus::Complete(
                terminal_interpreter::TerminalExecutionResult::Unit
            )
        ),
        "the program must complete without reaching its trap: {status:?}"
    );
}

/// A match arm reads its payload `s.n` with a call `arm` beside it. The
/// entry traps unless the arm yields 1, the payload the dispatch saw.
fn arm_read(arm: &str) -> String {
    format!(
        r#"
data Slot {{ case Full(n: u32); case Empty; }}
machine peek(held: @mutable Slot) -> u32 {{ 0 }}
machine refill(held: @mutable Slot) -> u32 {{
    held = Slot::Full {{ n: 9 }};
    0
}}
machine build_entry() crashes Trap {{
    let mutable s: Slot = Slot::Full {{ n: 1 }};
    let got: u32 = match s {{
        Slot::Full {{ .. }} -> {arm},
        Slot::Empty -> 0
    }};
    transition got == 1 {{
        true -> done(),
        false -> bad()
    }}
    machine done() {{ }}
    machine bad() {{ crash Trap; }}
}}
"#
    )
}

/// Inside an arm, `s.n` is the payload the dispatch selected only until a
/// write or exclusive loan may change `s`. After `refill` replaces `s`, the
/// arm reads `s` again (structural_access.md: "A read is not equality with
/// the initializer or a previous read: intervening stores and calls can
/// change storage"), and nothing establishes the case there. Lowering used
/// to return the dispatch-time 1 here, in Psi and natively, instead of 9.
#[test]
fn a_payload_read_after_the_subject_is_lent_exclusively_reads_the_subject_again() {
    assert_eq!(verdict(&arm_read("refill(@mutable s) + s.n")), NO_CASE);
    // A helper that writes nothing leaves the case established, so the
    // fresh read is accepted and still sees 1.
    execute_owned_store(&arm_read("peek(@mutable s) + s.n"));
    // A read before the loan is the dispatch-time payload.
    execute_owned_store(&arm_read("s.n + refill(@mutable s)"));
}

/// The callee can change this exact scalar payload, but cannot replace its
/// containing sum. A reread after handback retains the enclosing case fact.
#[test]
fn a_scalar_payload_loan_preserves_the_enclosing_case() {
    execute_owned_store(
        r#"
        data Slot { case Full(n:u32); case Empty; }
        machine bump(value:@mutable u32)->u32 { $value=9; 0 }
        machine last(first:u32, result:u32)->u32 { result }
        machine read(held:@mutable Slot)->u32 { match held {
            Slot::Full {n}->last(bump(@mutable held.n), n),
            Slot::Empty->0
        } }
        machine build_entry() crashes Trap {
            let mutable held:Slot=Slot::Full {n:1};
            transition read(@mutable held)==9 { true->done(), _->bad() }
            machine done() {}
            machine bad() { crash Trap; }
        }
    "#,
    );
}

#[test]
fn a_known_owned_case_payload_replacement_publishes_and_executes() {
    execute_owned_store(&owned_store_source());
}

#[test]
fn a_nested_scalar_store_keeps_the_selected_payload_path() {
    execute_owned_store(
        &owned_store_source().replace("choice.item = item(7);", "choice.item.value = 7;"),
    );
}

#[test]
fn an_owned_payload_store_requires_its_exact_selected_case() {
    let source = owned_store_source().replace(
        "Choice::Left { item: Item { value: 3 } }",
        "Choice::Right {}",
    );
    let verdict = judge_build(&source);
    let Ok(RouteVerdict::Rejected { unestablished, .. }) = verdict else {
        panic!("a different selected case cannot receive the payload: {verdict:?}");
    };
    // A member's source span names its selector. Pin it inside this exact
    // assignment target, independently of the later payload read.
    let target = "choice.item";
    let start = source.find(&format!("{target} =")).unwrap();
    let end = start + target.len();
    assert!(unestablished.iter().any(|failure| failure.detail == NO_CASE
        && matches!(&failure.site, Some(terminal_check_tables::program::SourceOrigin::Authored(span))
            if start <= span.span.start && span.span.end <= end)));
}

#[test]
fn an_owned_payload_store_requires_mutability_of_the_existing_root() {
    let source = owned_store_source().replace("let mutable choice:", "let choice:");
    let verdict = judge_build(&source);
    let Ok(RouteVerdict::Rejected { violations, .. }) = verdict else {
        panic!("the immutable root cannot receive the payload: {verdict:?}");
    };
    let [violation] = violations.as_slice() else {
        panic!("expected one mutability violation: {violations:?}");
    };
    assert_eq!(
        violation.statement,
        "writing to an immutable owned binding requires `mutable` while reading the body of `build_entry`"
    );
    let start = source.find("choice.item = item(7);").unwrap() + "choice.".len();
    assert_eq!(
        violation.site,
        Some(source_text::SourceSpan::new(
            source_text::SourceId(1),
            source_text::Span::new(start, start + "item".len())
        ))
    );
}

#[test]
fn a_payload_store_cannot_use_case_membership_invalidated_by_its_replacement() {
    let source = owned_store_source()
        .replace("machine item(value: u32) -> Item { Item { value: value } }",
            "data Holder { choice: Choice; }\nmachine item(value: u32, owner: @mutable Holder) -> Item { Item { value: value } }")
        .replace("machine consume(choice: Choice) { }", "machine consume(holder: Holder) { }")
        .replace("let mutable choice: Choice = Choice::Left { item: Item { value: 3 } };",
            "let mutable holder: Holder = Holder { choice: Choice::Left { item: Item { value: 3 } } };")
        .replace("choice.item = item(7);", "holder.choice.item = item(7, @mutable holder);")
        .replace("choice.item.value", "holder.choice.item.value")
        .replace("consume(choice)", "consume(holder)");
    execute_owned_store(&source);
    let changed = source.replace(
        "machine item(value: u32, owner: @mutable Holder) -> Item { Item { value: value } }",
        "machine item(value: u32, owner: @mutable Holder) -> Item { let replacement: Choice = Choice::Right {}; owner.choice = replacement; Item { value: value } }",
    );
    let verdict = judge_build(&changed);
    let Ok(RouteVerdict::Rejected { unestablished, .. }) = verdict else {
        panic!("the RHS cannot remove the case of the captured destination: {verdict:?}");
    };
    // A member's source span names its selector. Pin it inside this exact
    // assignment target, independently of the later payload read.
    let target = "holder.choice.item";
    let start = changed.find(&format!("{target} =")).unwrap();
    let end = start + target.len();
    assert!(unestablished.iter().any(|failure| failure.detail == NO_CASE
        && matches!(&failure.site, Some(terminal_check_tables::program::SourceOrigin::Authored(span))
            if start <= span.span.start && span.span.end <= end)));
}

/// Two copyable sums in an array, and `body` as the body of a machine that
/// is lent the array and an index `slot` chosen at run time.
fn shelf(body: &str) -> String {
    format!(
        r#"
data Choice [copy] {{ case Left(value: u32); case Right; }}
data Shelf [copy] {{ items: [Choice; 2]; }}
machine pick(shelf: @mutable Shelf, slot: u64) -> u32 requires slot < 2 {{
    {body}
}}
machine build_entry() {{
    let mutable shelf: Shelf;
    let seen: u32 = pick(@mutable shelf, 0u64);
}}
"#
    )
}

/// Storing `Left` into `items[0]` makes its case known only until a store
/// that may reach `items[0]`. A store to `items[slot]` overlaps every element
/// of the array (structural_access.md: a store crossing a runtime element
/// "overlaps every element of that array"), so with `slot == 0` it would
/// replace `Left` by `Right` before the read of `Left`'s payload. The twins
/// differ only in the element the second store selects: `items[1]` is a
/// different fixed element and leaves the case of `items[0]` known.
fn element_store(element: &str) -> String {
    shelf(&format!(
        "shelf.items[0] = Choice::Left {{ value: 3u32 }};
    shelf.items[{element}] = Choice::Right {{}};
    shelf.items[0].value"
    ))
}

#[test]
fn a_store_to_another_fixed_element_keeps_the_known_case() {
    assert_eq!(verdict(&element_store("1")), "accepted");
}

#[test]
fn a_store_to_a_runtime_element_retires_the_known_case_of_every_element() {
    assert_eq!(verdict(&element_store("slot")), NO_CASE);
}

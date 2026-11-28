//! Indexed contracts name the actual array leaf, including through record fields.

use super::*;

fn invocation(text: &str, root: &str) -> Checked<crate::lowering::Elaboration> {
    let typed = boundary_calls::typed(text);
    let entry = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == root)
        .unwrap()
        .symbol;
    let frozen: crate::source_state::FrozenTypedTrees = typed.into();
    crate::lowering::elaborate_for(
        &frozen,
        crate::lowering::Compilation {
            calling: None,
            providers: None,
            declaration_metadata: &[],
            entries: &[entry],
            subjects: &[],
            purpose: source_provenance::DependencyScope::Product,
            selected: &[],
            bindings: &[],
            supplies: &[],
        },
        terminal_codec::TerminalPublication::Invocation,
    )
}

#[test]
fn reparse_subtracted_interval_bounds_its_real_field() {
    // Keep the decoder's full record and argument order. The direct read
    // separates its interval proof from the later nested copy/store calls.
    const POSITIVE: &str = r#"data WindowsNativeReparseData {
    tag: u32;
    data_length: u16;
    reserved: u16;
    substitute_offset: u16;
    substitute_length: u16;
    print_offset: u16;
    print_length: u16;
    rest: [u16; 8184];
}

machine entry(data: @WindowsNativeReparseData, output: @mutable [u16; 8192], start: u64, count: u64, relative: bool, position: u64) -> u16
requires start <= 8184 && count > 0 && count < 8192 && count <= 8184 - start && position < count {
    let character: u16 = data.rest[start + position];
    character
}
"#;
    let negative_source = POSITIVE.replacen("&& position < count", "&& position <= count", 1);
    assert_ne!(negative_source, POSITIVE);

    let positive = judge(Ok(
        invocation(POSITIVE, "entry").expect("the full reparse record and bounded read must lower")
    ))
    .expect("the positive must reach the ordinary checker");
    let negative = judge(Ok(invocation(&negative_source, "entry")
        .expect("the one-bound negative must remain well typed")))
    .expect("the negative must reach the ordinary checker");

    // Print both outcomes before asserting either, without dumping accepted
    // proof trees. A setup, arithmetic or storage failure is a distinct result.
    let report = |label, verdict: &RouteVerdict| match verdict {
        RouteVerdict::Accepted(_) => eprintln!("{label}: accepted"),
        other => eprintln!("{label}: {other:#?}"),
    };
    report("positive reparse index", &positive);
    report("negative reparse index", &negative);

    let RouteVerdict::Rejected {
        formations,
        violations,
        unproved,
        unestablished,
        conflicts,
        exceeded,
    } = negative
    else {
        panic!("the inclusive bound must be rejected for its one-past-end index")
    };
    assert!(formations.is_empty());
    assert!(violations.is_empty());
    assert!(unestablished.is_empty());
    assert!(conflicts.is_empty());
    assert!(exceeded.is_empty());
    assert!(!unproved.is_empty());
    assert!(
        unproved.iter().all(|failure| {
            matches!(
                failure.kind,
                ObligationKind::OperationSafety {
                    site: terminal_psi::ExecutionSite::Operation(_)
                }
            ) && failure
                .statement
                .ends_with("cannot prove index is within length 8184")
        }),
        "only the actual field's index bound may fail: {unproved:#?}"
    );
    assert!(
        matches!(positive, RouteVerdict::Accepted(_)),
        "the strict subtracted interval must prove the real field's index"
    );
}

fn indexed_buffer(last: u16) -> String {
    format!(
        r#"
data Buffer [copy]
requires self.words[4095] == {last}
{{ before: u16; words: [u16; 4096]; after: u16; }}
machine entry(buffer: @Buffer) -> u16
{{ buffer.words[4095] + 65535u16 }}
"#
    )
}

#[test]
fn an_indexed_data_requirement_proves_the_consuming_arithmetic() {
    let accepted = judge(invocation(&indexed_buffer(0), "entry")).unwrap();
    assert!(
        matches!(accepted, RouteVerdict::Accepted(_)),
        "{accepted:?}"
    );
    let refused = judge(invocation(&indexed_buffer(1), "entry")).unwrap();
    assert!(
        matches!(refused, RouteVerdict::Rejected { ref unproved, .. }
            if unproved.iter().any(|row| matches!(row.kind, ObligationKind::OperationSafety { .. }))),
        "{refused:?}"
    );
}

// A classified borrowed array must keep the same indexed predicate when its
// callable is read. Prefix and sibling inputs give the array a nonfirst root.
fn qualified_array_input(last: u16, body: &str) -> String {
    format!(
        r#"
domain [u16; 4096]::Terminated requires self[4095] == {last};
data Prefix {{ unrelated: u16; }}
machine entry(before: Prefix, path: @[u16; 4096] in Terminated, after: @[u16; 1]) -> u16
{{ {body} + 65535u16 }}
"#
    )
}

#[test]
fn an_indexed_domain_formal_supplies_only_its_actual_array_leaf() {
    let positive = qualified_array_input(0, "(path as -Terminated)[4095]");
    let accepted = judge(invocation(&positive, "entry")).unwrap();
    assert!(
        matches!(accepted, RouteVerdict::Accepted(_)),
        "{accepted:?}"
    );

    for negative in [
        qualified_array_input(1, "(path as -Terminated)[4095]"),
        qualified_array_input(0, "after[0]"),
    ] {
        let verdict = judge(invocation(&negative, "entry")).unwrap();
        assert!(
            matches!(verdict, RouteVerdict::Rejected { ref unproved, .. }
                if unproved.iter().any(|row| matches!(row.kind, ObligationKind::OperationSafety { .. }))),
            "{verdict:?}",
        );
    }
}

#[test]
fn an_owned_array_domain_is_proved_again_at_its_declared_result() {
    let text = r#"
domain [u16; 4096]::Terminated requires self[4095] == 0;
data Prefix { unrelated: u16; }
machine entry(before: Prefix, path: [u16; 4096] in Terminated) -> [u16; 4096] in Terminated
{ path }
"#;
    let verdict = judge(invocation(text, "entry")).unwrap();
    assert!(matches!(verdict, RouteVerdict::Accepted(_)), "{verdict:?}");
}

#[test]
fn each_contract_index_is_checked_against_its_own_array() {
    let refused = invocation(
        r#"
data Buffer [copy]
requires self.large[4095] == 0 && self.small[2] == 0
{ large: [u16; 4096]; small: [u16; 2]; }
machine entry(buffer: @Buffer) -> u16 { buffer.large[4095] }
"#,
        "entry",
    )
    .unwrap_err();
    assert!(
        matches!(refused.reason, RefusalReason::Violation),
        "{refused:?}"
    );
    assert_eq!(
        refused.detail,
        "contract array index 2 is outside its fixed extent 2 while reading the signature of \
         `entry`"
    );
}

#[test]
fn a_recursive_edge_owes_the_element_at_its_own_selector() {
    let source = r#"machine selected(items: @[u8; 4], index: u64, remaining: u64) -> u8
        requires index < 4u64
        requires items[index] == 71u8
        terminates by remaining
        {
            transition remaining > 0u64 {
                true -> selected(items, index, remaining - 1u64),
                _ -> items[index]
            }
        }
        machine main() -> u8 {
            let bytes: [u8; 4] = [1, 2, 3, 71];
            selected(@bytes, 3u64, 2u64)
        }"#;
    let kept = judge(invocation(source, "main")).unwrap();
    assert!(matches!(kept, RouteVerdict::Accepted(_)), "{kept:?}");
    for (case, changed) in [
        (
            "different recursive selector",
            source.replace("selected(items, index,", "selected(items, 0u64,"),
        ),
        (
            "different input byte",
            source.replace("[1, 2, 3, 71]", "[1, 2, 3, 70]"),
        ),
        (
            "selector past the end",
            source.replace("@bytes, 3u64", "@bytes, 4u64"),
        ),
    ] {
        assert_ne!(source, changed);
        let refused = judge(invocation(&changed, "main")).unwrap();
        assert!(
            matches!(refused, RouteVerdict::Rejected { .. }),
            "{case}: {refused:?}"
        );
    }
}

const NATIVE: &str = include_str!("../../tests/fixtures/calls/run/indexed_array_contract/main.omg");

#[test]
fn a_mutable_call_can_update_a_runtime_indexed_inline_record() {
    let source = r#"
data Item [copy] { count: u32; }
data Bag { items: [Item; 2]; }
machine set(item: @mutable Item) { item.count = 7; }
machine entry(bag: @mutable Bag, index: u64) requires index < 2 {
    set(@mutable bag.items[index]);
}
"#;
    let verdict = judge(invocation(source, "entry")).unwrap();
    let RouteVerdict::Accepted(accepted) = verdict else {
        panic!("{verdict:?}");
    };
    let module = &accepted.elaboration.module;
    let semantic_vocabulary::TargetMachineCallable::Machine(entry) = module.entry.unwrap() else {
        panic!("the entry must be an ordinary machine");
    };
    let program = terminal_semantics::callable_values::terminal_callable_values(module).unwrap();
    let reached =
        terminal_semantics::callable_values::reachable_callable_invocations(&program, [entry])
            .expect("execution must retain the indexed actual at its call");
    assert!(reached.invocations.iter().any(|call| {
        call.invocation.structural_arguments.iter().any(|actual| {
            actual.path.iter().any(|step| {
                matches!(step, terminal_psi::ResolvedStructuralPathSegment::RuntimeIndex { .. })
            })
        })
    }));

    let unbounded = source.replace(" requires index < 2", "");
    let verdict = judge(invocation(&unbounded, "entry")).unwrap();
    assert!(
        matches!(verdict, RouteVerdict::Rejected { ref unproved, .. }
            if unproved.iter().any(|failure| matches!(failure.kind, ObligationKind::OperationSafety { .. })
                && failure.statement.ends_with("cannot prove index is within length 2"))),
        "{verdict:?}"
    );
}

#[test]
fn indexed_call_contracts_and_mutable_handback_keep_the_exact_leaf() {
    let accepted = judge(invocation(NATIVE, "main")).unwrap();
    assert!(
        matches!(accepted, RouteVerdict::Accepted(_)),
        "{accepted:?}"
    );
    let broken = NATIVE.replace("buffer.units[4095] = 0;", "buffer.units[4095] = 1;");
    assert_ne!(broken, NATIVE);
    let refused = judge(invocation(&broken, "main")).unwrap();
    assert!(
        matches!(refused, RouteVerdict::Rejected { ref unproved, .. }
            if unproved.iter().any(|row| matches!(row.kind, ObligationKind::DataDomain { .. }) && row.consumed.is_some())),
        "{refused:?}"
    );
}

#[test]
fn an_exclusive_call_does_not_restore_the_caller_record_around_its_argument() {
    let text = NATIVE.replace(
        "repair(@mutable buffer);",
        "repair(@mutable buffer); spoil(@mutable buffer.units);",
    );
    let text = text
        + r#"
machine spoil(units: @mutable [u16; 4096]) { units[4095] = 1; }
"#;
    let refused = judge(invocation(&text, "main")).unwrap();
    assert!(
        matches!(refused, RouteVerdict::Rejected { ref unproved, .. }
        if unproved.iter().any(|row| matches!(row.kind, ObligationKind::CallRequirement { .. }))),
        "{refused:?}"
    );
}

#[test]
fn repeated_exclusive_calls_create_distinct_versions_of_the_same_leaf() {
    let text = NATIVE.replace(
        "repair(@mutable buffer);",
        "repair(@mutable buffer); repair(@mutable buffer);",
    );
    let elaboration = invocation(&text, "main").unwrap();
    let semantic_vocabulary::TargetMachineCallable::Machine(main) =
        elaboration.module.entry.unwrap()
    else {
        panic!("the witness entry is a machine");
    };
    let tables = elaboration.tables.machines.get(main).unwrap();
    let mut versions: Vec<_> = tables
        .values
        .iter()
        .filter_map(|(value, row)| {
            if let terminal_check_tables::values::ValueOrigin::StorageContent {
                place,
                after: Some(call),
            } = row.origin
            {
                Some((place, call, value))
            } else {
                None
            }
        })
        .collect();
    versions.sort();
    assert!(
        versions
            .windows(2)
            .any(|pair| pair[0].0 == pair[1].0 && pair[0].1 != pair[1].1 && pair[0].2 != pair[1].2)
    );
    let accepted = judge(Ok(elaboration)).unwrap();
    assert!(
        matches!(accepted, RouteVerdict::Accepted(_)),
        "{accepted:?}"
    );
}

#[test]
fn a_storage_snapshot_requires_its_exact_establishing_call() {
    use terminal_check_tables::values::ValueOrigin;
    for shared_call in [false, true] {
        let mut elaboration = invocation(NATIVE, "main").unwrap();
        let semantic_vocabulary::TargetMachineCallable::Machine(entry) =
            elaboration.module.entry.unwrap()
        else {
            panic!("the witness entry is a machine");
        };
        let main = elaboration
            .module
            .machines
            .iter()
            .find(|machine| machine.id == entry)
            .unwrap();
        let operation = main
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .find(|operation| {
                if shared_call {
                    operation.kind.declared_invocation().is_some_and(|call| {
                        call.structural_arguments.iter().any(|argument| {
                            argument.access == terminal_psi::StructuralAccess::SharedBorrow
                        })
                    })
                } else {
                    operation.kind.declared_invocation().is_none()
                }
            })
            .unwrap()
            .id;
        let tables = elaboration.tables.machines.get_mut(entry).unwrap();
        let snapshot = tables
            .values
            .iter()
            .find_map(|(value, row)| {
                matches!(
                    row.origin,
                    ValueOrigin::StorageContent { after: Some(_), .. }
                )
                .then_some(value)
            })
            .unwrap();
        let row = tables.values.get_mut(snapshot).unwrap();
        let ValueOrigin::StorageContent { place, .. } = row.origin else {
            unreachable!()
        };
        row.origin = ValueOrigin::StorageContent {
            place,
            after: Some(operation),
        };
        let refused = judge(Ok(elaboration)).unwrap_err();
        assert_eq!(refused.reason, RefusalReason::ForeignIdentity);
        assert_eq!(
            refused.detail,
            if shared_call {
                "a returned storage snapshot is outside the call's established storage in machine `main`"
            } else {
                "a storage snapshot is not after a call in machine `main`"
            }
        );
    }
}

#[test]
fn stored_callable_result_bound_reads_its_mutable_output_after_the_call() {
    const SOURCE: &str = r#"
        data ReadInto<S> = machine(state: @mutable S, output: @mutable [u8]) -> u64
            ensures result <= output.length reaches none crashes none;
        data File { byte: u8; }
        machine read_one(read: ReadInto<File>, state: @mutable File, output: @mutable [u8]) -> u64
        ensures result <= output.length { (read)(state, output) }
    "#;
    let accepted = judge(invocation(SOURCE, "read_one")).unwrap();
    assert!(
        matches!(accepted, RouteVerdict::Accepted(_)),
        "{accepted:?}"
    );

    let missing_bound = SOURCE.replacen("ensures result <= output.length", "", 1);
    let rejected = judge(invocation(&missing_bound, "read_one")).unwrap();
    assert!(
        matches!(rejected, RouteVerdict::Rejected { ref unproved, .. }
            if unproved.iter().any(|failure| matches!(failure.kind, ObligationKind::NormalGuarantee { .. }))),
        "{rejected:?}"
    );
}

#[test]
fn a_stored_callable_may_change_an_exclusive_fields_entry_value() {
    const SOURCE: &str = r#"
        data Counter { count: u64; }
        data ReadCount = machine(output: @mutable Counter) -> u64
            ensures result <= output.count reaches none crashes none;
        machine relay(read: ReadCount, output: @mutable Counter) -> u64
            requires output.count == 0u64
            ensures result <= output.count { (read)(output) }
    "#;
    let accepted = judge(invocation(SOURCE, "relay")).unwrap();
    assert!(
        matches!(accepted, RouteVerdict::Accepted(_)),
        "{accepted:?}"
    );

    let stale = SOURCE.replace(
        "ensures result <= output.count {",
        "ensures result == 0u64 {",
    );
    let rejected = judge(invocation(&stale, "relay")).unwrap();
    assert!(
        matches!(rejected, RouteVerdict::Rejected { ref unproved, .. }
            if unproved.iter().any(|failure| matches!(failure.kind, ObligationKind::NormalGuarantee { .. }))),
        "{rejected:?}"
    );
}

#[test]
fn a_reader_package_keeps_its_callbacks_output_bound() {
    const SOURCE: &str = r#"
        data ReadInto<S> = machine(state: @mutable S, output: @mutable [u8]) -> u64
            ensures result <= output.length
            reaches none
            crashes none;

        data Reader<'a, some S> {
            state: @'a mutable S;
            read: ReadInto<S>;
        }

        machine read_one(reader: @mutable Reader, output: @mutable [u8]) -> u64
        ensures result <= output.length
        {
            (reader.read)(reader.state, output)
        }
    "#;
    let opened =
        invocation(SOURCE, "read_one").expect("open the package and call its stored descriptor");
    assert!(
        opened
            .module
            .execution
            .packages
            .sites
            .iter()
            .any(|site| matches!(
                site.event,
                terminal_psi::TerminalPackageEvent::Invoke { .. }
            ))
    );
    let accepted = judge(Ok(opened)).unwrap();
    assert!(
        matches!(accepted, RouteVerdict::Accepted(_)),
        "{accepted:?}"
    );

    let missing = SOURCE.replacen("ensures result <= output.length", "", 1);
    let rejected = judge(invocation(&missing, "read_one")).unwrap();
    assert!(
        matches!(rejected, RouteVerdict::Rejected { ref unproved, .. } if
            unproved.iter().any(|failure| matches!(failure.kind, ObligationKind::NormalGuarantee { .. }))
        ),
        "{rejected:?}"
    );
}

#[test]
fn reborrow_of_an_input_element_field_needs_the_real_index_bound() {
    const SOURCE: &str = r#"
        data File { byte: u8; }
        data Holder<'a> { state: @'a File; }
        machine inspect(state: @File) -> u8 { state.byte }
        machine main(items: @mutable [Holder], i: u64) -> u8
        requires i < items.length {
            let item: @Holder = @items[i];
            inspect(item.state)
        }
    "#;
    for input in [SOURCE.to_owned(), SOURCE.replace("items: @mutable [", "items: @[")] {
        let accepted = judge(invocation(&input, "main")).unwrap();
        assert!(matches!(accepted, RouteVerdict::Accepted(_)), "{accepted:?}");
    }
    let missing = SOURCE.replace("requires i < items.length", "");
    let rejected = judge(invocation(&missing, "main")).unwrap();
    assert!(
        matches!(rejected, RouteVerdict::Rejected { ref unproved, .. } if
        unproved.iter().any(|failure| matches!(
            failure.kind, ObligationKind::OperationSafety { .. }
        ) && failure.statement.contains("cannot prove storage index bounds"))),
        "{rejected:?}"
    );
}

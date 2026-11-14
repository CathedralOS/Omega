//! Indexed contracts name the actual array leaf, including through record fields.

use super::*;

fn invocation(text: &str, root: &str) -> Checked<typed_trees_to_terminal_psi::Elaboration> {
    let typed = boundary_calls::typed(text);
    let entry = typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == root)
        .unwrap()
        .symbol;
    let frozen: typed_trees::FrozenTypedTrees = typed.into();
    typed_trees_to_terminal_psi::elaborate_for(
        &frozen,
        typed_trees_to_terminal_psi::Compilation {
            declaration_metadata: &[],
            entries: &[entry],
            subjects: &[],
            purpose: source::DependencyScope::Product,
            selected: &[],
            bindings: &[],
            supplies: &[],
        },
        terminal_codec::TerminalPublication::Invocation,
    )
}

fn indexed_buffer(last: u16) -> String {
    format!(
        r#"
data Buffer [copy]
requires self.words[4095] == {last}
{{ before: u16; words: [u16; 4096]; after: u16; }}
machine entry(buffer: &Buffer) -> u16
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

#[test]
fn each_contract_index_is_checked_against_its_own_array() {
    let refused = invocation(
        r#"
data Buffer [copy]
requires self.large[4095] == 0 && self.small[2] == 0
{ large: [u16; 4096]; small: [u16; 2]; }
machine entry(buffer: &Buffer) -> u16 { buffer.large[4095] }
"#,
        "entry",
    )
    .unwrap_err();
    assert!(
        matches!(refused.reason, RefusalReason::Violation(_)),
        "{refused:?}"
    );
    assert_eq!(
        refused.detail,
        "contract array index 2 is outside its fixed extent 2"
    );
}

const NATIVE: &str =
    include_str!("../../../../../../../tests/fixtures/run/calls/indexed_array_contract/main.omg");

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
        "repair(&mut buffer);",
        "repair(&mut buffer); spoil(&mut buffer.units);",
    );
    let text = text
        + r#"
machine spoil(units: &mut [u16; 4096]) { units[4095] = 1; }
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
        "repair(&mut buffer);",
        "repair(&mut buffer); repair(&mut buffer);",
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
                "a returned storage snapshot is outside the call's established storage"
            } else {
                "a storage snapshot is not after a declared call"
            }
        );
    }
}

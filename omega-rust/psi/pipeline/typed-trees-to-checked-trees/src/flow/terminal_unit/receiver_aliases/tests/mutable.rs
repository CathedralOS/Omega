use super::*;

#[test]
fn mutable_root_cannot_widen_its_write_only_child() {
    for (access, borrow) in [("&mut", "&mut"), ("&", "&")] {
        let source = format!(
            "data Record [copy] {{ value: u16; }}
             machine Record::replace(&write self, value: u16) {{ self.value = value; }}
             machine forward(records: &mut [Record; 2], value: u16) {{
                 let parent: &mut [Record; 2] = &mut records;
                 let middle: &write [Record; 2] = &write parent;
                 let child: {access} [Record; 2] = {borrow} middle;
                 child[1].replace(value);
             }}"
        );
        let tokens = source_files_to_tokens::Lexer::new(&source)
            .tokenize()
            .unwrap();
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
        let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
        let typed =
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
        assert!(
            crate::lower_typed_trees(typed).is_err(),
            "write-only parent to {access}"
        );
    }
}

fn chain() -> checked_trees::CheckedTrees {
    fixture(
        "mut",
        "let parent: &mut [Record; 2] = &mut records;
         let middle: &mut [Record; 2] = &mut parent;
         let child: &write Record = &write middle[1];",
        "child.replace(17); child.replace(value);",
    )
}

#[test]
fn mutable_prefixes_retain_access_and_attenuate_at_the_selected_child() {
    let checked = chain();
    let aliases = aliases(&checked).expect("mutable mutable write chain");
    assert_eq!(aliases.len(), 3);
    assert_eq!(
        aliases[2].segments,
        [facts::PlaceSegment::FixedIndex { index: 1 }]
    );
    let resources = &checked.facts.borrow;
    let parent = resources
        .direct_loan_resources
        .iter()
        .find(|(_, row)| checked.symbols.name(row.owner_symbol) == "parent")
        .unwrap()
        .1;
    assert_eq!(parent.access, BorrowAccessKind::Mutable);
    for (name, access) in [
        ("middle", BorrowAccessKind::Mutable),
        ("child", BorrowAccessKind::WriteOnly),
    ] {
        let resource = resources
            .reborrow_loan_resources
            .iter()
            .find(|(_, row)| checked.symbols.name(row.owner_symbol) == name)
            .unwrap()
            .1;
        assert_eq!(resource.parent_access, BorrowAccessKind::Mutable);
        assert_eq!(resource.access, access);
    }
}

#[test]
fn mutable_alias_can_project_fields_before_fixed_indexes() {
    let checked = checked(
        "data Record [copy] { value: u16; }
         data Container [copy] { records: [Record; 2]; }
         machine Record::replace(&write self, value: u16) { self.value = value; }
         machine forward(destination: &mut Container, value: u16) {
             let parent: &mut Container = &mut destination;
             let middle: &mut [Record; 2] = &mut parent.records;
             let child: &write Record = &write middle[1];
             child.replace(17); child.replace(value);
         }",
    );
    let aliases = aliases(&checked).expect("field/index chain");
    assert_eq!(aliases.len(), 3);
    assert!(matches!(
        aliases[2].segments.as_slice(),
        [
            facts::PlaceSegment::Field { .. },
            facts::PlaceSegment::FixedIndex { index: 1 }
        ]
    ));
}

#[test]
fn mutable_prefix_replay_rejects_access_capture_and_lifecycle_drift() {
    let original = chain();
    let parent = original
        .facts
        .borrow
        .direct_loan_resources
        .iter()
        .find(|(_, row)| original.symbols.name(row.owner_symbol) == "parent")
        .unwrap()
        .0;
    let child = original
        .facts
        .borrow
        .reborrow_loan_resources
        .iter()
        .find(|(_, row)| original.symbols.name(row.owner_symbol) == "child")
        .unwrap()
        .0;
    let middle = original
        .facts
        .borrow
        .reborrow_loan_resources
        .iter()
        .find(|(_, row)| original.symbols.name(row.owner_symbol) == "middle")
        .unwrap()
        .0;
    for mutation in 0..8 {
        let mut checked = original.clone();
        match mutation {
            0 => {
                checked
                    .facts
                    .borrow
                    .direct_loan_resources
                    .get_mut(parent)
                    .access = BorrowAccessKind::WriteOnly
            }
            1 => {
                checked
                    .facts
                    .borrow
                    .reborrow_loan_resources
                    .get_mut(middle)
                    .access = BorrowAccessKind::WriteOnly
            }
            2 => {
                checked
                    .facts
                    .borrow
                    .reborrow_loan_resources
                    .get_mut(child)
                    .parent_access = BorrowAccessKind::WriteOnly
            }
            3 => {
                checked
                    .facts
                    .borrow
                    .reborrow_loan_resources
                    .get_mut(child)
                    .access = BorrowAccessKind::Mutable
            }
            4 => checked
                .facts
                .borrow
                .reborrow_loan_resources
                .get_mut(child)
                .captured_place
                .segments
                .clear(),
            5 => {
                checked
                    .facts
                    .borrow
                    .reborrow_loan_resources
                    .get_mut(child)
                    .parent_loan = checked.facts.borrow.direct_loan_resources.get(parent).loan
            }
            6 => {
                checked
                    .facts
                    .borrow
                    .reborrow_loan_resources
                    .get_mut(child)
                    .weakening_reason = FlowBorrowWeakeningReason::LastUseExpired
            }
            _ => checked.facts.borrow.reborrow_disposition_events.clear(),
        }
        assert!(aliases(&checked).is_none(), "mutation {mutation}");
    }
}

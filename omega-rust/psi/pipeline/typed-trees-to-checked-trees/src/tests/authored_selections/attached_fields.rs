use super::*;
use checked_trees::CheckedTrees;
use symbols::SymbolHandle;
use typed_trees::data::DataMember;

fn check_source(source: &str) -> CheckedTrees {
    let tokens = Lexer::new(source)
        .tokenize()
        .expect("tokenize attached fields");
    let syntax = parse_syntax_trees(&tokens).expect("parse attached fields");
    let resolved = lower_syntax_trees(&syntax).expect("resolve attached fields");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type attached fields");
    lower_typed_trees(typed).expect("bare attached fields complete checking")
}

fn field_symbol(program: &CheckedTrees, owner: &str, name: &str) -> SymbolHandle {
    let definition = program
        .data_definitions()
        .iter()
        .find(|definition| definition.name.as_str() == owner)
        .expect("exact nominal field owner");
    let symbol = program
        .data_members(definition)
        .iter()
        .find_map(|member| match member {
            DataMember::Field(field) if field.name.as_str() == name => Some(field.symbol),
            _ => None,
        })
        .expect("field declared under that owner");
    assert!(symbol.is_valid());
    symbol
}

fn assert_field_selections(program: &CheckedTrees, source: &str, expected: &[(&str, &str, &str)]) {
    let ledger = program.authored_declaration_selections();
    let members = ledger
        .iter()
        .filter(|selection| selection.kind() == AuthoredDeclarationSelectionKind::MemberAccess)
        .collect::<Vec<_>>();
    assert_eq!(members.len(), expected.len(), "member rows: {members:#?}");
    for &(path, owner, name) in expected {
        let occurrences = source.match_indices(path).collect::<Vec<_>>();
        let [(start, _)] = occurrences.as_slice() else {
            panic!("fixture must identify one authored `{path}`: {occurrences:?}")
        };
        let end = start + path.len();
        let start = end - name.len();
        let rows = members
            .iter()
            .filter(|selection| {
                let span = selection.source_span().span;
                span.start == start && span.end == end
            })
            .collect::<Vec<_>>();
        let [selection] = rows.as_slice() else {
            panic!("one MemberAccess row for `{path}`: {members:#?}")
        };
        let selected = field_symbol(program, owner, name);
        assert!(
            matches!(
                selection.target(),
                AuthoredDeclarationSelectionTarget::Resolved(target)
                    if target.selected_symbol() == selected
            ),
            "`{path}` must select `{owner}::{name}` ({selected:?}): {selection:#?}"
        );
    }
    assert!(ledger.all_finalized(), "selections={ledger:#?}");
}

fn receiver_source(receiver: &str, body: &str) -> String {
    format!(
        "data Inner {{ value: u16; values: [u16; 2]; }}
         data Record {{ inner: Inner; }}
         machine Record::exercise({receiver}) {{ {body} }}"
    )
}

#[test]
fn bare_nested_store_through_mutable_self_selects_exact_field() {
    let source = receiver_source("&mut self", "inner.value = 17;");
    let checked = check_source(&source);
    assert_field_selections(&checked, &source, &[("inner.value", "Inner", "value")]);
}

#[test]
fn bare_nested_store_through_write_only_self_selects_exact_field() {
    let source = receiver_source("&write self", "inner.value = 17;");
    let checked = check_source(&source);
    assert_field_selections(&checked, &source, &[("inner.value", "Inner", "value")]);
}

#[test]
fn bare_nested_read_through_shared_self_selects_exact_field() {
    let source = receiver_source("&self", "let observed: u16 = inner.value;");
    let checked = check_source(&source);
    assert_field_selections(&checked, &source, &[("inner.value", "Inner", "value")]);
}

#[test]
fn bare_literal_indexed_nested_store_selects_exact_array_field() {
    for receiver in ["&mut self", "&write self"] {
        let source = receiver_source(receiver, "inner.values[1] = 17;");
        let checked = check_source(&source);
        assert_field_selections(&checked, &source, &[("inner.values", "Inner", "values")]);
    }
}

#[test]
fn same_spelled_bare_nested_fields_select_each_exact_owner() {
    let source = r#"
        data Second { value: u16; }
        data First { value: u16; }
        data Record { first: First; second: Second; }
        machine Record::exercise(&self) {
            let first_value: u16 = first.value;
            let second_value: u16 = second.value;
        }
    "#;
    let checked = check_source(source);
    assert_ne!(
        field_symbol(&checked, "First", "value"),
        field_symbol(&checked, "Second", "value")
    );
    assert_field_selections(
        &checked,
        source,
        &[
            ("first.value", "First", "value"),
            ("second.value", "Second", "value"),
        ],
    );
}

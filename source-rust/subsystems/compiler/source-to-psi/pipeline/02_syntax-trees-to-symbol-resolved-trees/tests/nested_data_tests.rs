/*
    Nested data declarations resolve as the declarations the parser hoisted
    under their owners' names (`wiki/spec/language/data_and_literals.md`,
    "Records, sums and nested declarations"). `Command::Operation::GetUser`
    selects the case of the nested sum `Command::Operation`, each segment
    keeps its own declaration, and a nested declaration joins its owner's
    one member namespace with the cases.
*/

use crate::resolution::resolve;
use crate::source_resolved::expression::ExpressionNode;

const COMMAND: &str = include_str!("fixtures/nested_data/run/command/main.omg");
const MEMBER_COLLISION: &str =
    include_str!("fixtures/nested_data/fail/command_member_collision/main.omg");

#[test]
fn a_qualified_case_path_walks_through_each_nested_declaration() {
    let trees = resolve(super::source_request(COMMAND))
        .unwrap_or_else(|refusal| panic!("resolution refused: {refusal:?}"));
    let table = &trees.tables.bodies.expressions;
    let mut checked = Vec::new();
    for (_, node) in table.iter_expressions() {
        let ExpressionNode::Name(path) = node else {
            continue;
        };
        let spelled: Vec<_> = table
            .name_path_members(path.members)
            .iter()
            .map(|member| member.as_str().to_owned())
            .collect();
        if spelled.len() != 3 || spelled[..2] != ["Command", "Operation"] {
            continue;
        }
        let selected: Vec<_> = table
            .name_path_member_symbols(path.member_symbols)
            .iter()
            .map(|symbol| {
                assert!(symbol.is_valid(), "{spelled:?} leaves a segment unselected");
                (
                    format!("{:?}", trees.symbols.get(*symbol).kind),
                    trees.symbols.display_path(*symbol, "::"),
                )
            })
            .collect();
        assert_eq!(
            trees.symbols.display_path(path.symbol, "::"),
            format!("Command::Operation::{}", spelled[2])
        );
        checked.push((spelled[2].clone(), selected));
    }
    checked.sort();
    checked.dedup();
    let expected = |leaf: &str| {
        vec![
            ("Data".to_owned(), "Command".to_owned()),
            ("Data".to_owned(), "Command::Operation".to_owned()),
            ("Variant".to_owned(), format!("Command::Operation::{leaf}")),
        ]
    };
    assert_eq!(
        checked,
        [
            ("CountUsers".to_owned(), expected("CountUsers")),
            ("GetUser".to_owned(), expected("GetUser")),
        ]
    );
}

#[test]
fn a_nested_declaration_named_like_a_case_of_its_owner_is_refused() {
    let Err(diagnostics) = resolve(super::source_request(MEMBER_COLLISION)) else {
        panic!("a nested declaration and a case share one member name");
    };
    let messages: Vec<_> = diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect();
    assert!(
        messages.iter().any(|message| message.starts_with(
            "`Command::Operation::CountUsers` is declared twice: this nested data declaration and an earlier case"
        )),
        "{messages:?}"
    );
}

use super::*;
use language_semantics::declaration_selection::AuthoredDeclarationSelectionKind as Kind;
use typed_trees::expression::ExpressionNode;

const OUTCOME: &str =
    "module outcomes; pub data Outcome { common: u32; case Empty; case Value(item: u32); }";

#[test]
fn qualified_case_membership_retains_owner_occurrences_and_payload_semantics() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(root.join("outcomes.omg"), OUTCOME);
    Sources::write(
        root.join("other.omg"),
        "module other; pub data Outcome { case Empty; }",
    );
    let source = "use outcomes; use outcomes::Outcome; use other;
        machine empty(value: &outcomes::Outcome) -> bool { value in outcomes::Outcome::Empty }
        machine payload(value: &outcomes::Outcome) -> bool { value in outcomes::Outcome::Value }
        machine short(value: &outcomes::Outcome) -> bool { value in Outcome::Value }";
    Sources::write(root.join("main.omg"), source);
    let checked = compile(&root, root_inputs(&root));
    for (name, owner_spelling, case) in [
        ("empty", "outcomes::Outcome", "Empty"),
        ("payload", "outcomes::Outcome", "Value"),
        ("short", "Outcome", "Value"),
    ] {
        let machine = checked
            .typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == name)
            .unwrap();
        let state = &checked.typed.machine_states(machine)[0];
        let [StatementNode::Expression(expression)] = checked
            .typed
            .statement_table
            .statements(state.statement_nodes)
        else {
            panic!("one membership result");
        };
        let ExpressionNode::Binary(binary) = checked.typed.expression_table.expression(*expression)
        else {
            panic!("membership predicate");
        };
        assert!(validation::has_exact_case_membership_meaning(
            &checked.typed,
            machine,
            Some(state),
            *expression,
            binary
        ));
        let ExpressionNode::Name(path) = checked.typed.expression_table.expression(binary.right)
        else {
            panic!("exact classifier");
        };
        assert_eq!(
            checked.symbols.display_path(path.head_symbol, "::"),
            "outcomes::Outcome"
        );
        assert_eq!(
            checked.symbols.display_path(path.symbol, "::"),
            format!("outcomes::Outcome::{case}")
        );
        assert_eq!(
            checked
                .typed
                .expression_table
                .name_path_member_symbols(path.member_symbols)
                .len(),
            2
        );
        let rows = checked
            .typed
            .expression_table
            .authored_selection_occurrences(*expression)
            .map(|occurrence| {
                checked
                    .typed
                    .authored_declaration_selections()
                    .get(occurrence)
                    .unwrap()
            })
            .collect::<Vec<_>>();
        let owner = rows
            .iter()
            .find(|row| row.kind() == Kind::CaseReference)
            .unwrap();
        let member = rows
            .iter()
            .find(|row| row.kind() == Kind::CaseMembership)
            .unwrap();
        assert_eq!(
            &source[owner.source_span().span.start..owner.source_span().span.end],
            owner_spelling
        );
        assert_eq!(
            &source[member.source_span().span.start..member.source_span().span.end],
            case
        );
        // A valid classifier does not replace either independently retained
        // authored selection, even after qualification has been normalized.
        let occurrences = checked
            .typed
            .expression_table
            .authored_selection_occurrences(*expression)
            .collect::<Vec<_>>();
        assert_eq!(occurrences.len(), 2);
        for retained in [vec![], vec![occurrences[0]], vec![occurrences[1]]] {
            let mut forged = checked.typed.clone();
            let copy = forged
                .expression_table
                .insert(ExpressionNode::Binary(*binary));
            forged
                .expression_table
                .attach_authored_selection_occurrences(copy, retained);
            assert!(!validation::has_exact_case_membership_meaning(
                &forged,
                machine,
                Some(state),
                copy,
                binary
            ));
        }
        let mut forged = checked.typed.clone();
        let ExpressionNode::Name(path) = forged.expression_table.expression_mut(binary.right)
        else {
            unreachable!()
        };
        path.symbol = path.head_symbol;
        assert!(!validation::has_exact_case_membership_meaning(
            &forged,
            machine,
            Some(state),
            *expression,
            binary
        ));
    }
}

#[test]
fn attached_self_case_membership_uses_the_nominal_carrier() {
    let tree = Sources::new();
    let root = tree.package("root");
    Sources::write(root.join("outcomes.omg"), OUTCOME);
    Sources::write(
        root.join("other.omg"),
        "module other; pub data Outcome { case Empty; }",
    );
    Sources::write(
        root.join("main.omg"),
        "use outcomes; machine outcomes::Outcome::test(&self) -> bool { self in outcomes::Outcome::Empty }",
    );
    compile(&root, root_inputs(&root));
    Sources::write(
        root.join("main.omg"),
        "use outcomes; use other; machine outcomes::Outcome::test(&self) -> bool { self in other::Outcome::Empty }",
    );
    let Err(errors) =
        compile_to_checked_with_packages(&root.join("main.omg"), None, root_inputs(&root))
    else {
        panic!("attached self cannot observe a foreign carrier case");
    };
    assert!(
        errors.iter().any(|error| error
            .message
            .contains("case membership must test a value of the exact declaring data type")),
        "{errors:?}"
    );
}

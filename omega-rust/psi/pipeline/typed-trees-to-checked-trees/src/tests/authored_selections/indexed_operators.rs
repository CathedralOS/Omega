use super::*;
use checked_trees::CheckedOperatorResolutionStatus;
use language_core::operator_spelling::OperatorSpelling;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};

fn indexed_program() -> typed_trees::TypedTrees {
    let source = r#"
        boundary operator [] Slice::index<Element>(items: &[Element], position: u64) -> Element
        requires position < items.len;

        data Main { values: [i32; 3]; }
        machine Main::main(&mut self) {
            let first: i32 = self.values[0];
        }
    "#;
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    lower_symbol_resolved_trees(&resolved).expect("type")
}

#[test]
fn indexed_array_custody_selects_the_exact_checked_slice_declaration() {
    let checked = lower_typed_trees(indexed_program()).expect("checked indexed array");
    let selected = checked
        .operators()
        .iter()
        .find(|operator| operator.spelling == Some(OperatorSpelling::Index))
        .expect("declared indexing operator")
        .symbol;
    let indexed = checked
        .expression_table
        .iter_expressions()
        .find_map(|(expression, node)| {
            matches!(node, ExpressionNode::Indexed(_)).then_some(expression)
        })
        .expect("indexed expression");
    let operator_use = checked
        .facts
        .operators
        .expression_use(indexed)
        .expect("checked use for that exact expression");
    assert_eq!(
        operator_use.status,
        CheckedOperatorResolutionStatus::Resolved
    );
    assert_eq!(operator_use.selected_operator_symbol, selected);
    assert_eq!(
        checked
            .facts
            .operators
            .selected_candidate(operator_use)
            .expect("retained selected candidate")
            .operator_symbol,
        selected
    );
    let applications = checked
        .facts
        .operators
        .boundary_applications
        .iter()
        .filter(|application| {
            application.requirement_symbol == selected
                && matches!(
                    application.site,
                    checked_trees::CheckedBoundaryOperatorApplicationUseSite::Expression {
                        expression, ..
                    } if expression == indexed
                )
        })
        .collect::<Vec<_>>();
    assert!(
        !applications.is_empty(),
        "indexed adaptation retains its closed application"
    );
    for application in applications {
        let [
            checked_trees::CheckedBoundaryOperatorApplicationArgument::Type {
                binder_owner,
                binder_ordinal,
                type_reference,
                ..
            },
        ] = application.arguments.as_slice()
        else {
            panic!("indexing retains one exact element type argument")
        };
        assert_eq!(*binder_owner, selected);
        assert_eq!(*binder_ordinal, 0);
        assert_eq!(
            checked.primitive_type_reference(*type_reference),
            Some(typed_trees::types::PrimitiveType::I32)
        );
    }
    let selections = checked
        .expression_table
        .authored_selection_occurrences(indexed)
        .filter_map(|occurrence| checked.authored_declaration_selections().get(occurrence))
        .filter(|selection| selection.kind() == AuthoredDeclarationSelectionKind::Operator)
        .collect::<Vec<_>>();
    assert_eq!(selections.len(), 1);
    assert!(matches!(
        selections[0].target(),
        AuthoredDeclarationSelectionTarget::Resolved(target)
            if target.selected_symbol() == selected
    ));
    assert!(checked.authored_declaration_selections().all_finalized());
    assert!(
        !crate::authored_selections::typed_operator_has_no_authored_selection(&checked, indexed)
    );
}

#[test]
fn indexed_array_custody_rejects_missing_or_reassigned_checked_uses() {
    for reassign_expression in [false, true] {
        let typed = indexed_program();
        let authored = typed.authored_declaration_selections().clone();
        let mut checked = lower_typed_trees(typed).expect("checked indexed array");
        checked
            .typed
            .retain_authored_declaration_selections(authored);
        // First prove that the retained typed roots still rejoin this ledger.
        crate::authored_selections::finalize_checked_authored_selections(
            &mut checked.typed.clone(),
            &checked.facts,
        )
        .expect("untampered checked uses finalize the original occurrence");
        let mut changed = 0;
        let handles = checked
            .facts
            .operators
            .uses
            .iter()
            .map(|(handle, _)| handle)
            .collect::<Vec<_>>();
        for handle in handles {
            let operator_use = checked.facts.operators.uses.get_mut(handle);
            if operator_use.spelling != OperatorSpelling::Index {
                continue;
            }
            changed += 1;
            if reassign_expression {
                operator_use.expression = ExpressionHandle::invalid();
            } else {
                operator_use.status = CheckedOperatorResolutionStatus::Missing;
                operator_use.selected_operator_symbol = symbols::SymbolHandle::invalid();
            }
        }
        assert!(changed > 0);
        let diagnostic = crate::authored_selections::finalize_checked_authored_selections(
            &mut checked.typed,
            &checked.facts,
        )
        .expect_err("an indexed token cannot borrow another or missing checked use");
        assert!(
            diagnostic.message.contains("remained unresolved"),
            "{diagnostic:?}"
        );
        assert!(!checked.authored_declaration_selections().all_finalized());
    }
}

use crate::CheckingRequest;
use crate::lower_typed_trees;
use crate::tests::front_end::typed_program;
use checked_trees::CheckedOperatorResolutionStatus;
use language_core::operator_spelling::OperatorSpelling;
use typed_trees::expression::ExpressionHandle;

fn indexed_program() -> typed_trees::TypedTrees {
    let source = r#"
        boundary operator [] Slice::index<Element>(items: &[Element], position: u64) -> Element
        requires position < items.len;

        data Main { values: [i32; 3]; }
        machine Main::main(&mut self) {
            let first: i32 = self.values[0];
        }
    "#;
    typed_program(source)
}

#[test]
fn indexed_array_custody_rejects_missing_or_reassigned_checked_uses() {
    for reassign_expression in [false, true] {
        let typed = indexed_program();
        let authored = typed.authored_declaration_selections().clone();
        let mut checked =
            lower_typed_trees(typed, &CheckingRequest::settled()).expect("checked indexed array");
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

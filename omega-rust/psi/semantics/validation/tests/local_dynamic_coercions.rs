use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionNode;
use typed_trees::statement::StatementNode;
use validation::{collect_dynamic_conformance_selections, validate_program};

fn typed(source: &str) -> Result<TypedTrees, Vec<diagnostics::Diagnostic>> {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax)?;
    lower_symbol_resolved_trees(&resolved).map_err(|diagnostic| vec![diagnostic])
}

fn validate(source: &str) -> Result<(), Vec<diagnostics::Diagnostic>> {
    validate_program(&typed(source)?)
}

#[test]
fn admits_borrow_wrapped_exact_dynamic_coercion_receiver() {
    let program = typed(
        r#"
        trait Shape {
            machine code(&self) -> i32;
        }

        data Item {
            value: i32;
        }

        Primary: Item satisfies Shape {
            machine code(&self) -> i32 {
                transition {
                    _ -> self.value
                }
            }
        }

        data Reader {
            item: Item;
        }

        machine Reader::read(&self) -> i32 {
            let erased: &dyn Shape = &self.item as &dyn Item::Primary;
            let result: i32 = erased.code();
            result
        }
        "#,
    )
    .expect("dynamic coercion source should type-check");

    let initializer = program
        .machines()
        .iter()
        .flat_map(|machine| program.machine_states(machine))
        .flat_map(|state| {
            program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
        })
        .find_map(|statement| {
            let StatementNode::LocalData(local) = statement else {
                return None;
            };
            (local.name.as_str() == "erased").then_some(local.initial_value)
        })
        .expect("fixture should contain the erased local");
    let ExpressionNode::Borrow(borrow) = program.expression_table.expression(initializer) else {
        panic!("the typed initializer must retain its outer Borrow");
    };
    assert!(matches!(
        program.expression_table.expression(borrow.target),
        ExpressionNode::Cast(_)
    ));

    validate_program(&program)
        .expect("a Borrow(Cast(..)) exact dynamic coercion is not an ordinary LET receiver");
}

#[test]
fn resolves_dynamic_member_call_through_lifetime_applied_local_record() {
    validate(
        r#"
        trait Shape {
            machine code(&self) -> i32;
        }

        data Item {
            value: i32;
        }

        Primary: Item satisfies Shape {
            machine code(&self) -> i32 {
                transition { _ -> self.value }
            }
        }

        data Holder<'item> {
            handler: &'item dyn Shape;
        }

        data Reader {
            item: Item;
        }

        machine Reader::read<'item>(&self) {
            let erased: &'item dyn Shape = &self.item as &dyn Item::Primary;
            let holder: Holder<'item> = Holder { handler: erased };
            let result: i32 = holder.handler.code();
        }
        "#,
    )
    .expect("a lifetime-applied record member must retain its dynamic trait type");
}

#[test]
fn rejects_ordinary_let_bound_receiver() {
    let diagnostics = validate(
        r#"
        data Pair [copy] {
            left: u64;
            right: u64;
        }

        machine Pair::total(&self) -> u64 {
            self.left + self.right
        }

        data Reader {}

        machine Reader::read(&self) -> u64 {
            let pair: Pair = Pair { left: 40, right: 2 };
            let result: u64 = pair.total();
            result
        }
        "#,
    )
    .expect_err("an ordinary LET-bound receiver must remain fail-closed");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains("uses a LET-bound") && diagnostic.message.contains("pair.total")
    }));
}

#[test]
fn records_initializer_and_reassignment_selections_in_statement_order() {
    let program = typed(
        r#"
        trait Shape {
            machine code(&self) -> i32;
        }

        data Item {
            value: i32;
        }

        Primary: Item satisfies Shape {
            machine code(&self) -> i32 {
                transition {
                    _ -> self.value
                }
            }
        }

        data Reader {
            first: Item;
            second: Item;
        }

        machine Reader::read(&self) -> i32 {
            let mut erased: &dyn Shape = &self.first as &dyn Item::Primary;
            erased = &self.second as &dyn Item::Primary;
            let result: i32 = erased.code();
            result
        }
        "#,
    )
    .expect("dynamic selection source should type-check");

    let selections = collect_dynamic_conformance_selections(&program)
        .expect("initializer and reassignment should both select the exact conformance");

    assert_eq!(selections.len(), 2);
    assert_eq!(selections[0].source_name.as_str(), "first");
    assert_eq!(selections[1].source_name.as_str(), "second");
    assert!(selections[0].statement_index < selections[1].statement_index);
    assert_eq!(selections[0].binding, selections[1].binding);
    assert_eq!(selections[0].conformance, selections[1].conformance);
}

#[test]
fn rejects_boundary_trait_as_local_dynamic_descriptor() {
    let diagnostics = validate(
        r#"
        boundary trait Console {
            machine write(value: i32) reaches Console;
        }

        data Reader {}

        machine Reader::read(console: &dyn Console) {}
        "#,
    )
    .expect_err("a replaceable component boundary must not become a local dynamic table");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("local dynamic descriptors cannot cross a replaceable component boundary")
    }));
}

#[test]
fn admits_direct_same_trait_dynamic_parameter_forwarding() {
    validate(
        r#"
        trait Shape {
            machine code(&self) -> i32;
        }

        machine relay(value: &dyn Shape) -> i32 {
            let result: i32 = finish(value);
            transition { _ -> result }
        }

        machine finish(value: &dyn Shape) -> i32 {
            transition { _ -> value.code() }
        }
        "#,
    )
    .expect("an exact bare dynamic parameter may be passed onward unchanged");
}

#[test]
fn rejects_dynamic_parameter_forwarding_to_a_different_trait() {
    let diagnostics = validate(
        r#"
        trait Shape {
            machine code(&self) -> i32;
        }

        trait Other {
            machine code(&self) -> i32;
        }

        machine relay(value: &dyn Shape) -> i32 {
            let result: i32 = finish(value);
            transition { _ -> result }
        }

        machine finish(value: &dyn Other) -> i32 {
            transition { _ -> value.code() }
        }
        "#,
    )
    .expect_err("a parameter cannot change dynamic interfaces while forwarding");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message.contains(
            "without one earlier exact compatible local conformance selection or dynamic parameter",
        )
    }));
}

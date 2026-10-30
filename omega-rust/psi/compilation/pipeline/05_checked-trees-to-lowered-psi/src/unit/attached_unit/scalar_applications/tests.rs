use super::*;
use typed_trees::constant_recipes::{
    ConstantExpressionContext, RetainedConstantRecipe, create_evaluation_entry,
};
use typed_trees::expression::{BinaryOperator, TableBinaryExpression};
use typed_trees::name::Identifier;
use typed_trees::types::TypeReferenceNode;

fn shared_root() -> (
    typed_trees::TypedTrees,
    ConstantRecipeHandle,
    [ConstantExpressionContext; 2],
) {
    let mut program = typed_trees::TypedTrees::default();
    let mut symbols = symbols::SymbolTableBuilder::new();
    let source = symbols.insert_root(
        symbols::SymbolKind::Machine,
        symbols::SymbolNameRef::Static("source"),
    );
    program.symbols = symbols.finish();
    let carrier = program
        .type_reference_table
        .insert(TypeReferenceNode::Named {
            symbol: symbols::SymbolHandle::invalid(),
            name: Identifier::generated("u64"),
        });
    let leaf = program.expression_table.insert(ExpressionNode::Integer(
        numerics::literals::IntegerLiteral::from_parts(
            false,
            numerics::literals::IntegerRadix::Decimal,
            "8",
        )
        .expect("integer"),
    ));
    let root = program
        .expression_table
        .insert(ExpressionNode::Binary(TableBinaryExpression {
            left: leaf,
            operator: BinaryOperator::Add,
            right: leaf,
        }));
    let contexts = ["first", "second"].map(|name| {
        let owner = create_evaluation_entry(&mut program, source, name.into(), root, carrier);
        let owner_symbol = owner.symbol;
        program.push_machine(owner);
        let copied = program.expression_table.insert_copy(root);
        let entry = create_evaluation_entry(
            &mut program,
            owner_symbol,
            format!("{name}-constant"),
            copied,
            carrier,
        );
        let context = ConstantExpressionContext {
            owner: owner_symbol,
            result_type: carrier,
            evaluation_entry: entry.symbol,
            evaluation_expression: copied,
        };
        program.push_machine(entry);
        context
    });
    let contexts_span = program.constant_expression_contexts.insert_many(contexts);
    let recipe = program.constant_recipes.insert(RetainedConstantRecipe {
        position: RetainedConstantPosition::ContractExpression(root),
        contexts: contexts_span,
        ..Default::default()
    });
    (program, recipe, contexts)
}

fn checked_with_scope(program: typed_trees::TypedTrees) -> CheckedTrees {
    let mut facts = checked_trees::CheckFacts::default();
    for machine in program.machines() {
        facts
            .proof_applications
            .machines
            .insert(checked_trees::ProofApplicationMachine {
                machine: machine.symbol,
                product_selected: true,
                ..Default::default()
            });
    }
    CheckedTrees::with_roots(program, facts)
}

#[test]
fn constant_entries_keep_shared_expression_scopes_distinct() {
    let (program, recipe, contexts) = shared_root();
    let checked = checked_with_scope(program);
    let applications = SourceScalarApplications::prepare(&checked).expect("scoped applications");
    let first = applications
        .recipe_in(recipe, contexts[0].owner)
        .expect("first scope");
    let second = applications
        .recipe_in(recipe, contexts[1].owner)
        .expect("second scope");
    assert_ne!(first, second);
    assert!(
        applications.recipe(recipe).is_none(),
        "unscoped selection cannot choose a context"
    );
    assert_eq!(
        applications.evaluation_roots().collect::<Vec<_>>(),
        contexts.map(|context| context.evaluation_entry)
    );
}

#[test]
fn constant_entries_do_not_resurrect_a_pruned_context() {
    let (mut program, recipe, contexts) = shared_root();
    let retained = program
        .machines()
        .iter()
        .filter(|machine| {
            machine.symbol != contexts[1].owner && machine.symbol != contexts[1].evaluation_entry
        })
        .cloned()
        .collect::<Vec<_>>();
    program.roots.machines = arena::HandleSpan::empty();
    for machine in retained {
        program.push_machine(machine);
    }
    let checked = checked_with_scope(program);
    let applications = SourceScalarApplications::prepare(&checked).expect("remaining application");
    assert!(applications.recipe_in(recipe, contexts[0].owner).is_some());
    assert!(applications.recipe_in(recipe, contexts[1].owner).is_none());
    assert_eq!(
        applications.evaluation_roots().collect::<Vec<_>>(),
        [contexts[0].evaluation_entry]
    );
}

//! A copied contract keeps its constant jobs in the copied lexical scope.
//!
//! Expression copying supplies occurrence correspondence; ordinary body cloning
//! supplies symbol correspondence and subsequently substitutes the fresh type
//! payloads. This carries pending work, never a closed value or execution right.

use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::constant_recipes::{
    ConstantExpressionContext, RetainedConstantPosition, RetainedConstantRecipe,
    retain_constant_recipes,
};
use typed_trees::expression::{ExpressionCopy, ExpressionHandle};

pub(super) fn copy_expression(
    source: Option<&dyn validation::CheckingInput>,
    program: &mut TypedTrees,
    expression: ExpressionHandle,
    symbols: &[(SymbolHandle, SymbolHandle)],
) -> Result<ExpressionHandle, Vec<Diagnostic>> {
    let copied = program
        .expression_table
        .copy_graph(source.map(|source| &source.expression_table), expression);
    program
        .expression_table
        .remap_symbols_in(copied.root, symbols);
    copy_contexts(source, program, &copied, symbols)?;
    Ok(copied.root)
}

fn copy_contexts(
    source: Option<&dyn validation::CheckingInput>,
    program: &mut TypedTrees,
    copied: &ExpressionCopy,
    symbols: &[(SymbolHandle, SymbolHandle)],
) -> Result<(), Vec<Diagnostic>> {
    let source_program = source.unwrap_or(program);
    let mut pending = Vec::new();
    for (_, recipe) in source_program.constant_recipes.iter() {
        let RetainedConstantPosition::ContractExpression(root) = recipe.position else {
            continue;
        };
        let Some((_, copied_root)) = copied
            .correspondence
            .iter()
            .find(|(original, _)| *original == root)
        else {
            continue;
        };
        let contexts = source_program
            .constant_expression_contexts
            .span_or_empty(recipe.contexts);
        if contexts.len() != recipe.contexts.len() {
            return Err(vec![Diagnostic::error(
                "copied constant root lost its lexical context span",
            )]);
        }
        let contexts = contexts
            .iter()
            .filter_map(|context| {
                let (_, owner) = symbols
                    .iter()
                    .find(|(original, _)| *original == context.owner)?;
                Some(ConstantExpressionContext {
                    owner: *owner,
                    result_type: context.result_type,
                    ..Default::default()
                })
            })
            .collect::<Vec<_>>();
        if contexts.is_empty() {
            continue;
        }
        let invocations = source_program
            .expression_table
            .expression_handles(recipe.invocations);
        if invocations.len() != recipe.invocations.len() {
            return Err(vec![Diagnostic::error(
                "copied constant root lost its invocation span",
            )]);
        }
        let mut copied_invocations = Vec::with_capacity(invocations.len());
        for invocation in invocations {
            let Some((_, mapped)) = copied
                .correspondence
                .iter()
                .find(|(original, _)| original == invocation)
            else {
                return Err(vec![
                    Diagnostic::error(
                        "copied constant root has an invocation outside its expression graph",
                    )
                    .with_source_span(recipe.source_span),
                ]);
            };
            copied_invocations.push(*mapped);
        }
        let marked = copied
            .correspondence
            .iter()
            .filter(|(original, _)| {
                source_program
                    .pending_const_range_endpoints
                    .contains(original)
            })
            .map(|(_, mapped)| *mapped)
            .collect::<Vec<_>>();
        pending.push((
            recipe.clone(),
            *copied_root,
            contexts,
            copied_invocations,
            marked,
        ));
    }
    for (recipe, root, contexts, invocations, marked) in pending {
        let contexts = contexts
            .into_iter()
            .map(|context| ConstantExpressionContext {
                owner: context.owner,
                result_type: super::copy_type_reference(
                    source,
                    program,
                    context.result_type,
                    symbols,
                ),
                ..Default::default()
            })
            .collect::<Vec<_>>();
        let recipe = RetainedConstantRecipe {
            position: RetainedConstantPosition::ContractExpression(root),
            contexts: program.constant_expression_contexts.insert_many(contexts),
            invocations: program
                .expression_table
                .insert_expression_handles(invocations),
            machine: super::remapped_symbol(recipe.machine, symbols),
            name: recipe.name,
            source_span: recipe.source_span,
        };
        retain_constant_recipes(program, [recipe]);
        program.pending_const_range_endpoints.extend(marked);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use typed_trees::expression::{BinaryOperator, ExpressionNode, TableBinaryExpression};
    use typed_trees::types::TypeReferenceNode;

    #[test]
    fn constant_root_copy_retains_only_the_remapped_scope_and_fresh_type_payloads() {
        let mut program = TypedTrees::default();
        let owner = SymbolHandle::from_parts(17, 1);
        let foreign = SymbolHandle::from_parts(23, 1);
        let instance = SymbolHandle::from_parts(29, 1);
        let carrier = program
            .type_reference_table
            .insert(TypeReferenceNode::Named {
                symbol: SymbolHandle::invalid(),
                name: typed_trees::name::Identifier::generated("u64"),
            });
        let leaf =
            program
                .expression_table
                .insert_tree(&typed_trees::expression::Expression::Call(Box::new(
                    typed_trees::expression::CallExpression {
                        authored_call_selection: None,
                        authored_call_binding: None,
                        exact_coercion_target: Default::default(),
                        requirement_application: Default::default(),
                        receiver: None,
                        target_symbol: SymbolHandle::from_parts(37, 1),
                        target: typed_trees::name::Identifier::generated("extent"),
                        arguments: Default::default(),
                        evidence_arguments: Default::default(),
                        operational_acknowledgement: Default::default(),
                    },
                )));
        let root = program
            .expression_table
            .insert(ExpressionNode::Binary(TableBinaryExpression {
                left: leaf,
                operator: BinaryOperator::Add,
                right: leaf,
            }));
        let contexts = program
            .constant_expression_contexts
            .insert_many([owner, foreign].map(|owner| ConstantExpressionContext {
                owner,
                result_type: carrier,
                ..Default::default()
            }));
        let invocations = program.expression_table.insert_expression_handles([leaf]);
        retain_constant_recipes(
            &mut program,
            [RetainedConstantRecipe {
                position: RetainedConstantPosition::ContractExpression(root),
                contexts,
                invocations,
                ..Default::default()
            }],
        );
        program.pending_const_range_endpoints.extend([root, leaf]);
        let contract = typed_trees::signature::SignatureContract {
            facts: program
                .proof_facts
                .insert_many([typed_trees::domain::ProofFact::Expression(root)]),
            ..Default::default()
        };
        let copied = super::super::copy_signature_contract(
            None,
            &mut program,
            contract,
            &[(owner, instance)],
        )
        .expect("copy contract and retained constant");
        let [typed_trees::domain::ProofFact::Expression(copied_root)] =
            program.proof_facts.span_or_empty(copied.facts)
        else {
            panic!("copied contract root");
        };
        let copied_root = *copied_root;
        assert_ne!(root, copied_root);
        let recipe = program.constant_recipe_for_contract_expression(copied_root);
        let recipe = program.constant_recipes.get(recipe);
        let [context] = program
            .constant_expression_contexts
            .span_or_empty(recipe.contexts)
        else {
            panic!("only the copied owner belongs to this recipe");
        };
        assert_eq!(context.owner, instance);
        assert_ne!(context.result_type, carrier);
        assert_eq!(
            program
                .type_reference_table
                .type_reference(context.result_type),
            program.type_reference_table.type_reference(carrier)
        );
        let [invocation] = program
            .expression_table
            .expression_handles(recipe.invocations)
        else {
            panic!("one copied invocation");
        };
        assert_ne!(*invocation, leaf);
        let ExpressionNode::Binary(binary) = program.expression_table.expression(copied_root)
        else {
            panic!("complete copied root");
        };
        assert_eq!(binary.left, *invocation);
        assert_eq!(binary.right, *invocation);
        assert!(program.pending_const_range_endpoints.contains(&copied_root));
        assert!(program.pending_const_range_endpoints.contains(invocation));
        let original = program.constant_recipe_for_contract_expression(root);
        assert_eq!(
            program
                .constant_expression_contexts
                .span_or_empty(program.constant_recipes.get(original).contexts,)
                .len(),
            2
        );
    }

    #[test]
    fn constant_root_copy_refuses_a_dependency_outside_its_owned_graph() {
        let mut program = TypedTrees::default();
        let owner = SymbolHandle::from_parts(17, 1);
        let instance = SymbolHandle::from_parts(23, 1);
        let root = program.expression_table.insert(ExpressionNode::default());
        let outside = program.expression_table.insert(ExpressionNode::default());
        let contexts =
            program
                .constant_expression_contexts
                .insert_many([ConstantExpressionContext {
                    owner,
                    ..Default::default()
                }]);
        let invocations = program
            .expression_table
            .insert_expression_handles([outside]);
        retain_constant_recipes(
            &mut program,
            [RetainedConstantRecipe {
                position: RetainedConstantPosition::ContractExpression(root),
                contexts,
                invocations,
                ..Default::default()
            }],
        );
        let before = program.constant_recipes.len();
        let errors = copy_expression(None, &mut program, root, &[(owner, instance)])
            .expect_err("an unrelated invocation cannot become copied evidence");
        assert!(
            errors
                .iter()
                .any(|error| error.to_string().contains("outside its expression graph"))
        );
        assert_eq!(program.constant_recipes.len(), before);
    }
}

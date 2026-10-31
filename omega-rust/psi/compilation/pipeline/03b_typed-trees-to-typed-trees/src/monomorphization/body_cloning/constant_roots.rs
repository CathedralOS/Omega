//! Copied contracts and type arguments keep constant jobs in their lexical scope.
//!
//! Expression copying supplies occurrence correspondence; ordinary body cloning
//! supplies symbol correspondence and subsequently substitutes the fresh type
//! payloads. This carries pending work, never a closed value or execution right.

use crate::tables::{expressions_of, expressions_of_mut};
use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::constant_recipes::{
    ConstantExpressionContext, RetainedConstantPosition, RetainedConstantRecipe,
    retain_constant_recipes,
};
use typed_trees::expression::{ExpressionCopy, ExpressionHandle};

/// Staging handles belong to another forest. Rejoin the producer's exact
/// occurrence maps before transferring any source-owned constant context.
pub(super) fn compose_type_copies(
    staged: &[typed_trees::types::TypeExpressionCopy],
    copied: &mut [typed_trees::types::TypeExpressionCopy],
) -> Result<(), super::TypeCopyError> {
    for copied in copied {
        let staged = staged
            .iter()
            .find(|staged| staged.destination == copied.source)
            .ok_or(super::TypeCopyError(
                "type copy lost its staging occurrence",
            ))?;
        copied.source = staged.source;
        for (original, _) in &mut copied.expression.correspondence {
            *original = staged
                .expression
                .correspondence
                .iter()
                .find(|(_, intermediate)| intermediate == original)
                .map(|(source, _)| *source)
                .ok_or(super::TypeCopyError("type copy lost its staged expression"))?;
        }
    }
    Ok(())
}

/// Type copying owns expression correspondence just as contract copying does.
/// Keep the job in the copied scope without carrying an evaluated entry or value.
pub(super) fn copy_type_contexts(
    source: Option<&dyn validation::CheckingInput>,
    program: &mut TypedTrees,
    expressions: &[typed_trees::types::TypeExpressionCopy],
    symbols: &[(SymbolHandle, SymbolHandle)],
) -> Result<(), super::TypeCopyError> {
    let source_program = source.unwrap_or(program);
    let mut pending = Vec::new();
    let mut marked = Vec::new();
    for copied in expressions {
        marked.extend(
            copied
                .expression
                .correspondence
                .iter()
                .filter(|(original, _)| {
                    source_program
                        .pending_const_range_endpoints
                        .contains(original)
                })
                .map(|(_, destination)| *destination),
        );
        for (_, recipe) in source_program.constant_recipes.iter() {
            let RetainedConstantPosition::ConstApplication { site, .. } = recipe.position else {
                continue;
            };
            if site != copied.source {
                continue;
            }
            let contexts = source_program
                .constant_expression_contexts
                .span_or_empty(recipe.contexts);
            if contexts.len() != recipe.contexts.len() {
                return Err(super::TypeCopyError(
                    "copied constant root lost its lexical context span",
                ));
            }
            let contexts = contexts.to_vec();
            let mapped = |original| {
                copied
                    .expression
                    .correspondence
                    .iter()
                    .find(|(source, _)| *source == original)
                    .map(|(_, destination)| *destination)
                    .ok_or(super::TypeCopyError(
                        "copied constant root lost its invocation correspondence",
                    ))
            };
            let invocations = expressions_of(source_program).expression_handles(recipe.invocations);
            if invocations.len() != recipe.invocations.len() {
                return Err(super::TypeCopyError(
                    "copied constant root lost its invocation span",
                ));
            }
            let invocations = invocations
                .iter()
                .map(|invocation| mapped(*invocation))
                .collect::<Result<Vec<_>, _>>()?;
            pending.push((
                recipe.clone(),
                copied.destination,
                copied.expression.root,
                contexts,
                invocations,
            ));
        }
    }
    for (recipe, site, root, contexts, invocations) in pending {
        let contexts = contexts
            .into_iter()
            .map(|context| {
                Ok(ConstantExpressionContext::new(
                    super::remapped_symbol(context.owner, symbols),
                    super::copy_type_reference(source, program, context.result_type, symbols)?,
                ))
            })
            .collect::<Result<Vec<_>, super::TypeCopyError>>()?;
        let recipe = RetainedConstantRecipe {
            position: RetainedConstantPosition::ConstApplication { site, root },
            contexts: program.constant_expression_contexts.insert_many(contexts),
            invocations: expressions_of_mut(program).insert_expression_handles(invocations),
            machine: super::remapped_symbol(recipe.machine, symbols),
            name: recipe.name,
            source_span: recipe.source_span,
        };
        retain_constant_recipes(program, [recipe]);
    }
    program.pending_const_range_endpoints.extend(marked);
    Ok(())
}

pub(super) fn copy_expression(
    source: Option<&dyn validation::CheckingInput>,
    program: &mut TypedTrees,
    expression: ExpressionHandle,
    symbols: &[(SymbolHandle, SymbolHandle)],
) -> Result<ExpressionHandle, Vec<Diagnostic>> {
    let copied = expressions_of_mut(program)
        .copy_graph(source.map(|source| expressions_of(source)), expression);
    expressions_of_mut(program).remap_symbols_in(copied.root, symbols);
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
        let invocations = expressions_of(source_program).expression_handles(recipe.invocations);
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
            .map(|context| {
                Ok(ConstantExpressionContext {
                    owner: context.owner,
                    result_type: super::copy_type_reference(
                        source,
                        program,
                        context.result_type,
                        symbols,
                    )?,
                    ..Default::default()
                })
            })
            .collect::<Result<Vec<_>, super::TypeCopyError>>()?;
        let recipe = RetainedConstantRecipe {
            position: RetainedConstantPosition::ContractExpression(root),
            contexts: program.constant_expression_contexts.insert_many(contexts),
            invocations: expressions_of_mut(program).insert_expression_handles(invocations),
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
    fn constant_type_copy_retains_context_and_pending_work_without_closed_authority() {
        let mut program = TypedTrees::default();
        let owner = SymbolHandle::from_parts(17, 1);
        let instance = SymbolHandle::from_parts(23, 1);
        let root = expressions_of_mut(&mut program).insert(ExpressionNode::default());
        let site = program
            .type_reference_table
            .insert(TypeReferenceNode::ConstExpression(root));
        let carrier = program.type_reference_table.insert(TypeReferenceNode::Unit);
        let contexts =
            program
                .constant_expression_contexts
                .insert_many([ConstantExpressionContext {
                    owner,
                    result_type: carrier,
                    evaluation_entry: SymbolHandle::from_parts(31, 1),
                    ..Default::default()
                }]);
        let invocations = expressions_of_mut(&mut program).insert_expression_handles([root]);
        retain_constant_recipes(
            &mut program,
            [RetainedConstantRecipe {
                position: RetainedConstantPosition::ConstApplication { site, root },
                contexts,
                invocations,
                ..Default::default()
            }],
        );
        program.pending_const_range_endpoints.insert(root);
        let copied =
            super::super::copy_type_reference(None, &mut program, site, &[(owner, instance)])
                .expect("copy the type and its complete constant application");
        assert_ne!(copied, site);
        let recipe = program.constant_recipe_for_const_application(copied);
        assert!(recipe.is_valid());
        assert!(program.closed_constant_recipe_value(recipe).is_none());
        let recipe = program.constant_recipes.get(recipe);
        let copied_root = recipe.position.expression();
        assert_ne!(copied_root, root);
        let [context] = program
            .constant_expression_contexts
            .span_or_empty(recipe.contexts)
        else {
            panic!("one copied context");
        };
        assert_eq!(context.owner, instance);
        assert_ne!(context.result_type, carrier);
        assert!(!context.evaluation_entry.is_valid());
        assert_eq!(
            expressions_of(&program).expression_handles(recipe.invocations),
            &[copied_root]
        );
        assert!(program.pending_const_range_endpoints.contains(&copied_root));
    }

    #[test]
    fn constant_type_copy_keeps_preliminary_marks_before_recipes_exist() {
        let mut program = TypedTrees::default();
        let root = expressions_of_mut(&mut program).insert(ExpressionNode::default());
        let site = program
            .type_reference_table
            .insert(TypeReferenceNode::ConstExpression(root));
        program.pending_const_range_endpoints.insert(root);
        let copied = super::super::copy_type_reference(None, &mut program, site, &[])
            .expect("copy the pending type expression");
        let TypeReferenceNode::ConstExpression(copied_root) =
            program.type_reference_table.type_reference(copied)
        else {
            panic!("copied const argument");
        };
        assert_ne!(*copied_root, root);
        assert!(program.pending_const_range_endpoints.contains(copied_root));
        assert!(
            !program
                .constant_recipe_for_const_application(copied)
                .is_valid()
        );
    }

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
        let leaf = expressions_of_mut(&mut program).insert_tree(
            &typed_trees::expression::Expression::Call(Box::new(
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
            )),
        );
        let root = expressions_of_mut(&mut program).insert(ExpressionNode::Binary(
            TableBinaryExpression {
                left: leaf,
                operator: BinaryOperator::Add,
                right: leaf,
            },
        ));
        let contexts = program
            .constant_expression_contexts
            .insert_many([owner, foreign].map(|owner| ConstantExpressionContext {
                owner,
                result_type: carrier,
                ..Default::default()
            }));
        let invocations = expressions_of_mut(&mut program).insert_expression_handles([leaf]);
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
        let [invocation] = expressions_of(&program).expression_handles(recipe.invocations) else {
            panic!("one copied invocation");
        };
        assert_ne!(*invocation, leaf);
        let ExpressionNode::Binary(binary) = expressions_of(&program).expression(copied_root)
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
        let root = expressions_of_mut(&mut program).insert(ExpressionNode::default());
        let outside = expressions_of_mut(&mut program).insert(ExpressionNode::default());
        let contexts =
            program
                .constant_expression_contexts
                .insert_many([ConstantExpressionContext {
                    owner,
                    ..Default::default()
                }]);
        let invocations = expressions_of_mut(&mut program).insert_expression_handles([outside]);
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

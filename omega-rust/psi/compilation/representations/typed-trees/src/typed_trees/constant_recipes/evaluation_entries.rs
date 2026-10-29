//! Complete expression entries retain their source owner explicitly. Construction
//! creates ordinary typed bodies; checking and execution authority remain with
//! their existing producers. Generated spelling never establishes this relation.

use crate::TypedTrees;
use crate::expression::ExpressionHandle;
use crate::machine::Machine;
use crate::name::Identifier;
use crate::state::State;
use crate::statement::{StatementNode, TableTransition, TransitionTargetNode};
use crate::types::TypeReferenceHandle;
use symbols::{SymbolHandle, SymbolKind};

/// Borrowed ownership projection shared by checking and canonical lowering.
#[derive(Clone, Copy)]
pub struct ConstantEvaluationEntry {
    pub owner: SymbolHandle,
    pub source_expression: ExpressionHandle,
    pub evaluation_expression: ExpressionHandle,
    pub result_type: TypeReferenceHandle,
    pub entry: SymbolHandle,
}

impl TypedTrees {
    pub fn constant_evaluation_entry(
        &self,
        entry: SymbolHandle,
    ) -> Option<ConstantEvaluationEntry> {
        if !entry.is_valid() {
            return None;
        }
        if let Some(declaration) = self.const_declaration_for_evaluation_entry(entry) {
            return Some(ConstantEvaluationEntry {
                owner: declaration.symbol,
                source_expression: declaration.materialized_initializer,
                evaluation_expression: declaration.evaluation_expression,
                result_type: declaration.declared_type,
                entry,
            });
        }
        self.constant_recipes.iter().find_map(|(_, recipe)| {
            let super::RetainedConstantPosition::ContractExpression(source_expression) =
                recipe.position
            else {
                return None;
            };
            self.constant_expression_contexts
                .span_or_empty(recipe.contexts)
                .iter()
                .find(|context| context.evaluation_entry == entry)
                .map(|context| ConstantEvaluationEntry {
                    owner: context.owner,
                    source_expression,
                    evaluation_expression: context.evaluation_expression,
                    result_type: context.result_type,
                    entry,
                })
        })
    }

    /// Resolve a source root in its lexical context. An unscoped consumer may
    /// only use a unique entry; shared expression storage is not shared scope.
    pub fn constant_evaluation_for_expression(
        &self,
        expression: ExpressionHandle,
        owner: SymbolHandle,
    ) -> Result<Option<ConstantEvaluationEntry>, &'static str> {
        let mut found = None;
        for declaration in self.const_declarations() {
            if declaration.materialized_initializer == expression
                && (!owner.is_valid() || owner == declaration.symbol)
                && self.tables.machines.is_valid(declaration.evaluation_entry)
            {
                let entry = self
                    .tables
                    .machines
                    .get(declaration.evaluation_entry)
                    .symbol;
                found = self.constant_evaluation_entry(entry);
            }
        }
        for (_, recipe) in self.constant_recipes.iter() {
            if recipe.position != super::RetainedConstantPosition::ContractExpression(expression) {
                continue;
            }
            for context in self
                .constant_expression_contexts
                .span_or_empty(recipe.contexts)
            {
                if !context.evaluation_entry.is_valid()
                    || self.machine_by_symbol(context.evaluation_entry).is_none()
                    || (owner.is_valid() && context.owner != owner)
                {
                    continue;
                }
                if found.is_some_and(|previous: ConstantEvaluationEntry| {
                    previous.entry != context.evaluation_entry
                }) {
                    return Err(
                        "constant expression has multiple evaluation contexts; its lexical owner is required",
                    );
                }
                found = self.constant_evaluation_entry(context.evaluation_entry);
            }
        }
        Ok(found)
    }
}

/// Construct, but do not root, an ordinary zero-argument expression body. The
/// caller decides whether it belongs to a private forest or checked production.
pub fn create_evaluation_entry(
    program: &mut TypedTrees,
    owner: SymbolHandle,
    name: String,
    expression: ExpressionHandle,
    destination: TypeReferenceHandle,
) -> Machine {
    let symbol = program
        .symbols
        .insert_generated_root_from(owner, SymbolKind::Machine, &name);
    let children = program
        .symbols
        .insert_generated_children(symbol, [(SymbolKind::State, name.as_str())]);
    let target = program
        .statement_table
        .insert_transition_target(TransitionTargetNode::Value(expression));
    let mut state = State {
        symbol: children.start(),
        name: Identifier::generated(name.clone()),
        return_type: destination,
        ..Default::default()
    };
    let source_span = program.expression_table.source_span(expression);
    program.statement_table.push_statement(
        &mut state.statement_nodes,
        StatementNode::Transition(TableTransition {
            target,
            source_span,
            ..Default::default()
        }),
    );
    let mut machine = Machine {
        symbol,
        name: Identifier::generated(name),
        ..Default::default()
    };
    program.push_machine_state(&mut machine, state);
    machine
}

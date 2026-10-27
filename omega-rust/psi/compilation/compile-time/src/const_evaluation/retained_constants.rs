//! Retain deferred source before checking and close its values on private copies.
//!
//! Executable preparation may normalize boundary uses and append expressions.
//! Evaluate the captured continuations only against their authored forest; join
//! recipe values by their recorded identities, then close queries in the actual
//! checked forest. An expression handle in one forest is not a value in another.

use crate::{LayoutQueryProvider, PreCheckEvaluation, SelectedBuildTimeOperators};
use crate::{SelectedBuildTimeTargetBody, SelectedConstantEvaluationRequest};
use diagnostics::Diagnostic;
use std::sync::Arc;
use typed_trees::TypedTrees;
use typed_trees::constant_recipes::ConstantRecipeDependencyReceipt;
use typed_trees::references::DeclarationSelection;

/// Authored storage and the exact work deferred from it. Clones share immutable
/// input; each selected application evaluates an independent private copy.
#[derive(Clone)]
pub struct RetainedConstantEvaluation {
    input: Arc<ConstantEvaluationInput>,
}

struct ConstantEvaluationInput {
    source: TypedTrees,
    pending: Vec<PreCheckEvaluation>,
}

impl RetainedConstantEvaluation {
    /// Capture before executable preparation changes authored occurrences.
    /// No pending work means no retained source allocation.
    pub fn capture(source: &TypedTrees, pending: Vec<PreCheckEvaluation>) -> Option<Self> {
        if pending.is_empty() {
            return None;
        }
        Some(Self {
            input: Arc::new(ConstantEvaluationInput {
                source: source.clone(),
                pending,
            }),
        })
    }

    /// Read-only source for selecting the exact external execution inputs.
    pub fn source(&self) -> &TypedTrees {
        &self.input.source
    }
}

/// One selected value closure. Provider receipts and target geometry originate
/// outside Psi; source evaluation and checked-value publication do not.
pub struct ConstantValueClosureRequest<'request, Layouts> {
    pub retained: &'request RetainedConstantEvaluation,
    pub checked: &'request TypedTrees,
    pub operators: SelectedBuildTimeOperators<'request>,
    pub target_bodies: &'request [SelectedBuildTimeTargetBody],
    pub selection: &'request DeclarationSelection,
    pub layouts: Layouts,
    pub receipts: Vec<ConstantRecipeDependencyReceipt>,
    pub target_scope: &'request str,
}

impl<Layouts: LayoutQueryProvider> ConstantValueClosureRequest<'_, Layouts> {
    pub fn close(self) -> Result<TypedTrees, Vec<Diagnostic>> {
        let mut evaluated = self.retained.input.source.clone();
        SelectedConstantEvaluationRequest {
            program: &mut evaluated,
            pending: self.retained.input.pending.clone(),
            operators: self.operators,
            target_bodies: self.target_bodies,
            selection: self.selection,
            layouts: &self.layouts,
        }
        .evaluate()?;
        let mut closure = self
            .checked
            .read_constant_recipe_closure(&evaluated, self.receipts)?;
        closure.target_bodies = self.target_bodies.iter().map(|row| row.receipt()).collect();
        closure.target_scope = self.target_scope.to_owned();
        let mut closed = self.checked.clone().with_constant_recipe_closure(closure);
        closed.plan_laid_layouts = std::mem::take(&mut evaluated.plan_laid_layouts);
        closed.plan_laid_target_layouts = std::mem::take(&mut evaluated.plan_laid_target_layouts);
        closed.plan_laid_schema_fields = std::mem::take(&mut evaluated.plan_laid_schema_fields);
        let has_queries = typed_trees::references::collect_program_references(self.checked)
            .expressions
            .into_iter()
            .any(|expression| {
                matches!(
                    self.checked.expression_table.expression(expression),
                    typed_trees::expression::ExpressionNode::Call(call) if call.layout_query.is_some()
                )
            });
        if has_queries {
            // Preparation can append specialized queries. Close those actual
            // checked subjects, never read their handles in the authored forest.
            let selection = DeclarationSelection::from_program(&closed);
            super::data_layout_queries::fold_data_layout_queries(
                &mut closed,
                false,
                &selection,
                &self.layouts,
            )?;
            closed
                .tables
                .const_declarations
                .for_each_mut(|_, declaration| {
                    if declaration.evaluated_value.is_none() {
                        declaration.evaluated_value = evaluated
                            .const_declaration_by_symbol(declaration.symbol)
                            .and_then(|value| value.evaluated_value.clone());
                    }
                });
        }
        Ok(closed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LayoutQueryMeasurements;
    use typed_trees::expression::{DataLayoutQueryRequest, ExpressionHandle, ExpressionNode};

    fn deferred_query() -> (TypedTrees, PreCheckEvaluation, ExpressionHandle) {
        let tokens =
            source_files_to_tokens::Lexer::new("machine query() -> u64 { size_of<u64>() }")
                .tokenize()
                .expect("query tokens");
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("query syntax");
        let (syntax, pending) = crate::evaluate_pre_resolution(crate::BuildTimeEvaluationRequest {
            syntax_trees: syntax,
            source_context: Default::default(),
        })
        .expect("query elaboration")
        .into_syntax_and_pre_check();
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(syntax),
        )
        .expect("query resolution");
        let program = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
            .expect("query typing");
        let query = program
            .expression_table
            .iter_expressions()
            .find_map(|(handle, node)| {
                matches!(node, ExpressionNode::Call(call) if call.layout_query.is_some())
                    .then_some(handle)
            })
            .expect("query occurrence");
        (program, pending, query)
    }

    #[derive(Clone, Copy)]
    struct Measurements(Option<usize>);

    impl LayoutQueryProvider for Measurements {
        type Measurements = Self;

        fn prepare(&self, _: &TypedTrees) -> Self {
            *self
        }
    }

    impl LayoutQueryMeasurements for Measurements {
        fn measure(
            &self,
            _: &TypedTrees,
            _: &DataLayoutQueryRequest,
        ) -> Result<Option<usize>, Diagnostic> {
            Ok(self.0)
        }
    }

    fn close(
        retained: &RetainedConstantEvaluation,
        checked: &TypedTrees,
        size: Option<usize>,
    ) -> Result<TypedTrees, Vec<Diagnostic>> {
        let selection = DeclarationSelection::from_program(retained.source());
        ConstantValueClosureRequest {
            retained,
            checked,
            operators: SelectedBuildTimeOperators::default(),
            target_bodies: &[],
            selection: &selection,
            layouts: Measurements(size),
            receipts: Vec::new(),
            target_scope: "test",
        }
        .close()
    }

    #[test]
    fn no_pending_work_retains_no_source() {
        assert!(RetainedConstantEvaluation::capture(&TypedTrees::default(), Vec::new()).is_none());
    }

    #[test]
    fn target_closures_share_input_but_never_mutate_it_or_each_other() {
        let (checked, pending, query) = deferred_query();
        let retained =
            RetainedConstantEvaluation::capture(&checked, vec![pending]).expect("pending work");
        let sibling = retained.clone();
        assert!(Arc::ptr_eq(&retained.input, &sibling.input));
        let first = close(&retained, &checked, Some(8)).expect("first closure");
        let second = close(&sibling, &checked, Some(16)).expect("second closure");
        for (program, expected) in [(&first, "8"), (&second, "16")] {
            let ExpressionNode::Integer(value) = program.expression_table.expression(query) else {
                panic!("closed query must be a literal");
            };
            assert_eq!(value.to_string(), expected);
        }
        assert_eq!(retained.source(), &checked);
        assert!(matches!(
            checked.expression_table.expression(query),
            ExpressionNode::Call(_)
        ));
    }

    #[test]
    fn failed_closure_preserves_source_for_another_application() {
        let (checked, pending, _) = deferred_query();
        let retained =
            RetainedConstantEvaluation::capture(&checked, vec![pending]).expect("pending work");
        let errors = close(&retained, &checked, None).expect_err("unresolved query refuses");
        assert!(format!("{errors:?}").contains("unresolved array extent"));
        assert_eq!(retained.source(), &checked);
        close(&retained, &checked, Some(8)).expect("independent application still closes");
    }
}

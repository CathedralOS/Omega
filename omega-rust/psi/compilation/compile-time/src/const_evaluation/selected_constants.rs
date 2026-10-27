//! Resume one selected constant application, including its layout dependencies.
//!
//! The caller supplies exact checked provider/target-body selections and a
//! read-only geometry provider. Psi owns source mutation, continuation order
//! and initializer completion. Layout plans installed by evaluation must be
//! visible to the second query pass, never a reused first-pass view.

use crate::{
    LayoutQueryProvider, PreCheckEvaluation, SelectedBuildTimeOperators,
    SelectedBuildTimeTargetBody,
};
use diagnostics::Diagnostic;
use typed_trees::TypedTrees;
use typed_trees::references::DeclarationSelection;

/// One continuation roster and its exact supplied execution and layout inputs.
pub struct SelectedConstantEvaluationRequest<'request, Layouts> {
    pub program: &'request mut TypedTrees,
    pub pending: Vec<PreCheckEvaluation>,
    pub operators: SelectedBuildTimeOperators<'request>,
    pub target_bodies: &'request [SelectedBuildTimeTargetBody],
    pub selection: &'request DeclarationSelection,
    pub layouts: Layouts,
}

impl<Layouts: LayoutQueryProvider> SelectedConstantEvaluationRequest<'_, Layouts> {
    /// Resolve available geometry, evaluate plans and constants, then close
    /// remaining queries before finishing each initializer in original order.
    pub fn evaluate(self) -> Result<(), Vec<Diagnostic>> {
        crate::prepare_selected_target_family_calls(self.program, self.target_bodies)
            .map_err(|reason| vec![Diagnostic::error(reason)])?;
        super::data_layout_queries::fold_data_layout_queries(
            self.program,
            true,
            self.selection,
            &self.layouts,
        )?;
        let mut evaluated = Vec::with_capacity(self.pending.len());
        for continuation in self.pending {
            evaluated.push(continuation.evaluate_selected_operators(self.program, self.operators)?);
        }
        super::data_layout_queries::fold_data_layout_queries(
            self.program,
            false,
            self.selection,
            &self.layouts,
        )?;
        for continuation in evaluated {
            continuation.finish(self.program)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LayoutQueryMeasurements;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use typed_trees::expression::{DataLayoutQueryRequest, ExpressionNode};

    fn query_program() -> TypedTrees {
        let tokens =
            source_files_to_tokens::Lexer::new("machine query() -> u64 { size_of<u64>() }")
                .tokenize()
                .expect("query tokens");
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("query syntax");
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(syntax),
        )
        .expect("query resolution");
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
            .expect("query typing")
    }

    struct DeferredMeasurements {
        preparations: AtomicUsize,
        resolves: bool,
    }

    struct Measurement(Option<usize>);

    impl LayoutQueryProvider for &DeferredMeasurements {
        type Measurements = Measurement;

        fn prepare(&self, _: &TypedTrees) -> Measurement {
            let pass = self.preparations.fetch_add(1, Ordering::Relaxed);
            Measurement((self.resolves && pass > 0).then_some(8))
        }
    }

    impl LayoutQueryMeasurements for Measurement {
        fn measure(
            &self,
            _: &TypedTrees,
            _: &DataLayoutQueryRequest,
        ) -> Result<Option<usize>, Diagnostic> {
            Ok(self.0)
        }
    }

    #[test]
    fn deferred_query_uses_a_fresh_measurement_view() {
        let mut program = query_program();
        let query = program
            .expression_table
            .iter_expressions()
            .find_map(|(handle, node)| {
                matches!(node, ExpressionNode::Call(call) if call.layout_query.is_some())
                    .then_some(handle)
            })
            .expect("retained layout query");
        let selection = DeclarationSelection::from_program(&program);
        let layouts = DeferredMeasurements {
            preparations: AtomicUsize::new(0),
            resolves: true,
        };
        SelectedConstantEvaluationRequest {
            program: &mut program,
            pending: Vec::new(),
            operators: SelectedBuildTimeOperators::default(),
            target_bodies: &[],
            selection: &selection,
            layouts: &layouts,
        }
        .evaluate()
        .expect("second pass closes the query");
        assert_eq!(layouts.preparations.load(Ordering::Relaxed), 2);
        let ExpressionNode::Integer(value) = program.expression_table.expression(query) else {
            panic!("closed query must become its measured literal");
        };
        assert_eq!(value.to_string(), "8");
    }

    #[test]
    fn unresolved_second_pass_is_not_a_zero_sized_layout() {
        let mut program = query_program();
        let selection = DeclarationSelection::from_program(&program);
        let layouts = DeferredMeasurements {
            preparations: AtomicUsize::new(0),
            resolves: false,
        };
        let errors = SelectedConstantEvaluationRequest {
            program: &mut program,
            pending: Vec::new(),
            operators: SelectedBuildTimeOperators::default(),
            target_bodies: &[],
            selection: &selection,
            layouts: &layouts,
        }
        .evaluate()
        .expect_err("unresolved geometry must refuse");
        assert_eq!(layouts.preparations.load(Ordering::Relaxed), 2);
        assert!(format!("{errors:?}").contains("unresolved array extent"));
        assert!(
            program
                .expression_table
                .iter_expressions()
                .any(|(_, node)| {
                    matches!(node, ExpressionNode::Call(call) if call.layout_query.is_some())
                })
        );
    }
}

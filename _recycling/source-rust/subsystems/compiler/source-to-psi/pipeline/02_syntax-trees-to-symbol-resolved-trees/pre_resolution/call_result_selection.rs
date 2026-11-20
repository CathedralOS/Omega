//! Historical fixture-only call selection. Its callers now use normal body completion.

/*
    Select call-result inputs without materializing unrelated runtime expressions.
    Local headers establish the preceding-local scope; selected call roots retain
    their receivers, arguments and machine owner through ordinary final routing.
    This one-shot preparation product grants no body typing or execution authority.
    Receipt-required helper bodies still complete through normal settlement.
*/
pub fn resolve_call_result_selection(
    request: ResolutionRequest,
) -> Result<SymbolResolvedTrees, Vec<Diagnostic>> {
    let (syntax, mut lowerer) = begin(request, ConstResolutionMode::ArgumentSelection)?;
    index_declarations(&mut lowerer, &syntax)?;
    let inputs = assign_lexical_inputs(&mut lowerer, &syntax, LexicalInputDemand::CallResults)?;
    finish_resolution(&mut lowerer, &syntax, &inputs, &[])?;
    Ok(lowerer.into_trees())
}

    /*
        Call-result input inventory. A selected call retains its whole receiver
        and argument subtree through ordinary lowering, so nested calls are not
        independently materialized a second time. A lexical match capture is also
        indivisible: extracting an arm call would detach its saved-subject reads
        from the definition required to resolve them. Retain the enclosing match
        when it contains calls, without installing it as an executable body.
    */
    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn statement_call_inputs(
        syntax: &SyntaxTrees,
        statement: syntax_trees::statement::StatementHandle,
    ) -> Vec<ExpressionHandle> {
        struct Calls<'a> {
            syntax: &'a SyntaxTrees,
            handles: Vec<ExpressionHandle>,
        }
        impl ExpressionMarks for Calls<'_> {
            fn mark(&mut self, expression: ExpressionHandle) -> bool {
                let retain = match self.syntax.expressions.expression(expression) {
                    ExpressionNode::Call(_) => true,
                    ExpressionNode::Match(dispatch) if dispatch.capture.is_valid() => {
                        struct CallSearch<'a> {
                            syntax: &'a SyntaxTrees,
                            found: bool,
                        }
                        impl ExpressionMarks for CallSearch<'_> {
                            fn mark(&mut self, expression: ExpressionHandle) -> bool {
                                self.found |= matches!(
                                    self.syntax.expressions.expression(expression),
                                    ExpressionNode::Call(_)
                                );
                                !self.found
                            }
                        }
                        let mut search = CallSearch {
                            syntax: self.syntax,
                            found: false,
                        };
                        collect_expression_handles(self.syntax, expression, &mut search);
                        search.found
                    }
                    _ => false,
                };
                if retain {
                    self.handles.push(expression);
                    false
                } else {
                    true
                }
            }
        }
        let mut calls = Calls {
            syntax,
            handles: Vec::new(),
        };
        collect_statement_expression_handles(syntax, statement, &mut calls);
        calls.handles
    }

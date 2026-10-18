//! Executed, non-callable product requirement relationships. These declarations
//! authorize only runtime installation; they grant Build no proposition or callable.
//! Omega owns compatibility, selected implementation and receiving risk acceptance.
use checked_trees::typed_trees::TypedTrees;
use diagnostics::Diagnostic;
use language_semantics::declaration_selection::BuildOperation;
use symbols::SymbolHandle;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssumedBindingContract {
    pub required: SymbolHandle,
    pub supplied: SymbolHandle,
    pub selecting_machine: SymbolHandle,
    pub source_span: source::SourceSpan,
}

pub fn collect_executed_assumed_bindings(
    typed: &TypedTrees,
    root: SymbolHandle,
    executed: &[checked_interpreter::ExecutedBuildSelection],
) -> Result<Vec<AssumedBindingContract>, Vec<Diagnostic>> {
    let mut result = Vec::new();
    for selection in executed
        .iter()
        .filter(|row| row.operation == BuildOperation::AssumedBindingContract)
    {
        let span = typed.expression_table.source_span(selection.expression);
        let reject = |reason: &str| {
            vec![
                Diagnostic::error(format!("assumed binding contract {reason}"))
                    .with_source_span(span),
            ]
        };
        if typed.symbols.symbol_package_identity(root)
            != typed.symbols.symbol_package_identity(selection.machine)
        {
            return Err(reject(
                "may be authorized only by the consuming root, not a dependency helper",
            ));
        }
        let call = typed
            .expression_table
            .call_node(selection.expression)
            .ok_or_else(|| reject("lost its exact executed declaration"))?;
        if call.target_symbol.is_valid()
            || BuildOperation::from_call_target(call.target.as_str())
                != Some(BuildOperation::AssumedBindingContract)
            || call.machine_arguments.len() != 2
            || typed
                .expression_table
                .expression_handles(call.arguments)
                .len()
                != 1
            || !call.evidence_arguments.is_empty()
        {
            return Err(reject("does not rejoin its typed Build operation"));
        }
        let required = typed_trees::product_operand::resolve_requirement_operand(
            typed,
            &call.machine_arguments[0],
            span,
        )
        .ok_or_else(|| {
            reject("required operand must name an exact visible boundary requirement")
        })?;
        let supplied = typed_trees::product_operand::resolve_requirement_operand(
            typed,
            &call.machine_arguments[1],
            span,
        )
        .ok_or_else(|| {
            reject("supplied operand must name an exact visible boundary requirement")
        })?;
        result.push(AssumedBindingContract {
            required,
            supplied,
            selecting_machine: root,
            source_span: span,
        });
    }
    Ok(result)
}

//! Normalize exact toolchain query calls at their authored lexical occurrence.
//! A shared wrapper body would instead give every query the wrapper's scope.

use diagnostics::Diagnostic;
use symbol_resolved_trees::SymbolResolvedTrees;
use symbols::SymbolHandle;
use typed_trees::expression::{
    ExpressionHandle, ExpressionNode, TableCallExpression, TableNamePath,
};

pub(crate) struct ProductQueryOperation {
    pub namespace: SymbolHandle,
    pub target: SymbolHandle,
}

pub(crate) fn operation(
    source: &SymbolResolvedTrees,
    namespace: SymbolHandle,
    target: SymbolHandle,
    name: &str,
) -> Option<ProductQueryOperation> {
    if !source.symbols.is_build_prelude_symbol(namespace)
        || source.symbols.name(namespace) != "BuildProduct"
        || name != "entry"
        || (target.is_valid() && !source.symbols.is_build_prelude_symbol(target))
    {
        return None;
    }
    let protocol = source.traits.iter().find(|definition| {
        definition.is_boundary
            && language_semantics::build_requests::is_build_protocol(
                &source.symbols,
                definition.symbol,
                "BuildProductOperations",
            )
    })?;
    source
        .trait_machine_signatures(protocol.machines)
        .iter()
        .find(|signature| signature.name.as_str() == name)
        .map(|signature| ProductQueryOperation {
            namespace: protocol.symbol,
            target: signature.symbol,
        })
}

pub(crate) fn lower(
    source: &SymbolResolvedTrees,
    target: &mut typed_trees::TypedTrees,
    operation: ProductQueryOperation,
    arguments: Vec<ExpressionHandle>,
    span: source::SourceSpan,
    acknowledgement: language_semantics::CallOperationalAcknowledgement,
) -> Result<ExpressionHandle, Diagnostic> {
    let protocol = source
        .traits
        .iter()
        .find(|definition| definition.symbol == operation.namespace)
        .ok_or_else(|| {
            Diagnostic::error("Product query lost its toolchain protocol").with_source_span(span)
        })?;
    let signature = source
        .trait_machine_signatures(protocol.machines)
        .iter()
        .find(|signature| signature.symbol == operation.target)
        .ok_or_else(|| {
            Diagnostic::error("Product query lost its exact operation").with_source_span(span)
        })?;
    let table = &mut target.expression_table;
    let mut members = Default::default();
    let mut symbols = Default::default();
    table.push_name_path_member(
        &mut members,
        crate::lowerer::name::lower_name(&protocol.name),
    );
    table.push_name_path_member_symbol(&mut symbols, protocol.symbol);
    let receiver = table.insert(ExpressionNode::Name(TableNamePath::new(
        members,
        symbols,
        protocol.symbol,
        protocol.symbol,
    )));
    let arguments = table.insert_expression_handles(arguments);
    let expression = table.insert(ExpressionNode::Call(TableCallExpression {
        authored_machine_application: Default::default(),
        exact_coercion_target: Default::default(),
        requirement_application: Default::default(),
        receiver,
        target_symbol: operation.target,
        static_machine_parameter: Default::default(),
        target: crate::lowerer::name::lower_name(&signature.name),
        static_requirement_dispatch: None,
        machine_arguments: Box::default(),
        quotient_operation: None,
        private_layout_operation: None,
        layout_query: None,
        native_code_layout: None,
        build_selection: Default::default(),
        arguments,
        evidence_arguments: Box::default(),
        operational_acknowledgement: acknowledgement,
    }));
    table.set_source_span(expression, span);
    Ok(expression)
}

//! Designated product operands become inert call context, not executable AST.
//!
//! The receiver and an optional issued description are ordinary call arguments.
//! Checking and Terminal lowering therefore use the same boundary-call route as
//! other host operations. Only the compiler-owned declaration can carry this
//! context; a similarly named source requirement grants no Build authority.

use crate::lowerer::Lowerer;
use diagnostics::Diagnostic;
use language_semantics::declaration_selection::{
    BuildSelectionRequest, RootBindingOperand, RootBindingRequest,
};
use typed_trees::expression::{ExpressionNode, TableCallExpression, TableNamePath};

pub(super) fn lower(
    lowerer: &mut Lowerer,
    binding: &symbol_resolved_trees::statement::RootBinding,
) -> Result<typed_trees::statement::StatementNode, Diagnostic> {
    let described = binding.implementation_operand.is_valid();
    let operation = if described {
        "bind_description"
    } else {
        "bind"
    };
    let source = lowerer.source_trees;
    let owner = source
        .traits
        .iter()
        .filter(|definition| {
            definition.is_boundary && definition.name.as_str() == "BuildBindingOperations"
        })
        .find(|definition| {
            source
                .symbols
                .symbol_source_span(definition.symbol)
                .and_then(|span| source.symbols.source_file(span))
                .is_some_and(|file| {
                    file.origin == source::SourceOrigin::Toolchain
                        && file.path == std::path::Path::new("<build-prelude>")
                })
        })
        .ok_or_else(|| {
            Diagnostic::error("root binding requires a compiler-issued &mut Build receiver")
                .with_source_span(binding.source_span)
        })?;
    let find_operation = |name| {
        source
            .trait_machine_signatures(owner.machines)
            .iter()
            .find(|signature| signature.name.as_str() == name)
    };
    let requirement = find_operation(operation)
        .ok_or_else(|| Diagnostic::error("root binding protocol has no commit operation"))?;
    let target_symbol = requirement.symbol;
    let target = crate::lowerer::name::lower_name(&requirement.name);
    let preparation = find_operation("prepare")
        .ok_or_else(|| Diagnostic::error("root binding protocol has no receiver preparation"))?;
    let preparation_symbol = preparation.symbol;
    let preparation_name = crate::lowerer::name::lower_name(&preparation.name);
    let owner_symbol = owner.symbol;
    let owner_name = crate::lowerer::name::lower_name(&owner.name);
    let mut arguments = vec![super::arguments::lower_statement_expression(
        lowerer,
        binding.receiver,
    )?];
    let implementation = if described {
        arguments.push(super::arguments::lower_statement_expression(
            lowerer,
            binding.implementation_operand,
        )?);
        RootBindingOperand::Description
    } else {
        RootBindingOperand::AuthoredPath(product_path(&binding.implementation))
    };
    let table = &mut lowerer.typed_trees.expression_table;
    let mut members = Default::default();
    let mut member_symbols = Default::default();
    table.push_name_path_member(&mut members, owner_name);
    table.push_name_path_member_symbol(&mut member_symbols, owner_symbol);
    let receiver = table.insert(ExpressionNode::Name(TableNamePath::new(
        members,
        member_symbols,
        owner_symbol,
        owner_symbol,
    )));
    let build_selection =
        table.insert_build_selection_request(BuildSelectionRequest::Entry(RootBindingRequest {
            source_span: binding.source_span,
            slot: product_path(&binding.slot),
            implementation,
        }));
    // Receiver evaluation and authentication precede description evaluation,
    // but do not hold a mutable loan across it. The private scalar ticket is
    // the completed result of an ordinary nested call, not a source permission.
    let preparation_arguments = table.insert_expression_handles([arguments[0]]);
    let preparation = table.insert(ExpressionNode::Call(TableCallExpression {
        authored_machine_application: Default::default(),
        exact_coercion_target: Default::default(),
        requirement_application: Default::default(),
        receiver,
        target_symbol: preparation_symbol,
        static_machine_parameter: Default::default(),
        target: preparation_name,
        static_requirement_dispatch: None,
        machine_arguments: Box::default(),
        quotient_operation: None,
        private_layout_operation: None,
        layout_query: None,
        native_code_layout: None,
        build_selection,
        arguments: preparation_arguments,
        evidence_arguments: Box::default(),
        operational_acknowledgement: Default::default(),
    }));
    table.set_source_span(preparation, binding.source_span);
    arguments[0] = preparation;
    let arguments = table.insert_expression_handles(arguments);
    let expression = table.insert(ExpressionNode::Call(TableCallExpression {
        authored_machine_application: Default::default(),
        exact_coercion_target: Default::default(),
        requirement_application: Default::default(),
        receiver,
        target_symbol,
        static_machine_parameter: Default::default(),
        target,
        static_requirement_dispatch: None,
        machine_arguments: Box::default(),
        quotient_operation: None,
        private_layout_operation: None,
        layout_query: None,
        native_code_layout: None,
        build_selection,
        arguments,
        evidence_arguments: Box::default(),
        operational_acknowledgement: Default::default(),
    }));
    table.set_source_span(expression, binding.source_span);
    Ok(typed_trees::statement::StatementNode::Expression(
        expression,
    ))
}

fn product_path(members: &[symbol_resolved_trees::name::DiagnosticName]) -> String {
    let mut path = String::new();
    for member in members {
        if !path.is_empty() {
            path.push_str("::");
        }
        path.push_str(member.as_str());
    }
    path
}

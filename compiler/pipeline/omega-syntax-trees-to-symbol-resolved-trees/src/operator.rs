use crate::data::lower_type_parameters;
use crate::lowerer::Lowerer;
use crate::state::{lower_signature_contracts, lower_state_parameters};
use crate::type_reference::lower_type_reference_handle;
use omega_core::arena::HandleSpan;
use omega_core::diagnostics::Diagnostic;
use omega_syntax_trees::{self as syntax, SyntaxTrees};

pub(crate) fn lower_operator_definition(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    operator: &syntax::item::OperatorDefinition,
) -> Result<omega_symbol_resolved_trees::operator::OperatorDefinition, Diagnostic> {
    Ok(omega_symbol_resolved_trees::operator::OperatorDefinition {
        is_boundary: operator.is_boundary,
        symbol: Default::default(),
        name: lower_operator_name(lowerer, syntax_trees, operator.name),
        type_parameters: lower_type_parameters(lowerer, syntax_trees, operator.type_parameters),
        parameters: lower_state_parameters(lowerer, syntax_trees, operator.parameters)?,
        return_type: operator
            .return_type
            .is_valid()
            .then(|| lower_type_reference_handle(lowerer, syntax_trees, operator.return_type))
            .transpose()?,
        contracts: lower_signature_contracts(lowerer, syntax_trees, operator.contracts)?,
        token_count: operator.token_count,
    })
}

fn lower_operator_name(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    name: HandleSpan<syntax::identifier::Identifier>,
) -> HandleSpan<omega_symbol_resolved_trees::name::DiagnosticName> {
    let mut span = HandleSpan::empty();

    for member in syntax_trees.items.identifier_path_members(name) {
        lowerer
            .symbol_resolved_trees
            .tables
            .declarations
            .operator_path_members
            .append_to_span(&mut span, crate::name::lower_name(member));
    }

    span
}

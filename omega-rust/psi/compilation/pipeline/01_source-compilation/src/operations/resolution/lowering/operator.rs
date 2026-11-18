/*
    Operator definitions. Domain homes are selected later by
    `selections::domain_operator_homes`.
*/

use crate::resolution::lowering::data::lower_type_parameters;
use crate::resolution::lowering::lowerer::Lowerer;
use crate::resolution::lowering::name::lower_names;
use crate::resolution::lowering::name::lower_operator_path_name;
use crate::resolution::lowering::state::{lower_signature_contracts, lower_state_parameters};
use crate::resolution::lowering::type_reference::lower_type_reference_handle;
use diagnostics::Diagnostic;
use syntax_trees::{self as syntax, SyntaxTrees};

pub(crate) fn lower_operator_definition(
    lowerer: &mut Lowerer,
    syntax_trees: &SyntaxTrees,
    operator: &syntax::item::OperatorDefinition,
) -> Result<crate::source_resolved::operator::OperatorDefinition, Diagnostic> {
    Ok(crate::source_resolved::operator::OperatorDefinition {
        is_public: operator.is_public,
        is_boundary: operator.is_boundary,
        symbol: Default::default(),
        name: lower_operator_path_name(lowerer, syntax_trees, operator.name),
        lifetime_parameters: lower_names(syntax_trees, &operator.lifetime_parameters),
        type_parameters: lower_type_parameters(lowerer, syntax_trees, operator.type_parameters)?,
        parameters: lower_state_parameters(lowerer, syntax_trees, operator.parameters)?,
        return_type: operator
            .return_type
            .is_valid()
            .then(|| lower_type_reference_handle(lowerer, syntax_trees, operator.return_type))
            .transpose()?,
        contracts: lower_signature_contracts(lowerer, syntax_trees, operator.contracts)?,
        spelling: operator.spelling,
        token_count: operator.token_count,
    })
}

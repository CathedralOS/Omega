use crate::lowerer::Lowerer;
use crate::signatures::type_parameters::lower_type_parameters;
use crate::type_reference::lower_type_reference_into_table;
use diagnostics::Diagnostic;

pub(crate) fn lower_operator_definition(
    lowerer: &mut Lowerer,
    operator: &symbol_resolved_trees::operator::OperatorDefinition,
) -> Result<typed_trees::operator::OperatorDefinition, Diagnostic> {
    let mut typed_operator = typed_trees::operator::OperatorDefinition {
        is_public: operator.is_public,
        is_boundary: operator.is_boundary,
        symbol: operator.symbol,
        name: Default::default(),
        lifetime_parameters: operator
            .lifetime_parameters
            .iter()
            .map(crate::lowerer::name::lower_name)
            .collect(),
        type_parameters: Default::default(),
        parameters: Default::default(),
        return_type: operator
            .return_type
            .as_ref()
            .map(|type_reference| lower_type_reference_into_table(lowerer, type_reference))
            .transpose()?
            .unwrap_or_else(typed_trees::types::TypeReferenceHandle::invalid),
        contracts: Default::default(),
        spelling: operator.spelling,
        token_count: operator.token_count,
        home_domain: symbols::SymbolHandle::invalid(),
    };

    for member in lowerer.source_trees.operator_path_members(operator.name) {
        lowerer.typed_trees.push_operator_path_member(
            &mut typed_operator,
            crate::lowerer::name::lower_name(member),
        );
    }

    typed_operator.type_parameters = lower_type_parameters(lowerer, operator.type_parameters)?;

    for parameter in lowerer.source_trees.state_parameters(operator.parameters) {
        let type_reference = lower_type_reference_into_table(lowerer, &parameter.type_reference)?;
        lowerer.typed_trees.push_operator_parameter(
            &mut typed_operator,
            typed_trees::signature::StateParameter {
                symbol: parameter.symbol,
                name: crate::lowerer::name::lower_name(&parameter.name),
                type_reference,
                is_const: parameter.is_const,
                is_mutable: parameter.is_mutable,
                relevance: parameter.relevance,
            },
        );
    }

    for contract in lowerer.source_trees.signature_contracts(operator.contracts) {
        let contract =
            crate::contracts::proof_facts::lower_signature_contract(lowerer, contract)?;
        lowerer.typed_trees.push_operator_contract(&mut typed_operator, contract);
    }

    Ok(typed_operator)
}

use crate::data::lower_type_parameter_kind;
use crate::domain::lower_proof_facts;
use crate::lowerer::Lowerer;
use crate::type_reference::lower_type_reference_into_table;
use omega_core::diagnostics::Diagnostic;

pub(crate) fn lower_operator_definition(
    lowerer: &mut Lowerer,
    operator: &omega_symbol_resolved_trees::operator::OperatorDefinition,
) -> Result<omega_typed_trees::operator::OperatorDefinition, Diagnostic> {
    let mut typed_operator = omega_typed_trees::operator::OperatorDefinition {
        is_boundary: operator.is_boundary,
        symbol: operator.symbol,
        name: Default::default(),
        type_parameters: Default::default(),
        parameters: Default::default(),
        return_type: operator
            .return_type
            .as_ref()
            .map(|type_reference| lower_type_reference_into_table(lowerer, type_reference))
            .transpose()?
            .unwrap_or_else(omega_typed_trees::types::TypeReferenceHandle::invalid),
        contracts: Default::default(),
        spelling: operator.spelling,
        token_count: operator.token_count,
    };

    for member in lowerer.source_trees.operator_path_members(operator.name) {
        lowerer
            .typed_trees
            .push_operator_path_member(&mut typed_operator, crate::name::lower_name(member));
    }

    for parameter in lowerer
        .source_trees
        .data_type_parameters(operator.type_parameters)
    {
        let kind = lower_type_parameter_kind(lowerer, &parameter.kind)?;
        lowerer.typed_trees.push_operator_type_parameter(
            &mut typed_operator,
            omega_typed_trees::data::TypeParameter {
                symbol: parameter.symbol,
                name: crate::name::lower_name(&parameter.name),
                kind,
            },
        );
    }

    for parameter in lowerer.source_trees.state_parameters(operator.parameters) {
        let type_reference = lower_type_reference_into_table(lowerer, &parameter.type_reference)?;
        lowerer.typed_trees.push_operator_parameter(
            &mut typed_operator,
            omega_typed_trees::signature::StateParameter {
                symbol: parameter.symbol,
                name: crate::name::lower_name(&parameter.name),
                type_reference,
                is_const: parameter.is_const,
                is_mutable: parameter.is_mutable,
                is_self: parameter.is_self,
            },
        );
    }

    for contract in lowerer.source_trees.signature_contracts(operator.contracts) {
        let facts = lower_proof_facts(lowerer, contract.facts)?;
        lowerer.typed_trees.push_operator_contract(
            &mut typed_operator,
            omega_typed_trees::signature::SignatureContract {
                kind: match contract.kind {
                    omega_symbol_resolved_trees::signature::SignatureContractKind::Requires => {
                        omega_typed_trees::signature::SignatureContractKind::Requires
                    }
                    omega_symbol_resolved_trees::signature::SignatureContractKind::Ensures => {
                        omega_typed_trees::signature::SignatureContractKind::Ensures
                    }
                    omega_symbol_resolved_trees::signature::SignatureContractKind::Boundary => {
                        omega_typed_trees::signature::SignatureContractKind::Boundary
                    }
                },
                facts,
                token_count: contract.token_count,
            },
        );
    }

    Ok(typed_operator)
}

use crate::domain::lower_proof_facts;
use crate::program::Lowerer;
use crate::statement::lower_statement_node;
use crate::type_reference::lower_type_reference_into_table;
use omega_core::diagnostics::Diagnostic;
use omega_symbol_resolved_trees as resolved;
use omega_typed_trees as typed;

pub(crate) fn lower_state(
    lowerer: &mut Lowerer,
    state: &resolved::state::State,
) -> Result<typed::state::State, Diagnostic> {
    let mut typed_state = typed::state::State {
        symbol: state.symbol,
        name: crate::name::lower_name(&state.name),
        parameters: Default::default(),
        return_type: state
            .return_type
            .as_ref()
            .map(|type_reference| lower_type_reference_into_table(lowerer, type_reference))
            .transpose()?
            .unwrap_or_else(typed::types::TypeReferenceHandle::invalid),
        statement_nodes: Default::default(),
    };

    for parameter in lowerer.source_trees.state_parameters(state.parameters) {
        let parameter = lower_state_parameter(lowerer, parameter)?;
        lowerer
            .typed_trees
            .push_state_parameter(&mut typed_state, parameter);
    }

    for statement in lowerer
        .source_trees
        .tables
        .bodies
        .statements
        .statements(state.statement_nodes)
    {
        let statement = lower_statement_node(lowerer, statement)?;
        lowerer
            .typed_trees
            .statement_table
            .push_statement(&mut typed_state.statement_nodes, statement);
    }

    Ok(typed_state)
}

pub(crate) fn lower_state_signature(
    lowerer: &mut Lowerer,
    signature: &resolved::signature::StateSignature,
) -> Result<typed::signature::StateSignature, Diagnostic> {
    let mut typed_signature = typed::signature::StateSignature {
        symbol: signature.symbol,
        name: crate::name::lower_name(&signature.name),
        parameters: Default::default(),
        return_type: signature
            .return_type
            .as_ref()
            .map(|type_reference| lower_type_reference_into_table(lowerer, type_reference))
            .transpose()?
            .unwrap_or_else(typed::types::TypeReferenceHandle::invalid),
        effects: Default::default(),
        contracts: Default::default(),
    };

    for parameter in lowerer.source_trees.state_parameters(signature.parameters) {
        let parameter = lower_state_parameter(lowerer, parameter)?;
        lowerer
            .typed_trees
            .push_state_signature_parameter(&mut typed_signature, parameter);
    }

    for effect in lowerer.source_trees.signature_effects(signature.effects) {
        lowerer
            .typed_trees
            .push_state_signature_effect(&mut typed_signature, crate::name::lower_name(effect));
    }

    for contract in lowerer
        .source_trees
        .signature_contracts(signature.contracts)
    {
        let facts = lower_proof_facts(lowerer, contract.facts)?;
        lowerer.typed_trees.push_state_signature_contract(
            &mut typed_signature,
            typed::signature::SignatureContract {
                kind: match contract.kind {
                    resolved::signature::SignatureContractKind::Requires => {
                        typed::signature::SignatureContractKind::Requires
                    }
                    resolved::signature::SignatureContractKind::Ensures => {
                        typed::signature::SignatureContractKind::Ensures
                    }
                    resolved::signature::SignatureContractKind::Boundary => {
                        typed::signature::SignatureContractKind::Boundary
                    }
                },
                facts,
                token_count: contract.token_count,
            },
        );
    }

    Ok(typed_signature)
}

fn lower_state_parameter(
    lowerer: &mut Lowerer,
    parameter: &resolved::signature::StateParameter,
) -> Result<typed::signature::StateParameter, Diagnostic> {
    Ok(typed::signature::StateParameter {
        symbol: parameter.symbol,
        name: crate::name::lower_name(&parameter.name),
        type_reference: lower_type_reference_into_table(lowerer, &parameter.type_reference)?,
        is_const: parameter.is_const,
        is_mutable: parameter.is_mutable,
        is_self: parameter.is_self,
    })
}

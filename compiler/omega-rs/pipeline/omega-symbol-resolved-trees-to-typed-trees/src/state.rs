use crate::domain::lower_proof_facts;
use crate::lowerer::Lowerer;
use crate::statement::lower_statement_node;
use crate::type_reference::lower_type_reference_into_table;
use omega_core::diagnostics::Diagnostic;
use omega_symbol_resolved_trees as resolved;
use omega_typed_trees as typed;

pub(crate) fn lower_state(
    lowerer: &mut Lowerer,
    attached_data: Option<&resolved::name::DiagnosticName>,
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
        contracts: Default::default(),
        statement_nodes: Default::default(),
    };

    for parameter in lowerer.source_trees.state_parameters(state.parameters) {
        let parameter = lower_state_parameter(lowerer, parameter)?;
        lowerer
            .typed_trees
            .push_state_parameter(&mut typed_state, parameter);
    }

    for contract in lowerer.source_trees.signature_contracts(state.contracts) {
        let facts = lower_proof_facts(lowerer, contract.facts)?;
        lowerer.typed_trees.push_state_contract(
            &mut typed_state,
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

    for statement in lowerer
        .source_trees
        .tables
        .bodies
        .statements
        .statements(state.statement_nodes)
    {
        let statement = lower_statement_node(lowerer, attached_data, state, statement)?;
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
        type_parameters: Default::default(),
        is_default: signature.is_default,
        parameters: Default::default(),
        return_type: signature
            .return_type
            .as_ref()
            .map(|type_reference| lower_type_reference_into_table(lowerer, type_reference))
            .transpose()?
            .unwrap_or_else(typed::types::TypeReferenceHandle::invalid),
        effects: Default::default(),
        contracts: Default::default(),
        // TPR4: copied, never re-derived.
        terminates_guarantee: signature.terminates_guarantee,
    };

    for parameter in lowerer
        .source_trees
        .data_type_parameters(signature.type_parameters)
    {
        let parameter = crate::data::lower_type_parameter(lowerer, parameter)?;
        lowerer
            .typed_trees
            .data_type_parameters
            .append_to_span(&mut typed_signature.type_parameters, parameter);
    }

    // #66/DOM1: collect every predicate facet on constrained parameters. Each
    // desugars below into its own implicit `requires <param> in <domain>`
    // membership contract; semantic-only facets do not enter the fact lattice.
    let mut domain_constrained_parameters: Vec<(
        omega_core::symbols::SymbolHandle,
        typed::name::Identifier,
        omega_core::symbols::SymbolHandle,
        String,
    )> = Vec::new();
    for parameter in lowerer.source_trees.state_parameters(signature.parameters) {
        let parameter = lower_state_parameter(lowerer, parameter)?;
        for (domain_symbol, domain_full_name) in
            predicate_domain_constraints(&lowerer.typed_trees, parameter.type_reference)
        {
            domain_constrained_parameters.push((
                parameter.symbol,
                parameter.name.clone(),
                domain_symbol,
                domain_full_name,
            ));
        }
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

    // #66/DOM1: desugar each predicate facet into an implicit `requires
    // <param> in <domain>` membership contract (here on a trait/platform
    // signature; the regular-machine path is `lower_machine`).
    for (param_symbol, param_name, domain_symbol, domain_full_name) in domain_constrained_parameters
    {
        let contract = build_domain_membership_contract(
            lowerer,
            param_symbol,
            param_name,
            domain_symbol,
            &domain_full_name,
        );
        lowerer
            .typed_trees
            .push_state_signature_contract(&mut typed_signature, contract);
    }

    Ok(typed_signature)
}

/// Every normalized predicate facet on a parameter type, looking through a
/// leading reference. Semantic-only qualifications never synthesize proof
/// contracts.
pub(crate) fn predicate_domain_constraints(
    typed_trees: &typed::TypedTrees,
    type_reference: typed::types::TypeReferenceHandle,
) -> Vec<(omega_core::symbols::SymbolHandle, String)> {
    match typed_trees
        .type_reference_table
        .type_reference(type_reference)
    {
        typed::types::TypeReferenceNode::Reference { referee, .. } => {
            predicate_domain_constraints(typed_trees, *referee)
        }
        typed::types::TypeReferenceNode::Constrained { constraints, .. } => typed_trees
            .type_reference_table
            .constraints(*constraints)
            .iter()
            .filter_map(|constraint| match constraint {
                typed::types::TypeConstraintNode::Domain(domain)
                    if domain.symbol.is_valid() && domain.facets.predicate =>
                {
                    typed_trees
                        .domain_definitions()
                        .iter()
                        .find(|definition| definition.symbol == domain.symbol)
                        .map(|definition| (domain.symbol, definition.name.as_str().to_owned()))
                }
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// Build the implicit `requires <param> in <domain>` membership contract for a
/// normalized predicate constraint. The caller attaches the contract at the
/// right level (machine vs trait signature).
pub(crate) fn build_domain_membership_contract(
    lowerer: &mut Lowerer,
    param_symbol: omega_core::symbols::SymbolHandle,
    param_name: typed::name::Identifier,
    domain_symbol: omega_core::symbols::SymbolHandle,
    domain_full_name: &str,
) -> typed::signature::SignatureContract {
    let mut members = omega_core::arena::HandleSpan::empty();
    lowerer
        .typed_trees
        .expression_table
        .push_name_path_member(&mut members, param_name);
    let mut member_symbols = omega_core::arena::HandleSpan::empty();
    lowerer
        .typed_trees
        .expression_table
        .push_name_path_member_symbol(&mut member_symbols, param_symbol);
    let value =
        lowerer
            .typed_trees
            .expression_table
            .insert(typed::expression::ExpressionNode::Name(
                typed::expression::TableNamePath {
                    members,
                    member_symbols,
                    head_symbol: param_symbol,
                    symbol: param_symbol,
                },
            ));

    let mut domain = omega_core::arena::HandleSpan::empty();
    for part in domain_full_name.split("::") {
        lowerer
            .typed_trees
            .domain_path_members
            .append_to_span(&mut domain, typed::name::Identifier::generated(part));
    }

    let mut facts = omega_core::arena::HandleSpan::empty();
    lowerer.typed_trees.proof_facts.append_to_span(
        &mut facts,
        typed::domain::ProofFact::Membership(typed::domain::ProofMembershipFact {
            value,
            domain,
            domain_symbol,
        }),
    );

    typed::signature::SignatureContract {
        kind: typed::signature::SignatureContractKind::Requires,
        facts,
        token_count: 0,
    }
}

fn lower_state_parameter(
    lowerer: &mut Lowerer,
    parameter: &resolved::signature::StateParameter,
) -> Result<typed::signature::StateParameter, Diagnostic> {
    let type_reference = lower_type_reference_into_table(lowerer, &parameter.type_reference)?;
    crate::domain_constraints::normalize_domain_constraints_for_type(
        &mut lowerer.typed_trees,
        type_reference,
    );
    Ok(typed::signature::StateParameter {
        symbol: parameter.symbol,
        name: crate::name::lower_name(&parameter.name),
        type_reference,
        is_const: parameter.is_const,
        is_mutable: parameter.is_mutable,
        is_self: parameter.is_self,
    })
}

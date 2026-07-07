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
    };

    // #66: collect parameters whose type carries an encoding-DOMAIN constraint
    // (`bytes: [u8] in Utf8`). Each desugars below into an implicit
    // `requires <param> in <domain>` membership contract.
    let mut domain_constrained_parameters: Vec<(
        omega_core::symbols::SymbolHandle,
        typed::name::Identifier,
        typed::name::Identifier,
    )> = Vec::new();
    for parameter in lowerer.source_trees.state_parameters(signature.parameters) {
        let parameter = lower_state_parameter(lowerer, parameter)?;
        if let Some(domain_name) =
            domain_constraint_name(&lowerer.typed_trees, parameter.type_reference)
        {
            domain_constrained_parameters.push((
                parameter.symbol,
                parameter.name.clone(),
                domain_name,
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

    // #66 Phase 1: desugar each Domain-constrained parameter into an implicit
    // `requires <param> in <domain>` membership contract (here on a trait/platform
    // signature; the regular-machine path is `lower_machine`). The existing
    // `in Domain` entailment then enforces it at every call site.
    for (param_symbol, param_name, domain_name) in domain_constrained_parameters {
        if let Some(contract) =
            build_domain_membership_contract(lowerer, param_symbol, param_name, domain_name.as_str())
        {
            lowerer
                .typed_trees
                .push_state_signature_contract(&mut typed_signature, contract);
        }
    }

    Ok(typed_signature)
}

/// The declared encoding domain (`[u8] in Utf8`) on a parameter type, if any,
/// looking through a leading reference (`&[u8] in Utf8`).
pub(crate) fn domain_constraint_name(
    typed_trees: &typed::TypedTrees,
    type_reference: typed::types::TypeReferenceHandle,
) -> Option<typed::name::Identifier> {
    match typed_trees
        .type_reference_table
        .type_reference(type_reference)
    {
        typed::types::TypeReferenceNode::Reference { referee, .. } => {
            domain_constraint_name(typed_trees, *referee)
        }
        typed::types::TypeReferenceNode::Constrained { constraints, .. } => typed_trees
            .type_reference_table
            .constraints(*constraints)
            .iter()
            .find_map(|constraint| match constraint {
                typed::types::TypeConstraintNode::Domain(name) => Some(name.clone()),
                _ => None,
            }),
        _ => None,
    }
}

/// Build the implicit `requires <param> in <domain>` membership contract for a
/// Domain-constrained parameter (#66 Phase 1). Returns `None` if the domain name
/// resolves to no declared `domain` (validation reports that separately). The
/// caller attaches the contract at the right level (machine vs trait signature).
pub(crate) fn build_domain_membership_contract(
    lowerer: &mut Lowerer,
    param_symbol: omega_core::symbols::SymbolHandle,
    param_name: typed::name::Identifier,
    domain_name: &str,
) -> Option<typed::signature::SignatureContract> {
    let (domain_symbol, domain_full_name) = resolve_domain(lowerer.source_trees, domain_name)?;

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
    let value = lowerer
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

    Some(typed::signature::SignatureContract {
        kind: typed::signature::SignatureContractKind::Requires,
        facts,
        token_count: 0,
    })
}

fn resolve_domain(
    source: &resolved::SymbolResolvedTrees,
    wanted: &str,
) -> Option<(omega_core::symbols::SymbolHandle, String)> {
    source.domain_definitions.iter().find_map(|domain| {
        let full = domain.name.as_str();
        (full.rsplit("::").next().unwrap_or(full) == wanted).then(|| (domain.symbol, full.to_owned()))
    })
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

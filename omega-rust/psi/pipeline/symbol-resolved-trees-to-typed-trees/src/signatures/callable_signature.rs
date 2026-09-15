use crate::lowerer::Lowerer;
use crate::type_reference::lower_type_reference_into_table;
use diagnostics::Diagnostic;
use symbol_resolved_trees as resolved;
use typed_trees as typed;

use crate::contracts::invocations::lower_authored_invocations;
use crate::contracts::parameter_domains::{build_domain_membership_contract, domain_constraints};
use crate::contracts::proof_facts::lower_proof_facts;
use crate::signatures::parameters::lower_state_parameter;

pub(crate) fn lower_state_signature(
    lowerer: &mut Lowerer,
    signature: &resolved::signature::StateSignature,
) -> Result<typed::signature::StateSignature, Diagnostic> {
    let mut typed_signature = typed::signature::StateSignature {
        symbol: signature.symbol,
        name: crate::lowerer::name::lower_name(&signature.name),
        spelling: signature.spelling,
        lifetime_parameters: signature
            .lifetime_parameters
            .iter()
            .map(crate::lowerer::name::lower_name)
            .collect(),
        type_parameters: Default::default(),
        is_default: signature.is_default,
        parameters: Default::default(),
        native_callback_parameters: signature
            .native_callback_parameters
            .iter()
            .map(|parameter| typed::signature::NativeCallbackParameter {
                name: crate::lowerer::name::lower_name(&parameter.name),
                binder: crate::lowerer::name::lower_name(&parameter.binder),
                native_ordinal: parameter.native_ordinal,
            })
            .collect(),
        return_type: signature
            .return_type
            .as_ref()
            .map(|type_reference| lower_type_reference_into_table(lowerer, type_reference))
            .transpose()?
            .unwrap_or_else(typed::types::TypeReferenceHandle::invalid),
        invokes: Default::default(),
        service_reach_row: signature.service_reach_row,
        service_reach_is_installation_bound: signature.service_reach_is_installation_bound,
        suspends_keyword_source_spans: signature.suspends_keyword_source_spans.clone(),
        blocks_keyword_source_spans: signature.blocks_keyword_source_spans.clone(),
        suspends: signature.suspends,
        blocks: signature.blocks,
        contracts: Default::default(),
        where_facts: Default::default(),
        // The final subject-bearing guarantee is normalized after all typed
        // domains, contracts, and requirements are available.
        termination_guarantee: if signature.terminates_guarantee {
            language_semantics::TerminationGuarantee::Terminates {
                premises: Vec::new(),
            }
        } else {
            language_semantics::TerminationGuarantee::NoGuarantee
        },
    };

    typed_signature.type_parameters = crate::signatures::type_parameters::lower_type_parameters(
        lowerer,
        signature.type_parameters,
    )?;
    typed_signature.where_facts = lower_proof_facts(lowerer, signature.where_facts)?;

    // #66/DOM1/P1a: collect every declared domain on constrained parameters.
    // Each desugars below into its own implicit `requires <param> in <domain>`
    // membership contract. Predicate-bearing domains discharge by proof;
    // bodyless domains require retained qualification evidence.
    let mut domain_constrained_parameters: Vec<(
        symbols::SymbolHandle,
        typed::name::Identifier,
        symbols::SymbolHandle,
        String,
        Vec<typed::types::TypeReferenceHandle>,
        language_semantics::SemanticDomainId,
    )> = Vec::new();
    for parameter in lowerer.source_trees.state_parameters(signature.parameters) {
        let parameter = lower_state_parameter(lowerer, parameter)?;
        for (domain_symbol, domain_full_name, domain_arguments, semantic_domain) in
            domain_constraints(&lowerer.typed_trees, parameter.type_reference)
        {
            domain_constrained_parameters.push((
                parameter.symbol,
                parameter.name.clone(),
                domain_symbol,
                domain_full_name,
                domain_arguments,
                semantic_domain,
            ));
        }
        lowerer
            .typed_trees
            .push_state_signature_parameter(&mut typed_signature, parameter);
    }

    let invocations = lower_authored_invocations(
        lowerer.source_trees,
        lowerer.source_trees.signature_invokes(signature.invokes),
        lowerer.source_trees.state_parameters(signature.parameters),
        signature.name.as_str(),
    )?;
    for invocation in invocations {
        lowerer
            .typed_trees
            .push_state_signature_invoke(&mut typed_signature, invocation);
    }

    for contract in lowerer
        .source_trees
        .signature_contracts(signature.contracts)
    {
        let facts = lower_proof_facts(lowerer, contract.facts)?;
        lowerer.typed_trees.push_state_signature_contract(
            &mut typed_signature,
            typed::signature::SignatureContract {
                kind: match &contract.kind {
                    resolved::signature::SignatureContractKind::Requires => {
                        typed::signature::SignatureContractKind::Requires
                    }
                    resolved::signature::SignatureContractKind::Ensures => {
                        typed::signature::SignatureContractKind::Ensures
                    }
                    resolved::signature::SignatureContractKind::EnsuresForResultCase {
                        result_data,
                        result_case,
                    } => typed::signature::SignatureContractKind::EnsuresForResultCase {
                        result_data: *result_data,
                        result_case: *result_case,
                    },
                    resolved::signature::SignatureContractKind::Crashes { cause } => {
                        typed::signature::SignatureContractKind::Crashes {
                            cause: match cause {
                                resolved::signature::CrashCause::Trap => {
                                    typed::signature::CrashCause::Trap
                                }
                                resolved::signature::CrashCause::Abort => {
                                    typed::signature::CrashCause::Abort
                                }
                            },
                        }
                    }
                },
                keyword_source_span: contract.keyword_source_span,
                binding: contract
                    .binding
                    .as_ref()
                    .map(crate::lowerer::name::lower_name),
                facts,
                token_count: contract.token_count,
            },
        );
    }

    // #66/DOM1/P1a: desugar each declared domain into an implicit `requires
    // <param> in <domain>` membership contract (here on a trait/platform
    // signature; the regular-machine path is `lower_machine`).
    for (
        param_symbol,
        param_name,
        domain_symbol,
        domain_full_name,
        domain_arguments,
        semantic_domain,
    ) in domain_constrained_parameters
    {
        let contract = build_domain_membership_contract(
            lowerer,
            param_symbol,
            param_name,
            domain_symbol,
            &domain_full_name,
            domain_arguments,
            semantic_domain,
        );
        lowerer
            .typed_trees
            .push_state_signature_contract(&mut typed_signature, contract);
    }

    Ok(typed_signature)
}

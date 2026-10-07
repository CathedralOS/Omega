use crate::lowerer::Lowerer;
use crate::type_reference::lower_type_reference_into_table;
use diagnostics::Diagnostic;
use symbol_resolved_trees as resolved;
use typed_trees as typed;

use crate::contracts::invocations::lower_authored_invocations;

use crate::contracts::proof_facts::lower_proof_facts;

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
    let domain_constrained_parameters =
        crate::contracts::parameter_domains::lower_state_parameters_collecting_domain_constraints(
            lowerer,
            signature.parameters,
            |typed_trees, parameter| {
                typed_trees.push_state_signature_parameter(&mut typed_signature, parameter)
            },
        )?;

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
        let contract = crate::contracts::proof_facts::lower_signature_contract(lowerer, contract)?;
        lowerer
            .typed_trees
            .push_state_signature_contract(&mut typed_signature, contract);
    }

    // #66/DOM1/P1a: desugar each declared domain into an implicit `requires
    // <param> in <domain>` membership contract (here on a trait/platform
    // signature; the regular-machine path is `lower_machine`).
    crate::contracts::parameter_domains::push_domain_membership_contracts(
        lowerer,
        domain_constrained_parameters,
        |typed_trees, contract| {
            typed_trees.push_state_signature_contract(&mut typed_signature, contract)
        },
    );

    Ok(typed_signature)
}

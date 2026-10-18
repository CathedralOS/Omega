use crate::lowerer::Lowerer;
use crate::signatures::parameters::lower_state_parameter;
use symbol_resolved_trees as resolved;
use typed_trees as typed;

pub(crate) use validation::parameter_domain_constraints as domain_constraints;

/// One `(parameter, declared domain)` pair collected while lowering state
/// parameters; each pair desugars into its own membership contract.
pub(crate) type DomainConstrainedParameter = (
    symbols::SymbolHandle,
    typed::name::Identifier,
    symbols::SymbolHandle,
    String,
    Vec<typed::types::TypeReferenceHandle>,
    language_semantics::SemanticDomainId,
);

/// Lower each authored state parameter, collecting every declared domain
/// constraint for `push_domain_membership_contracts`. `push_parameter`
/// attaches each lowered parameter at the right level (state vs trait/machine
/// signature).
pub(crate) fn lower_state_parameters_collecting_domain_constraints(
    lowerer: &mut Lowerer,
    parameters: arena::HandleSpan<resolved::signature::StateParameter>,
    mut push_parameter: impl FnMut(&mut typed::TypedTrees, typed::signature::StateParameter),
) -> Result<Vec<DomainConstrainedParameter>, diagnostics::Diagnostic> {
    let mut domain_constrained_parameters: Vec<DomainConstrainedParameter> = Vec::new();
    for parameter in lowerer.source_trees.state_parameters(parameters) {
        let parameter = lower_state_parameter(lowerer, parameter)?;
        for (domain_symbol, domain_full_name, domain_arguments, semantic_domain) in
            domain_constraints(&lowerer.typed_trees, parameter.type_reference, &[])
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
        push_parameter(&mut lowerer.typed_trees, parameter);
    }
    Ok(domain_constrained_parameters)
}

/// Desugar each collected declared domain into an implicit `requires <param>
/// in <domain>` membership contract. `push_contract` attaches each contract at
/// the right level (state vs trait/machine signature).
pub(crate) fn push_domain_membership_contracts(
    lowerer: &mut Lowerer,
    domain_constrained_parameters: Vec<DomainConstrainedParameter>,
    mut push_contract: impl FnMut(&mut typed::TypedTrees, typed::signature::SignatureContract),
) {
    for (
        param_symbol,
        param_name,
        domain_symbol,
        domain_full_name,
        domain_arguments,
        semantic_domain,
    ) in domain_constrained_parameters
    {
        let contract = lowerer.typed_trees.parameter_domain_membership_contract(
            param_symbol,
            param_name,
            domain_symbol,
            &domain_full_name,
            domain_arguments,
            semantic_domain,
        );
        push_contract(&mut lowerer.typed_trees, contract);
    }
}

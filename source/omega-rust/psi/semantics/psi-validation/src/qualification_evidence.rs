use crate::places::unwrapped_type_reference;
use crate::type_references::type_references_match;
use psi_diagnostics::Diagnostic;
use psi_language_semantics::MachineSupplyMode;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::domain::ProofFact;
use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use psi_typed_trees::signature::SignatureContractKind;

pub(crate) fn validate_qualification_authorization(
    program: &TypedTrees,
    diagnostics: &mut Vec<Diagnostic>,
) {
    validate_boundary_requirements(program, diagnostics);
    validate_external_machine_claims(program, diagnostics);
}

fn validate_boundary_requirements(program: &TypedTrees, diagnostics: &mut Vec<Diagnostic>) {
    for trait_definition in program
        .traits()
        .iter()
        .filter(|definition| definition.is_boundary)
    {
        for signature in program.trait_machine_signatures(trait_definition) {
            for contract in program
                .state_signature_contracts(signature)
                .iter()
                .filter(|contract| contract.kind == SignatureContractKind::Ensures)
            {
                for fact in program.proof_facts.span_or_empty(contract.facts) {
                    let ProofFact::Membership(membership) = fact else {
                        continue;
                    };
                    if let Some(permission) =
                        crate::proof_facts::carry_permission(program, membership.domain)
                    {
                        if !expression_is_bare_result(program, membership.value) {
                            diagnostics.push(Diagnostic::error(format!(
                                "boundary requirement `{}::{}` may admit carry permission `{permission}` only for its exact `result`",
                                trait_definition.name, signature.name,
                            )));
                        }
                        continue;
                    }
                    if !membership.domain_symbol.is_valid() {
                        continue;
                    }
                    let Some(domain) = program
                        .domain_definitions()
                        .iter()
                        .find(|domain| domain.symbol == membership.domain_symbol)
                    else {
                        continue;
                    };

                    if !expression_is_bare_result(program, membership.value) {
                        diagnostics.push(Diagnostic::error(format!(
                            "boundary requirement `{}::{}` may admit domain `{}` only for its exact `result`; move the membership guarantee to `ensures result in {}`",
                            trait_definition.name,
                            signature.name,
                            domain.name,
                            domain.name,
                        )));
                        continue;
                    }

                    let return_carrier = unwrapped_type_reference(program, signature.return_type);
                    let domain_carrier = unwrapped_type_reference(program, domain.target_type);
                    if !matches!(
                        (return_carrier, domain_carrier),
                        (Some(return_carrier), Some(domain_carrier))
                            if type_references_match(program, return_carrier, domain_carrier)
                                || lifetime_erased_nominal_carriers_match(
                                    program,
                                    return_carrier,
                                    domain_carrier,
                                )
                    ) {
                        diagnostics.push(Diagnostic::error(format!(
                            "boundary requirement `{}::{}` cannot admit `result in {}`: result carrier `{}` does not match domain target `{}`",
                            trait_definition.name,
                            signature.name,
                            domain.name,
                            program.display_type_reference_with_constraints(signature.return_type),
                            program.display_type_reference_with_constraints(domain.target_type),
                        )));
                    }
                }
            }
        }
    }
}

/// Domain declarations name their nominal carrier without a lifetime
/// telescope. A result may instantiate that same carrier's erased lifetime
/// parameters, but this must not widen ordinary generic matching: runtime type
/// arguments still require exact normalized identity.
fn lifetime_erased_nominal_carriers_match(
    program: &TypedTrees,
    application: psi_typed_trees::types::TypeReferenceHandle,
    nominal: psi_typed_trees::types::TypeReferenceHandle,
) -> bool {
    let psi_typed_trees::types::TypeReferenceNode::Generic {
        base_symbol,
        lifetime_arguments,
        arguments,
        ..
    } = program.type_reference_table.type_reference(application)
    else {
        return false;
    };
    let psi_typed_trees::types::TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(nominal)
    else {
        return false;
    };
    *base_symbol == *symbol
        && symbol.is_valid()
        && program
            .type_reference_table
            .type_reference_handles(*arguments)
            .is_empty()
        && program
            .data_definitions()
            .iter()
            .find(|definition| definition.symbol == *base_symbol)
            .is_some_and(|definition| {
                !definition.lifetime_parameters.is_empty()
                    && definition.lifetime_parameters.len() == lifetime_arguments.len()
            })
}

fn validate_external_machine_claims(program: &TypedTrees, diagnostics: &mut Vec<Diagnostic>) {
    for machine in program.machines().iter().filter(|machine| {
        matches!(
            machine.supply_mode,
            MachineSupplyMode::AdmissionClaim
                | MachineSupplyMode::Requirement
                | MachineSupplyMode::ExternalRealization { .. }
        )
    }) {
        for contract in program
            .machine_contracts(machine)
            .iter()
            .chain(
                program
                    .machine_states(machine)
                    .iter()
                    .flat_map(|state| program.state_contracts(state)),
            )
            .filter(|contract| contract.kind == SignatureContractKind::Ensures)
        {
            for fact in program.proof_facts.span_or_empty(contract.facts) {
                let ProofFact::Membership(membership) = fact else {
                    continue;
                };
                if let Some(permission) =
                    crate::proof_facts::carry_permission(program, membership.domain)
                {
                    diagnostics.push(Diagnostic::error(format!(
                        "external machine `{}` cannot directly admit carry permission `{permission}`; publish it on an owner-authorized boundary trait requirement and satisfy that requirement through the provider",
                        machine.name,
                    )));
                    continue;
                }
                if !membership.domain_symbol.is_valid() {
                    continue;
                }
                let domain_name = program
                    .domain_definitions()
                    .iter()
                    .find(|domain| domain.symbol == membership.domain_symbol)
                    .map(|domain| domain.name.as_str())
                    .unwrap_or("<unknown>");
                diagnostics.push(Diagnostic::error(format!(
                    "external machine `{}` cannot directly admit domain membership `{}`; publish `ensures result in {}` on an owner-authorized boundary trait requirement and satisfy that requirement through the provider",
                    machine.name, domain_name, domain_name,
                )));
            }
        }
    }
}

fn expression_is_bare_result(program: &TypedTrees, expression: ExpressionHandle) -> bool {
    let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
        return false;
    };
    let [name] = program.expression_table.name_path_members(path.members) else {
        return false;
    };
    name.as_str() == "result"
}

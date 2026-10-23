//! Qualification originates at an authorized result boundary, not from a type
//! annotation. Exact-machine routes name authored declarations; executable
//! specialization and target membership change the body symbol but not the
//! invoked declaration. Follow retained application ownership and the resolved
//! family roster for that join. Domain indices and
//! result subjects remain in the ordinary contract facts and are not erased.

use checked_trees::ContractProofFact;
use facts::{FactPayload, QualificationEvidence};
use language_semantics::{
    DomainEstablishmentRoute, MachineSupplyMode, QualificationEvidenceOrigin,
};
use symbols::SymbolHandle;
use typed_trees::machine::Machine;
use typed_trees::types::TypeReferenceHandle;

pub(crate) fn domain_definition(
    program: &dyn ::validation::CheckingInput,
    domain_symbol: SymbolHandle,
) -> Option<&typed_trees::domain::DomainDefinition> {
    program.domain_definition(domain_symbol)
}

pub(crate) fn call_contract_evidence(
    program: &dyn ::validation::CheckingInput,
    target_symbol: SymbolHandle,
    target_state_symbol: SymbolHandle,
    contract: &ContractProofFact,
    payload: FactPayload,
    is_ensures: bool,
) -> Option<QualificationEvidence> {
    if !is_ensures {
        return Some(QualificationEvidence::default());
    }
    if matches!(payload, FactPayload::ContractCarryPermission { .. }) {
        return call_carry_permission_evidence(
            program,
            target_symbol,
            target_state_symbol,
            contract,
        );
    }
    let FactPayload::ContractDomainMembership { domain_symbol, .. } = payload else {
        return Some(QualificationEvidence::default());
    };
    let Some(domain) = domain_definition(program, domain_symbol) else {
        return Some(QualificationEvidence::default());
    };

    if let Some(machine) = program.machine_by_symbol(target_symbol) {
        let checked_body = machine.body_is_present
            && matches!(
                machine.supply_mode,
                MachineSupplyMode::CheckedBody | MachineSupplyMode::Boundary
            );
        if checked_body {
            // A checked adapter inherits the boundary requirement so its body
            // can be validated against that requirement, but a direct call to
            // the adapter is not the admitted crossing. Only a call whose
            // target is the boundary trait/signature may consume the attached
            // authorization and originate its qualified result.
            if contract.qualification_authorization.is_some() {
                return None;
            }
            let origin =
                if machine_domain_establishment_route(program, machine, domain_symbol).is_some() {
                    QualificationEvidenceOrigin::AuthorizedRouteEstablishment
                } else if domain.predicate_body.is_present() {
                    QualificationEvidenceOrigin::Prover
                } else {
                    QualificationEvidenceOrigin::CheckedTransformation
                };
            return Some(QualificationEvidence::from_origin(origin, target_symbol));
        }

        return contract.qualification_authorization.map(|authorization| {
            QualificationEvidence::from_admitted_requirement(
                authorization.requirement_symbol,
                authorization.signature_symbol,
            )
        });
    }

    if let Some(trait_definition) = program.trait_definition_by_symbol(target_symbol)
        && trait_definition.is_boundary
    {
        return contract.qualification_authorization.map(|authorization| {
            QualificationEvidence::from_admitted_requirement(
                authorization.requirement_symbol,
                authorization.signature_symbol,
            )
        });
    }

    Some(QualificationEvidence::from_origin(
        QualificationEvidenceOrigin::Propagated,
        target_symbol,
    ))
}

pub(crate) fn machine_has_checked_domain_establishment(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    domain_symbol: SymbolHandle,
) -> bool {
    machine_domain_establishment_route(program, machine, domain_symbol).is_some()
}

pub(crate) fn machine_domain_establishment_route(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    domain_symbol: SymbolHandle,
) -> Option<DomainEstablishmentRoute> {
    if !matches!(
        machine.supply_mode,
        MachineSupplyMode::CheckedBody | MachineSupplyMode::Boundary
    ) {
        return None;
    }
    let domain = domain_definition(program, domain_symbol)?;
    domain
        .establishment_routes
        .iter()
        .find_map(|route| match route {
            DomainEstablishmentRoute::CheckedRequirement {
                trait_definition,
                requirement,
            } => {
                // Named maps own their realization bodies instead of adding
                // attached `satisfies` edges. Both forms select exact rows;
                // spelling or signature compatibility grants no authority.
                // Executable copies retain invocation/capture ownership of
                // their authored row, including generic named maps.
                let attached =
                    program
                        .machine_trait_conformances(machine)
                        .iter()
                        .any(|conformance| {
                            conformance.symbol == *trait_definition
                                && conformance.requirement_symbol == *requirement
                        });
                let named = program.conformances().iter().any(|conformance| {
                    conformance.rows.iter().any(|row| {
                        row.source.owns_realization()
                            && row.declaring_trait == *trait_definition
                            && row.requirement == *requirement
                            && program
                                .machine_invokes_declaration(
                                    machine.symbol,
                                    row.realization_machine,
                                )
                                .unwrap_or(false)
                    })
                });
                (attached || named).then_some(*route)
            }
            DomainEstablishmentRoute::ExactMachine {
                machine: authorized,
            } => program
                .machine_invokes_declaration(machine.symbol, *authorized)
                .unwrap_or(false)
                .then_some(route.clone()),
            DomainEstablishmentRoute::BoundaryRequirement { .. } => None,
        })
}

fn call_carry_permission_evidence(
    program: &dyn ::validation::CheckingInput,
    target_symbol: SymbolHandle,
    _target_state_symbol: SymbolHandle,
    contract: &ContractProofFact,
) -> Option<QualificationEvidence> {
    if let Some(machine) = program.machine_by_symbol(target_symbol) {
        let checked_body = machine.body_is_present
            && matches!(
                machine.supply_mode,
                MachineSupplyMode::CheckedBody | MachineSupplyMode::Boundary
            );
        if checked_body {
            if contract.qualification_authorization.is_some() {
                return None;
            }
            return Some(QualificationEvidence::from_origin(
                QualificationEvidenceOrigin::CheckedTransformation,
                target_symbol,
            ));
        }
        return contract.qualification_authorization.map(|authorization| {
            QualificationEvidence::from_admitted_requirement(
                authorization.requirement_symbol,
                authorization.signature_symbol,
            )
        });
    }

    if program
        .traits()
        .iter()
        .any(|definition| definition.symbol == target_symbol && definition.is_boundary)
    {
        return contract.qualification_authorization.map(|authorization| {
            QualificationEvidence::from_admitted_requirement(
                authorization.requirement_symbol,
                authorization.signature_symbol,
            )
        });
    }

    Some(QualificationEvidence::from_origin(
        QualificationEvidenceOrigin::Propagated,
        target_symbol,
    ))
}

pub(crate) fn operator_contract_evidence(
    program: &dyn ::validation::CheckingInput,
    operator_symbol: SymbolHandle,
    payload: FactPayload,
) -> QualificationEvidence {
    let FactPayload::ContractDomainMembership { domain_symbol, .. } = payload else {
        return QualificationEvidence::default();
    };
    let Some(domain) = domain_definition(program, domain_symbol) else {
        return QualificationEvidence::default();
    };
    if domain.predicate_body.is_present() {
        return QualificationEvidence::from_origin(
            QualificationEvidenceOrigin::Prover,
            operator_symbol,
        );
    }

    QualificationEvidence::from_origin(
        QualificationEvidenceOrigin::CheckedTransformation,
        operator_symbol,
    )
}
pub(crate) fn unwrapped_type_references_match(
    program: &dyn ::validation::CheckingInput,
    left: TypeReferenceHandle,
    right: TypeReferenceHandle,
) -> bool {
    let Some(left) = program.unwrapped_type_reference(left) else {
        return false;
    };
    let Some(right) = program.unwrapped_type_reference(right) else {
        return false;
    };
    program.normalized_type_identity(left) == program.normalized_type_identity(right)
}

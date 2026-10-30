//! Qualification originates at an authorized result boundary, not from a type
//! annotation. Exact-machine routes name authored declarations; executable
//! specialization and target membership change the body symbol but not the
//! invoked declaration. Follow retained application ownership and the resolved
//! family roster for that join. Domain indices and
//! result subjects remain in the ordinary contract facts and are not erased.

use checked_trees::ContractProofFact;
use facts::{BoundaryQualificationIssuerRecord, FactPayload, FactPlan, QualificationEvidence};
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

/// Evidence for a row the state's own declaration already establishes: the
/// caller inherited the claim, it did not validate it here.
pub(crate) fn propagated_evidence(state_symbol: SymbolHandle) -> QualificationEvidence {
    QualificationEvidence::from_origin(QualificationEvidenceOrigin::Propagated, state_symbol)
}

/// Evidence for a row checked validation itself established at this point, not
/// one the caller inherited.
pub(crate) fn checked_validation_evidence(state_symbol: SymbolHandle) -> QualificationEvidence {
    QualificationEvidence::from_origin(QualificationEvidenceOrigin::CheckedValidation, state_symbol)
}

/// Project authorized contract issuers once, before flow forks the fact plan.
/// The shared baseline gives repeated calls and independent workers the same
/// evidence identity. Unused declarations are inventory, not grant obligations:
/// consumers follow issuer handles from actual admitted facts only.
pub(crate) fn prepare_issuers(
    program: &dyn ::validation::CheckingInput,
    proof: &checked_trees::ProofFacts,
    facts: &mut FactPlan,
) -> Result<(), diagnostics::Diagnostic> {
    for (_, contract) in proof.contract_facts.iter() {
        let Some(authorization) = contract.qualification_authorization else {
            continue;
        };
        if issuer_handle(facts, authorization).is_valid() {
            continue;
        }
        let owner = program
            .trait_definition_by_symbol(authorization.requirement_symbol)
            .filter(|owner| owner.is_boundary)
            .ok_or_else(|| {
                diagnostics::Diagnostic::error(format!(
                    "qualification issuer {:?} has no exact boundary declaration",
                    authorization.requirement_symbol,
                ))
            })?;
        let requirement = program
            .trait_machine_signature_by_symbol(owner, authorization.signature_symbol)
            .ok_or_else(|| {
                diagnostics::Diagnostic::error(format!(
                    "qualification issuer {:?} does not own exact requirement {:?}",
                    authorization.requirement_symbol, authorization.signature_symbol,
                ))
            })?;
        let is_product_owner = program
            .symbols
            .symbol_provenance_source_span(owner.symbol)
            .and_then(|span| program.symbols.source_file(span))
            .is_none_or(|file| file.dependency_scope == source::DependencyScope::Product);
        facts
            .qualification_issuers
            .append(BoundaryQualificationIssuerRecord {
                source_symbol: authorization.requirement_symbol,
                requirement_symbol: authorization.signature_symbol,
                issuer: language_semantics::BoundaryQualificationIssuer {
                    requirement_identity: program
                        .trait_requirement_callable_identity_text(owner, requirement),
                    boundary_path: program.trait_declaration_path(owner),
                    boundary_package: program.symbols.symbol_package_identity(owner.symbol),
                    is_product_owner,
                },
            });
    }
    Ok(())
}

fn issuer_handle(
    facts: &FactPlan,
    authorization: checked_trees::BoundaryQualificationAuthorization,
) -> arena::Handle<BoundaryQualificationIssuerRecord> {
    facts
        .qualification_issuers
        .iter()
        .find_map(|(handle, record)| {
            (record.source_symbol == authorization.requirement_symbol
                && record.requirement_symbol == authorization.signature_symbol)
                .then_some(handle)
        })
        .unwrap_or_default()
}

fn admitted_evidence(
    facts: &FactPlan,
    authorization: checked_trees::BoundaryQualificationAuthorization,
) -> Option<QualificationEvidence> {
    let issuer = issuer_handle(facts, authorization);
    issuer.is_valid().then(|| {
        QualificationEvidence::from_admitted_requirement(
            authorization.requirement_symbol,
            authorization.signature_symbol,
            issuer,
        )
    })
}

pub(crate) fn call_contract_evidence(
    program: &dyn ::validation::CheckingInput,
    facts: &FactPlan,
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
            facts,
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

    match ::validation::call_qualification_source(
        program,
        target_symbol,
        contract.qualification_authorization,
    )? {
        ::validation::CallQualificationSource::CheckedBody(machine) => {
            let origin =
                if machine_domain_establishment_route(program, machine, domain_symbol).is_some() {
                    QualificationEvidenceOrigin::AuthorizedRouteEstablishment
                } else if domain.predicate_presence.is_present() {
                    QualificationEvidenceOrigin::Prover
                } else {
                    QualificationEvidenceOrigin::CheckedTransformation
                };
            Some(QualificationEvidence::from_origin(origin, target_symbol))
        }
        ::validation::CallQualificationSource::AdmittedRequirement(authorization) => {
            admitted_evidence(facts, authorization)
        }
        ::validation::CallQualificationSource::Propagated => {
            Some(propagated_evidence(target_symbol))
        }
    }
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
    facts: &FactPlan,
    target_symbol: SymbolHandle,
    _target_state_symbol: SymbolHandle,
    contract: &ContractProofFact,
) -> Option<QualificationEvidence> {
    match ::validation::call_qualification_source(
        program,
        target_symbol,
        contract.qualification_authorization,
    )? {
        ::validation::CallQualificationSource::CheckedBody(_) => {
            Some(QualificationEvidence::from_origin(
                QualificationEvidenceOrigin::CheckedTransformation,
                target_symbol,
            ))
        }
        ::validation::CallQualificationSource::AdmittedRequirement(authorization) => {
            admitted_evidence(facts, authorization)
        }
        ::validation::CallQualificationSource::Propagated => {
            Some(propagated_evidence(target_symbol))
        }
    }
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
    if domain.predicate_presence.is_present() {
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

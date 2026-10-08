use arena::Handle;
use checked_trees::{BoundaryQualificationAuthorization, ContractProofFact};
use facts::{FactPayload, QualificationEvidence};
use language_semantics::{
    DomainEstablishmentRoute, MachineSupplyMode, QualificationEvidenceOrigin,
};
use symbols::JoinedPath;
use symbols::SymbolHandle;
use typed_trees::domain::ProofFact;
use typed_trees::machine::Machine;
use typed_trees::signature::{SignatureContractKind, StateSignature};
use typed_trees::types::TypeReferenceHandle;

pub(crate) fn domain_definition(
    program: &dyn ::validation::CheckingInput,
    domain_symbol: SymbolHandle,
) -> Option<&typed_trees::domain::DomainDefinition> {
    program.domain_definition(domain_symbol)
}

pub(crate) fn boundary_qualification_authorization(
    program: &dyn ::validation::CheckingInput,
    owner_symbol: SymbolHandle,
    signature: &StateSignature,
    contract_kind: SignatureContractKind,
    fact: Handle<ProofFact>,
) -> Option<BoundaryQualificationAuthorization> {
    if contract_kind != SignatureContractKind::Ensures
        || !program
            .traits()
            .iter()
            .any(|definition| definition.symbol == owner_symbol && definition.is_boundary)
    {
        return None;
    }

    let membership = program.proof_facts.get(fact).as_membership()?;
    let subject_carrier = ensured_subject_carrier(program, signature, membership.value)?;
    if membership_carry_permission(program, membership).is_some() {
        return Some(BoundaryQualificationAuthorization {
            requirement_symbol: owner_symbol,
            signature_symbol: signature.symbol,
        });
    }
    if !membership.domain_symbol.is_valid() {
        return None;
    }
    let domain = domain_definition(program, membership.domain_symbol)?;
    if membership.authored_domain_selection.is_none()
        || membership.domain_arguments.len() != domain.type_parameters.len()
    {
        return None;
    }
    if !domain.establishment_routes.iter().any(|route| {
        matches!(
            route,
            DomainEstablishmentRoute::BoundaryRequirement {
                boundary_trait,
                requirement,
            } if *boundary_trait == owner_symbol && *requirement == signature.symbol
        )
    }) {
        return None;
    }
    if !return_type_matches_domain_target(program, subject_carrier, domain.target_type) {
        return None;
    }

    Some(BoundaryQualificationAuthorization {
        requirement_symbol: owner_symbol,
        signature_symbol: signature.symbol,
    })
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
            let origin = if let Some(origin) =
                machine_domain_establishment_origin(program, machine, domain_symbol)
            {
                origin
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
    machine_domain_establishment_origin(program, machine, domain_symbol).is_some()
}

fn machine_domain_establishment_origin(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    domain_symbol: SymbolHandle,
) -> Option<QualificationEvidenceOrigin> {
    if !matches!(
        machine.supply_mode,
        MachineSupplyMode::CheckedBody | MachineSupplyMode::Boundary
    ) {
        return None;
    }
    let domain = domain_definition(program, domain_symbol)?;
    let machine_name = machine
        .name
        .as_str()
        .rsplit("::")
        .next()
        .unwrap_or(machine.name.as_str());

    domain
        .establishment_routes
        .iter()
        .find_map(|route| match route {
            DomainEstablishmentRoute::CheckedRequirement {
                trait_definition,
                requirement,
            } => {
                let trait_definition = program.trait_definition_by_symbol(*trait_definition)?;
                let requirement =
                    program.trait_machine_signature_by_symbol(trait_definition, *requirement)?;
                program
                    .machine_trait_conformances(machine)
                    .iter()
                    .any(|conformance| {
                        conformance.symbol == trait_definition.symbol
                            && conformance
                                .requirement
                                .as_ref()
                                .map(|name| name.as_str())
                                .unwrap_or(machine_name)
                                == requirement.name.as_str()
                    })
                    .then_some(QualificationEvidenceOrigin::AuthorizedRouteEstablishment)
            }
            DomainEstablishmentRoute::ExactMachine {
                machine: authorized,
            } => (*authorized == machine.symbol)
                .then_some(QualificationEvidenceOrigin::AuthorizedRouteEstablishment),
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
/// The carrier an `ensures` membership mints into: `result` for the exact
/// result, or the exact `&mut` non-self parameter a boundary requirement
/// mints through caller storage it borrows mutably. Other parameters are
/// caller premises — nothing may establish into them.
fn ensured_subject_carrier(
    program: &dyn ::validation::CheckingInput,
    signature: &StateSignature,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<TypeReferenceHandle> {
    if program.expression_table.names_reserved_result(expression) {
        return Some(signature.return_type);
    }
    let Some((path, name)) = program.expression_table.single_name_member(expression) else {
        return None;
    };
    program
        .state_signature_parameters(signature)
        .iter()
        .find(|parameter| {
            (crate::checks::contracts::is_readable_mutable_reference(
                program,
                parameter.type_reference,
            )) && (parameter.name.as_str() == name.as_str()
                || parameter.symbol == path.symbol
                || parameter.symbol == path.head_symbol)
        })
        .map(|parameter| parameter.type_reference)
}

fn membership_carry_permission(
    program: &dyn ::validation::CheckingInput,
    membership: &typed_trees::domain::ProofMembershipFact,
) -> Option<language_semantics::CarryPermission> {
    carry_permission_from_path(program, membership.domain)
}

fn carry_permission_from_path(
    program: &dyn ::validation::CheckingInput,
    domain: arena::HandleSpan<typed_trees::name::Identifier>,
) -> Option<language_semantics::CarryPermission> {
    let name = program.domain_path_members(domain).joined_path();
    language_semantics::CarryPermission::from_name(&name)
}

/// A boundary requirement's return type realizes the domain target. An
/// explicitly lifetime-parameterized carrier spells its result as
/// `PendingRead<'a>` while the domain target is the bare nominal; the lifetime
/// slot is erased from the semantic identity, so both sides compare by head
/// nominal when no runtime generic arguments remain.
fn return_type_matches_domain_target(
    program: &dyn ::validation::CheckingInput,
    return_type: TypeReferenceHandle,
    domain_target: TypeReferenceHandle,
) -> bool {
    if unwrapped_type_references_match(program, return_type, domain_target) {
        return true;
    }
    nominal_head_symbol(program, return_type)
        .zip(nominal_head_symbol(program, domain_target))
        .is_some_and(|(left, right)| left == right)
}

fn nominal_head_symbol(
    program: &dyn ::validation::CheckingInput,
    type_reference: TypeReferenceHandle,
) -> Option<SymbolHandle> {
    let unwrapped = program.unwrapped_type_reference(type_reference)?;
    program
        .type_reference_table
        .closed_nominal_symbol(unwrapped)
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

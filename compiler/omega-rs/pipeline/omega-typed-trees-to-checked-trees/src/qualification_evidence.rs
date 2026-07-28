use omega_checked_trees::{BoundaryQualificationAuthorization, ContractProofFact};
use omega_core::arena::Handle;
use omega_core::semantics::{
    DomainEstablishmentRoute, DomainPredicateBody, MachineSupplyMode, QualificationEvidenceOrigin,
};
use omega_core::symbols::SymbolHandle;
use omega_facts::{FactPayload, QualificationEvidence};
use omega_typed_trees::TypedTrees;
use omega_typed_trees::domain::ProofFact;
use omega_typed_trees::expression::ExpressionNode;
use omega_typed_trees::machine::Machine;
use omega_typed_trees::signature::{SignatureContractKind, StateSignature};
use omega_typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

pub(crate) fn domain_definition<'program>(
    program: &'program TypedTrees,
    domain_symbol: SymbolHandle,
) -> Option<&'program omega_typed_trees::domain::DomainDefinition> {
    program
        .domain_definitions()
        .iter()
        .find(|domain| domain.symbol == domain_symbol)
}

pub(crate) fn machine_owns_bodyless_domain(
    program: &TypedTrees,
    machine: &Machine,
    domain_symbol: SymbolHandle,
) -> bool {
    if machine.supply_mode != MachineSupplyMode::CheckedBody {
        return false;
    }
    machine_matches_bodyless_domain_owner(program, machine, domain_symbol)
}

fn machine_matches_bodyless_domain_owner(
    program: &TypedTrees,
    machine: &Machine,
    domain_symbol: SymbolHandle,
) -> bool {
    let Some(domain) = domain_definition(program, domain_symbol) else {
        return false;
    };
    if domain.predicate_body != DomainPredicateBody::Bodyless {
        return false;
    }
    domain.establishment_routes.iter().any(|route| {
        let DomainEstablishmentRoute::OwnerCheckedMachine {
            machine: route_machine,
        } = route
        else {
            return false;
        };
        *route_machine == machine.symbol
            || program
                .machine_specializations
                .iter()
                .any(|specialization| {
                    specialization.template == *route_machine
                        && specialization.instance == machine.symbol
                })
    })
}

pub(crate) fn boundary_qualification_authorization(
    program: &TypedTrees,
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

    let ProofFact::Membership(membership) = program.proof_facts.get(fact) else {
        return None;
    };
    if !expression_is_bare_result(program, membership.value) {
        return None;
    }
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
    if !unwrapped_type_references_match(program, signature.return_type, domain.target_type) {
        return None;
    }

    Some(BoundaryQualificationAuthorization {
        requirement_symbol: owner_symbol,
        signature_symbol: signature.symbol,
    })
}

pub(crate) fn call_contract_evidence(
    program: &TypedTrees,
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

    if let Some(machine) = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == target_symbol)
    {
        let checked_body = match machine.supply_mode {
            MachineSupplyMode::CheckedBody => true,
            MachineSupplyMode::Boundary => program
                .machine_states(machine)
                .iter()
                .find(|state| state.symbol == target_state_symbol)
                .is_some_and(|state| {
                    !program
                        .statement_table
                        .statements(state.statement_nodes)
                        .is_empty()
                }),
            MachineSupplyMode::Accepted
            | MachineSupplyMode::Requirement
            | MachineSupplyMode::ExternalRealization { .. } => false,
        };
        if checked_body {
            // A checked adapter inherits the boundary requirement so its body
            // can be validated against that requirement, but a direct call to
            // the adapter is not the admitted crossing. Only a call whose
            // target is the boundary trait/signature may consume the attached
            // authorization and originate its qualified result.
            if contract.qualification_authorization.is_some() {
                return None;
            }
            let origin = if domain.predicate_body.is_present() {
                QualificationEvidenceOrigin::Prover
            } else if machine_matches_bodyless_domain_owner(program, machine, domain_symbol) {
                QualificationEvidenceOrigin::OwnerEstablishment
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

    if let Some(trait_definition) = program
        .traits()
        .iter()
        .find(|definition| definition.symbol == target_symbol)
    {
        if trait_definition.is_boundary {
            return contract.qualification_authorization.map(|authorization| {
                QualificationEvidence::from_admitted_requirement(
                    authorization.requirement_symbol,
                    authorization.signature_symbol,
                )
            });
        }
    }

    Some(QualificationEvidence::from_origin(
        QualificationEvidenceOrigin::Propagated,
        target_symbol,
    ))
}

fn call_carry_permission_evidence(
    program: &TypedTrees,
    target_symbol: SymbolHandle,
    target_state_symbol: SymbolHandle,
    contract: &ContractProofFact,
) -> Option<QualificationEvidence> {
    if let Some(machine) = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == target_symbol)
    {
        let checked_body = match machine.supply_mode {
            MachineSupplyMode::CheckedBody => true,
            MachineSupplyMode::Boundary => program
                .machine_states(machine)
                .iter()
                .find(|state| state.symbol == target_state_symbol)
                .is_some_and(|state| {
                    !program
                        .statement_table
                        .statements(state.statement_nodes)
                        .is_empty()
                }),
            MachineSupplyMode::Accepted
            | MachineSupplyMode::Requirement
            | MachineSupplyMode::ExternalRealization { .. } => false,
        };
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
    program: &TypedTrees,
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

    let operator_is_owned_by_domain = domain.establishment_routes.iter().any(|route| {
        matches!(
            route,
            DomainEstablishmentRoute::OwnerOperator { operator }
                if *operator == operator_symbol
        )
    });
    QualificationEvidence::from_origin(
        if operator_is_owned_by_domain {
            QualificationEvidenceOrigin::OwnerEstablishment
        } else {
            QualificationEvidenceOrigin::CheckedTransformation
        },
        operator_symbol,
    )
}

fn expression_is_bare_result(
    program: &TypedTrees,
    expression: omega_typed_trees::expression::ExpressionHandle,
) -> bool {
    let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
        return false;
    };
    let [name] = program.expression_table.name_path_members(path.members) else {
        return false;
    };
    name.as_str() == "result"
}

fn membership_carry_permission(
    program: &TypedTrees,
    membership: &omega_typed_trees::domain::ProofMembershipFact,
) -> Option<omega_core::semantics::CarryPermission> {
    carry_permission_from_path(program, membership.domain)
}

fn carry_permission_from_path(
    program: &TypedTrees,
    domain: omega_core::arena::HandleSpan<omega_typed_trees::name::Identifier>,
) -> Option<omega_core::semantics::CarryPermission> {
    let name = program
        .domain_path_members(domain)
        .iter()
        .map(|member| member.as_str())
        .collect::<Vec<_>>()
        .join("::");
    omega_core::semantics::CarryPermission::from_name(&name)
}

fn unwrapped_type_references_match(
    program: &TypedTrees,
    left: TypeReferenceHandle,
    right: TypeReferenceHandle,
) -> bool {
    let Some(left) = unwrapped_type_reference(program, left) else {
        return false;
    };
    let Some(right) = unwrapped_type_reference(program, right) else {
        return false;
    };
    program.normalized_type_identity(left) == program.normalized_type_identity(right)
}

fn unwrapped_type_reference(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<TypeReferenceHandle> {
    if !type_reference.is_valid() {
        return None;
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => unwrapped_type_reference(program, *referee),
        TypeReferenceNode::Constrained { base_type, .. } => {
            unwrapped_type_reference(program, *base_type)
        }
        _ => Some(type_reference),
    }
}

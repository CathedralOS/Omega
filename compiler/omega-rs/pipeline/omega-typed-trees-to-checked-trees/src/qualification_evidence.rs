use omega_core::semantics::{DomainPredicateBody, MachineSupplyMode, QualificationEvidenceOrigin};
use omega_core::symbols::SymbolHandle;
use omega_facts::{FactPayload, QualificationEvidence};
use omega_typed_trees::TypedTrees;
use omega_typed_trees::machine::Machine;
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
    let Some(domain) = domain_definition(program, domain_symbol) else {
        return false;
    };
    if domain.predicate_body != DomainPredicateBody::Bodyless {
        return false;
    }
    let Some(attached) = machine.attached_data.as_ref() else {
        return false;
    };
    named_carrier(program, domain.target_type)
        .is_some_and(|carrier| same_semantic_name(attached.as_str(), carrier))
}

pub(crate) fn call_contract_evidence(
    program: &TypedTrees,
    target_symbol: SymbolHandle,
    payload: FactPayload,
    is_ensures: bool,
) -> QualificationEvidence {
    if !is_ensures {
        return QualificationEvidence::default();
    }
    let FactPayload::ContractDomainMembership { domain_symbol, .. } = payload else {
        return QualificationEvidence::default();
    };
    let Some(domain) = domain_definition(program, domain_symbol) else {
        return QualificationEvidence::default();
    };

    if let Some(machine) = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == target_symbol)
    {
        let origin = match machine.supply_mode {
            MachineSupplyMode::Accepted
            | MachineSupplyMode::Boundary
            | MachineSupplyMode::Requirement
            | MachineSupplyMode::ExternalRealization { .. } => {
                QualificationEvidenceOrigin::AdmittedReceipt
            }
            MachineSupplyMode::CheckedBody if domain.predicate_body.is_present() => {
                QualificationEvidenceOrigin::Prover
            }
            MachineSupplyMode::CheckedBody
                if machine_owns_bodyless_domain(program, machine, domain_symbol) =>
            {
                QualificationEvidenceOrigin::OwnerEstablishment
            }
            MachineSupplyMode::CheckedBody => QualificationEvidenceOrigin::CheckedTransformation,
        };
        return QualificationEvidence::from_origin(origin, target_symbol);
    }

    if program
        .traits()
        .iter()
        .any(|definition| definition.symbol == target_symbol && definition.is_boundary)
    {
        return QualificationEvidence::from_origin(
            QualificationEvidenceOrigin::AdmittedReceipt,
            target_symbol,
        );
    }

    QualificationEvidence::from_origin(QualificationEvidenceOrigin::Propagated, target_symbol)
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

    let operator_is_owned_by_domain = program.domain_definitions().iter().any(|owner| {
        owner.symbol == domain_symbol
            && program
                .domain_operators(owner)
                .iter()
                .any(|operator| operator.symbol == operator_symbol)
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

fn named_carrier(program: &TypedTrees, type_reference: TypeReferenceHandle) -> Option<&str> {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Named { name, .. } => Some(name.as_str()),
        TypeReferenceNode::Reference { referee, .. } => named_carrier(program, *referee),
        TypeReferenceNode::Constrained { base_type, .. } => named_carrier(program, *base_type),
        _ => None,
    }
}

fn same_semantic_name(left: &str, right: &str) -> bool {
    left == right
        || (!left.contains("::") && right.rsplit("::").next().is_some_and(|leaf| leaf == left))
        || (!right.contains("::") && left.rsplit("::").next().is_some_and(|leaf| leaf == right))
}

#[cfg(test)]
mod tests {
    use super::same_semantic_name;

    #[test]
    fn owner_name_matching_accepts_only_equal_or_unqualified_paths() {
        assert!(same_semantic_name("Token", "Token"));
        assert!(same_semantic_name("Token", "pkg::Token"));
        assert!(same_semantic_name("pkg::Token", "Token"));
        assert!(!same_semantic_name("left::Token", "right::Token"));
        assert!(!same_semantic_name("Token", "Other"));
    }
}

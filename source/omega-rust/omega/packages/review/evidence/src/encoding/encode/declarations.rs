use super::PackageReviewEncodingError;
use super::encoder::Encoder;
use super::values::contracts::{encode_callable_contract, encode_contract_fact};
use super::values::crashes::encode_crash_route;
use super::values::declarations::encode_evidence_interface;
use super::values::effects::{encode_synchronous_invocation, encode_termination};
use super::values::expressions::encode_contract_static_argument;
use super::values::identity::encode_nominal;
use crate::record::{
    PackageReviewConformanceBound, PackageReviewConformanceShape, PackageReviewConformanceSubject,
    PackageReviewDangerousAuthority, PackageReviewDangerousAuthorityClass,
    PackageReviewDangerousAuthoritySlack, PackageReviewDataField, PackageReviewDataKind,
    PackageReviewDataMember, PackageReviewDataProperties, PackageReviewDataShape,
    PackageReviewDomainAliasAtom, PackageReviewDomainClassification,
    PackageReviewDomainEstablishmentKind, PackageReviewDomainEstablishmentRoute,
    PackageReviewDomainSemanticRole, PackageReviewDomainShape,
    PackageReviewMachineParameterContract, PackageReviewMachineParameterSignature,
    PackageReviewRepresentationTcb, PackageReviewRepresentationTcbKind,
    PackageReviewSemanticDependency, PackageReviewSemanticDependencyExposure,
    PackageReviewSemanticDependencyKind, PackageReviewTraitCompositionKind,
    PackageReviewTraitParent, PackageReviewTraitRequirement, PackageReviewTraitShape,
    PackageReviewTypeIdentity, PackageReviewTypeParameter, PackageReviewTypeParameterKind,
};

pub(crate) fn encode_conformance_shape(
    encoder: &mut Encoder,
    shape: &PackageReviewConformanceShape,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &shape.identity)?;
    encoder.usize(shape.lifetime_parameter_count)?;
    encoder.sequence(&shape.type_parameters, encode_type_parameter)?;
    match &shape.subject {
        PackageReviewConformanceSubject::Subjectless => encoder.byte(0),
        PackageReviewConformanceSubject::TypeParameter(ordinal) => {
            encoder.byte(1);
            encoder.u32(*ordinal);
        }
        PackageReviewConformanceSubject::Nominal(identity) => {
            encoder.byte(2);
            encode_nominal(encoder, identity)?;
        }
    }
    encode_evidence_interface(encoder, &shape.interface)
}

pub(crate) fn encode_semantic_dependency_key(
    encoder: &mut Encoder,
    dependency: &PackageReviewSemanticDependency,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &dependency.consumer)?;
    encode_nominal(encoder, &dependency.dependency)?;
    encoder.byte(semantic_dependency_kind_tag(dependency.kind));
    Ok(())
}

pub(crate) fn encode_semantic_dependency(
    encoder: &mut Encoder,
    dependency: &PackageReviewSemanticDependency,
) -> Result<(), PackageReviewEncodingError> {
    encode_semantic_dependency_key(encoder, dependency)?;
    encoder.byte(match dependency.exposure {
        PackageReviewSemanticDependencyExposure::PrivateImplementation => 0,
        PackageReviewSemanticDependencyExposure::PublicInterface => 1,
    });
    Ok(())
}

pub(crate) const fn semantic_dependency_kind_tag(kind: PackageReviewSemanticDependencyKind) -> u8 {
    match kind {
        PackageReviewSemanticDependencyKind::NominalIdentity => 0,
        PackageReviewSemanticDependencyKind::Layout => 1,
        PackageReviewSemanticDependencyKind::OwnershipBehavior => 2,
        PackageReviewSemanticDependencyKind::AutomaticCleanup => 3,
        PackageReviewSemanticDependencyKind::AutomaticCleanupMachine => 4,
    }
}

pub(crate) fn encode_representation_tcb_key(
    encoder: &mut Encoder,
    row: &PackageReviewRepresentationTcb,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &row.declaration)?;
    match &row.kind {
        PackageReviewRepresentationTcbKind::Unbound => encoder.byte(0),
        PackageReviewRepresentationTcbKind::ProducerAvailability { conformance, .. } => {
            encoder.byte(1);
            encode_nominal(encoder, conformance)?;
        }
    }
    Ok(())
}

pub(crate) fn encode_representation_tcb(
    encoder: &mut Encoder,
    row: &PackageReviewRepresentationTcb,
) -> Result<(), PackageReviewEncodingError> {
    encode_representation_tcb_key(encoder, row)?;
    if let PackageReviewRepresentationTcbKind::ProducerAvailability { carrier, .. } = &row.kind {
        encode_nominal(encoder, carrier)?;
    }
    Ok(())
}

pub(crate) fn encode_dangerous_authority(
    encoder: &mut Encoder,
    authority: &PackageReviewDangerousAuthority,
) -> Result<(), PackageReviewEncodingError> {
    encoder.byte(match authority.class {
        PackageReviewDangerousAuthorityClass::Filesystem => 0,
        PackageReviewDangerousAuthorityClass::MachineControl => 1,
        PackageReviewDangerousAuthorityClass::PortIo => 2,
        PackageReviewDangerousAuthorityClass::InterruptControl => 3,
        PackageReviewDangerousAuthorityClass::InterruptEntry => 4,
        PackageReviewDangerousAuthorityClass::RootMemory => 5,
        PackageReviewDangerousAuthorityClass::Process => 6,
    });
    encode_nominal(encoder, &authority.service)
}

pub(crate) fn encode_dangerous_authority_slack(
    encoder: &mut Encoder,
    slack: &PackageReviewDangerousAuthoritySlack,
) -> Result<(), PackageReviewEncodingError> {
    encoder.byte(match slack.class {
        PackageReviewDangerousAuthorityClass::Filesystem => 0,
        PackageReviewDangerousAuthorityClass::MachineControl => 1,
        PackageReviewDangerousAuthorityClass::PortIo => 2,
        PackageReviewDangerousAuthorityClass::InterruptControl => 3,
        PackageReviewDangerousAuthorityClass::InterruptEntry => 4,
        PackageReviewDangerousAuthorityClass::RootMemory => 5,
        PackageReviewDangerousAuthorityClass::Process => 6,
    });
    encode_nominal(encoder, &slack.callable)?;
    encode_nominal(encoder, &slack.service)
}

pub(crate) fn encode_trait_shape(
    encoder: &mut Encoder,
    shape: &PackageReviewTraitShape,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &shape.identity)?;
    encoder.boolean(shape.is_boundary);
    encoder.usize(shape.lifetime_parameter_count)?;
    encoder.sequence(&shape.type_parameters, encode_type_parameter)?;
    encoder.sequence(&shape.conformance_bounds, encode_conformance_bound)?;
    encoder.sequence(&shape.parents, encode_trait_parent)?;
    encoder.sequence(&shape.requirements, encode_trait_requirement)
}

pub(crate) fn encode_conformance_bound(
    encoder: &mut Encoder,
    bound: &PackageReviewConformanceBound,
) -> Result<(), PackageReviewEncodingError> {
    match bound.binder_ordinal {
        None => encoder.byte(0),
        Some(ordinal) => {
            encoder.byte(1);
            encoder.u32(ordinal);
        }
    }
    encoder.u32(bound.subject_parameter);
    match (&bound.selected_conformance, &bound.selected_subject) {
        (None, None)
            if bound.selected_lifetime_arguments.is_empty()
                && bound.selected_arguments.is_empty() =>
        {
            encoder.byte(0)
        }
        (Some(conformance), Some(subject)) => {
            encoder.byte(1);
            encode_nominal(encoder, conformance)?;
            encoder.sequence(&bound.selected_lifetime_arguments, |encoder, argument| {
                encoder.u32(*argument);
                Ok(())
            })?;
            encoder.sequence(&bound.selected_arguments, encode_contract_static_argument)?;
            encode_contract_static_argument(encoder, subject)?;
        }
        _ => {
            return Err(PackageReviewEncodingError::new(
                "selected conformance review row has an incomplete application identity",
            ));
        }
    }
    encode_nominal(encoder, &bound.trait_identity)?;
    encoder.sequence(&bound.trait_lifetime_arguments, |encoder, argument| {
        encoder.u32(*argument);
        Ok(())
    })?;
    encoder.sequence(&bound.arguments, encode_type_identity)
}

pub(crate) fn encode_trait_parent(
    encoder: &mut Encoder,
    parent: &PackageReviewTraitParent,
) -> Result<(), PackageReviewEncodingError> {
    encoder.byte(match parent.kind {
        PackageReviewTraitCompositionKind::Policy => 0,
        PackageReviewTraitCompositionKind::ServiceReach => 1,
    });
    encode_nominal(encoder, &parent.identity)?;
    encoder.sequence(&parent.lifetime_arguments, |encoder, argument| {
        encoder.u32(*argument);
        Ok(())
    })?;
    encoder.sequence(&parent.arguments, encode_type_identity)
}

pub(crate) fn encode_trait_requirement(
    encoder: &mut Encoder,
    requirement: &PackageReviewTraitRequirement,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &requirement.identity)?;
    match requirement.spelling {
        None => encoder.byte(0),
        Some(spelling) => {
            encoder.byte(1);
            encoder.byte(match spelling {
                psi_language_core::OperatorSpelling::Add => 0,
                psi_language_core::OperatorSpelling::Subtract => 1,
                psi_language_core::OperatorSpelling::Multiply => 2,
                psi_language_core::OperatorSpelling::Divide => 3,
                psi_language_core::OperatorSpelling::Modulo => 4,
                psi_language_core::OperatorSpelling::Equal => 5,
                psi_language_core::OperatorSpelling::NotEqual => 6,
                psi_language_core::OperatorSpelling::Less => 7,
                psi_language_core::OperatorSpelling::LessEqual => 8,
                psi_language_core::OperatorSpelling::Greater => 9,
                psi_language_core::OperatorSpelling::GreaterEqual => 10,
                psi_language_core::OperatorSpelling::Index => 11,
                psi_language_core::OperatorSpelling::Range => 12,
            });
        }
    }
    encoder.boolean(requirement.has_default_realization);
    encoder.usize(requirement.lifetime_parameter_count)?;
    encoder.sequence(&requirement.type_parameters, encode_type_parameter)?;
    encoder.sequence(&requirement.parameters, |encoder, parameter| {
        encoder.string(&parameter.name)?;
        encode_type_identity(encoder, &parameter.type_identity)?;
        encoder.boolean(parameter.is_const);
        encoder.boolean(parameter.is_mutable);
        encoder.boolean(parameter.is_self);
        Ok(())
    })?;
    encode_type_identity(encoder, &requirement.return_type)?;
    encoder.sequence(&requirement.contracts, encode_callable_contract)?;
    encoder.sequence(&requirement.published_crash, encode_crash_route)?;
    encoder.sequence(&requirement.service_reach, encode_nominal)?;
    encoder.boolean(requirement.service_reach_is_installation_bound);
    encoder.sequence(
        &requirement.synchronous_invocations,
        encode_synchronous_invocation,
    )?;
    encoder.boolean(requirement.suspends);
    encoder.boolean(requirement.blocks);
    encode_termination(encoder, &requirement.termination)?;
    Ok(())
}

pub(crate) fn encode_domain_shape(
    encoder: &mut Encoder,
    shape: &PackageReviewDomainShape,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &shape.identity)?;
    encoder.sequence(&shape.type_parameters, encode_type_parameter)?;
    encode_type_identity(encoder, &shape.target_type)?;
    encoder.sequence(&shape.index_arguments, encode_type_identity)?;
    encoder.byte(match shape.predicate_body {
        psi_language_semantics::DomainPredicateBody::Bodyless => 0,
        psi_language_semantics::DomainPredicateBody::Present => 1,
    });
    encoder.sequence(&shape.predicate_facts, encode_contract_fact)?;
    match &shape.alias_expansion {
        None => encoder.byte(0),
        Some(atoms) => {
            encoder.byte(1);
            encoder.sequence(atoms, encode_domain_alias_atom)?;
        }
    }
    match shape.classification {
        None => encoder.byte(0),
        Some(PackageReviewDomainClassification::ProgressProfile) => encoder.byte(1),
    }
    encoder.sequence(&shape.semantic_roles, |encoder, role| {
        encoder.byte(match role {
            PackageReviewDomainSemanticRole::DenotationDimension => 0,
            PackageReviewDomainSemanticRole::ArithmeticPolicy => 1,
        });
        Ok(())
    })?;
    encoder.sequence(
        &shape.establishment_routes,
        encode_domain_establishment_route,
    )
}

pub(crate) fn encode_domain_alias_atom(
    encoder: &mut Encoder,
    atom: &PackageReviewDomainAliasAtom,
) -> Result<(), PackageReviewEncodingError> {
    match atom {
        PackageReviewDomainAliasAtom::Declared(identity) => {
            encoder.byte(0);
            encode_nominal(encoder, identity)
        }
        PackageReviewDomainAliasAtom::Carry(permission) => {
            encoder.byte(1);
            encoder.byte(match permission {
                psi_language_semantics::CarryPermission::AcrossSuspend => 0,
                psi_language_semantics::CarryPermission::AnyCpu => 1,
                psi_language_semantics::CarryPermission::AnyThread => 2,
                psi_language_semantics::CarryPermission::MovableAddress => 3,
            });
            Ok(())
        }
    }
}

pub(crate) fn encode_domain_establishment_route(
    encoder: &mut Encoder,
    route: &PackageReviewDomainEstablishmentRoute,
) -> Result<(), PackageReviewEncodingError> {
    encoder.byte(match route.kind {
        PackageReviewDomainEstablishmentKind::CheckedRequirement => 0,
        PackageReviewDomainEstablishmentKind::BoundaryRequirement => 1,
    });
    encode_nominal(encoder, &route.trait_identity)?;
    encode_nominal(encoder, &route.requirement_identity)
}

pub(crate) fn encode_data_shape(
    encoder: &mut Encoder,
    shape: &PackageReviewDataShape,
) -> Result<(), PackageReviewEncodingError> {
    encode_nominal(encoder, &shape.identity)?;
    match &shape.kind {
        PackageReviewDataKind::Ordinary => encoder.byte(0),
        PackageReviewDataKind::Quotient { carrier, relation } => {
            encoder.byte(1);
            encode_type_identity(encoder, carrier)?;
            encode_nominal(encoder, relation)?;
        }
    }
    encoder.byte(match shape.supply {
        psi_language_semantics::DataSupplyMode::CheckedShape => 0,
        psi_language_semantics::DataSupplyMode::BoundaryOpaque => 1,
    });
    encoder.usize(shape.lifetime_parameter_count)?;
    encoder.sequence(&shape.type_parameters, encode_type_parameter)?;
    encode_data_properties(encoder, shape.properties);
    encoder.boolean(shape.zero_gated);
    encoder.sequence(&shape.invariants, encode_contract_fact)?;
    encoder.sequence(&shape.retired_identities, |encoder, identity| {
        encoder.u64(*identity);
        Ok(())
    })?;
    encoder.sequence(&shape.members, encode_data_member)
}

pub(crate) fn encode_type_parameter(
    encoder: &mut Encoder,
    parameter: &PackageReviewTypeParameter,
) -> Result<(), PackageReviewEncodingError> {
    match &parameter.kind {
        PackageReviewTypeParameterKind::Type => encoder.byte(0),
        PackageReviewTypeParameterKind::Const(type_identity) => {
            encoder.byte(1);
            encode_type_identity(encoder, type_identity)?;
        }
        PackageReviewTypeParameterKind::Machine(contract) => {
            encoder.byte(2);
            encode_machine_parameter_contract(encoder, contract)?;
        }
        PackageReviewTypeParameterKind::Proposition(signature) => {
            encoder.byte(3);
            encoder.sequence(&signature.parameters, |encoder, parameter| {
                encode_type_identity(encoder, &parameter.type_identity)
            })?;
        }
    }
    encode_data_properties(encoder, parameter.bounds);
    Ok(())
}

pub(crate) fn encode_machine_parameter_contract(
    encoder: &mut Encoder,
    contract: &PackageReviewMachineParameterContract,
) -> Result<(), PackageReviewEncodingError> {
    match contract {
        PackageReviewMachineParameterContract::Structural(signature) => {
            encoder.byte(0);
            encode_machine_parameter_signature(encoder, signature)
        }
        PackageReviewMachineParameterContract::Nominal {
            trait_identity,
            requirement_identity,
        } => {
            encoder.byte(1);
            encode_nominal(encoder, trait_identity)?;
            encode_nominal(encoder, requirement_identity)
        }
        PackageReviewMachineParameterContract::RequirementIdentity => {
            encoder.byte(2);
            Ok(())
        }
    }
}

pub(crate) fn encode_machine_parameter_signature(
    encoder: &mut Encoder,
    signature: &PackageReviewMachineParameterSignature,
) -> Result<(), PackageReviewEncodingError> {
    encoder.usize(signature.lifetime_parameter_count)?;
    encoder.sequence(&signature.type_parameters, encode_type_parameter)?;
    encoder.sequence(&signature.parameters, |encoder, parameter| {
        encoder.string(&parameter.name)?;
        encode_type_identity(encoder, &parameter.type_identity)?;
        encoder.boolean(parameter.is_const);
        encoder.boolean(parameter.is_mutable);
        encoder.boolean(parameter.is_self);
        Ok(())
    })?;
    encode_type_identity(encoder, &signature.return_type)?;
    encoder.sequence(&signature.contracts, encode_callable_contract)?;
    encoder.sequence(&signature.published_crash, encode_crash_route)?;
    encoder.sequence(&signature.service_reach, encode_nominal)?;
    encoder.boolean(signature.service_reach_is_installation_bound);
    encoder.sequence(
        &signature.synchronous_invocations,
        encode_synchronous_invocation,
    )?;
    encoder.boolean(signature.suspends);
    encoder.boolean(signature.blocks);
    encode_termination(encoder, &signature.termination)
}

pub(crate) fn encode_data_properties(
    encoder: &mut Encoder,
    properties: PackageReviewDataProperties,
) {
    encoder.byte(match properties.multiplicity {
        psi_language_semantics::Multiplicity::Unrestricted => 0,
        psi_language_semantics::Multiplicity::Affine => 1,
        psi_language_semantics::Multiplicity::Linear => 2,
    });
    match properties.carry {
        None => encoder.byte(0),
        Some(carry) => {
            encoder.byte(1);
            encoder.byte(match carry.suspension {
                psi_language_semantics::CarrySuspension::Forbidden => 0,
                psi_language_semantics::CarrySuspension::Allowed => 1,
            });
            encoder.byte(match carry.cpu {
                psi_language_semantics::CarryCpu::Origin => 0,
                psi_language_semantics::CarryCpu::Any => 1,
            });
            encoder.byte(match carry.host_thread {
                psi_language_semantics::CarryHostThread::Origin => 0,
                psi_language_semantics::CarryHostThread::Any => 1,
            });
            encoder.byte(match carry.address {
                psi_language_semantics::CarryAddress::Stable => 0,
                psi_language_semantics::CarryAddress::Movable => 1,
            });
        }
    }
}

pub(crate) fn encode_data_member(
    encoder: &mut Encoder,
    member: &PackageReviewDataMember,
) -> Result<(), PackageReviewEncodingError> {
    match member {
        PackageReviewDataMember::Field(field) => {
            encoder.byte(0);
            encode_data_field(encoder, field)?;
        }
        PackageReviewDataMember::Variant {
            identity,
            name,
            payload,
            retired_payload_identities,
        } => {
            encoder.byte(1);
            encode_optional_u64(encoder, *identity);
            encoder.string(name)?;
            encoder.sequence(payload, encode_data_field)?;
            encoder.sequence(retired_payload_identities, |encoder, identity| {
                encoder.u64(*identity);
                Ok(())
            })?;
        }
    }
    Ok(())
}

pub(crate) fn encode_data_field(
    encoder: &mut Encoder,
    field: &PackageReviewDataField,
) -> Result<(), PackageReviewEncodingError> {
    encode_optional_u64(encoder, field.identity);
    encoder.string(&field.name)?;
    encode_relevance(encoder, field.relevance);
    encode_type_identity(encoder, &field.type_identity)
}

pub(crate) fn encode_type_identity(
    encoder: &mut Encoder,
    identity: &PackageReviewTypeIdentity,
) -> Result<(), PackageReviewEncodingError> {
    encoder.string(&identity.canonical)
}

pub(crate) fn encode_relevance(
    encoder: &mut Encoder,
    relevance: psi_language_core::BindingRelevance,
) {
    encoder.byte(match relevance {
        psi_language_core::BindingRelevance::Relevant => 0,
        psi_language_core::BindingRelevance::Erased => 1,
    });
}

pub(crate) fn encode_optional_u64(encoder: &mut Encoder, value: Option<u64>) {
    match value {
        None => encoder.byte(0),
        Some(value) => {
            encoder.byte(1);
            encoder.u64(value);
        }
    }
}

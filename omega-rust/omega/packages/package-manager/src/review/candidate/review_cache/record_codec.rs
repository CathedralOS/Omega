//! Record codec. Sequences are u64 length-prefixed; byte strings are framed;
//! every enum is a checked tag. Unknown tags, trailing bytes and length
//! overflows are all decode failures.

use super::{CachedCompiledReview, CachedOccurrenceFacts};
use effects::provider_plan::{
    BoundaryCallingPlanCommitment, ProviderIdentityTable, ProviderPlanDigest,
    ServiceEntryAuthorityFlow, ServiceEntryClaim, ServiceMethod, ServiceProgressEstablishmentRoute,
    ServiceProgressEstablishmentRouteKind, ServiceProgressPremise, ServiceProgressSubject,
    ServiceReceiverApplication, ServiceResultClaim, ServiceSchema, ServiceSchemaDigest,
};
use effects::{ServiceTerminalAuthorityPermission, TerminalAuthorityDisposition};
use filesystem_authority::observation_codec::{ObservationCursor, push_framed};
use language_semantics::{CarryPolicy, DomainPredicateBody};
use package_compilation::build_execution::build_record_codec;
use package_compilation::review_record_encoding::{
    authority_class_tag, declaration_kind_tag, encode_optional, purpose_tag,
    recover_authority_class, recover_declaration_kind, recover_optional, recover_purpose,
    recover_semantic_binding_role, semantic_binding_role_tag,
};
use package_compilation::{
    AcceptedSemanticBinding, ConsumedSourceUnit, ConsumedSourceUnitKind,
    IndependentComponentDescription, PackageDependencyBinding, PackageDependencyClosure,
    PackageGeneratedSourceBundle, PackageSourceConsumptionCommitment,
};
use package_evidence::record::SemanticBindingReviewCandidate;
use semantic_vocabulary::PackageKeyIdentity;

const RECORD_MAGIC: &[u8; 8] = b"OMGCREV\x01";
// Entry-field tolerance is retired. Older records may describe provisional
// reviews and must miss, never acquire complete admission by dropping their flag.
const RECORD_VERSION: u16 = 11;
const RECORD_SEQUENCES_LIMIT: u64 = 1 << 20;

fn cursor_bool(cursor: &mut ObservationCursor<'_>) -> Option<bool> {
    match cursor.u8()? {
        0 => Some(false),
        1 => Some(true),
        _ => None,
    }
}

fn cursor_digest(cursor: &mut ObservationCursor<'_>) -> Option<[u8; 32]> {
    cursor.bytes(32)?.try_into().ok()
}

fn cursor_string<'a>(cursor: &mut ObservationCursor<'a>) -> Option<&'a str> {
    std::str::from_utf8(cursor.framed()?).ok()
}

fn cursor_vec<'a, T>(
    cursor: &mut ObservationCursor<'a>,
    mut decode: impl FnMut(&mut ObservationCursor<'_>) -> Option<T>,
) -> Option<Vec<T>> {
    let count = cursor.u64()?;
    if count > RECORD_SEQUENCES_LIMIT {
        return None;
    }
    let mut values = Vec::with_capacity(usize::try_from(count).ok()?.min(1 << 20));
    for _ in 0..count {
        values.push(decode(cursor)?);
    }
    Some(values)
}

fn encode_string(value: &str, out: &mut Vec<u8>) {
    push_framed(out, value.as_bytes());
}

fn encode_target(profile: target::TargetProfile, out: &mut Vec<u8>) {
    encode_string(profile.target_name(), out);
}

fn recover_target(cursor: &mut ObservationCursor<'_>) -> Option<target::TargetProfile> {
    let name = cursor_string(cursor)?;
    target::TargetProfile::ALL
        .iter()
        .copied()
        .find(|profile| profile.target_name() == name)
}

fn encode_identity(identity: PackageKeyIdentity, out: &mut Vec<u8>) {
    out.extend_from_slice(&identity.digest());
}

fn recover_identity(cursor: &mut ObservationCursor<'_>) -> Option<PackageKeyIdentity> {
    PackageKeyIdentity::from_digest(cursor_digest(cursor)?)
}

fn encode_disposition(disposition: &TerminalAuthorityDisposition, out: &mut Vec<u8>) {
    crate::codec_bytes::push_encoded(out, (disposition.classes().len() as u64).to_le_bytes());
    for class in disposition.classes() {
        out.push(authority_class_tag(*class));
    }
}

fn recover_disposition(cursor: &mut ObservationCursor<'_>) -> Option<TerminalAuthorityDisposition> {
    let classes = cursor_vec(cursor, recover_authority_class)?;
    TerminalAuthorityDisposition::try_from_canonical_classes(classes).ok()
}

fn encode_permission(
    permission: &ServiceTerminalAuthorityPermission,
    identities: &terminal_psi::TerminalIdentityTable,
    out: &mut Vec<u8>,
) {
    out.extend_from_slice(permission.service_schema().as_bytes());
    encode_string(
        identities
            .text(permission.requirement_identity())
            .unwrap_or_default(),
        out,
    );
    encode_disposition(permission.permitted(), out);
}

fn recover_permission(
    cursor: &mut ObservationCursor<'_>,
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
) -> Option<ServiceTerminalAuthorityPermission> {
    Some(ServiceTerminalAuthorityPermission::new(
        ServiceSchemaDigest::from_digest(cursor_digest(cursor)?),
        identities.intern(cursor_string(cursor)?),
        recover_disposition(cursor)?,
    ))
}

fn encode_binding(binding: &AcceptedSemanticBinding, out: &mut Vec<u8>) {
    out.push(semantic_binding_role_tag(binding.role()));
    encode_identity(binding.package(), out);
    encode_string(binding.declaration_path(), out);
    out.extend_from_slice(binding.normalized_schema_digest().as_bytes());
    encode_optional(
        binding.selected_provider_plan_digest().as_ref(),
        out,
        |digest, out| out.extend_from_slice(digest.as_bytes()),
    );
    crate::codec_bytes::push_encoded(
        out,
        (binding.terminal_authority_permissions().len() as u64).to_le_bytes(),
    );
    for permission in binding.terminal_authority_permissions() {
        encode_permission(
            permission,
            binding.terminal_authority_permission_identities(),
            out,
        );
    }
}

fn recover_binding(cursor: &mut ObservationCursor<'_>) -> Option<AcceptedSemanticBinding> {
    let role = recover_semantic_binding_role(cursor)?;
    let package = recover_identity(cursor)?;
    let declaration_path = cursor_string(cursor)?;
    let schema = ServiceSchemaDigest::from_digest(cursor_digest(cursor)?);
    let plan_digest = recover_optional(cursor, |cursor| {
        Some(ProviderPlanDigest::from_digest(cursor_digest(cursor)?))
    })?;
    let binding = match plan_digest {
        Some(digest) => {
            AcceptedSemanticBinding::new(role, package, declaration_path, schema, digest).ok()?
        }
        None => {
            AcceptedSemanticBinding::new_service(role, package, declaration_path, schema).ok()?
        }
    };
    let mut permission_identities = terminal_psi::TerminalIdentityTableBuilder::default();
    let permissions = cursor_vec(cursor, |cursor| {
        recover_permission(cursor, &mut permission_identities)
    })?;
    binding
        .with_terminal_authority_permissions(permissions, &permission_identities.build())
        .ok()
}

fn encode_carry(carry: &CarryPolicy, out: &mut Vec<u8>) {
    out.push(match carry.suspension {
        language_semantics::CarrySuspension::Forbidden => 0,
        language_semantics::CarrySuspension::Allowed => 1,
    });
    out.push(match carry.cpu {
        language_semantics::CarryCpu::Origin => 0,
        language_semantics::CarryCpu::Any => 1,
    });
    out.push(match carry.host_thread {
        language_semantics::CarryHostThread::Origin => 0,
        language_semantics::CarryHostThread::Any => 1,
    });
    out.push(match carry.address {
        language_semantics::CarryAddress::Stable => 0,
        language_semantics::CarryAddress::Movable => 1,
    });
}

fn recover_carry(cursor: &mut ObservationCursor<'_>) -> Option<CarryPolicy> {
    let suspension = match cursor.u8()? {
        0 => language_semantics::CarrySuspension::Forbidden,
        1 => language_semantics::CarrySuspension::Allowed,
        _ => return None,
    };
    let cpu = match cursor.u8()? {
        0 => language_semantics::CarryCpu::Origin,
        1 => language_semantics::CarryCpu::Any,
        _ => return None,
    };
    let host_thread = match cursor.u8()? {
        0 => language_semantics::CarryHostThread::Origin,
        1 => language_semantics::CarryHostThread::Any,
        _ => return None,
    };
    let address = match cursor.u8()? {
        0 => language_semantics::CarryAddress::Stable,
        1 => language_semantics::CarryAddress::Movable,
        _ => return None,
    };
    Some(CarryPolicy {
        suspension,
        cpu,
        host_thread,
        address,
    })
}

fn encode_predicate_body(body: &DomainPredicateBody, out: &mut Vec<u8>) {
    out.push(match body {
        DomainPredicateBody::Bodyless => 0,
        DomainPredicateBody::Present => 1,
    });
}

fn recover_predicate_body(cursor: &mut ObservationCursor<'_>) -> Option<DomainPredicateBody> {
    Some(match cursor.u8()? {
        0 => DomainPredicateBody::Bodyless,
        1 => DomainPredicateBody::Present,
        _ => return None,
    })
}

fn encode_service_method(
    method: &ServiceMethod,
    identities: &ProviderIdentityTable,
    out: &mut Vec<u8>,
) {
    encode_string(&method.name, out);
    encode_string(&method.requirement_owner, out);
    encode_optional(
        method.requirement_owner_package_identity.as_ref(),
        out,
        |identity, out| encode_identity(*identity, out),
    );
    encode_string(identities.text(method.requirement_identity), out);
    crate::codec_bytes::push_encoded(out, (method.parameter_count as u64).to_le_bytes());
    crate::codec_bytes::push_encoded(
        out,
        (method.parameter_type_identities.len() as u64).to_le_bytes(),
    );
    for identity in &method.parameter_type_identities {
        encode_string(identities.text(*identity), out);
    }
    crate::codec_bytes::push_encoded(out, (method.entry_claims.len() as u64).to_le_bytes());
    for claim in &method.entry_claims {
        crate::codec_bytes::push_encoded(out, (claim.parameter_index as u64).to_le_bytes());
        encode_string(identities.text(claim.carrier_identity), out);
        encode_string(&claim.domain, out);
        encode_predicate_body(&claim.predicate_body, out);
        encode_carry(&claim.effective_carry, out);
        encode_string(claim.authority_flow.as_str(), out);
    }
    out.push(match method.completion {
        semantic_vocabulary::BoundaryCompletion::NormalReturn => 0,
        semantic_vocabulary::BoundaryCompletion::External(
            semantic_vocabulary::ExternalCompletionKind::ProcessExit,
        ) => 1,
    });
    out.push(u8::from(method.has_result));
    encode_optional(
        method
            .has_result
            .then(|| identities.text(method.result_type_identity).to_owned())
            .as_ref(),
        out,
        |identity, out| encode_string(identity, out),
    );
    crate::codec_bytes::push_encoded(out, (method.result_claims.len() as u64).to_le_bytes());
    for claim in &method.result_claims {
        encode_string(&claim.domain, out);
        encode_carry(&claim.effective_carry, out);
    }
    crate::codec_bytes::push_encoded(out, (method.service_reach.len() as u64).to_le_bytes());
    for reach in &method.service_reach {
        encode_string(reach, out);
    }
    crate::codec_bytes::push_encoded(
        out,
        (method.synchronous_invocations.len() as u64).to_le_bytes(),
    );
    for invocation in &method.synchronous_invocations {
        encode_string(invocation, out);
    }
    out.push(u8::from(method.may_suspend));
    out.push(u8::from(method.may_block));
    out.push(u8::from(method.terminates_guarantee));
    crate::codec_bytes::push_encoded(
        out,
        (method.termination_premises.len() as u64).to_le_bytes(),
    );
    for premise in &method.termination_premises {
        encode_string(&premise.profile, out);
        match premise.subject {
            ServiceProgressSubject::Parameter(index) => {
                out.push(1);
                crate::codec_bytes::push_encoded(out, (index as u64).to_le_bytes());
            }
        }
        crate::codec_bytes::push_encoded(
            out,
            (premise.subject_projections.len() as u64).to_le_bytes(),
        );
        for projection in &premise.subject_projections {
            encode_string(projection, out);
        }
        crate::codec_bytes::push_encoded(
            out,
            (premise.establishment_routes.len() as u64).to_le_bytes(),
        );
        for route in &premise.establishment_routes {
            out.push(match route.kind {
                ServiceProgressEstablishmentRouteKind::CheckedRequirement => 0,
                ServiceProgressEstablishmentRouteKind::BoundaryRequirement => 1,
            });
            encode_string(identities.text(route.requirement_identity), out);
        }
    }
    encode_optional(
        method.calling_plan_report_fingerprint.as_ref(),
        out,
        |value, out| crate::codec_bytes::push_encoded(out, value.to_le_bytes()),
    );
    encode_optional(
        method.calling_plan_commitment.as_ref(),
        out,
        |commitment, out| push_framed(out, &commitment.as_bytes()),
    );
}

fn recover_service_method(
    cursor: &mut ObservationCursor<'_>,
    identities: &mut ProviderIdentityTable,
) -> Option<ServiceMethod> {
    let name = cursor_string(cursor)?.to_owned();
    let requirement_owner = cursor_string(cursor)?.to_owned();
    let requirement_owner_package_identity = recover_optional(cursor, recover_identity)?;
    let requirement_identity = identities.intern(cursor_string(cursor)?);
    let parameter_count = usize::try_from(cursor.u64()?).ok()?;
    let parameter_type_identities = cursor_vec(cursor, |cursor| {
        cursor_string(cursor).map(|text| identities.intern(text))
    })?;
    let entry_claims = cursor_vec(cursor, |cursor| {
        let parameter_index = usize::try_from(cursor.u64()?).ok()?;
        let carrier_identity = identities.intern(cursor_string(cursor)?);
        let domain = cursor_string(cursor)?.to_owned();
        let predicate_body = recover_predicate_body(cursor)?;
        let effective_carry = recover_carry(cursor)?;
        let authority_flow = match cursor_string(cursor)? {
            "accepts" => ServiceEntryAuthorityFlow::Accepts,
            _ => return None,
        };
        Some(ServiceEntryClaim {
            parameter_index,
            carrier_identity,
            domain,
            predicate_body,
            effective_carry,
            authority_flow,
        })
    })?;
    let completion = match cursor.u8()? {
        0 => semantic_vocabulary::BoundaryCompletion::NormalReturn,
        1 => semantic_vocabulary::BoundaryCompletion::External(
            semantic_vocabulary::ExternalCompletionKind::ProcessExit,
        ),
        _ => return None,
    };
    let has_result = cursor_bool(cursor)?;
    let result_type_identity =
        recover_optional(cursor, |cursor| cursor_string(cursor).map(str::to_owned))?
            .map(|text| identities.intern(&text))
            .unwrap_or_default();
    let result_claims = cursor_vec(cursor, |cursor| {
        Some(ServiceResultClaim {
            domain: cursor_string(cursor)?.to_owned(),
            effective_carry: recover_carry(cursor)?,
        })
    })?;
    let service_reach = cursor_vec(cursor, |cursor| cursor_string(cursor).map(str::to_owned))?;
    let synchronous_invocations =
        cursor_vec(cursor, |cursor| cursor_string(cursor).map(str::to_owned))?;
    let may_suspend = cursor_bool(cursor)?;
    let may_block = cursor_bool(cursor)?;
    let terminates_guarantee = cursor_bool(cursor)?;
    let termination_premises = cursor_vec(cursor, |cursor| {
        let profile = cursor_string(cursor)?.to_owned();
        let subject = match cursor.u8()? {
            1 => ServiceProgressSubject::Parameter(usize::try_from(cursor.u64()?).ok()?),
            _ => return None,
        };
        let subject_projections =
            cursor_vec(cursor, |cursor| cursor_string(cursor).map(str::to_owned))?;
        let establishment_routes = cursor_vec(cursor, |cursor| {
            let kind = match cursor.u8()? {
                0 => ServiceProgressEstablishmentRouteKind::CheckedRequirement,
                1 => ServiceProgressEstablishmentRouteKind::BoundaryRequirement,
                _ => return None,
            };
            Some(ServiceProgressEstablishmentRoute {
                kind,
                requirement_identity: identities.intern(cursor_string(cursor)?),
            })
        })?;
        Some(ServiceProgressPremise {
            profile,
            subject,
            subject_projections,
            establishment_routes,
        })
    })?;
    let calling_plan_report_fingerprint = recover_optional(cursor, |cursor| {
        Some(u64::from_le_bytes(cursor.bytes(8)?.try_into().ok()?))
    })?;
    let calling_plan_commitment = recover_optional(cursor, |cursor| {
        Some(BoundaryCallingPlanCommitment::from_digest(
            cursor.framed()?.try_into().ok()?,
        ))
    })?;
    match (calling_plan_report_fingerprint, calling_plan_commitment) {
        (Some(_), None) | (None, Some(_)) => return None,
        _ => {}
    }
    Some(ServiceMethod {
        completion,
        name,
        requirement_owner,
        requirement_owner_package_identity,
        requirement_identity,
        parameter_count,
        parameter_type_identities,
        entry_claims,
        has_result,
        result_type_identity,
        result_claims,
        service_reach,
        synchronous_invocations,
        may_suspend,
        may_block,
        terminates_guarantee,
        termination_premises,
        calling_plan_report_fingerprint,
        calling_plan_commitment,
    })
}

fn encode_service_schema(schema: &ServiceSchema, out: &mut Vec<u8>) {
    // The identity table rides the wire as a roster ahead of every field that
    // points into it: recovery re-mints the texts in order so a decoded schema
    // holds the same ids, not merely the same texts.
    crate::codec_bytes::push_encoded(
        out,
        (schema.identities.texts().count() as u64).to_le_bytes(),
    );
    for text in schema.identities.texts() {
        encode_string(text, out);
    }
    encode_string(schema.identity_text(schema.trait_name), out);
    encode_optional(
        schema.trait_package_identity.as_ref(),
        out,
        |identity, out| encode_identity(*identity, out),
    );
    crate::codec_bytes::push_encoded(
        out,
        (schema.receiver_applications.len() as u64).to_le_bytes(),
    );
    for application in &schema.receiver_applications {
        encode_string(schema.identity_text(application.trait_name), out);
        encode_optional(
            application.trait_package_identity.as_ref(),
            out,
            |identity, out| encode_identity(*identity, out),
        );
        for roster in [
            &application.argument_identities,
            &application.lifetime_argument_identities,
            &application.requirement_identities,
        ] {
            crate::codec_bytes::push_encoded(out, (roster.len() as u64).to_le_bytes());
            for identity in roster {
                encode_string(schema.identity_text(*identity), out);
            }
        }
    }
    crate::codec_bytes::push_encoded(out, (schema.methods.len() as u64).to_le_bytes());
    for method in &schema.methods {
        encode_service_method(method, &schema.identities, out);
    }
}

fn recover_service_schema(cursor: &mut ObservationCursor<'_>) -> Option<ServiceSchema> {
    let mut identities = ProviderIdentityTable::default();
    let identity_count = cursor.u64()?;
    if identity_count > RECORD_SEQUENCES_LIMIT {
        return None;
    }
    for _ in 0..identity_count {
        identities.intern(cursor_string(cursor)?);
    }
    Some(ServiceSchema {
        trait_name: identities.intern(cursor_string(cursor)?),
        trait_package_identity: recover_optional(cursor, recover_identity)?,
        receiver_applications: cursor_vec(cursor, |cursor| {
            Some(ServiceReceiverApplication {
                trait_name: identities.intern(cursor_string(cursor)?),
                trait_package_identity: recover_optional(cursor, recover_identity)?,
                argument_identities: cursor_vec(cursor, |cursor| {
                    Some(identities.intern(cursor_string(cursor)?))
                })?,
                lifetime_argument_identities: cursor_vec(cursor, |cursor| {
                    Some(identities.intern(cursor_string(cursor)?))
                })?,
                requirement_identities: cursor_vec(cursor, |cursor| {
                    Some(identities.intern(cursor_string(cursor)?))
                })?,
            })
        })?,
        methods: cursor_vec(cursor, |cursor| {
            recover_service_method(cursor, &mut identities)
        })?,
        identities,
    })
}

fn encode_candidate(candidate: &SemanticBindingReviewCandidate, out: &mut Vec<u8>) {
    encode_binding(candidate.binding(), out);
    encode_service_schema(candidate.service_schema(), out);
}

fn recover_candidate(cursor: &mut ObservationCursor<'_>) -> Option<SemanticBindingReviewCandidate> {
    let binding = recover_binding(cursor)?;
    let service_schema = recover_service_schema(cursor)?;
    // The constructor's digest invariant is a decode check here: a record
    // whose schema does not normalize to the binding's declared digest is
    // corrupt, not a different valid record.
    if binding.normalized_schema_digest()
        != package_compilation::accepted_service_schema_digest(binding.role(), &service_schema)
    {
        return None;
    }
    Some(SemanticBindingReviewCandidate::new(binding, service_schema))
}

fn encode_dependency_binding(binding: &PackageDependencyBinding, out: &mut Vec<u8>) {
    encode_identity(binding.requester(), out);
    encode_string(binding.alias(), out);
    encode_identity(binding.target(), out);
    out.push(purpose_tag(binding.purpose()));
}

fn recover_dependency_binding(
    cursor: &mut ObservationCursor<'_>,
) -> Option<PackageDependencyBinding> {
    Some(PackageDependencyBinding::for_purpose(
        recover_identity(cursor)?,
        cursor_string(cursor)?,
        recover_identity(cursor)?,
        recover_purpose(cursor)?,
    ))
}

fn encode_generated_source_bundle(bundle: &PackageGeneratedSourceBundle, out: &mut Vec<u8>) {
    encode_identity(bundle.package(), out);
    out.push(purpose_tag(bundle.purpose()));
    encode_target(bundle.target(), out);
    encode_optional(
        bundle.build_execution_profile().as_ref(),
        out,
        |profile, out| encode_target(*profile, out),
    );
    let closure = bundle.dependency_closure();
    encode_identity(closure.root(), out);
    out.push(declaration_kind_tag(closure.root_role()));
    crate::codec_bytes::push_encoded(out, (closure.packages().len() as u64).to_le_bytes());
    for package in closure.packages() {
        encode_identity(*package, out);
    }
    crate::codec_bytes::push_encoded(out, (closure.dependencies().len() as u64).to_le_bytes());
    for binding in closure.dependencies() {
        encode_dependency_binding(binding, out);
    }
    out.extend_from_slice(&bundle.source_consumption_commitment().digest());
    crate::codec_bytes::push_encoded(out, (bundle.sources().len() as u64).to_le_bytes());
    for source in bundle.sources() {
        push_framed(out, source.relative_path());
        push_framed(out, source.bytes());
    }
}

fn recover_generated_source_bundle(
    cursor: &mut ObservationCursor<'_>,
) -> Option<PackageGeneratedSourceBundle> {
    let package = recover_identity(cursor)?;
    let purpose = recover_purpose(cursor)?;
    let target = recover_target(cursor)?;
    let build_execution_profile = recover_optional(cursor, recover_target)?;
    let root = recover_identity(cursor)?;
    let root_role = recover_declaration_kind(cursor)?;
    let packages = cursor_vec(cursor, recover_identity)?;
    let dependencies = cursor_vec(cursor, recover_dependency_binding)?;
    let dependency_closure =
        PackageDependencyClosure::from_canonical_parts(root, root_role, packages, dependencies)
            .ok()?;
    let commitment =
        PackageSourceConsumptionCommitment::from_retained_digest(cursor_digest(cursor)?);
    let sources = cursor_vec(cursor, |cursor| {
        let relative_path = cursor.framed()?.to_vec();
        let bytes = cursor.framed()?;
        Some(build_output::PackageGeneratedSource::from_retained_parts(
            relative_path,
            bytes.into(),
        ))
    })?;
    Some(PackageGeneratedSourceBundle::from_checked(
        package,
        purpose,
        target,
        build_execution_profile,
        dependency_closure,
        commitment,
        sources,
    ))
}

fn encode_consumed_unit(unit: &ConsumedSourceUnit, out: &mut Vec<u8>) {
    match unit.kind() {
        ConsumedSourceUnitKind::PackageAuthored => out.push(0),
        ConsumedSourceUnitKind::PackageGenerated(scope) => {
            out.push(1);
            out.push(match scope {
                source::DependencyScope::Product => 0,
                source::DependencyScope::Build => 1,
            });
        }
        ConsumedSourceUnitKind::ToolchainVirtual => out.push(2),
        ConsumedSourceUnitKind::ToolchainOwned => out.push(3),
    }
    encode_optional(unit.package().as_ref(), out, |identity, out| {
        encode_identity(*identity, out)
    });
    encode_optional(
        unit.toolchain_namespace().as_ref(),
        out,
        |namespace, out| encode_string(namespace, out),
    );
    crate::codec_bytes::push_encoded(out, (unit.relative_path().len() as u64).to_le_bytes());
    for component in unit.relative_path() {
        encode_string(component, out);
    }
    crate::codec_bytes::push_encoded(out, unit.byte_count().to_le_bytes());
    out.extend_from_slice(&unit.content_digest());
}

fn recover_consumed_unit(cursor: &mut ObservationCursor<'_>) -> Option<ConsumedSourceUnit> {
    let kind = match cursor.u8()? {
        0 => ConsumedSourceUnitKind::PackageAuthored,
        1 => ConsumedSourceUnitKind::PackageGenerated(match cursor.u8()? {
            0 => source::DependencyScope::Product,
            1 => source::DependencyScope::Build,
            _ => return None,
        }),
        2 => ConsumedSourceUnitKind::ToolchainVirtual,
        3 => ConsumedSourceUnitKind::ToolchainOwned,
        _ => return None,
    };
    let package = recover_optional(cursor, recover_identity)?;
    let toolchain_namespace =
        recover_optional(cursor, |cursor| cursor_string(cursor).map(str::to_owned))?;
    let relative_path = cursor_vec(cursor, |cursor| cursor_string(cursor).map(str::to_owned))?;
    let byte_count = cursor.u64()?;
    let content_digest = cursor_digest(cursor)?;
    ConsumedSourceUnit::from_retained(
        kind,
        package,
        toolchain_namespace,
        relative_path,
        byte_count,
        content_digest,
    )
    .ok()
}

fn encode_component_description(description: &IndependentComponentDescription, out: &mut Vec<u8>) {
    encode_identity(description.package(), out);
    let subject = description.expected_subject();
    crate::codec_bytes::push_encoded(out, subject.vocabulary_marker.get().to_le_bytes());
    out.extend_from_slice(subject.program_fingerprint.as_bytes());
    push_framed(out, description.description());
}

fn recover_component_description(
    cursor: &mut ObservationCursor<'_>,
) -> Option<IndependentComponentDescription> {
    let package = recover_identity(cursor)?;
    let vocabulary_marker = terminal_psi::VocabularyMarker::new(cursor.u16()?)?;
    let program_fingerprint = terminal_psi::SemanticFingerprint::from_bytes(cursor_digest(cursor)?);
    let description = cursor.framed()?.to_vec();
    Some(IndependentComponentDescription::new(
        package,
        terminal_psi::TerminalPsiIdentity {
            vocabulary_marker,
            program_fingerprint,
        },
        description,
    ))
}

/// Encode one record's payload.
pub(super) fn encode_record(review: &CachedCompiledReview, out: &mut Vec<u8>) {
    out.extend_from_slice(RECORD_MAGIC);
    crate::codec_bytes::push_encoded(out, RECORD_VERSION.to_le_bytes());
    out.extend_from_slice(&review.package_identity.digest());
    encode_string(&review.name, out);
    out.extend_from_slice(&review.lineage);
    out.extend_from_slice(&review.resolution);
    out.push(purpose_tag(review.purpose));
    encode_target(review.target, out);
    encode_optional(
        review.build_execution_profile.as_ref(),
        out,
        |profile, out| encode_target(*profile, out),
    );
    encode_string(&review.selected_build_machine_identity, out);
    encode_optional(review.build_evaluation_usage.as_ref(), out, |usage, out| {
        let mut framed = Vec::new();
        build_record_codec::encode_build_evaluation_usage(usage, &mut framed);
        push_framed(out, &framed);
    });
    encode_optional(
        review.build_observation_summary.as_ref(),
        out,
        |summary, out| {
            let mut framed = Vec::new();
            build_record_codec::encode_observation_summary(summary, &mut framed);
            push_framed(out, &framed);
        },
    );
    crate::codec_bytes::push_encoded(
        out,
        (review.restricted_build_requests.len() as u64).to_le_bytes(),
    );
    for request in &review.restricted_build_requests {
        let mut framed = Vec::new();
        build_record_codec::encode_restricted_build_request(request, &mut framed);
        push_framed(out, &framed);
    }
    crate::codec_bytes::push_encoded(out, (review.semantic_bindings.len() as u64).to_le_bytes());
    for binding in &review.semantic_bindings {
        encode_binding(binding, out);
    }
    crate::codec_bytes::push_encoded(
        out,
        (review.semantic_binding_candidates.len() as u64).to_le_bytes(),
    );
    for candidate in &review.semantic_binding_candidates {
        encode_candidate(candidate, out);
    }
    encode_generated_source_bundle(&review.generated_source_bundle, out);
    push_framed(out, &review.policy_bytes);
    crate::codec_bytes::push_encoded(
        out,
        (review.facts.consumed_units.len() as u64).to_le_bytes(),
    );
    for unit in &review.facts.consumed_units {
        encode_consumed_unit(unit, out);
    }
    out.extend_from_slice(&review.facts.source_consumption_commitment.digest());
    encode_optional(
        review.facts.component_description.as_ref(),
        out,
        encode_component_description,
    );
    encode_optional(
        review.facts.target_entry_candidate.as_ref(),
        out,
        encode_candidate,
    );
    crate::codec_bytes::push_encoded(out, review.policy_canonical_bytes.to_le_bytes());
}

pub(super) fn decode_record(bytes: &[u8]) -> Option<CachedCompiledReview> {
    if bytes.len() as u64 > super::DEFAULT_MAX_RECORD_BYTES + 64 {
        return None;
    }
    if bytes.len() < 8 + 2 + 32 {
        return None;
    }
    let (payload, digest) = bytes.split_at(bytes.len() - 32);
    let expected: [u8; 32] = crate::digest_encoder::content_identity(&payload).into();
    if expected != digest {
        return None;
    }
    let mut cursor = ObservationCursor::new(payload);
    if cursor.bytes(8)? != RECORD_MAGIC {
        return None;
    }
    if cursor.u16()? != RECORD_VERSION {
        return None;
    }
    let package_identity = recover_identity(&mut cursor)?;
    let name = cursor_string(&mut cursor)?.to_owned();
    let lineage = cursor_digest(&mut cursor)?;
    let resolution = cursor_digest(&mut cursor)?;
    let purpose = recover_purpose(&mut cursor)?;
    let target = recover_target(&mut cursor)?;
    let build_execution_profile = recover_optional(&mut cursor, recover_target)?;
    let selected_build_machine_identity = cursor_string(&mut cursor)?.to_owned();
    let build_evaluation_usage = recover_optional(&mut cursor, |cursor| {
        let mut usage_cursor = ObservationCursor::new(cursor.framed()?);
        let usage = build_record_codec::recover_build_evaluation_usage(&mut usage_cursor)?;
        usage_cursor.exhausted().then_some(usage)
    })?;
    let build_observation_summary = recover_optional(&mut cursor, |cursor| {
        build_record_codec::recover_observation_summary(&mut ObservationCursor::new(
            cursor.framed()?,
        ))
    })?;
    let restricted_build_requests = cursor_vec(&mut cursor, |cursor| {
        build_record_codec::recover_restricted_build_request(&mut ObservationCursor::new(
            cursor.framed()?,
        ))
    })?;
    let semantic_bindings = cursor_vec(&mut cursor, recover_binding)?;
    let semantic_binding_candidates = cursor_vec(&mut cursor, recover_candidate)?;
    let generated_source_bundle = recover_generated_source_bundle(&mut cursor)?;
    let policy_bytes = cursor.framed()?.to_vec();
    let consumed_units = cursor_vec(&mut cursor, recover_consumed_unit)?;
    let source_consumption_commitment =
        PackageSourceConsumptionCommitment::from_retained_digest(cursor_digest(&mut cursor)?);
    let component_description = recover_optional(&mut cursor, recover_component_description)?;
    let target_entry_candidate = recover_optional(&mut cursor, recover_candidate)?;
    let policy_canonical_bytes = cursor.u64()?;
    if !cursor.exhausted() {
        return None;
    }
    Some(CachedCompiledReview {
        package_identity,
        name,
        lineage,
        resolution,
        purpose,
        target,
        build_execution_profile,
        selected_build_machine_identity,
        build_evaluation_usage,
        build_observation_summary,
        restricted_build_requests,
        semantic_bindings,
        semantic_binding_candidates,
        generated_source_bundle,
        policy_bytes,
        facts: CachedOccurrenceFacts {
            consumed_units,
            source_consumption_commitment,
            component_description,
            target_entry_candidate,
        },
        policy_canonical_bytes,
    })
}

#[cfg(test)]
mod tests {
    //! Codec round-trip proofs for interned provider identities: decoding a
    //! record interns the same canonical texts encoding projected, so a
    //! recovered record is semantically equal and re-encodes byte-identical.

    use super::*;

    #[test]
    fn review_record_roundtrip_refuses_retired_versions_and_corruption() {
        let package = PackageKeyIdentity::from_digest([1; 32]).expect("package identity");
        let commitment = PackageSourceConsumptionCommitment::from_retained_digest([2; 32]);
        let profile = target::TargetProfile::ALL[0];
        let closure = PackageDependencyClosure::from_canonical_parts(
            package,
            package_compilation::BuildDeclarationKind::Package,
            vec![package],
            Vec::new(),
        )
        .expect("single-package closure");
        let review = CachedCompiledReview {
            package_identity: package,
            name: "review_codec".to_owned(),
            lineage: [3; 32],
            resolution: [4; 32],
            purpose: crate::declarations::DependencyPurpose::Product,
            target: profile,
            build_execution_profile: Some(profile),
            selected_build_machine_identity: "review_codec::build".to_owned(),
            build_evaluation_usage: None,
            build_observation_summary: None,
            restricted_build_requests: Vec::new(),
            semantic_bindings: Vec::new(),
            semantic_binding_candidates: Vec::new(),
            generated_source_bundle: PackageGeneratedSourceBundle::from_checked(
                package,
                crate::declarations::DependencyPurpose::Product,
                profile,
                Some(profile),
                closure,
                commitment,
                Vec::new(),
            ),
            // The codec frames policy bytes; policy admission is a separate consumer.
            policy_bytes: vec![17, 29, 43],
            facts: CachedOccurrenceFacts {
                consumed_units: Vec::new(),
                source_consumption_commitment: commitment,
                component_description: None,
                target_entry_candidate: None,
            },
            policy_canonical_bytes: 3,
        };
        fn signed(mut payload: Vec<u8>) -> Vec<u8> {
            let digest: [u8; 32] = crate::digest_encoder::content_identity(&payload).into();
            payload.extend_from_slice(&digest);
            payload
        }
        let mut payload = Vec::new();
        encode_record(&review, &mut payload);
        let encoded = signed(payload.clone());
        let recovered = decode_record(&encoded).expect("current record decodes");
        let mut reencoded = Vec::new();
        encode_record(&recovered, &mut reencoded);
        assert_eq!(reencoded, payload);
        for retired_version in 1u16..RECORD_VERSION {
            let mut retired = payload.clone();
            retired[8..10].copy_from_slice(&retired_version.to_le_bytes());
            assert!(decode_record(&signed(retired)).is_none());
        }
        let mut corrupt = encoded.clone();
        corrupt[10] ^= 1;
        assert!(decode_record(&corrupt).is_none());
        assert!(decode_record(&encoded[..encoded.len() - 1]).is_none());
        payload.push(1);
        assert!(decode_record(&signed(payload)).is_none());
    }

    #[test]
    fn service_schema_codec_roundtrips_interned_identities() {
        let mut identities = ProviderIdentityTable::default();
        let trait_name = identities.intern("omega.test.Trait");
        let requirement_identity = identities.intern("omega.test.Trait.method");
        let alpha = identities.intern("omega.test.Trait.alpha");
        let beta = identities.intern("omega.test.Trait.beta");
        let u32_type_identity = identities.intern("omega.core.u32");
        let schema = ServiceSchema {
            trait_name,
            receiver_applications: vec![ServiceReceiverApplication {
                trait_name,
                trait_package_identity: None,
                argument_identities: vec![u32_type_identity],
                lifetime_argument_identities: Vec::new(),
                requirement_identities: vec![alpha, beta, requirement_identity],
            }],
            identities,
            trait_package_identity: None,
            methods: vec![ServiceMethod {
                name: "method".to_owned(),
                requirement_owner: "omega.test.Trait".to_owned(),
                requirement_identity,
                parameter_count: 1,
                parameter_type_identities: vec![u32_type_identity],
                has_result: true,
                result_type_identity: u32_type_identity,
                terminates_guarantee: true,
                termination_premises: vec![ServiceProgressPremise {
                    profile: "terminates".to_owned(),
                    subject: ServiceProgressSubject::Parameter(0),
                    subject_projections: Vec::new(),
                    establishment_routes: vec![
                        ServiceProgressEstablishmentRoute {
                            kind: ServiceProgressEstablishmentRouteKind::CheckedRequirement,
                            requirement_identity: alpha,
                        },
                        ServiceProgressEstablishmentRoute {
                            kind: ServiceProgressEstablishmentRouteKind::BoundaryRequirement,
                            requirement_identity: beta,
                        },
                    ],
                }],
                ..Default::default()
            }],
        };

        let mut encoded = Vec::new();
        encode_service_schema(&schema, &mut encoded);
        let recovered = recover_service_schema(&mut ObservationCursor::new(&encoded))
            .expect("service schema decodes");
        assert_eq!(schema, recovered);

        let mut reencoded = Vec::new();
        encode_service_schema(&recovered, &mut reencoded);
        assert_eq!(
            encoded, reencoded,
            "a decoded record must re-encode to identical bytes"
        );
    }
}

//! Project local input and result subjects into portable service coordinates.

use super::*;

/// A declaration's exact input/normal-result subjects in the namespace of
/// its checked public service, independent of local value, place and domain IDs.
fn service_memberships(
    module: &TerminalModule,
    boundary: &terminal_psi::BoundaryMachineDeclaration,
) -> Result<Vec<service_surface::ServiceDomainMembership>, String> {
    use semantic_vocabulary::CanonicalStructuralPathSegment as Path;
    use service_surface::{
        ServiceDomainMembership, ServiceMembershipProjection, ServiceMembershipSubject,
    };
    if boundary.membership_requires.is_empty() && boundary.membership_ensures.is_empty() {
        return Ok(Vec::new());
    }
    validate_roster(None, &boundary.membership_ensures).map_err(|error| error.detail)?;
    let binding = boundary_binding(None, boundary).map_err(|error| error.detail)?;
    let identity = |index| {
        module
            .identities
            .text(index)
            .filter(|text| !text.is_empty())
            .ok_or("boundary membership lost its exact semantic identity")
    };
    validate_roster(None, &boundary.membership_requires).map_err(|error| error.detail)?;
    let scalar = boundary
        .scalar_contract_parameters()
        .ok_or("membership surface lost its boundary parameters")?;
    let outputs = boundary
        .membership_ensures
        .iter()
        .map(|row| Ok((ServiceMembershipSubject::NormalResult, binding.clone(), row)));
    let inputs = boundary.membership_requires.iter().map(|row| {
        let (kind, index, binding) =
            inputs::formal_binding(module, &scalar, &boundary.structural_parameters, row, None)
                .map_err(|error| error.detail)?;
        let position = boundary
            .parameter_positions(kind)
            .nth(index)
            .and_then(|position| u32::try_from(position).ok())
            .ok_or("input membership does not identify one ordered formal")?;
        Ok::<_, &'static str>((ServiceMembershipSubject::Input(position), binding, row))
    });
    let mut actual =
        Vec::with_capacity(boundary.membership_ensures.len() + boundary.membership_requires.len());
    for projected in inputs.chain(outputs) {
        let (subject, binding, row) = projected?;
        let _ = binding
            .membership(module, None, row)
            .map_err(|error| error.detail)?;
        let (path, domain) = match row {
            RuntimeDomainMembership::Scalar { subject, domain } => {
                let path = match subject {
                    ScalarTerm::Value { .. } => &[][..],
                    ScalarTerm::BooleanField { path, .. }
                    | ScalarTerm::IntegerField { path, .. }
                    | ScalarTerm::IeeeFloatField { path, .. } => path.as_slice(),
                    _ => {
                        return Err(
                            "boundary membership has no exact formal or result selector".into()
                        );
                    }
                };
                let domain = crate::scalar_qualifications::scalar_domain(module, *domain)
                    .ok_or("boundary membership names an absent scalar domain")?;
                (path, identity(domain.identity)?)
            }
            RuntimeDomainMembership::Structural { path, domain, .. } => {
                let domain = crate::exactly_one::exactly_one(
                    module.types.structural_domains.iter(),
                    |row| row.id == *domain,
                )
                .ok_or("boundary membership names no unique structural domain")?;
                (path.as_slice(), identity(domain.identity)?)
            }
        };
        let mut portable = Vec::with_capacity(path.len());
        if !path.is_empty() {
            let SubjectBinding::Structural { carrier, .. } = &binding else {
                return Err("membership projection has no structural subject".into());
            };
            for (ordinal, step) in path.iter().enumerate() {
                portable.push(match step {
                    Path::Field(field) => {
                        let field = crate::observed_field_declaration(
                            module.types.structural_types.iter(),
                            *carrier,
                            &path[..ordinal],
                            *field,
                        )
                        .ok_or("boundary membership field has no exact declaration")?;
                        ServiceMembershipProjection::Field(identity(field.identity)?.to_owned())
                    }
                    Path::Case(case) => {
                        let tip = crate::canonical_structural_path_tip(
                            module.types.structural_types.iter(),
                            *carrier,
                            &path[..ordinal],
                        )
                        .ok_or("boundary membership case has no exact carrier")?;
                        let cases = tip
                            .shape
                            .sum_cases()
                            .ok_or("boundary membership case is not a sum")?;
                        let case =
                            crate::exactly_one::exactly_one(cases.iter(), |row| row.id == *case)
                                .ok_or("boundary membership case has no unique declaration")?;
                        ServiceMembershipProjection::Case(identity(case.identity)?.to_owned())
                    }
                    Path::FixedIndex(index) => ServiceMembershipProjection::FixedIndex(*index),
                    Path::Referent => continue,
                });
            }
        }
        actual.push(ServiceDomainMembership {
            subject,
            path: portable,
            domain: domain.to_owned(),
        });
    }
    actual.sort();
    if actual.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err("boundary domain memberships repeat an exact service subject".into());
    }
    Ok(actual)
}

/// Compare declarations from separately checked components in their own
/// namespaces. Rehashed source headers cannot move an obligation to another
/// argument or turn a premise into a normal-return promise.
pub fn same_boundary_memberships(
    receiver: &TerminalModule,
    required: &terminal_psi::BoundaryMachineDeclaration,
    provider: &TerminalModule,
    supplied: &terminal_psi::BoundaryMachineDeclaration,
) -> Result<bool, String> {
    Ok(service_memberships(receiver, required)? == service_memberships(provider, supplied)?)
}

/// Join a native boundary to the full, independently authored service method.
pub fn validate_boundary_memberships(
    module: &TerminalModule,
    boundary: &terminal_psi::BoundaryMachineDeclaration,
    method: &service_surface::ServiceMethod,
) -> Result<(), String> {
    let actual = service_memberships(module, boundary)?;
    if method.domain_memberships.iter().any(|row| {
        !row.has_exact_identity() || !row.has_subject(method.parameter_count, method.has_result)
    }) || method
        .domain_memberships
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
        || actual != method.domain_memberships
    {
        return Err("native requirement changes its authored domain membership contract".into());
    }
    Ok(())
}

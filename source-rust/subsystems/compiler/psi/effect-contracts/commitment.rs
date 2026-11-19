use super::{ConditionalCarryRequirement, ContractDependency};

pub fn validate_published_dependency(
    dependency: &ContractDependency<bool, Vec<u32>>,
    parameter_count: usize,
) -> Result<(), &'static str> {
    if (dependency.concrete && !dependency.parameters.is_empty())
        || dependency
            .parameters
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || dependency
            .parameters
            .iter()
            .any(|parameter| *parameter as usize >= parameter_count)
    {
        return Err("published callback dependency is not a normalized term over its telescope");
    }
    Ok(())
}

pub fn template_contract_commitment(
    base: [u8; 32],
    parameter_count: usize,
    suspension: &ContractDependency<bool, Vec<u32>>,
    blocking: &ContractDependency<bool, Vec<u32>>,
    carry: &[ConditionalCarryRequirement],
) -> Result<[u8; 32], &'static str> {
    // The published identity binds the retained subjects as well as the
    // callback condition. Removing a private acquisition or changing its
    // origin must not leave an apparently unchanged generic interface.
    validate_published_dependency(suspension, parameter_count)?;
    validate_published_dependency(blocking, parameter_count)?;
    if carry.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err("conditional carry requirements are not uniquely ordered");
    }
    let mut digest =
        ::wire::digest_encoder::DomainDigestEncoder::new(b"omega.machine-template.v4\0");
    digest.raw(&base);
    digest.len(parameter_count);
    fn term(
        digest: &mut ::wire::digest_encoder::DomainDigestEncoder,
        dependency: &ContractDependency<bool, Vec<u32>>,
    ) {
        digest.byte(u8::from(dependency.concrete));
        digest.len(dependency.parameters.len());
        for parameter in &dependency.parameters {
            digest.u64(u64::from(*parameter));
        }
    }
    term(&mut digest, suspension);
    term(&mut digest, blocking);
    digest.len(carry.len());
    for row in carry {
        validate_published_dependency(&row.condition, parameter_count)?;
        if row.value_type.is_empty()
            || row.condition.concrete
            || row.condition.parameters.is_empty()
            || row.claims.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err("conditional carry requirement lost its subject, provenance or guard");
        }
        digest.u64(u64::from(row.state));
        digest.u64(row.statement_index);
        digest.u64(row.call_ordinal);
        subject(&mut digest, &row.subject)?;
        digest.bytes(row.value_type.as_bytes());
        digest.len(row.claims.len());
        for claim in &row.claims {
            arrival(&mut digest, claim)?;
        }
        term(&mut digest, &row.condition);
    }
    Ok(digest.finish())
}

fn identity(
    digest: &mut ::wire::digest_encoder::DomainDigestEncoder,
    identity: &str,
) -> Result<(), &'static str> {
    if identity.is_empty() {
        return Err("conditional carry provenance has an empty declaration identity");
    }
    digest.bytes(identity.as_bytes());
    Ok(())
}

fn subject(
    digest: &mut ::wire::digest_encoder::DomainDigestEncoder,
    subject: &crate::SuspensionValueOrigin<String>,
) -> Result<(), &'static str> {
    use crate::SuspensionValueOrigin;
    match subject {
        SuspensionValueOrigin::Parameter { symbol, position } => {
            digest.byte(0);
            identity(digest, symbol)?;
            digest.len(*position);
        }
        SuspensionValueOrigin::Local {
            symbol,
            statement_index,
            environment_position,
        } => {
            digest.byte(1);
            identity(digest, symbol)?;
            digest.len(*statement_index);
            digest.len(*environment_position);
        }
        SuspensionValueOrigin::CallArgument { position } => {
            digest.byte(2);
            digest.len(*position);
        }
    }
    Ok(())
}

fn permission_source(
    digest: &mut ::wire::digest_encoder::DomainDigestEncoder,
    source: &crate::ScopedPermissionEventSource<String>,
) -> Result<(), &'static str> {
    use crate::ScopedPermissionEventSource;
    match source {
        ScopedPermissionEventSource::StateEntry => digest.byte(0),
        ScopedPermissionEventSource::Statement { statement_index } => {
            digest.byte(1);
            digest.len(*statement_index);
        }
        ScopedPermissionEventSource::Call {
            statement_index,
            call_ordinal,
            target_symbol,
        } => {
            digest.byte(2);
            digest.len(*statement_index);
            digest.len(*call_ordinal);
            identity(digest, target_symbol)?;
        }
        ScopedPermissionEventSource::InstructionDisposition { statement_index } => {
            digest.byte(3);
            digest.len(*statement_index);
        }
        ScopedPermissionEventSource::InstructionExitDisposition { statement_index } => {
            digest.byte(4);
            digest.len(*statement_index);
        }
        ScopedPermissionEventSource::StateExit => digest.byte(5),
    }
    Ok(())
}

fn arrival(
    digest: &mut ::wire::digest_encoder::DomainDigestEncoder,
    arrival: &crate::PermissionArrival<String>,
) -> Result<(), &'static str> {
    use crate::ScopedPermissionClaimIdentity;
    use crate::ScopedPermissionProvenance;
    let ScopedPermissionClaimIdentity::Established {
        machine_symbol,
        state_symbol,
        source,
        ordinal,
    } = &arrival.claim_identity
    else {
        return Err("conditional carry requires an exact receiving claim identity");
    };
    identity(digest, machine_symbol)?;
    identity(digest, state_symbol)?;
    permission_source(digest, source)?;
    digest.u64(u64::from(*ordinal));
    match &arrival.provenance {
        ScopedPermissionProvenance::Unknown => {
            return Err("conditional carry requires its exact acquisition join");
        }
        ScopedPermissionProvenance::Established {
            machine_symbol,
            state_symbol,
            source,
        } => {
            digest.byte(1);
            identity(digest, machine_symbol)?;
            identity(digest, state_symbol)?;
            permission_source(digest, source)?;
        }
        ScopedPermissionProvenance::Joined {
            machine_symbol,
            state_symbol,
            source,
            ordinal,
        } => {
            digest.byte(2);
            identity(digest, machine_symbol)?;
            identity(digest, state_symbol)?;
            permission_source(digest, source)?;
            digest.u64(u64::from(*ordinal));
        }
    }
    Ok(())
}

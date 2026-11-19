//! Encode and read the canonical OMGCMPD component carrier.
use crate::records::*;
use semantic_vocabulary::ServiceId;
use terminal_psi::TerminalPsiIdentity;
const DESCRIPTION_MAGIC: &[u8; 8] = b"OMGCMPD\0";

use ::wire::cursor::{Cursor, Truncated};

type Reader<'a> = Cursor<'a>;

impl From<Truncated> for DescriptionDecodeRejection {
    fn from(_: Truncated) -> Self {
        Self::Corrupt("truncated input")
    }
}

fn roster_len(
    reader: &mut Reader<'_>,
    what: &'static str,
) -> Result<usize, DescriptionDecodeRejection> {
    let count = reader.u32()? as usize;
    if count > MAX_DESCRIPTION_ROSTER {
        return Err(DescriptionDecodeRejection::RosterBoundExceeded(what));
    }
    Ok(count)
}

fn read_identity(reader: &mut Reader<'_>) -> Result<String, DescriptionDecodeRejection> {
    let length = reader.u32()? as usize;
    String::from_utf8(reader.take(length)?.to_vec())
        .map_err(|_| DescriptionDecodeRejection::IdentityInvalid("not utf-8"))
}

fn push_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn push_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn push_identity(out: &mut Vec<u8>, identity: &str) {
    push_u32(out, identity.len() as u32);
    out.extend_from_slice(identity.as_bytes());
}

fn check_publication_bounds(
    description: &ComponentDescription,
    encoded: &[u8],
) -> Result<(), DescribeError> {
    if encoded.len() > MAX_COMPONENT_DESCRIPTION_BYTES {
        return Err(DescribeError::BoundExceeded("byte length"));
    }
    let rosters = [
        ("import roster", description.imports.len()),
        ("export roster", description.exports.len()),
        ("entry roster", description.entries.len()),
        ("outgoing authority roster", description.outgoing.len()),
        ("service bound roster", description.service_bounds.len()),
        ("custody roster", description.custody.len()),
        ("provider roster", description.providers.len()),
        ("obligation roster", description.obligations.len()),
        ("assumption roster", description.assumptions.len()),
    ]
    .into_iter()
    .chain(
        description
            .service_bounds
            .iter()
            .map(|row| ("service bound services roster", row.bound.len())),
    )
    .chain(description.providers.iter().map(|provider| {
        (
            "provider requirement roster",
            provider.requirement_identities.len(),
        )
    }));
    for (what, count) in rosters {
        if count > MAX_DESCRIPTION_ROSTER {
            return Err(DescribeError::BoundExceeded(what));
        }
    }
    Ok(())
}

/// Encode a publication and enforce the receiving protocol's size and roster limits.
pub fn encode_component_description_checked(
    description: &ComponentDescription,
) -> Result<Vec<u8>, DescribeError> {
    let encoded = encode_component_description(description);
    check_publication_bounds(description, &encoded)?;
    Ok(encoded)
}

pub fn encode_component_description(description: &ComponentDescription) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(DESCRIPTION_MAGIC);
    push_u32(&mut out, description.schema);
    out.push(description.frontier.tag());

    push_u32(&mut out, description.artifact_bytes.len() as u32);
    out.extend_from_slice(&description.artifact_bytes);

    let mut imports = description.imports.clone();
    imports.sort();
    push_u32(&mut out, imports.len() as u32);
    for slot in &imports {
        push_u32(&mut out, slot.slot);
        push_identity(&mut out, &slot.requirement_identity);
        out.extend_from_slice(&slot.contract_identity);
    }

    let mut exports = description.exports.clone();
    exports.sort();
    push_u32(&mut out, exports.len() as u32);
    for export in &exports {
        push_identity(&mut out, &export.identity);
    }

    let mut entries = description.entries.clone();
    entries.sort();
    push_u32(&mut out, entries.len() as u32);
    for entry in &entries {
        out.push(entry.kind.tag());
        push_identity(&mut out, &entry.identity);
        match entry.evidence {
            EntryEvidence::ModuleDerived => out.push(0),
            EntryEvidence::AssumptionBound(digest) => {
                out.push(1);
                out.extend_from_slice(&digest);
            }
        }
    }

    let mut outgoing = description.outgoing.clone();
    outgoing.sort();
    push_u32(&mut out, outgoing.len() as u32);
    for row in &outgoing {
        out.push(row.class.tag());
        push_identity(&mut out, &row.identity);
        match row.evidence {
            OutgoingEvidence::ModuleDerived => out.push(0),
            OutgoingEvidence::ProviderSealed(digest)
            | OutgoingEvidence::PhysicalMechanism(digest)
            | OutgoingEvidence::AssumptionBound(digest) => {
                out.push(match row.evidence {
                    OutgoingEvidence::ProviderSealed(_) => 1,
                    OutgoingEvidence::PhysicalMechanism(_) => 2,
                    OutgoingEvidence::AssumptionBound(_) => 3,
                    OutgoingEvidence::ModuleDerived => unreachable!(),
                });
                out.extend_from_slice(&digest);
            }
        }
    }

    let mut service_bounds = description.service_bounds.clone();
    service_bounds.sort();
    push_u32(&mut out, service_bounds.len() as u32);
    for row in &service_bounds {
        push_identity(&mut out, &row.requirement_identity);
        let mut bound = row.bound.clone();
        bound.sort();
        push_u32(&mut out, bound.len() as u32);
        for service in &bound {
            push_u64(&mut out, service.get());
        }
    }

    let mut custody = description.custody.clone();
    custody.sort();
    push_u32(&mut out, custody.len() as u32);
    for row in &custody {
        out.push(row.kind.tag());
        push_identity(&mut out, &row.identity);
        match row.evidence {
            CustodyEvidence::ModuleDerived => out.push(0),
            CustodyEvidence::AssumptionBound(digest) => {
                out.push(1);
                out.extend_from_slice(&digest);
            }
        }
    }

    let mut providers = description.providers.clone();
    providers.sort();
    push_u32(&mut out, providers.len() as u32);
    for provider in &providers {
        push_u64(&mut out, provider.report_identity);
        out.extend_from_slice(&provider.plan_digest);
        push_u32(&mut out, provider.plan_input.len() as u32);
        out.extend_from_slice(&provider.plan_input);
        push_u32(&mut out, provider.requirement_identities.len() as u32);
        for requirement in &provider.requirement_identities {
            push_identity(&mut out, requirement);
        }
    }
    out.extend_from_slice(&description.provider_closure_digest);

    let mut obligations = description.obligations.clone();
    obligations.sort();
    push_u32(&mut out, obligations.len() as u32);
    for obligation in &obligations {
        out.push(obligation.kind.tag());
        push_identity(&mut out, &obligation.identity);
        push_identity(&mut out, &obligation.detail);
    }

    let mut assumptions = description.assumptions.clone();
    assumptions.sort();
    push_u32(&mut out, assumptions.len() as u32);
    for assumption in &assumptions {
        out.extend_from_slice(assumption);
    }

    match description.realization_identity {
        Some(identity) => {
            out.push(1);
            out.extend_from_slice(&identity);
        }
        None => out.push(0),
    }
    out
}

pub fn decode_component_description(
    bytes: &[u8],
) -> Result<ComponentDescription, DescriptionDecodeRejection> {
    decode_component_description_with_artifact(bytes).map(|(description, _artifact)| description)
}

pub fn decode_component_description_with_artifact(
    bytes: &[u8],
) -> Result<
    (
        ComponentDescription,
        terminal_codec::CanonicalTerminalArtifact,
    ),
    DescriptionDecodeRejection,
> {
    if bytes.len() > MAX_COMPONENT_DESCRIPTION_BYTES {
        return Err(DescriptionDecodeRejection::RosterBoundExceeded(
            "description byte length",
        ));
    }
    let mut reader = Cursor::new(bytes);
    if reader.take(DESCRIPTION_MAGIC.len())? != DESCRIPTION_MAGIC {
        return Err(DescriptionDecodeRejection::InvalidMagic);
    }
    let schema = reader.u32()?;
    if schema != COMPONENT_DESCRIPTION_SCHEMA_V3 {
        return Err(DescriptionDecodeRejection::UnsupportedTag(
            "description schema",
        ));
    }
    let frontier = DescriptionFrontier::from_tag(reader.byte()?)
        .ok_or(DescriptionDecodeRejection::UnsupportedTag("frontier tag"))?;

    let artifact_length = reader.u32()? as usize;
    let artifact_bytes = reader.take(artifact_length)?.to_vec();
    let artifact = terminal_codec::CanonicalTerminalArtifact::from_bytes(&artifact_bytes)
        .map_err(|_| DescriptionDecodeRejection::Corrupt("embedded artifact did not decode"))?;

    let mut imports = Vec::new();
    let mut last_import: Option<(u32, String)> = None;
    for _ in 0..roster_len(&mut reader, "import")? {
        let slot = reader.u32()?;
        let requirement_identity = read_identity(&mut reader)?;
        let contract_identity = reader.array::<32>()?;
        let key = (slot, requirement_identity.clone());
        if last_import.as_ref() >= Some(&key) {
            return Err(DescriptionDecodeRejection::NonCanonicalOrder("import"));
        }
        last_import = Some(key);
        imports.push(ImportSlot {
            slot,
            requirement_identity,
            contract_identity,
        });
    }

    let mut exports = Vec::new();
    let mut last_export: Option<String> = None;
    for _ in 0..roster_len(&mut reader, "export")? {
        let identity = read_identity(&mut reader)?;
        if last_export.as_ref() >= Some(&identity) {
            return Err(DescriptionDecodeRejection::NonCanonicalOrder("export"));
        }
        last_export = Some(identity.clone());
        exports.push(ExportSurface { identity });
    }

    let mut entries = Vec::new();
    let mut last_entry: Option<(u8, String)> = None;
    for _ in 0..roster_len(&mut reader, "entry")? {
        let kind = ComponentEntryKind::from_tag(reader.byte()?)
            .ok_or(DescriptionDecodeRejection::UnsupportedTag("entry kind"))?;
        let identity = read_identity(&mut reader)?;
        let evidence = match reader.byte()? {
            0 => EntryEvidence::ModuleDerived,
            1 => EntryEvidence::AssumptionBound(reader.array::<32>()?),
            _ => return Err(DescriptionDecodeRejection::UnsupportedTag("entry evidence")),
        };
        let key = (kind.tag(), identity.clone());
        if last_entry.as_ref() >= Some(&key) {
            return Err(DescriptionDecodeRejection::NonCanonicalOrder("entry"));
        }
        last_entry = Some(key);
        entries.push(ComponentEntry {
            kind,
            identity,
            evidence,
        });
    }

    let mut outgoing = Vec::new();
    let mut last_outgoing: Option<(u8, String)> = None;
    for _ in 0..roster_len(&mut reader, "outgoing authority")? {
        let class = OutgoingAuthorityClass::from_tag(reader.byte()?).ok_or(
            DescriptionDecodeRejection::UnsupportedTag("authority class"),
        )?;
        let identity = read_identity(&mut reader)?;
        let evidence = match reader.byte()? {
            0 => OutgoingEvidence::ModuleDerived,
            1 => OutgoingEvidence::ProviderSealed(reader.array::<32>()?),
            2 => OutgoingEvidence::PhysicalMechanism(reader.array::<32>()?),
            3 => OutgoingEvidence::AssumptionBound(reader.array::<32>()?),
            _ => {
                return Err(DescriptionDecodeRejection::UnsupportedTag(
                    "authority evidence",
                ));
            }
        };
        let key = (class.tag(), identity.clone());
        if last_outgoing.as_ref() >= Some(&key) {
            return Err(DescriptionDecodeRejection::NonCanonicalOrder(
                "outgoing authority",
            ));
        }
        last_outgoing = Some(key);
        outgoing.push(OutgoingAuthority {
            class,
            identity,
            evidence,
        });
    }

    let mut service_bounds = Vec::new();
    let mut last_bound: Option<(String, Vec<ServiceId>)> = None;
    for _ in 0..roster_len(&mut reader, "service bound")? {
        let requirement_identity = read_identity(&mut reader)?;
        let mut bound = Vec::new();
        let mut last_service: Option<ServiceId> = None;
        for _ in 0..roster_len(&mut reader, "service bound services")? {
            let service = ServiceId::new(reader.u64()?)
                .ok_or(DescriptionDecodeRejection::Corrupt("zero service identity"))?;
            if last_service >= Some(service) {
                return Err(DescriptionDecodeRejection::NonCanonicalOrder(
                    "service bound services",
                ));
            }
            last_service = Some(service);
            bound.push(service);
        }
        let key = (requirement_identity.clone(), bound.clone());
        if last_bound.as_ref() >= Some(&key) {
            return Err(DescriptionDecodeRejection::NonCanonicalOrder(
                "service bound",
            ));
        }
        last_bound = Some(key);
        service_bounds.push(InstallationServiceBound {
            requirement_identity,
            bound,
        });
    }

    let mut custody = Vec::new();
    let mut last_custody: Option<(u8, String)> = None;
    for _ in 0..roster_len(&mut reader, "custody")? {
        let kind = CustodyKind::from_tag(reader.byte()?)
            .ok_or(DescriptionDecodeRejection::UnsupportedTag("custody kind"))?;
        let identity = read_identity(&mut reader)?;
        let evidence = match reader.byte()? {
            0 => CustodyEvidence::ModuleDerived,
            1 => CustodyEvidence::AssumptionBound(reader.array::<32>()?),
            _ => {
                return Err(DescriptionDecodeRejection::UnsupportedTag(
                    "custody evidence",
                ));
            }
        };
        let key = (kind.tag(), identity.clone());
        if last_custody.as_ref() >= Some(&key) {
            return Err(DescriptionDecodeRejection::NonCanonicalOrder("custody"));
        }
        last_custody = Some(key);
        custody.push(CustodyConstraint {
            kind,
            identity,
            evidence,
        });
    }

    let mut providers = Vec::new();
    let mut last_provider: Option<u64> = None;
    for _ in 0..roster_len(&mut reader, "provider")? {
        let report_identity = reader.u64()?;
        let plan_digest = reader.array::<32>()?;
        let plan_input_length = reader.u32()? as usize;
        if plan_input_length > MAX_COMPONENT_DESCRIPTION_BYTES {
            return Err(DescriptionDecodeRejection::RosterBoundExceeded(
                "provider plan input",
            ));
        }
        let plan_input = reader.take(plan_input_length)?.to_vec();
        let requirement_count = roster_len(&mut reader, "provider requirement")?;
        let mut requirement_identities = Vec::with_capacity(requirement_count);
        let mut last_requirement: Option<String> = None;
        for _ in 0..requirement_count {
            let requirement = read_identity(&mut reader)?;
            if last_requirement.as_ref() >= Some(&requirement) {
                return Err(DescriptionDecodeRejection::NonCanonicalOrder(
                    "provider requirement",
                ));
            }
            last_requirement = Some(requirement.clone());
            requirement_identities.push(requirement);
        }
        if last_provider >= Some(report_identity) {
            return Err(DescriptionDecodeRejection::NonCanonicalOrder("provider"));
        }
        last_provider = Some(report_identity);
        providers.push(RetainedProvider {
            report_identity,
            plan_digest,
            plan_input,
            requirement_identities,
        });
    }
    let provider_closure_digest = reader.array::<32>()?;

    let mut obligations = Vec::new();
    let mut last_obligation: Option<(u8, String)> = None;
    for _ in 0..roster_len(&mut reader, "obligation")? {
        let kind = ObligationKind::from_tag(reader.byte()?).ok_or(
            DescriptionDecodeRejection::UnsupportedTag("obligation kind"),
        )?;
        let identity = read_identity(&mut reader)?;
        let detail = read_identity(&mut reader)?;
        let key = (kind.tag(), identity.clone());
        if last_obligation.as_ref() >= Some(&key) {
            return Err(DescriptionDecodeRejection::NonCanonicalOrder("obligation"));
        }
        last_obligation = Some(key);
        obligations.push(InstallationObligation {
            kind,
            identity,
            detail,
        });
    }

    let mut assumptions = Vec::new();
    let mut last_assumption: Option<[u8; 32]> = None;
    for _ in 0..roster_len(&mut reader, "assumption")? {
        let assumption = reader.array::<32>()?;
        if last_assumption >= Some(assumption) {
            return Err(DescriptionDecodeRejection::NonCanonicalOrder("assumption"));
        }
        last_assumption = Some(assumption);
        assumptions.push(assumption);
    }

    let realization_identity = match reader.byte()? {
        0 => None,
        1 => Some(reader.array::<32>()?),
        _ => {
            return Err(DescriptionDecodeRejection::UnsupportedTag(
                "realization presence",
            ));
        }
    };

    if reader.remaining() != 0 {
        return Err(DescriptionDecodeRejection::Corrupt("trailing bytes"));
    }

    Ok((
        ComponentDescription {
            schema,
            frontier,
            artifact_bytes,
            imports,
            exports,
            entries,
            outgoing,
            service_bounds,
            custody,
            providers,
            provider_closure_digest,
            obligations,
            assumptions,
            realization_identity,
        },
        artifact,
    ))
}

pub fn description_subject(
    description: &ComponentDescription,
) -> Result<TerminalPsiIdentity, DescribeError> {
    let artifact =
        terminal_codec::CanonicalTerminalArtifact::from_bytes(&description.artifact_bytes)
            .map_err(DescribeError::ArtifactFormat)?;
    let module = artifact.semantic_module();
    terminal_codec::terminal_psi_identity(module).map_err(DescribeError::ArtifactIdentity)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_longer_than_one_kibibyte_round_trips() {
        let identity = "export:requirement:".to_owned() + &"x".repeat(4096);
        let mut encoded = Vec::new();
        push_identity(&mut encoded, &identity);
        let mut reader = Cursor::new(&encoded);
        assert_eq!(read_identity(&mut reader), Ok(identity));
    }
}

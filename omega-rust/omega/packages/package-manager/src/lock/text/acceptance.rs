//! Length-framed acceptance meanings. No recovery of old compiler IR.
use super::{
    MAXIMUM_POLICY_TEXT_BYTES,
    framing::{Reader, Writer},
};
use crate::lock::{PackageAcceptanceRow, PackageLockError as Error, PackagePolicyAcceptance};
use package_evidence::record::{PACKAGE_POLICY_ROW_VERSION, PackagePolicyRowKind};
use semantic_vocabulary::PackageKeyIdentity;

use target::TargetProfile;

pub(in crate::lock) fn write(policy: &PackagePolicyAcceptance) -> Result<String, Error> {
    let mut writer = Writer::new(MAXIMUM_POLICY_TEXT_BYTES);
    writer.row("acceptance_schema", policy.schema_version)?;
    writer.row("rows", policy.rows.len())?;
    for row in &policy.rows {
        writer.row("row", row.kind.as_str())?;
        writer.row("key", crate::Hex(&row.key).to_string())?;
        writer.section("meaning", &row.text)?;
    }
    writer.append("end_acceptance\n")?;
    writer.finish()
}

pub(in crate::lock) fn read(
    text: &str,
    package: PackageKeyIdentity,
    target: TargetProfile,
    maximum_rows: usize,
    maximum_owned: usize,
) -> Result<(PackagePolicyAcceptance, usize), Error> {
    let mut reader = Reader::new(text);
    let schema_version = reader.count("acceptance_schema", usize::MAX)?;
    // Schema 4 has the same opaque row framing, but predates authored target
    // context. Retain it only as historical consent for an explicit review;
    // never relabel its meanings or infer absent context. Ordinary recovery
    // and compilation reject this historical schema at their own boundary.
    if schema_version != usize::from(PACKAGE_POLICY_ROW_VERSION) && schema_version != 4 {
        return Err(Error::UnsupportedVersion);
    }
    let count = reader.count("rows", maximum_rows)?;
    // Bound both the element allocation and strings before retaining anything.
    let owned = count
        .checked_mul(std::mem::size_of::<PackageAcceptanceRow>())
        .and_then(|bytes| bytes.checked_add(text.len()))
        .filter(|bytes| *bytes <= maximum_owned)
        .ok_or(Error::AllocationLimitExceeded)?;
    if count > text.len() / "row x\nkey x\nmeaning 0\n".len() {
        return Err(Error::InvalidFraming);
    }
    let mut rows: Vec<PackageAcceptanceRow> = Vec::new();
    rows.try_reserve_exact(count)
        .map_err(|_| Error::AllocationFailed)?;
    for _ in 0..count {
        let kind = match reader.field("row")? {
            "callable" => PackagePolicyRowKind::Callable,
            "terminal_permission" => PackagePolicyRowKind::TerminalPermission,
            "external_supply" => PackagePolicyRowKind::ExternalSupply,
            "dangerous_capability" => PackagePolicyRowKind::DangerousCapability,
            "restricted_build_request" => PackagePolicyRowKind::RestrictedBuildRequest,
            "assumed_binding_contract" => PackagePolicyRowKind::AssumedBindingContract,
            _ => return Err(Error::InvalidFraming),
        };
        let hex = reader.field("key")?;
        if hex.len() != 64
            || !hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(Error::InvalidFraming);
        }
        let mut key = [0u8; 32];
        for (index, byte) in key.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16)
                .map_err(|_| Error::InvalidFraming)?;
        }
        if rows
            .last()
            .is_some_and(|last| (last.kind, last.key) >= (kind, key))
        {
            return Err(Error::InvalidFraming);
        }
        let meaning = reader.section("meaning", MAXIMUM_POLICY_TEXT_BYTES)?;
        if meaning.is_empty() || !meaning.ends_with('\n') {
            return Err(Error::InvalidFraming);
        }
        rows.push(PackageAcceptanceRow {
            kind,
            key,
            text: meaning.to_owned(),
        });
    }
    reader.expect("end_acceptance")?;
    reader.finish()?;
    Ok((
        PackagePolicyAcceptance {
            schema_version: schema_version as u16,
            package,
            target,
            rows,
        },
        owned,
    ))
}

// Length-framed encoding and bounded recovery for package policy acceptance rows.
// Earlier schema-4/5 opaque rows remain historical consent for comparison only, whereas superseded schema-8 rows are rejected with `omega update` guidance because version 9 renumbered builtin slots; both element and string storage are bounded before retaining rows.

use super::{
    MAXIMUM_POLICY_TEXT_BYTES,
    framing::{Reader, Writer},
};
use crate::hex_digest;
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
    if schema_version != usize::from(PACKAGE_POLICY_ROW_VERSION) && !matches!(schema_version, 4 | 5)
    {
        return Err(Error::UnsupportedVersion);
    }
    let count = reader.count("rows", maximum_rows)?;
    let owned = count
        .checked_mul(std::mem::size_of::<PackageAcceptanceRow>())
        .and_then(|bytes| bytes.checked_add(text.len()))
        .filter(|bytes| *bytes <= maximum_owned)
        .ok_or(Error::AllocationLimitExceeded)?;
    if count > text.len() / "row x\nkey x\nmeaning 0\n".len() {
        return Err(Error::InvalidFraming);
    }
    let mut rows: Vec<PackageAcceptanceRow> = Vec::new();
    crate::reserve!(rows, count, Error);
    for _ in 0..count {
        let kind = match reader.field("row")? {
            "callable" => PackagePolicyRowKind::Callable,
            "terminal_permission" => PackagePolicyRowKind::TerminalPermission,
            "external_supply" => PackagePolicyRowKind::ExternalSupply,
            "dangerous_capability" => PackagePolicyRowKind::DangerousCapability,
            "restricted_build_request" => PackagePolicyRowKind::RestrictedBuildRequest,
            "assumed_binding_contract" => PackagePolicyRowKind::AssumedBindingContract,
            "service_audit" => PackagePolicyRowKind::ServiceAudit,
            "native_binding" => PackagePolicyRowKind::NativeBinding,
            "component_assumption" => PackagePolicyRowKind::ComponentAssumption,
            _ => return Err(Error::InvalidFraming),
        };
        let key = hex_digest(reader.field("key")?).ok_or(Error::InvalidFraming)?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn superseded_policy_row_schema_rejects_with_recovery_guidance() {
        let package = PackageKeyIdentity::from_digest([7; 32]).unwrap();
        let text = "acceptance_schema 8\nrows 0\n";
        let error = read(text, package, TargetProfile::LinuxX64, 16, 1 << 16)
            .expect_err("a superseded policy row schema must not load");
        assert!(matches!(error, Error::UnsupportedVersion));
        assert!(error.to_string().contains("omega update"));
    }
}

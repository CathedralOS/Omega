// Length-framed encoding and bounded recovery for package policy acceptance rows.
// Recovery requires the current row schema before allocating or interpreting acceptance.

use super::{
    MAXIMUM_POLICY_TEXT_BYTES,
    framing::{Reader, Writer},
};
use crate::hex_digest;
use crate::lock::{PackageAcceptanceRow, PackageLockError as Error, PackagePolicyAcceptance};
use review_evidence::record::{PACKAGE_POLICY_ROW_VERSION, PackagePolicyRowKind};
use semantic_vocabulary::PackageKeyIdentity;

use target::TargetProfile;

pub(in crate::lockfile) fn write(policy: &PackagePolicyAcceptance) -> Result<String, Error> {
    let mut writer = Writer::new(MAXIMUM_POLICY_TEXT_BYTES);
    writer.row("acceptance_schema", PACKAGE_POLICY_ROW_VERSION)?;
    writer.row("rows", policy.rows.len())?;
    for row in &policy.rows {
        writer.row("row", row.kind.as_str())?;
        writer.row("key", crate::Hex(&row.key).to_string())?;
        writer.section("meaning", &row.text)?;
    }
    writer.append("end_acceptance\n")?;
    writer.finish()
}

pub(in crate::lockfile) fn read(
    text: &str,
    package: PackageKeyIdentity,
    target: TargetProfile,
    maximum_rows: usize,
    maximum_owned: usize,
) -> Result<(PackagePolicyAcceptance, usize), Error> {
    let mut reader = Reader::new(text);
    let schema_version = reader.count("acceptance_schema", usize::MAX)?;
    if schema_version != usize::from(PACKAGE_POLICY_ROW_VERSION) {
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
    fn only_the_current_acceptance_rows_can_supply_a_review_baseline() {
        let package = PackageKeyIdentity::from_digest([7; 32]).unwrap();
        for version in [4, 5, 8, PACKAGE_POLICY_ROW_VERSION] {
            let record = format!("acceptance_schema {version}\nrows 0\nend_acceptance\n");
            let result = read(&record, package, TargetProfile::WindowsX64, 1, 1024);
            if version == PACKAGE_POLICY_ROW_VERSION {
                assert_eq!(write(&result.unwrap().0).unwrap(), record);
            } else {
                let error = result.expect_err("a superseded schema cannot supply consent");
                assert!(matches!(error, Error::UnsupportedVersion));
                assert!(error.to_string().contains("omega packages update"));
            }
        }
    }

    #[test]
    fn a_retired_binding_contract_is_rejected_even_with_the_current_row_schema() {
        let row = format!(
            "acceptance_schema {PACKAGE_POLICY_ROW_VERSION}\nrows 1\nrow terminal_permission\nkey {}\nmeaning 2\nx\nend_acceptance\n",
            "00".repeat(32),
        );
        let package = PackageKeyIdentity::from_digest([1; 32]).unwrap();
        let read_row = |text: &str| read(text, package, TargetProfile::WindowsX64, 1, 4096);
        assert!(read_row(&row).is_ok());
        let retired = row.replace("terminal_permission", "assumed_binding_contract");
        assert!(matches!(read_row(&retired), Err(Error::InvalidFraming)));
    }
}

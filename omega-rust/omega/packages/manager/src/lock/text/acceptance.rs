//! Length-framed acceptance meanings. No recovery of old compiler IR.
use super::{
    MAXIMUM_POLICY_TEXT_BYTES,
    framing::{Reader, Writer},
};
use crate::lock::{PackageAcceptanceRow, PackageLockError as Error, PackagePolicyAcceptance};
use package_evidence::record::{PACKAGE_POLICY_ROW_VERSION, PackagePolicyRowKind};
use semantic_vocabulary::PackageKeyIdentity;
use std::fmt::Write as _;
use target::TargetProfile;

pub(in crate::lock) fn write(policy: &PackagePolicyAcceptance) -> Result<String, Error> {
    let mut writer = Writer::new(MAXIMUM_POLICY_TEXT_BYTES);
    writer.row("acceptance_schema", PACKAGE_POLICY_ROW_VERSION)?;
    writer.row("rows", policy.rows.len())?;
    for row in &policy.rows {
        writer.row("row", row.kind.as_str())?;
        let mut key = String::with_capacity(64);
        for byte in row.key {
            write!(&mut key, "{byte:02x}").map_err(|_| Error::InvalidFraming)?;
        }
        writer.row("key", key)?;
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
    if reader.count("acceptance_schema", usize::MAX)? != usize::from(PACKAGE_POLICY_ROW_VERSION) {
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
    use package_evidence::{
        encoding::PackagePolicyTextRecoveryLimits, record::PackagePolicyBaseline,
    };

    #[test]
    #[ignore = "measurement against a caller-supplied v1 lock; no source replay"]
    fn measure_real_lock_compaction() {
        let path = std::env::var("OMEGA_LOCK_SIZE_FIXTURE").expect("set OMEGA_LOCK_SIZE_FIXTURE");
        let old = std::fs::read_to_string(path).unwrap();
        let mut reader = Reader::new(old.strip_prefix("omega_lock 1\n").unwrap());
        let mut writer = Writer::new(old.len());
        writer.append(super::super::HEADER).unwrap();
        let targets = reader.count("targets", 32).unwrap();
        writer.row("targets", targets).unwrap();
        let mut total_rows = 0;
        for _ in 0..targets {
            writer
                .row("target", reader.field("target").unwrap())
                .unwrap();
            // Source pins and decisions remain byte-identical, not interpreted on
            // this host (a Mac external-local path is not a Windows local path).
            writer
                .section("source", reader.section("source", old.len()).unwrap())
                .unwrap();
            let packages = reader.count("baselines", 16384).unwrap();
            writer.row("acceptances", packages).unwrap();
            for _ in 0..packages {
                let policy = PackagePolicyBaseline::recover_text(
                    reader.section("baseline", old.len()).unwrap(),
                    PackagePolicyTextRecoveryLimits::default(),
                )
                .unwrap();
                let acceptance = PackagePolicyAcceptance::from_policy(&policy).unwrap();
                let text = acceptance.canonical_text().unwrap();
                let (recovered, _) = read(
                    &text,
                    acceptance.package,
                    acceptance.target,
                    65536,
                    old.len(),
                )
                .unwrap();
                assert_eq!(acceptance, recovered);
                total_rows += acceptance.rows().len();
                writer.section("acceptance", &text).unwrap();
            }
            writer
                .section("decisions", reader.section("decisions", old.len()).unwrap())
                .unwrap();
            reader.expect("end_target").unwrap();
            writer.append("end_target\n").unwrap();
        }
        reader.expect("end").unwrap();
        reader.finish().unwrap();
        writer.append("end\n").unwrap();
        let compact = writer.finish().unwrap();
        println!(
            "lock bytes: {} -> {}; acceptance rows: {}",
            old.len(),
            compact.len(),
            total_rows
        );
        if let Ok(output) = std::env::var("OMEGA_COMPACT_LOCK_OUTPUT") {
            std::fs::write(output, &compact).unwrap();
        }
        assert!(
            compact.len() < old.len() / 5,
            "real lock must lose at least 80% of snapshot storage"
        );
    }
}

use super::super::construction::{clone_bytes, finish_ledger};
use super::super::limits::{
    LEDGER_MAGIC, MAXIMUM_LEDGER_ALIAS_BYTES, MAXIMUM_LEDGER_DEPENDENCIES,
    MAXIMUM_LEDGER_ENCODING_BYTES, MAXIMUM_LEDGER_KEY_BYTES, MAXIMUM_LEDGER_PACKAGES,
    MAXIMUM_LEDGER_ROW_BYTES, MAXIMUM_LEDGER_ROWS, MAXIMUM_LEDGER_TARGET_BYTES,
    MAXIMUM_LEDGER_TOTAL_ROW_BYTES, ORDINARY_PACKAGE_OBLIGATION_LEDGER_ENCODING_VERSION,
};
use super::super::model::{
    OrdinaryPackageObligationLedger, OrdinaryPackageObligationLedgerRecoveryError,
    OrdinaryPackageObligationRow, OrdinaryPackageObligationSchemaIdentity,
};
use super::encoding::encode_ordinary_package_obligation_ledger;
use crate::encoding::recovery::canonical_row_framing_for_ledger;
use crate::encoding::{PACKAGE_REVIEW_ENCODING_VERSION, PACKAGE_REVIEW_ROW_ENCODING_VERSION};
use omega_package_compilation::{PackageDependencyBinding, PackageDependencyClosure};
use omega_target::TargetProfile;
use psi_core::PackageKeyIdentity;

/// Decode canonical ledger framing. Decoding establishes shape only; callers
/// must still invoke [`super::super::validate_ordinary_package_obligation_ledger`]
/// against the exact locally checked source subject.
pub fn decode_ordinary_package_obligation_ledger(
    bytes: &[u8],
) -> Result<OrdinaryPackageObligationLedger, OrdinaryPackageObligationLedgerRecoveryError> {
    if bytes.len() > MAXIMUM_LEDGER_ENCODING_BYTES {
        return Err(OrdinaryPackageObligationLedgerRecoveryError::new(
            "ordinary package obligation ledger encoding exceeds its byte ceiling",
        ));
    }
    let mut decoder = LedgerDecoder::new(bytes);
    decoder.fixed_bytes(LEDGER_MAGIC)?;
    if decoder.u16()? != ORDINARY_PACKAGE_OBLIGATION_LEDGER_ENCODING_VERSION {
        return Err(OrdinaryPackageObligationLedgerRecoveryError::new(
            "unsupported ordinary package obligation ledger encoding version",
        ));
    }
    let schema = OrdinaryPackageObligationSchemaIdentity::decode(decoder.u16()?)?;
    if decoder.u16()? != PACKAGE_REVIEW_ENCODING_VERSION
        || decoder.u16()? != PACKAGE_REVIEW_ROW_ENCODING_VERSION
    {
        return Err(OrdinaryPackageObligationLedgerRecoveryError::new(
            "ordinary package obligation ledger names an unsupported review-row vocabulary",
        ));
    }
    let package = decoder.package_identity()?;
    let target_name = decoder.string(MAXIMUM_LEDGER_TARGET_BYTES)?;
    let target = TargetProfile::from_omega_target_name(Some(&target_name)).map_err(|_| {
        OrdinaryPackageObligationLedgerRecoveryError::new(
            "ordinary package obligation ledger contains a noncanonical target",
        )
    })?;
    if target.target_name() != target_name {
        return Err(OrdinaryPackageObligationLedgerRecoveryError::new(
            "ordinary package obligation ledger contains a noncanonical target",
        ));
    }

    let closure_root = decoder.package_identity()?;
    let package_count = decoder.count(
        MAXIMUM_LEDGER_PACKAGES,
        "ordinary package obligation ledger package count exceeds its ceiling",
    )?;
    let mut packages = Vec::new();
    packages.try_reserve_exact(package_count).map_err(|_| {
        OrdinaryPackageObligationLedgerRecoveryError::new(
            "ordinary package obligation ledger package allocation failed",
        )
    })?;
    for _ in 0..package_count {
        packages.push(decoder.package_identity()?);
    }

    let dependency_count = decoder.count(
        MAXIMUM_LEDGER_DEPENDENCIES,
        "ordinary package obligation ledger dependency count exceeds its ceiling",
    )?;
    let mut dependencies = Vec::new();
    dependencies
        .try_reserve_exact(dependency_count)
        .map_err(|_| {
            OrdinaryPackageObligationLedgerRecoveryError::new(
                "ordinary package obligation ledger dependency allocation failed",
            )
        })?;
    for _ in 0..dependency_count {
        let requester = decoder.package_identity()?;
        let alias = decoder.string(MAXIMUM_LEDGER_ALIAS_BYTES)?;
        let target = decoder.package_identity()?;
        dependencies.push(PackageDependencyBinding::new(requester, alias, target));
    }
    let dependency_closure =
        PackageDependencyClosure::from_canonical_parts(closure_root, packages, dependencies)
            .map_err(OrdinaryPackageObligationLedgerRecoveryError::new)?;

    let row_count = decoder.count(
        MAXIMUM_LEDGER_ROWS,
        "ordinary package obligation ledger row count exceeds its ceiling",
    )?;
    if row_count == 0 {
        return Err(OrdinaryPackageObligationLedgerRecoveryError::new(
            "ordinary package obligation ledger has no rows",
        ));
    }
    let mut rows = Vec::new();
    rows.try_reserve_exact(row_count).map_err(|_| {
        OrdinaryPackageObligationLedgerRecoveryError::new(
            "ordinary package obligation ledger row allocation failed",
        )
    })?;
    let mut total_row_bytes = 0usize;
    for _ in 0..row_count {
        let canonical_bytes = decoder.bytes(MAXIMUM_LEDGER_ROW_BYTES)?;
        let framing = canonical_row_framing_for_ledger(canonical_bytes).map_err(|_| {
            OrdinaryPackageObligationLedgerRecoveryError::new(
                "ordinary package obligation ledger contains an invalid canonical row",
            )
        })?;
        if framing.package != package {
            return Err(OrdinaryPackageObligationLedgerRecoveryError::new(
                "ordinary package obligation ledger row has a different package identity",
            ));
        }
        if framing.target != target {
            return Err(OrdinaryPackageObligationLedgerRecoveryError::new(
                "ordinary package obligation ledger row has a different target",
            ));
        }
        if framing.key_bytes.len() > MAXIMUM_LEDGER_KEY_BYTES {
            return Err(OrdinaryPackageObligationLedgerRecoveryError::new(
                "ordinary package obligation ledger row key exceeds its byte ceiling",
            ));
        }
        total_row_bytes = total_row_bytes
            .checked_add(framing.key_bytes.len())
            .and_then(|total| total.checked_add(canonical_bytes.len()))
            .ok_or_else(|| {
                OrdinaryPackageObligationLedgerRecoveryError::new(
                    "ordinary package obligation ledger row-byte accounting overflowed",
                )
            })?;
        if total_row_bytes > MAXIMUM_LEDGER_TOTAL_ROW_BYTES {
            return Err(OrdinaryPackageObligationLedgerRecoveryError::new(
                "ordinary package obligation ledger exceeds its total row-byte ceiling",
            ));
        }
        rows.push(OrdinaryPackageObligationRow::from_parts(
            framing.kind,
            framing.risk,
            framing.key_bytes,
            clone_bytes(canonical_bytes)?,
        ));
    }
    decoder.finish()?;

    let ledger = finish_ledger(schema, package, target, dependency_closure, rows)?;
    if encode_ordinary_package_obligation_ledger(&ledger)? != bytes {
        return Err(OrdinaryPackageObligationLedgerRecoveryError::new(
            "ordinary package obligation ledger encoding is not canonical",
        ));
    }
    Ok(ledger)
}

struct LedgerDecoder<'bytes> {
    bytes: &'bytes [u8],
    position: usize,
}

impl<'bytes> LedgerDecoder<'bytes> {
    const fn new(bytes: &'bytes [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    fn take(
        &mut self,
        count: usize,
    ) -> Result<&'bytes [u8], OrdinaryPackageObligationLedgerRecoveryError> {
        let end = self.position.checked_add(count).ok_or_else(|| {
            OrdinaryPackageObligationLedgerRecoveryError::new(
                "ordinary package obligation ledger length frame overflowed",
            )
        })?;
        let value = self.bytes.get(self.position..end).ok_or_else(|| {
            OrdinaryPackageObligationLedgerRecoveryError::new(
                "ordinary package obligation ledger encoding is truncated",
            )
        })?;
        self.position = end;
        Ok(value)
    }

    fn fixed_bytes(
        &mut self,
        expected: &[u8],
    ) -> Result<(), OrdinaryPackageObligationLedgerRecoveryError> {
        if self.take(expected.len())? != expected {
            return Err(OrdinaryPackageObligationLedgerRecoveryError::new(
                "ordinary package obligation ledger has invalid framing magic",
            ));
        }
        Ok(())
    }

    fn u16(&mut self) -> Result<u16, OrdinaryPackageObligationLedgerRecoveryError> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().expect(
            "ordinary package obligation ledger u16 width is fixed",
        )))
    }

    fn u64(&mut self) -> Result<u64, OrdinaryPackageObligationLedgerRecoveryError> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().expect(
            "ordinary package obligation ledger u64 width is fixed",
        )))
    }

    fn count(
        &mut self,
        maximum: usize,
        exceeded_message: &'static str,
    ) -> Result<usize, OrdinaryPackageObligationLedgerRecoveryError> {
        let count = usize::try_from(self.u64()?)
            .map_err(|_| OrdinaryPackageObligationLedgerRecoveryError::new(exceeded_message))?;
        if count > maximum {
            return Err(OrdinaryPackageObligationLedgerRecoveryError::new(
                exceeded_message,
            ));
        }
        Ok(count)
    }

    fn bytes(
        &mut self,
        maximum: usize,
    ) -> Result<&'bytes [u8], OrdinaryPackageObligationLedgerRecoveryError> {
        let length = self.count(
            maximum,
            "ordinary package obligation ledger byte field exceeds its ceiling",
        )?;
        self.take(length)
    }

    fn string(
        &mut self,
        maximum: usize,
    ) -> Result<String, OrdinaryPackageObligationLedgerRecoveryError> {
        let bytes = self.bytes(maximum)?;
        let value = std::str::from_utf8(bytes).map_err(|_| {
            OrdinaryPackageObligationLedgerRecoveryError::new(
                "ordinary package obligation ledger contains invalid UTF-8",
            )
        })?;
        let mut output = String::new();
        output.try_reserve_exact(value.len()).map_err(|_| {
            OrdinaryPackageObligationLedgerRecoveryError::new(
                "ordinary package obligation ledger string allocation failed",
            )
        })?;
        output.push_str(value);
        Ok(output)
    }

    fn package_identity(
        &mut self,
    ) -> Result<PackageKeyIdentity, OrdinaryPackageObligationLedgerRecoveryError> {
        let digest: [u8; 32] = self
            .take(32)?
            .try_into()
            .expect("ordinary package obligation ledger package identity width is fixed");
        PackageKeyIdentity::from_digest(digest).ok_or_else(|| {
            OrdinaryPackageObligationLedgerRecoveryError::new(
                "ordinary package obligation ledger contains an invalid package identity",
            )
        })
    }

    fn finish(self) -> Result<(), OrdinaryPackageObligationLedgerRecoveryError> {
        if self.position != self.bytes.len() {
            return Err(OrdinaryPackageObligationLedgerRecoveryError::new(
                "ordinary package obligation ledger encoding has trailing bytes",
            ));
        }
        Ok(())
    }
}

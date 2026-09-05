use super::super::construction::validate_encoding_budget;
use super::super::limits::{
    LEDGER_FINGERPRINT_DOMAIN, LEDGER_MAGIC, MAXIMUM_LEDGER_ENCODING_BYTES,
    ORDINARY_PACKAGE_OBLIGATION_LEDGER_ENCODING_VERSION,
};
use super::super::model::{
    OrdinaryPackageObligationLedger, OrdinaryPackageObligationLedgerFingerprint,
    OrdinaryPackageObligationLedgerRecoveryError,
};
use crate::encoding::{PACKAGE_REVIEW_ENCODING_VERSION, PACKAGE_REVIEW_ROW_ENCODING_VERSION};
use semantic_vocabulary::PackageKeyIdentity;
use sha2::{Digest, Sha256};

/// Encode the complete source-path-free replay question in one bounded,
/// canonical frame. The resulting bytes are not a certificate, discharge
/// result, admission decision, package instance, or lock payload.
pub fn encode_ordinary_package_obligation_ledger(
    ledger: &OrdinaryPackageObligationLedger,
) -> Result<Vec<u8>, OrdinaryPackageObligationLedgerRecoveryError> {
    validate_encoding_budget(ledger)?;
    let mut encoder = LedgerEncoder::bounded(MAXIMUM_LEDGER_ENCODING_BYTES);
    encoder.fixed_bytes(LEDGER_MAGIC);
    encoder.u16(ORDINARY_PACKAGE_OBLIGATION_LEDGER_ENCODING_VERSION);
    encoder.u16(ledger.schema().version());
    encoder.u16(PACKAGE_REVIEW_ENCODING_VERSION);
    encoder.u16(PACKAGE_REVIEW_ROW_ENCODING_VERSION);
    encoder.package_identity(ledger.package());
    encoder.string(ledger.target().target_name())?;
    encoder.package_identity(ledger.dependency_closure().root());
    encoder.byte(match ledger.dependency_closure().root_role() {
        package_compilation::BuildDeclarationKind::Package => 0,
        package_compilation::BuildDeclarationKind::Application => 1,
        package_compilation::BuildDeclarationKind::Workspace => 2,
    });
    encoder.usize(ledger.dependency_closure().packages().len())?;
    for package in ledger.dependency_closure().packages() {
        encoder.package_identity(*package);
    }
    encoder.usize(ledger.dependency_closure().dependencies().len())?;
    for dependency in ledger.dependency_closure().dependencies() {
        encoder.package_identity(dependency.requester());
        encoder.string(dependency.alias())?;
        encoder.package_identity(dependency.target());
    }
    encoder.usize(ledger.rows().len())?;
    for row in ledger.rows() {
        encoder.bytes(row.canonical_bytes())?;
    }
    encoder.finish()
}

pub fn ordinary_package_obligation_ledger_fingerprint(
    ledger: &OrdinaryPackageObligationLedger,
) -> Result<OrdinaryPackageObligationLedgerFingerprint, OrdinaryPackageObligationLedgerRecoveryError>
{
    let bytes = encode_ordinary_package_obligation_ledger(ledger)?;
    let mut digest = Sha256::new();
    digest.update(LEDGER_FINGERPRINT_DOMAIN);
    digest.update(
        u64::try_from(bytes.len())
            .expect("bounded ordinary package obligation ledger length fits u64")
            .to_le_bytes(),
    );
    digest.update(bytes);
    Ok(OrdinaryPackageObligationLedgerFingerprint::from_digest(
        digest.finalize().into(),
    ))
}

struct LedgerEncoder {
    output: Vec<u8>,
    maximum_bytes: usize,
    exceeded: bool,
}

impl LedgerEncoder {
    fn bounded(maximum_bytes: usize) -> Self {
        Self {
            output: Vec::new(),
            maximum_bytes,
            exceeded: false,
        }
    }

    fn append(&mut self, bytes: &[u8]) {
        if self.exceeded {
            return;
        }
        let Some(required) = self.output.len().checked_add(bytes.len()) else {
            self.exceeded = true;
            return;
        };
        if required > self.maximum_bytes || self.output.try_reserve(bytes.len()).is_err() {
            self.exceeded = true;
            return;
        }
        self.output.extend_from_slice(bytes);
    }

    fn finish(self) -> Result<Vec<u8>, OrdinaryPackageObligationLedgerRecoveryError> {
        if self.exceeded {
            Err(OrdinaryPackageObligationLedgerRecoveryError::new(
                "ordinary package obligation ledger encoding exceeds its byte ceiling",
            ))
        } else {
            Ok(self.output)
        }
    }

    fn fixed_bytes(&mut self, bytes: &[u8]) {
        self.append(bytes);
    }

    fn u16(&mut self, value: u16) {
        self.append(&value.to_le_bytes());
    }

    fn byte(&mut self, value: u8) {
        self.append(&[value]);
    }

    fn u64(&mut self, value: u64) {
        self.append(&value.to_le_bytes());
    }

    fn usize(&mut self, value: usize) -> Result<(), OrdinaryPackageObligationLedgerRecoveryError> {
        self.u64(u64::try_from(value).map_err(|_| {
            OrdinaryPackageObligationLedgerRecoveryError::new(
                "ordinary package obligation ledger value exceeds the portable encoding range",
            )
        })?);
        Ok(())
    }

    fn bytes(&mut self, bytes: &[u8]) -> Result<(), OrdinaryPackageObligationLedgerRecoveryError> {
        self.usize(bytes.len())?;
        self.append(bytes);
        Ok(())
    }

    fn string(&mut self, value: &str) -> Result<(), OrdinaryPackageObligationLedgerRecoveryError> {
        self.bytes(value.as_bytes())
    }

    fn package_identity(&mut self, identity: PackageKeyIdentity) {
        self.append(&identity.digest());
    }
}

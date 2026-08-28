//! Domain-separated fingerprints for format-36 installation evidence.

use sha2::{Digest, Sha256};

use super::{
    ImageFingerprint, InstallationFingerprint, NativeFuelSourceFingerprint,
    NativeFuelTransferTextFingerprint,
};

const IMAGE_DOMAIN: &[u8] = b"omega-installed-image\0";
const RECORD_DOMAIN: &[u8] = b"omega-installation-record\0";
const NATIVE_FUEL_SOURCE_DOMAIN: &[u8] = b"omega-native-fuel-source\0";
const NATIVE_FUEL_TRANSFER_UNRELOCATED_DOMAIN: &[u8] = b"omega-native-fuel-transfer-unrelocated\0";
const NATIVE_FUEL_TRANSFER_FINAL_DOMAIN: &[u8] = b"omega-native-fuel-transfer-final\0";

pub(super) fn fingerprint_image(bytes: &[u8]) -> ImageFingerprint {
    ImageFingerprint(hash(IMAGE_DOMAIN, bytes))
}

pub(super) fn fingerprint_record(bytes: &[u8]) -> InstallationFingerprint {
    InstallationFingerprint(hash(RECORD_DOMAIN, bytes))
}

pub(super) fn fingerprint_native_fuel_source(bytes: &[u8]) -> NativeFuelSourceFingerprint {
    NativeFuelSourceFingerprint(hash(NATIVE_FUEL_SOURCE_DOMAIN, bytes))
}

pub(super) fn fingerprint_native_fuel_transfer_unrelocated(
    bytes: &[u8],
) -> NativeFuelTransferTextFingerprint {
    NativeFuelTransferTextFingerprint(hash(NATIVE_FUEL_TRANSFER_UNRELOCATED_DOMAIN, bytes))
}

pub(super) fn fingerprint_native_fuel_transfer_final(
    bytes: &[u8],
) -> NativeFuelTransferTextFingerprint {
    NativeFuelTransferTextFingerprint(hash(NATIVE_FUEL_TRANSFER_FINAL_DOMAIN, bytes))
}

fn hash(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update(
        u64::try_from(bytes.len())
            .expect("terminal artifact bytes fit the digest domain")
            .to_le_bytes(),
    );
    digest.update(bytes);
    digest.finalize().into()
}

pub(super) fn write_hex(
    formatter: &mut std::fmt::Formatter<'_>,
    bytes: &[u8; 32],
) -> std::fmt::Result {
    for byte in bytes {
        write!(formatter, "{byte:02x}")?;
    }
    Ok(())
}

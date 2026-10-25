//! Physical linkage coordinates for a separately published component image.
//!
//! A content-addressed image and version-qualified export slot are one locator.
//! Normalizing coordinates establishes no checked provider, ABI compatibility,
//! package permission, installed mapping or historical-era authority. Those
//! owners independently join the locator to their complete retained products.

use crate::{NormalizedForeignLocator, TargetProfile};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedComponentLocator {
    target: TargetProfile,
    image_digest: [u8; 32],
    slot_commitment: [u8; 32],
    identity_digest: [u8; 32],
}

impl NormalizedComponentLocator {
    pub fn new(
        target: TargetProfile,
        image_digest: [u8; 32],
        slot_commitment: [u8; 32],
    ) -> Result<Self, &'static str> {
        if !matches!(
            target,
            TargetProfile::LinuxX64
                | TargetProfile::LinuxArm64
                | TargetProfile::MacosX64
                | TargetProfile::MacosArm64
                | TargetProfile::WindowsX64
        ) {
            return Err("component linkage requires an admitted ELF, Mach-O, or Windows PE target");
        }
        if image_digest == [0; 32] || slot_commitment == [0; 32] {
            return Err("component linkage requires exact image and export-slot commitments");
        }
        let mut digest =
            crate::digest_encoder::DomainDigestEncoder::new(b"omega.component-import-locator.v1");
        digest.bytes(target.target_name().as_bytes());
        digest.raw(&image_digest);
        digest.raw(&slot_commitment);
        Ok(Self {
            target,
            image_digest,
            slot_commitment,
            identity_digest: digest.finish(),
        })
    }

    /// Normalize the exact container bytes; publication independently verifies
    /// those bytes before placing the content-addressed sibling file.
    pub fn for_image(
        target: TargetProfile,
        contents: &[u8],
        slot: [u8; 32],
    ) -> Result<Self, &'static str> {
        let mut digest =
            crate::digest_encoder::DomainDigestEncoder::new(b"omega.component-shared-image.v1");
        digest.bytes(target.target_name().as_bytes());
        digest.bytes(contents);
        Self::new(target, digest.finish(), slot)
    }

    pub const fn target(&self) -> TargetProfile {
        self.target
    }
    pub const fn image_digest(&self) -> &[u8; 32] {
        &self.image_digest
    }
    pub const fn slot_commitment(&self) -> &[u8; 32] {
        &self.slot_commitment
    }
    pub const fn identity_digest(&self) -> &[u8; 32] {
        &self.identity_digest
    }
    pub fn non_authoritative_compatibility_fingerprint(&self) -> u64 {
        u64::from_le_bytes(
            self.identity_digest[..8]
                .try_into()
                .expect("fixed digest width"),
        )
    }

    /// A fixed adjacent filename, never an ambient package/library selector.
    /// Publication must retain and verify the exact bytes with this digest.
    pub fn image_filename(&self) -> String {
        let suffix = match self.target {
            TargetProfile::LinuxX64 | TargetProfile::LinuxArm64 => "so",
            TargetProfile::MacosX64 | TargetProfile::MacosArm64 => "dylib",
            TargetProfile::WindowsX64 => "dll",
            _ => unreachable!("component locator construction admits only supported containers"),
        };
        format!(
            "libomega-component-{}.{suffix}",
            hexadecimal(&self.image_digest)
        )
    }
    pub fn export_symbol(&self) -> String {
        format!("__omega_slot_{}", hexadecimal(&self.slot_commitment))
    }
}

/// Shared physical import vocabulary. Source foreign binding and checked
/// component call admission remain distinct; sharing relocation transport
/// cannot turn either locator into the other's authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NormalizedImportLocator {
    Foreign(NormalizedForeignLocator),
    Component(NormalizedComponentLocator),
}

/// Strong identity of one complete physical import locator, in its own
/// foreign/component normalization domain. It supplies no binding authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ImportLocatorIdentityDigest([u8; 32]);
impl ImportLocatorIdentityDigest {
    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NormalizedImportLocatorRef<'locator> {
    Foreign(&'locator NormalizedForeignLocator),
    Component(&'locator NormalizedComponentLocator),
}

impl NormalizedImportLocatorRef<'_> {
    pub fn to_owned(self) -> NormalizedImportLocator {
        match self {
            Self::Foreign(locator) => NormalizedImportLocator::Foreign(locator.clone()),
            Self::Component(locator) => NormalizedImportLocator::Component(locator.clone()),
        }
    }
    pub const fn target(self) -> TargetProfile {
        match self {
            Self::Foreign(locator) => locator.target(),
            Self::Component(locator) => locator.target(),
        }
    }
    pub fn identity_digest(self) -> ImportLocatorIdentityDigest {
        match self {
            Self::Foreign(locator) => {
                ImportLocatorIdentityDigest(locator.identity_digest().as_bytes())
            }
            Self::Component(locator) => ImportLocatorIdentityDigest(*locator.identity_digest()),
        }
    }
}

impl From<NormalizedForeignLocator> for NormalizedImportLocator {
    fn from(locator: NormalizedForeignLocator) -> Self {
        Self::Foreign(locator)
    }
}

impl From<NormalizedComponentLocator> for NormalizedImportLocator {
    fn from(locator: NormalizedComponentLocator) -> Self {
        Self::Component(locator)
    }
}

impl NormalizedImportLocator {
    pub const fn as_ref(&self) -> NormalizedImportLocatorRef<'_> {
        match self {
            Self::Foreign(locator) => NormalizedImportLocatorRef::Foreign(locator),
            Self::Component(locator) => NormalizedImportLocatorRef::Component(locator),
        }
    }
    pub const fn target(&self) -> TargetProfile {
        match self {
            Self::Foreign(locator) => locator.target(),
            Self::Component(locator) => locator.target(),
        }
    }
    pub fn identity_digest(&self) -> ImportLocatorIdentityDigest {
        match self {
            Self::Foreign(locator) => {
                ImportLocatorIdentityDigest(locator.identity_digest().as_bytes())
            }
            Self::Component(locator) => ImportLocatorIdentityDigest(*locator.identity_digest()),
        }
    }
    pub fn non_authoritative_compatibility_fingerprint(&self) -> u64 {
        match self {
            Self::Foreign(locator) => locator.non_authoritative_compatibility_fingerprint(),
            Self::Component(locator) => u64::from_le_bytes(
                locator.identity_digest()[..8]
                    .try_into()
                    .expect("fixed digest width"),
            ),
        }
    }
}

/// Version-qualified physical symbol key. Native admission separately proves
/// that this semantic subject and complete ABI application were selected.
pub fn component_export_slot(
    vocabulary: u16,
    semantic_subject: &[u8; 32],
    calling_application: &[u8; 32],
) -> [u8; 32] {
    let mut digest =
        crate::digest_encoder::DomainDigestEncoder::new(b"omega.component-export-slot.v1");
    digest.raw(&vocabulary.to_le_bytes());
    digest.raw(semantic_subject);
    digest.raw(calling_application);
    digest.finish()
}

#[cfg(test)]
mod tests {
    use super::NormalizedComponentLocator;
    use crate::TargetProfile;

    #[test]
    fn windows_component_locator_names_a_pe_sibling() {
        let locator = NormalizedComponentLocator::for_image(
            TargetProfile::WindowsX64,
            b"component bytes",
            [7; 32],
        )
        .expect("windows x86-64 is an admitted component target");
        let name = locator.image_filename();
        assert!(name.starts_with("libomega-component-"));
        assert!(name.ends_with(".dll"));
        assert!(locator.export_symbol().starts_with("__omega_slot_"));
    }
}

fn hexadecimal(bytes: &[u8; 32]) -> String {
    use std::fmt::Write;
    let mut result = String::with_capacity(64);
    for byte in bytes {
        write!(&mut result, "{byte:02x}").expect("writing into a String");
    }
    result
}

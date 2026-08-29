use sha2::{Digest, Sha256};

use super::{NativeArtifactIdentity, hash_bytes};

const IDENTITY_DOMAIN: &[u8] = b"omega.native-artifact.ranked-native-fuel.sha256.v1\0";

#[derive(Debug)]
pub struct RankedNativeFuelArtifactParts {
    pub psi_artifact: psi_terminal_codec::CanonicalTerminalArtifact,
    pub image: omega_image_emission::NativeFuelExecutableImage,
    pub installation: omega_image_emission::InstallationRecord,
}

/// Source-free native-artifact custody for the exact ranked-`u32` bootstrap
/// slice. It deliberately cannot represent a second general native pipeline.
#[derive(Debug)]
#[must_use = "ranked native-fuel custody must remain joined to its canonical semantics"]
pub struct RankedNativeFuelArtifact {
    psi_artifact: psi_terminal_codec::CanonicalTerminalArtifact,
    image: omega_image_emission::NativeFuelExecutableImage,
    installation: omega_image_emission::InstallationRecord,
    identity: NativeArtifactIdentity,
}

impl RankedNativeFuelArtifact {
    pub fn from_replayed_parts(parts: RankedNativeFuelArtifactParts) -> Result<Self, &'static str> {
        let mut artifact = Self {
            psi_artifact: parts.psi_artifact,
            image: parts.image,
            installation: parts.installation,
            identity: NativeArtifactIdentity([0; 32]),
        };
        artifact.identity = artifact.recomputed_identity()?;
        artifact.validate()?;
        Ok(artifact)
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        self.psi_artifact
            .validate()
            .map_err(|_| "ranked native-fuel artifact contains invalid canonical semantics")?;
        let semantic = self.semantic_artifact();
        if semantic.psi() != self.psi_artifact.manifest().semantic() {
            return Err("ranked native-fuel artifact semantic identity disagrees with its image");
        }
        let module = psi_terminal_codec::decode_module(self.psi_artifact.semantic_bytes())
            .map_err(|_| "ranked native-fuel canonical semantics failed to decode")?;
        if module.entry != semantic.entry()
            || semantic.functions().len() != 1
            || semantic.functions()[0].ranked_u32_countdown.is_none()
            || !semantic.boundary_settlements().is_empty()
        {
            return Err("ranked native-fuel artifact is not the exact closed ranked body");
        }
        omega_image_emission::validate_native_fuel_executable_image(&self.image)
            .map_err(|_| "ranked native-fuel image failed independent replay")?;
        omega_image_emission::validate_native_fuel_installation_record(
            &self.installation,
            &self.image,
        )
        .map_err(|_| "ranked native-fuel installation disagrees with its image")?;
        if self.identity != self.recomputed_identity()? {
            return Err("ranked native-fuel artifact identity disagrees with retained custody");
        }
        Ok(())
    }

    fn semantic_artifact(&self) -> &omega_image_emission::ObjectArtifact {
        self.image.semantic_artifact()
    }

    fn recomputed_identity(&self) -> Result<NativeArtifactIdentity, &'static str> {
        let mut digest = Sha256::new();
        digest.update(IDENTITY_DOMAIN);
        digest.update(self.psi_artifact.manifest().identity().as_bytes());
        digest.update([1]);
        digest.update(self.image.final_image_symbol_digest().as_bytes());
        let output = self.image.output();
        hash_bytes(&mut digest, self.semantic_artifact().text_bytes());
        hash_bytes(&mut digest, &output.bytes);
        hash_bytes(&mut digest, &output.final_text_bytes);
        let installation = omega_image_emission::encode_installation_record(&self.installation)
            .map_err(|_| "ranked native-fuel installation failed canonical encoding")?;
        hash_bytes(&mut digest, &installation);
        Ok(NativeArtifactIdentity(digest.finalize().into()))
    }

    pub const fn psi_artifact(&self) -> &psi_terminal_codec::CanonicalTerminalArtifact {
        &self.psi_artifact
    }

    pub const fn image(&self) -> &omega_image_emission::NativeFuelExecutableImage {
        &self.image
    }

    pub const fn installation(&self) -> &omega_image_emission::InstallationRecord {
        &self.installation
    }

    pub const fn identity(&self) -> NativeArtifactIdentity {
        self.identity
    }
}

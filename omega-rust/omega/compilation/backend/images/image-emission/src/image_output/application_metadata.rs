//! Rejoin an image request to its application metadata before realization.
//!
//! Build extraction and retained-artifact admission supply the same metadata.
//! The image owner checks its physical meaning without consulting either source
//! trees or product coordinators. Metadata does not grant execution authority.

use super::ExecutableImageEmissionRequest;
use target::{ApplicationIdentifier, HostedApplicationIntent, NativeTarget, ObjectFormat};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageApplicationMetadataError {
    SubsystemMismatch,
    IdentifierMismatch,
    MissingGuiIdentifier,
}

impl std::fmt::Display for ImageApplicationMetadataError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::SubsystemMismatch => "image request subsystem differs from application metadata",
            Self::IdentifierMismatch => {
                "image request code signature identifier differs from application metadata"
            }
            Self::MissingGuiIdentifier => {
                "signed macOS GUI image emission requires an application identifier"
            }
        })
    }
}

impl std::error::Error for ImageApplicationMetadataError {}

impl ExecutableImageEmissionRequest {
    /// Rejoin exact loader/signing metadata, inheriting an omitted direct
    /// signing identifier. A caller-supplied identifier is permitted when the
    /// application supplied none. Only Mach-O GUI realization requires one;
    /// raw PE subsystem words do not imply GUI intent.
    ///
    /// This borrows the request so rejection cannot discard loader custody.
    /// Dynamic ELF inputs keep their interpreter unchanged and carry no signing
    /// identity. Writer selection and executable validation remain separate.
    pub fn settle_application_metadata(
        &mut self,
        target: NativeTarget,
        intent: Option<HostedApplicationIntent>,
        subsystem: u16,
        identifier: Option<&ApplicationIdentifier>,
    ) -> Result<(), ImageApplicationMetadataError> {
        if let Self::Direct {
            subsystem: requested_subsystem,
            code_signature_identifier,
        } = self
        {
            if *requested_subsystem != subsystem {
                return Err(ImageApplicationMetadataError::SubsystemMismatch);
            }
            if let (Some(authored), Some(requested)) =
                (identifier, code_signature_identifier.as_deref())
                && requested != authored.as_str()
            {
                return Err(ImageApplicationMetadataError::IdentifierMismatch);
            }
            if let Some(authored) = identifier
                && code_signature_identifier.is_none()
            {
                *code_signature_identifier = Some(authored.as_str().to_owned());
            }
        }
        if target.object_format == ObjectFormat::MachO
            && intent == Some(HostedApplicationIntent::Gui)
            && self.code_signature_identifier().is_none()
        {
            return Err(ImageApplicationMetadataError::MissingGuiIdentifier);
        }
        Ok(())
    }
}

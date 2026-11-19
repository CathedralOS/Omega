//! The requested executable format and its agreement with application metadata.
use crate::{
    ApplicationIdentifier, HostedApplicationIntent, NativeTarget, NormalizedElfInterpreterPlan,
    ObjectFormat,
};

#[derive(Debug, Clone)]
pub enum ExecutableImageEmissionRequest {
    Direct {
        subsystem: u16,
        code_signature_identifier: Option<String>,
    },
    DynamicElf {
        interpreter: NormalizedElfInterpreterPlan,
    },
}

impl ExecutableImageEmissionRequest {
    pub const fn direct(subsystem: u16) -> Self {
        Self::Direct {
            subsystem,
            code_signature_identifier: None,
        }
    }

    pub fn code_signature_identifier(&self) -> Option<&str> {
        match self {
            Self::Direct {
                code_signature_identifier,
                ..
            } => code_signature_identifier.as_deref(),
            Self::DynamicElf { .. } => None,
        }
    }

    pub const fn dynamic_elf(interpreter: NormalizedElfInterpreterPlan) -> Self {
        Self::DynamicElf { interpreter }
    }
}

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

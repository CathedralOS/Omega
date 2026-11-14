/*
Whole-artifact `CanonicalTerminalArtifact` container codec (`encode_canonical_artifact`,
`decode_canonical_artifact`) and retained execution-code wrapper (`TerminalExecutionCode`,
`encode_execution_code`, `decode_execution_code`).
*/

use crate::cursor::Cursor;
use terminal_psi::TerminalModule;

use crate::{
    ArtifactManifestError, ClosedTerminalModule, CodecError, DebugMapError,
    PsiOptimizationExecutionRecord, PsiOptimizationExecutionRecordDecodeError,
    TerminalArtifactManifest, TerminalDebugMap, decode_debug_map,
    decode_psi_optimization_execution_record, encode_debug_map,
    encode_psi_optimization_execution_record,
};

const ARTIFACT_MAGIC: &[u8; 8] = b"PSIART\0\0";
const ARTIFACT_FORMAT_MARKER: u16 = 3;

#[derive(Debug, Clone, Copy)]
pub struct TerminalArtifactView<'artifact> {
    closed: &'artifact ClosedTerminalModule,
    identity: terminal_psi::TerminalPsiIdentity,
}

impl<'artifact> TerminalArtifactView<'artifact> {
    pub fn module(self) -> &'artifact TerminalModule {
        self.closed.module()
    }

    pub const fn closed_module(self) -> &'artifact ClosedTerminalModule {
        self.closed
    }

    pub const fn identity(self) -> terminal_psi::TerminalPsiIdentity {
        self.identity
    }
}

#[derive(Clone)]
#[must_use = "a canonical Terminal artifact owns the Psi-to-Omega handoff bytes"]
pub struct CanonicalTerminalArtifact {
    optimization_bytes: Vec<u8>,
    optimization: PsiOptimizationExecutionRecord,
    debug_bytes: Option<Vec<u8>>,
    manifest: TerminalArtifactManifest,
    semantic_module: std::sync::Arc<ClosedTerminalModule>,
}

impl PartialEq for CanonicalTerminalArtifact {
    fn eq(&self, other: &Self) -> bool {
        self.manifest == other.manifest
            && self.semantic_bytes() == other.semantic_bytes()
            && self.optimization_bytes == other.optimization_bytes
            && self.debug_bytes == other.debug_bytes
    }
}

impl Eq for CanonicalTerminalArtifact {}

impl std::fmt::Debug for CanonicalTerminalArtifact {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CanonicalTerminalArtifact")
            .field("manifest", &self.manifest)
            .field("semantic_byte_count", &self.semantic_bytes().len())
            .field("optimization", &self.optimization)
            .field("debug_byte_count", &self.debug_bytes.as_ref().map(Vec::len))
            .finish_non_exhaustive()
    }
}

impl CanonicalTerminalArtifact {
    pub fn from_parts(
        semantic_module: TerminalModule,
        optimization: PsiOptimizationExecutionRecord,
        debug_map: Option<&TerminalDebugMap>,
    ) -> Result<Self, CanonicalTerminalArtifactError> {
        let closed = ClosedTerminalModule::new(semantic_module)
            .map_err(CanonicalTerminalArtifactError::Semantic)?;
        Self::from_closed_parts(closed, optimization, debug_map)
    }

    pub fn from_closed_parts(
        closed: ClosedTerminalModule,
        optimization: PsiOptimizationExecutionRecord,
        debug_map: Option<&TerminalDebugMap>,
    ) -> Result<Self, CanonicalTerminalArtifactError> {
        let semantic_module = closed.module();
        if semantic_module.identities.invocation_scope().is_some() {
            return Err(CanonicalTerminalArtifactError::Semantic(
                CodecError::MalformedStructuralFoundation(
                    "invocation code cannot enter a portable Terminal artifact",
                ),
            ));
        }
        let optimization_bytes = encode_psi_optimization_execution_record(&optimization);
        let debug_bytes = debug_map
            .map(|debug_map| {
                encode_debug_map(semantic_module, debug_map)
                    .map_err(CanonicalTerminalArtifactError::Debug)
            })
            .transpose()?;
        let semantic = closed.identity();
        let manifest = crate::sections::artifact_manifest::manifest_for_semantic(
            semantic,
            &optimization,
            None,
            debug_bytes.as_deref(),
        )
        .map_err(CanonicalTerminalArtifactError::Manifest)?;
        Ok(Self {
            optimization_bytes,
            optimization,
            debug_bytes,
            manifest,
            semantic_module: std::sync::Arc::new(closed),
        })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let debug_header_bytes = if self.debug_bytes.is_some() { 9 } else { 1 };
        let capacity = ARTIFACT_MAGIC
            .len()
            .saturating_add(2)
            .saturating_add(16)
            .saturating_add(debug_header_bytes)
            .saturating_add(self.semantic_bytes().len())
            .saturating_add(self.optimization_bytes.len())
            .saturating_add(self.debug_bytes.as_ref().map_or(0, Vec::len));
        let mut bytes = Vec::with_capacity(capacity);
        bytes.extend_from_slice(ARTIFACT_MAGIC);
        crate::codec_bytes::push_encoded(&mut bytes, ARTIFACT_FORMAT_MARKER.to_le_bytes());
        encode_section_len(&mut bytes, self.semantic_bytes().len());
        encode_section_len(&mut bytes, self.optimization_bytes.len());
        match &self.debug_bytes {
            None => bytes.push(0),
            Some(debug) => {
                bytes.push(1);
                encode_section_len(&mut bytes, debug.len());
            }
        }
        bytes.extend_from_slice(self.semantic_bytes());
        bytes.extend_from_slice(&self.optimization_bytes);
        if let Some(debug) = &self.debug_bytes {
            bytes.extend_from_slice(debug);
        }
        bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CanonicalTerminalArtifactError> {
        let mut cursor = Cursor::new(bytes);
        if cursor.take(ARTIFACT_MAGIC.len())? != ARTIFACT_MAGIC {
            return Err(CanonicalTerminalArtifactEnvelopeError::InvalidMagic.into());
        }
        let marker = cursor.u16()?;
        if marker != ARTIFACT_FORMAT_MARKER {
            return Err(
                CanonicalTerminalArtifactEnvelopeError::UnsupportedFormatMarker(marker).into(),
            );
        }
        let semantic_len = section_len(&mut cursor, "semantic")?;
        let optimization_len = section_len(&mut cursor, "optimization")?;
        let debug_len = match cursor.byte()? {
            0 => None,
            1 => Some(section_len(&mut cursor, "debug")?),
            tag => return Err(CanonicalTerminalArtifactEnvelopeError::InvalidDebugTag(tag).into()),
        };
        let semantic_bytes = cursor.take(semantic_len)?;
        let optimization_bytes = cursor.take(optimization_len)?;
        let debug_bytes = debug_len.map(|len| cursor.take(len)).transpose()?;
        if cursor.remaining() != 0 {
            return Err(
                CanonicalTerminalArtifactEnvelopeError::TrailingBytes(cursor.remaining()).into(),
            );
        }

        let closed = ClosedTerminalModule::from_semantic_bytes(semantic_bytes)
            .map_err(CanonicalTerminalArtifactError::Semantic)?;
        let semantic_module = closed.module();
        let optimization = decode_psi_optimization_execution_record(optimization_bytes)
            .map_err(CanonicalTerminalArtifactError::Optimization)?;
        let debug_map = debug_bytes
            .map(|debug| {
                decode_debug_map(semantic_module, debug)
                    .map_err(CanonicalTerminalArtifactError::Debug)
            })
            .transpose()?;
        let artifact = Self::from_closed_parts(closed, optimization, debug_map.as_ref())?;
        if artifact.semantic_bytes() != semantic_bytes
            || artifact.optimization_bytes() != optimization_bytes
            || artifact.debug_bytes() != debug_bytes
        {
            return Err(CanonicalTerminalArtifactEnvelopeError::NonCanonicalSections.into());
        }
        Ok(artifact)
    }

    pub fn semantic_bytes(&self) -> &[u8] {
        self.semantic_module.semantic_bytes()
    }

    pub fn view(&self) -> TerminalArtifactView<'_> {
        TerminalArtifactView {
            closed: &self.semantic_module,
            identity: self.manifest.semantic(),
        }
    }

    pub fn into_semantics(self) -> (ClosedTerminalModule, terminal_psi::TerminalPsiIdentity) {
        (
            std::sync::Arc::unwrap_or_clone(self.semantic_module),
            self.manifest.semantic(),
        )
    }

    pub fn retain_code(&self) -> crate::RetainedTerminalCode {
        crate::RetainedTerminalCode::new(
            std::sync::Arc::clone(&self.semantic_module),
            self.manifest.semantic(),
        )
    }

    pub fn semantic_module(&self) -> &TerminalModule {
        self.semantic_module.module()
    }

    pub fn closed_module(&self) -> &ClosedTerminalModule {
        &self.semantic_module
    }

    pub fn optimization_bytes(&self) -> &[u8] {
        &self.optimization_bytes
    }

    pub const fn optimization(&self) -> &PsiOptimizationExecutionRecord {
        &self.optimization
    }

    pub fn debug_bytes(&self) -> Option<&[u8]> {
        self.debug_bytes.as_deref()
    }

    pub const fn manifest(&self) -> TerminalArtifactManifest {
        self.manifest
    }
}

fn encode_section_len(bytes: &mut Vec<u8>, len: usize) {
    bytes.extend_from_slice(
        &u64::try_from(len)
            .expect("an in-memory Terminal artifact section fits u64")
            .to_le_bytes(),
    );
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CanonicalTerminalArtifactError {
    Semantic(CodecError),
    Optimization(PsiOptimizationExecutionRecordDecodeError),
    Debug(DebugMapError),
    Manifest(ArtifactManifestError),
    Envelope(CanonicalTerminalArtifactEnvelopeError),
}

optimization::truncated_from_adapter!(CanonicalTerminalArtifactError => Self::Envelope(CanonicalTerminalArtifactEnvelopeError::UnexpectedEnd));

impl From<CanonicalTerminalArtifactEnvelopeError> for CanonicalTerminalArtifactError {
    fn from(error: CanonicalTerminalArtifactEnvelopeError) -> Self {
        Self::Envelope(error)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CanonicalTerminalArtifactEnvelopeError {
    InvalidMagic,
    UnsupportedFormatMarker(u16),
    InvalidDebugTag(u8),
    SectionTooLong(&'static str),
    UnexpectedEnd,
    TrailingBytes(usize),
    NonCanonicalSections,
}

impl std::fmt::Display for CanonicalTerminalArtifactError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for CanonicalTerminalArtifactError {}

optimization::truncated_from_adapter!(CanonicalTerminalArtifactEnvelopeError => Self::UnexpectedEnd);

fn section_len(
    cursor: &mut Cursor,
    section: &'static str,
) -> Result<usize, CanonicalTerminalArtifactEnvelopeError> {
    usize::try_from(cursor.u64()?)
        .map_err(|_| CanonicalTerminalArtifactEnvelopeError::SectionTooLong(section))
}

pub(crate) mod execution_code {

    use crate::{
        CanonicalTerminalArtifact, CanonicalTerminalArtifactError, ClosedTerminalModule,
        TerminalArtifactView,
    };
    use std::sync::Arc;
    use terminal_psi::{TerminalModule, TerminalPsiIdentity};

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
    pub enum TerminalPublication {
        #[default]
        Portable,
        Invocation,
    }

    #[derive(Debug, Clone, Copy)]
    pub struct TerminalExecutionView<'code> {
        closed: &'code ClosedTerminalModule,
        identity: TerminalPsiIdentity,
    }

    impl<'code> TerminalExecutionView<'code> {
        pub fn custody_identity(self) -> TerminalExecutionIdentity {
            TerminalExecutionIdentity {
                semantic: self.identity,
                invocation: self.closed.module().identities.invocation_scope().cloned(),
            }
        }
        pub fn module(self) -> &'code TerminalModule {
            self.closed.module()
        }
        pub const fn closed_module(self) -> &'code ClosedTerminalModule {
            self.closed
        }
        pub const fn identity(self) -> TerminalPsiIdentity {
            self.identity
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct TerminalExecutionIdentity {
        semantic: TerminalPsiIdentity,
        invocation: Option<terminal_psi::InvocationCodeScope>,
    }

    impl TerminalExecutionIdentity {
        pub const fn portable(semantic: TerminalPsiIdentity) -> Self {
            Self {
                semantic,
                invocation: None,
            }
        }

        pub const fn semantic(&self) -> TerminalPsiIdentity {
            self.semantic
        }
    }

    impl<'code> From<TerminalArtifactView<'code>> for TerminalExecutionView<'code> {
        fn from(artifact: TerminalArtifactView<'code>) -> Self {
            Self {
                closed: artifact.closed_module(),
                identity: artifact.identity(),
            }
        }
    }

    #[derive(Clone)]
    pub struct RetainedTerminalCode {
        module: Arc<ClosedTerminalModule>,
        identity: TerminalPsiIdentity,
    }

    impl RetainedTerminalCode {
        pub(crate) fn new(
            module: Arc<ClosedTerminalModule>,
            identity: TerminalPsiIdentity,
        ) -> Self {
            Self { module, identity }
        }

        pub fn view(&self) -> TerminalExecutionView<'_> {
            TerminalExecutionView {
                closed: &self.module,
                identity: self.identity,
            }
        }
    }

    #[derive(Debug, PartialEq, Eq)]
    pub struct InvocationTerminalCode {
        module: Arc<ClosedTerminalModule>,
        identity: TerminalPsiIdentity,
    }

    impl InvocationTerminalCode {
        pub fn new(module: TerminalModule) -> Result<Self, crate::CodecError> {
            if module.identities.invocation_scope().is_none() {
                return Err(crate::CodecError::MalformedStructuralFoundation(
                    "invocation code requires a retained local-code owner",
                ));
            }
            Self::from_closed_module(ClosedTerminalModule::new(module)?)
        }

        pub fn from_closed_module(module: ClosedTerminalModule) -> Result<Self, crate::CodecError> {
            if module.module().identities.invocation_scope().is_none() {
                return Err(crate::CodecError::MalformedStructuralFoundation(
                    "invocation code requires a retained local-code owner",
                ));
            }
            let identity = module.identity();
            Ok(Self {
                module: Arc::new(module),
                identity,
            })
        }

        pub fn view(&self) -> TerminalExecutionView<'_> {
            TerminalExecutionView {
                closed: &self.module,
                identity: self.identity,
            }
        }

        pub fn retain_code(&self) -> RetainedTerminalCode {
            RetainedTerminalCode::new(Arc::clone(&self.module), self.identity)
        }
    }

    #[derive(Debug, PartialEq, Eq)]
    pub enum TerminalProgramProduct {
        Portable(CanonicalTerminalArtifact),
        Invocation(InvocationTerminalCode),
    }

    impl TerminalProgramProduct {
        pub fn retain_code(&self) -> RetainedTerminalCode {
            match self {
                Self::Portable(artifact) => artifact.retain_code(),
                Self::Invocation(code) => code.retain_code(),
            }
        }

        pub fn view(&self) -> TerminalExecutionView<'_> {
            match self {
                Self::Portable(artifact) => artifact.view().into(),
                Self::Invocation(code) => code.view(),
            }
        }

        pub fn portable(
            &self,
        ) -> Result<&CanonicalTerminalArtifact, CanonicalTerminalArtifactError> {
            match self {
                Self::Portable(artifact) => Ok(artifact),
                Self::Invocation(_) => Err(private_publication_error()),
            }
        }

        pub fn into_portable(
            self,
        ) -> Result<CanonicalTerminalArtifact, CanonicalTerminalArtifactError> {
            match self {
                Self::Portable(artifact) => Ok(artifact),
                Self::Invocation(_) => Err(private_publication_error()),
            }
        }
    }

    fn private_publication_error() -> CanonicalTerminalArtifactError {
        CanonicalTerminalArtifactError::Semantic(crate::CodecError::MalformedStructuralFoundation(
            "invocation code is not a portable Terminal artifact",
        ))
    }
}

/*
Canonical artifacts retain portable code, optimization records and debug data.
Execution views share that code while keeping each activation's private custody
outside the portable bytes. Invocation-local declarations retain their owner too.
*/

use crate::cursor::Cursor;
use terminal_psi::TerminalModule;

use crate::{
    ArtifactManifestError, CodecError, DebugMapError, PsiOptimizationExecutionRecord,
    PsiOptimizationExecutionRecordDecodeError, TerminalArtifactManifest, TerminalDebugMap,
    decode_debug_map, decode_module, decode_psi_optimization_execution_record, encode_debug_map,
    encode_module, encode_psi_optimization_execution_record,
};

const ARTIFACT_MAGIC: &[u8; 8] = b"PSIART\0\0";
const ARTIFACT_FORMAT_MARKER: u16 = 3;

#[derive(Debug, Clone, Copy)]
pub struct TerminalArtifactView<'artifact> {
    module: &'artifact TerminalModule,
    identity: terminal_psi::TerminalPsiIdentity,
}

impl<'artifact> TerminalArtifactView<'artifact> {
    pub const fn module(self) -> &'artifact TerminalModule {
        self.module
    }

    pub const fn identity(self) -> terminal_psi::TerminalPsiIdentity {
        self.identity
    }
}

#[derive(Clone)]
#[must_use = "a canonical Terminal artifact owns the Psi-to-Omega handoff bytes"]
pub struct CanonicalTerminalArtifact {
    semantic_bytes: Vec<u8>,
    optimization_bytes: Vec<u8>,
    optimization: PsiOptimizationExecutionRecord,
    debug_bytes: Option<Vec<u8>>,
    manifest: TerminalArtifactManifest,
    semantic_module: std::sync::Arc<TerminalModule>,
}

impl PartialEq for CanonicalTerminalArtifact {
    fn eq(&self, other: &Self) -> bool {
        self.manifest == other.manifest
            && self.semantic_bytes == other.semantic_bytes
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
            .field("semantic_byte_count", &self.semantic_bytes.len())
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
        let semantic_bytes =
            encode_module(&semantic_module).map_err(CanonicalTerminalArtifactError::Semantic)?;
        let optimization_bytes = encode_psi_optimization_execution_record(&optimization);
        let debug_bytes = debug_map
            .map(|debug_map| {
                encode_debug_map(&semantic_module, debug_map)
                    .map_err(CanonicalTerminalArtifactError::Debug)
            })
            .transpose()?;
        let semantic = terminal_psi::TerminalPsiIdentity {
            vocabulary_marker: semantic_module.vocabulary_marker,
            program_fingerprint: crate::sections::semantic_module::encoded_fingerprint(
                &semantic_module,
                &semantic_bytes,
            ),
        };
        let manifest = crate::sections::artifact_manifest::manifest_for_semantic(
            semantic,
            &optimization,
            None,
            debug_bytes.as_deref(),
        )
        .map_err(CanonicalTerminalArtifactError::Manifest)?;
        Ok(Self {
            semantic_bytes,
            optimization_bytes,
            optimization,
            debug_bytes,
            manifest,
            semantic_module: std::sync::Arc::new(semantic_module),
        })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let debug_header_bytes = if self.debug_bytes.is_some() { 9 } else { 1 };
        let capacity = ARTIFACT_MAGIC
            .len()
            .saturating_add(2)
            .saturating_add(16)
            .saturating_add(debug_header_bytes)
            .saturating_add(self.semantic_bytes.len())
            .saturating_add(self.optimization_bytes.len())
            .saturating_add(self.debug_bytes.as_ref().map_or(0, Vec::len));
        let mut bytes = Vec::with_capacity(capacity);
        bytes.extend_from_slice(ARTIFACT_MAGIC);
        crate::codec_bytes::push_encoded(&mut bytes, ARTIFACT_FORMAT_MARKER.to_le_bytes());
        encode_section_len(&mut bytes, self.semantic_bytes.len());
        encode_section_len(&mut bytes, self.optimization_bytes.len());
        match &self.debug_bytes {
            None => bytes.push(0),
            Some(debug) => {
                bytes.push(1);
                encode_section_len(&mut bytes, debug.len());
            }
        }
        bytes.extend_from_slice(&self.semantic_bytes);
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

        let semantic_module =
            decode_module(semantic_bytes).map_err(CanonicalTerminalArtifactError::Semantic)?;
        let optimization = decode_psi_optimization_execution_record(optimization_bytes)
            .map_err(CanonicalTerminalArtifactError::Optimization)?;
        let debug_map = debug_bytes
            .map(|debug| {
                decode_debug_map(&semantic_module, debug)
                    .map_err(CanonicalTerminalArtifactError::Debug)
            })
            .transpose()?;
        let artifact = Self::from_parts(semantic_module, optimization, debug_map.as_ref())?;
        if artifact.semantic_bytes() != semantic_bytes
            || artifact.optimization_bytes() != optimization_bytes
            || artifact.debug_bytes() != debug_bytes
        {
            return Err(CanonicalTerminalArtifactEnvelopeError::NonCanonicalSections.into());
        }
        Ok(artifact)
    }

    pub fn semantic_bytes(&self) -> &[u8] {
        &self.semantic_bytes
    }

    pub fn view(&self) -> TerminalArtifactView<'_> {
        TerminalArtifactView {
            module: &self.semantic_module,
            identity: self.manifest.semantic(),
        }
    }

    pub fn into_semantics(self) -> (TerminalModule, terminal_psi::TerminalPsiIdentity) {
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

    use crate::{CanonicalTerminalArtifact, CanonicalTerminalArtifactError, TerminalArtifactView};
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
        module: &'code TerminalModule,
        identity: TerminalPsiIdentity,
        invocation: Option<&'code terminal_psi::InvocationCodeScope>,
    }

    impl<'code> TerminalExecutionView<'code> {
        pub fn custody_identity(self) -> TerminalExecutionIdentity {
            TerminalExecutionIdentity {
                semantic: self.identity,
                invocation: self.invocation.cloned(),
            }
        }
        pub const fn module(self) -> &'code TerminalModule {
            self.module
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
                module: artifact.module(),
                identity: artifact.identity(),
                invocation: artifact.module().identities.invocation_scope(),
            }
        }
    }

    #[derive(Clone)]
    pub struct RetainedTerminalCode {
        module: Arc<TerminalModule>,
        identity: TerminalPsiIdentity,
        invocation: Option<terminal_psi::InvocationCodeScope>,
    }

    impl RetainedTerminalCode {
        pub(crate) fn new(module: Arc<TerminalModule>, identity: TerminalPsiIdentity) -> Self {
            let invocation = module.identities.invocation_scope().cloned();
            Self {
                module,
                identity,
                invocation,
            }
        }

        pub fn view(&self) -> TerminalExecutionView<'_> {
            TerminalExecutionView {
                module: &self.module,
                identity: self.identity,
                invocation: self.invocation.as_ref(),
            }
        }
    }

    #[derive(Debug, PartialEq, Eq)]
    pub struct InvocationTerminalCode {
        module: Arc<TerminalModule>,
        identity: TerminalPsiIdentity,
        invocation: terminal_psi::InvocationCodeScope,
    }

    impl InvocationTerminalCode {
        pub fn new(module: TerminalModule) -> Result<Self, crate::CodecError> {
            let invocation = module.identities.invocation_scope().cloned().ok_or(
                crate::CodecError::MalformedStructuralFoundation(
                    "invocation code requires a retained local-code owner",
                ),
            )?;
            let identity = crate::terminal_psi_identity(&module)?;
            Ok(Self {
                module: Arc::new(module),
                identity,
                invocation,
            })
        }

        /// Give this activation its own custody while sharing the checked code.
        /// The module and its proof identities stay unchanged, so code retained
        /// across suspension keeps both its evidence and this activation owner.
        pub fn from_artifact(artifact: &CanonicalTerminalArtifact) -> Self {
            Self {
                module: Arc::clone(&artifact.semantic_module),
                identity: artifact.manifest.semantic(),
                invocation: terminal_psi::InvocationCodeScope::default(),
            }
        }

        pub fn view(&self) -> TerminalExecutionView<'_> {
            TerminalExecutionView {
                module: &self.module,
                identity: self.identity,
                invocation: Some(&self.invocation),
            }
        }

        pub fn retain_code(&self) -> RetainedTerminalCode {
            RetainedTerminalCode {
                module: Arc::clone(&self.module),
                identity: self.identity,
                invocation: Some(self.invocation.clone()),
            }
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

    #[cfg(test)]
    mod invocation_tests {
        use super::*;
        use crate::sections::semantic_module::target_family_tests::{identity_table, unit_machine};
        use semantic_vocabulary::MachineId;

        fn module() -> TerminalModule {
            let entry = MachineId::new(1).unwrap();
            let mut module = TerminalModule::for_entry(entry);
            module.identities = identity_table(2);
            module.machines.push(unit_machine(entry));
            module
        }

        #[test]
        fn separate_activations_share_checked_code_and_keep_distinct_custody() {
            let module = module();
            let record = crate::build_identity_optimization_execution_record(&module).unwrap();
            let artifact = CanonicalTerminalArtifact::from_parts(module, record, None).unwrap();
            let original_bytes = artifact.to_bytes();
            let first = InvocationTerminalCode::from_artifact(&artifact);
            let second = InvocationTerminalCode::from_artifact(&artifact);

            assert!(std::ptr::eq(
                first.view().module(),
                artifact.semantic_module()
            ));
            assert_eq!(first.view().identity(), artifact.view().identity());
            assert_eq!(second.view().identity(), artifact.view().identity());
            assert_eq!(artifact.to_bytes(), original_bytes);
            assert!(
                artifact
                    .semantic_module()
                    .identities
                    .invocation_scope()
                    .is_none()
            );
            assert_ne!(
                first.view().custody_identity(),
                second.view().custody_identity()
            );

            let retained = first.retain_code();
            let identity = first.view().custody_identity();
            drop(first);
            assert_eq!(retained.clone().view().custody_identity(), identity);
            assert_ne!(
                retained.view().custody_identity(),
                second.view().custody_identity()
            );
            assert!(
                TerminalProgramProduct::Invocation(second)
                    .portable()
                    .is_err()
            );
        }

        #[test]
        fn existing_local_code_keeps_its_owner_and_requires_that_owner() {
            assert!(InvocationTerminalCode::new(module()).is_err());
            let mut module = module();
            let mut identities = terminal_psi::TerminalIdentityTableBuilder::for_invocation();
            for text in module.identities.entries().iter().skip(1) {
                identities.intern(text);
            }
            identities
                .intern_invocation_local("local::continuation")
                .unwrap();
            module.identities = identities.build();
            let original_scope = module.identities.invocation_scope().unwrap().clone();
            let code = InvocationTerminalCode::new(module).unwrap();
            assert_eq!(
                code.view().custody_identity().invocation,
                Some(original_scope)
            );
            assert_eq!(
                code.retain_code().view().custody_identity(),
                code.view().custody_identity()
            );
        }
    }
}

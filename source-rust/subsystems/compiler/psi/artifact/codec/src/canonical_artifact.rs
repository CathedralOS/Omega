/*
Whole-artifact encoding and retained execution-code ownership. The immutable closed
module owns semantic identity; borrowing or retaining it never creates a second
module/identity pair that a later consumer has to reconcile.
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
const PUBLISHED_ARTIFACT_FORMAT_MARKER: u16 = 4;

/// Facts needed to reopen a published program after the producing process exits.
#[derive(Debug, Clone)]
struct PublicationSection {
    metadata: std::sync::Arc<psi_publication_records::PublishedEntryMetadata>,
    evidence: Option<std::sync::Arc<crate::EvidenceSeals>>,
    bytes: std::sync::Arc<[u8]>,
}

#[derive(Debug, Clone, Copy)]
pub struct TerminalArtifactView<'artifact> {
    closed: &'artifact ClosedTerminalModule,
}

impl<'artifact> TerminalArtifactView<'artifact> {
    pub fn module(self) -> &'artifact TerminalModule {
        self.closed.module()
    }

    pub const fn closed_module(self) -> &'artifact ClosedTerminalModule {
        self.closed
    }

    pub const fn identity(self) -> terminal_psi::TerminalPsiIdentity {
        self.closed.identity()
    }
}

#[derive(Clone)]
#[must_use = "a canonical Terminal artifact owns the Psi-to-Omega handoff bytes"]
pub struct CanonicalTerminalArtifact {
    optimization_bytes: Vec<u8>,
    optimization: PsiOptimizationExecutionRecord,
    debug_bytes: Option<Vec<u8>>,
    manifest: TerminalArtifactManifest,
    publication: Option<PublicationSection>,
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
        let manifest = terminal_wire::sealing::manifest_for_semantic(
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
            publication: None,
            semantic_module: std::sync::Arc::new(closed),
        })
    }

    pub fn with_publication(
        mut self,
        metadata: std::sync::Arc<psi_publication_records::PublishedEntryMetadata>,
        evidence: Option<std::sync::Arc<crate::EvidenceSeals>>,
    ) -> Result<Self, CanonicalTerminalArtifactError> {
        if self.publication.is_some()
            || !metadata.validate_declarations(self.semantic_module())
            || evidence
                .as_ref()
                .is_some_and(|evidence| evidence.module != self.view().identity())
        {
            return Err(CanonicalTerminalArtifactEnvelopeError::InvalidPublication.into());
        }
        let metadata_bytes = record_storage::encode(metadata.as_ref(), Default::default())
            .map_err(|error| CanonicalTerminalArtifactError::Publication(error.to_string()))?;
        let evidence_bytes = evidence
            .as_deref()
            .map(crate::encode_operation_evidence)
            .transpose()
            .map_err(CanonicalTerminalArtifactError::Semantic)?;
        let mut bytes = Vec::new();
        encode_section_len(&mut bytes, metadata_bytes.len());
        bytes.extend_from_slice(&metadata_bytes);
        if let Some(evidence) = evidence_bytes {
            bytes.extend_from_slice(&evidence);
        }
        self.manifest = self.manifest.with_publication(&bytes);
        self.publication = Some(PublicationSection {
            metadata,
            evidence,
            bytes: bytes.into(),
        });
        Ok(self)
    }

    pub fn publication_metadata(
        &self,
    ) -> Option<&std::sync::Arc<psi_publication_records::PublishedEntryMetadata>> {
        self.publication.as_ref().map(|section| &section.metadata)
    }

    pub fn calling_declaration_owner(
        &self,
        subject: crate::TerminalExecutionView<'_>,
        declaration: terminal_psi::TerminalIdentityIndex,
    ) -> Result<semantic_vocabulary::compiler_evaluation::DeclarationCustody, String> {
        if !std::ptr::eq(subject.module(), self.semantic_module()) {
            return Err(
                "source declaration custody belongs to another program or invocation".into(),
            );
        }
        self.publication_metadata()
            .ok_or_else(|| {
                "saved Psi lacks calling declaration owners; recompile it to publish them"
                    .to_owned()
            })?
            .calling_schema_custody(declaration)
    }

    pub fn published_evidence(&self) -> Option<&std::sync::Arc<crate::EvidenceSeals>> {
        self.publication.as_ref()?.evidence.as_ref()
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let debug_header_bytes = if self.debug_bytes.is_some() { 9 } else { 1 };
        let capacity = ARTIFACT_MAGIC
            .len()
            .saturating_add(2)
            .saturating_add(16)
            .saturating_add(debug_header_bytes)
            .saturating_add(
                self.publication
                    .as_ref()
                    .map_or(0, |section| 8 + section.bytes.len()),
            )
            .saturating_add(self.semantic_bytes().len())
            .saturating_add(self.optimization_bytes.len())
            .saturating_add(self.debug_bytes.as_ref().map_or(0, Vec::len));
        let mut bytes = Vec::with_capacity(capacity);
        bytes.extend_from_slice(ARTIFACT_MAGIC);
        let marker = if self.publication.is_some() {
            PUBLISHED_ARTIFACT_FORMAT_MARKER
        } else {
            ARTIFACT_FORMAT_MARKER
        };
        crate::codec_bytes::push_encoded(&mut bytes, marker.to_le_bytes());
        encode_section_len(&mut bytes, self.semantic_bytes().len());
        encode_section_len(&mut bytes, self.optimization_bytes.len());
        match &self.debug_bytes {
            None => bytes.push(0),
            Some(debug) => {
                bytes.push(1);
                encode_section_len(&mut bytes, debug.len());
            }
        }
        if let Some(section) = &self.publication {
            encode_section_len(&mut bytes, section.bytes.len());
        }
        bytes.extend_from_slice(self.semantic_bytes());
        bytes.extend_from_slice(&self.optimization_bytes);
        if let Some(debug) = &self.debug_bytes {
            bytes.extend_from_slice(debug);
        }
        if let Some(section) = &self.publication {
            bytes.extend_from_slice(&section.bytes);
        }
        bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, CanonicalTerminalArtifactError> {
        Self::from_bytes_within(bytes, Default::default())
    }

    /// Decodes a received envelope under the receiving host's load limits.
    /// The size limit applies to the whole envelope before anything is read,
    /// and one work account covers every section, their canonical
    /// re-encoding and, at authorization, execution preparation.
    pub fn from_bytes_within(
        bytes: &[u8],
        limits: terminal_wire::PsiLoadLimits,
    ) -> Result<Self, CanonicalTerminalArtifactError> {
        let mut account = terminal_wire::LoadAccount::open(limits, bytes.len())
            .map_err(CanonicalTerminalArtifactError::Semantic)?;
        let mut cursor = Cursor::new(bytes);
        if cursor.take(ARTIFACT_MAGIC.len())? != ARTIFACT_MAGIC {
            return Err(CanonicalTerminalArtifactEnvelopeError::InvalidMagic.into());
        }
        let marker = cursor.u16()?;
        if ![ARTIFACT_FORMAT_MARKER, PUBLISHED_ARTIFACT_FORMAT_MARKER].contains(&marker) {
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
        let publication_len = (marker == PUBLISHED_ARTIFACT_FORMAT_MARKER)
            .then(|| section_len(&mut cursor, "publication"))
            .transpose()?;
        let semantic_bytes = cursor.take(semantic_len)?;
        let optimization_bytes = cursor.take(optimization_len)?;
        let debug_bytes = debug_len.map(|len| cursor.take(len)).transpose()?;
        let publication = publication_len.map(|len| cursor.take(len)).transpose()?;
        if cursor.remaining() != 0 {
            return Err(
                CanonicalTerminalArtifactEnvelopeError::TrailingBytes(cursor.remaining()).into(),
            );
        }

        // The companion sections are parsed once and re-encoded once to prove
        // their canonical form; pay for both before reading them.
        let companion_bytes = optimization_bytes.len()
            + debug_bytes.map_or(0, <[u8]>::len)
            + publication.map_or(0, <[u8]>::len);
        account
            .charge(
                "parse and re-encode artifact sections",
                u64::try_from(companion_bytes)
                    .unwrap_or(u64::MAX)
                    .saturating_mul(2),
            )
            .map_err(CanonicalTerminalArtifactError::Semantic)?;
        let closed = ClosedTerminalModule::decode_within(semantic_bytes, &mut account)
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
        let mut artifact = Self::from_closed_parts(closed, optimization, debug_map.as_ref())?;
        if let Some(section) = publication {
            let mut fields = Cursor::new(section);
            let metadata_len = section_len(&mut fields, "publication declarations")?;
            let metadata = record_storage::decode(fields.take(metadata_len)?, Default::default())
                .map_err(|error| CanonicalTerminalArtifactError::Publication(error.to_string()))?;
            let evidence = if fields.remaining() == 0 {
                None
            } else {
                let evidence = crate::decode_operation_evidence(fields.take(fields.remaining())?)
                    .map_err(CanonicalTerminalArtifactError::Semantic)?;
                crate::EvidenceSealValidator::new(artifact.semantic_module(), &evidence.seals)
                    .validate_all()
                    .map_err(|error| {
                        CanonicalTerminalArtifactError::Publication(error.to_string())
                    })?;
                Some(std::sync::Arc::new(evidence))
            };
            artifact = artifact.with_publication(std::sync::Arc::new(metadata), evidence)?;
            if artifact
                .publication
                .as_ref()
                .is_none_or(|stored| stored.bytes.as_ref() != section)
            {
                return Err(CanonicalTerminalArtifactEnvelopeError::NonCanonicalSections.into());
            }
        }
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

    /// Whether these bytes were received rather than produced in this
    /// process; see [`ClosedTerminalModule::requires_host_trust`].
    pub fn requires_host_trust(&self) -> bool {
        self.semantic_module.requires_host_trust()
    }

    /// Records the receiving host's acceptance of this exact artifact.
    ///
    /// This is the authorization crate's hook, not an embedding API: call
    /// `psi_authorization::authorize_terminal_artifact` instead, which
    /// requires the host's producer trust. A repository test refuses any
    /// other caller. The returned artifact has the same bytes and identity;
    /// the received original stays non-executable. A received artifact also
    /// passes its execution-only checks here, and fails if the lowered-form
    /// checker would have refused it.
    #[doc(hidden)]
    pub fn record_host_authorization(
        self: &std::sync::Arc<Self>,
    ) -> Result<std::sync::Arc<Self>, CodecError> {
        if !self.requires_host_trust() {
            return Ok(std::sync::Arc::clone(self));
        }
        Ok(std::sync::Arc::new(Self {
            semantic_module: std::sync::Arc::new(self.semantic_module.host_admitted()?),
            ..Self::clone(self)
        }))
    }

    /// Whether the host admitted this artifact only for contained
    /// interpretation, without producer trust.
    pub fn admitted_for_contained_interpretation(&self) -> bool {
        self.semantic_module.admitted_for_contained_interpretation()
    }

    /// Records the receiving host's admission of this exact artifact for
    /// contained interpretation, without producer trust.
    ///
    /// This is the authorization crate's hook: call
    /// `psi_authorization::admit_contained_interpretation` instead. A
    /// repository test refuses any other caller. The artifact passes the same
    /// execution-only checks as an authorized one and still requires host
    /// trust for every trusted route.
    #[doc(hidden)]
    pub fn record_contained_admission(
        self: &std::sync::Arc<Self>,
    ) -> Result<std::sync::Arc<Self>, CodecError> {
        if self.admitted_for_contained_interpretation() {
            return Ok(std::sync::Arc::clone(self));
        }
        Ok(std::sync::Arc::new(Self {
            semantic_module: std::sync::Arc::new(self.semantic_module.contained_admitted()?),
            ..Self::clone(self)
        }))
    }

    pub fn view(&self) -> TerminalArtifactView<'_> {
        TerminalArtifactView {
            closed: &self.semantic_module,
        }
    }

    pub fn into_semantics(self) -> ClosedTerminalModule {
        std::sync::Arc::unwrap_or_clone(self.semantic_module)
    }

    pub fn retain_code(&self) -> crate::RetainedTerminalCode {
        crate::RetainedTerminalCode::new(std::sync::Arc::clone(&self.semantic_module))
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
    Publication(String),
    Envelope(CanonicalTerminalArtifactEnvelopeError),
}

::wire::truncated_from_adapter!(CanonicalTerminalArtifactError => Self::Envelope(CanonicalTerminalArtifactEnvelopeError::UnexpectedEnd));

impl From<CanonicalTerminalArtifactEnvelopeError> for CanonicalTerminalArtifactError {
    fn from(error: CanonicalTerminalArtifactEnvelopeError) -> Self {
        Self::Envelope(error)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CanonicalTerminalArtifactEnvelopeError {
    InvalidMagic,
    InvalidPublication,
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

impl CanonicalTerminalArtifactError {
    /// Whether the receiving host's input size or work limit refused the
    /// artifact, as opposed to malformed input.
    pub const fn is_resource_refusal(&self) -> bool {
        matches!(self, Self::Semantic(error) if error.is_resource_refusal())
    }
}

::wire::truncated_from_adapter!(CanonicalTerminalArtifactEnvelopeError => Self::UnexpectedEnd);

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
        /// A selected fixed-layout policy may record private callback placements.
        LayoutPolicyInvocation,
    }

    #[derive(Debug, Clone, Copy)]
    pub struct TerminalExecutionView<'code>(&'code ClosedTerminalModule);

    impl<'code> TerminalExecutionView<'code> {
        pub fn custody_identity(self) -> TerminalExecutionIdentity {
            TerminalExecutionIdentity {
                semantic: self.identity(),
                invocation: self.module().identities.invocation_scope().cloned(),
            }
        }
        pub fn module(self) -> &'code TerminalModule {
            self.0.module()
        }
        pub const fn closed_module(self) -> &'code ClosedTerminalModule {
            self.0
        }
        pub const fn closed_types(self) -> &'code crate::ClosedTypeCatalog {
            self.0.closed_types()
        }
        pub const fn identity(self) -> TerminalPsiIdentity {
            self.0.identity()
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
            Self(artifact.closed_module())
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct RetainedTerminalCode(Arc<ClosedTerminalModule>);

    impl RetainedTerminalCode {
        pub(crate) fn new(module: Arc<ClosedTerminalModule>) -> Self {
            Self(module)
        }
        pub fn view(&self) -> TerminalExecutionView<'_> {
            TerminalExecutionView(&self.0)
        }
    }

    #[derive(Debug, PartialEq, Eq)]
    pub struct InvocationTerminalCode(RetainedTerminalCode);

    impl InvocationTerminalCode {
        pub fn new(module: TerminalModule) -> Result<Self, crate::CodecError> {
            Self::from_closed_module(ClosedTerminalModule::new(module)?)
        }

        pub fn from_closed_module(module: ClosedTerminalModule) -> Result<Self, crate::CodecError> {
            if module.module().identities.invocation_scope().is_none() {
                return Err(crate::CodecError::MalformedStructuralFoundation(
                    "invocation code requires a retained local-code owner",
                ));
            }
            Ok(Self(RetainedTerminalCode::new(Arc::new(module))))
        }

        pub fn view(&self) -> TerminalExecutionView<'_> {
            self.0.view()
        }

        pub fn retain_code(&self) -> RetainedTerminalCode {
            self.0.clone()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{InvocationTerminalCode, TerminalExecutionView};
    use terminal_psi::TerminalIdentityTableBuilder;

    fn closed_module(invocation: bool) -> ClosedTerminalModule {
        let mut module = TerminalModule::declarations();
        if invocation {
            let mut identities = TerminalIdentityTableBuilder::for_invocation();
            identities
                .intern_invocation_local("local_callable")
                .unwrap();
            module.identities = identities.build();
        }
        ClosedTerminalModule::new(module).unwrap()
    }

    fn optimization_record(module: &ClosedTerminalModule) -> PsiOptimizationExecutionRecord {
        PsiOptimizationExecutionRecord::new(
            optimization::PsiOptimizationSelections::default(),
            module.identity(),
            module.identity(),
        )
        .unwrap()
    }

    #[test]
    fn application_checks_view_identity_and_retains_portability_errors_on_the_receipt() {
        use crate::entry_receipt::application::{
            TerminalApplicationCustody, TerminalCompositionRoots,
        };
        use psi_publication_records::TerminalMachineProvenance;
        use terminal_psi::PreparedNominalRoot;

        let closed = closed_module(false);
        let record = optimization_record(&closed);
        let artifact = CanonicalTerminalArtifact::from_closed_parts(closed, record, None).unwrap();
        let invocation = InvocationTerminalCode::from_closed_module(closed_module(true)).unwrap();
        let provenance = TerminalMachineProvenance::default();
        let selected = TerminalApplicationCustody::prepare(
            Ok(PreparedNominalRoot::new("Root", "Root", None).unwrap()),
            Some([7; 32]),
            Some(TerminalCompositionRoots::default()),
        );

        let mismatch = selected.attach(invocation.view(), Ok(&artifact), &provenance);
        assert!(mismatch.is_err());
        let local = selected
            .attach(
                invocation.view(),
                Err(CanonicalTerminalArtifactError::Publication(
                    "invocation only".into(),
                )),
                &provenance,
            )
            .unwrap();
        assert_eq!(local.roots.len(), 1);
        let error = local.entry_receipt.unwrap().check().unwrap_err();
        assert!(error[0].message.contains("invocation only"));

        let empty = TerminalApplicationCustody::prepare(
            Err(vec![diagnostics::Diagnostic::error("unused entry")]),
            None,
            None,
        );
        let attached = empty
            .attach(artifact.view().into(), Ok(&artifact), &provenance)
            .unwrap();
        assert!(attached.roots.is_empty());
        assert!(attached.entry_receipt.is_none());
    }

    // The pre-namespace specimen was saved under semantic format marker 289.
    // Format 290 (#5035) retired its sealed byte reader, and the encoding
    // rejects older markers before reading their payload
    // (source-rust/subsystems/compiler/psi/artifact/wire/encoding.md#machine-rows).
    #[test]
    fn pre_namespace_linux_program_is_refused_by_its_retired_format_marker() {
        let bytes = include_bytes!(
            "../../../../../../../tests/fixtures/calls/run/scalar_contract_route_exit/linux_x86_64.pre_namespace.psi"
        );
        assert!(matches!(
            CanonicalTerminalArtifact::from_bytes(bytes),
            Err(CanonicalTerminalArtifactError::Semantic(
                CodecError::UnsupportedFormatMarker(289)
            ))
        ));
    }

    #[test]
    fn saved_linux_program_reencodes_from_its_decoded_module_and_publication() {
        use ::wire::digest_encoder::{content_identity, hex};

        let bytes = include_bytes!(
            "../../../../../../../tests/fixtures/calls/run/scalar_contract_route_exit/linux_x86_64.psi"
        );
        let received = CanonicalTerminalArtifact::from_bytes(bytes).unwrap();
        // A fresh seal encodes the decoded fields without retaining the received semantic bytes.
        let closed = ClosedTerminalModule::new(received.semantic_module().clone()).unwrap();
        assert!(closed.semantic_bytes() == received.semantic_bytes());
        assert_eq!(
            hex(&content_identity(closed.semantic_bytes())),
            "181b50378d1b1957c307cf60ece66b8ac8f4e271f9bf69916e20c7b00182bab9"
        );
        assert_eq!(
            closed.identity().program_fingerprint.to_string(),
            "9b392febbfd2c2ed66cffe72649f7d44e5cbc381be71a3824ab7fa163a748b23"
        );
        let debug = received
            .debug_bytes()
            .map(|bytes| decode_debug_map(closed.module(), bytes))
            .transpose()
            .unwrap();
        let rebuilt = CanonicalTerminalArtifact::from_closed_parts(
            closed,
            received.optimization().clone(),
            debug.as_ref(),
        )
        .unwrap()
        .with_publication(
            received.publication_metadata().unwrap().clone(),
            received.published_evidence().cloned(),
        )
        .unwrap();
        assert_eq!(
            rebuilt.manifest().identity().to_string(),
            "ae41dd2b2f34176539a445d830683e341f6a11715e679d83f380ff16ea6cad9b"
        );
        let encoded = rebuilt.to_bytes();
        assert_eq!(
            hex(&content_identity(&encoded)),
            "66875677277885c569c5d31e0de570127fc2d4358909d37c10618cdddc49d80a"
        );
        assert!(encoded.as_slice() == bytes);
    }

    /// A real emitted program loads under host limits that admit it. A
    /// smaller input cap or work budget refuses it as a resource refusal,
    /// before reading it or before the phase that would overrun, and both
    /// differ from malformed input.
    #[test]
    fn host_load_limits_admit_or_refuse_a_saved_program_by_resource() {
        let bytes = include_bytes!(
            "../../../../../../../tests/fixtures/calls/run/scalar_contract_route_exit/linux_x86_64.psi"
        );
        let size = bytes.len() as u64;
        let admitted = CanonicalTerminalArtifact::from_bytes_within(
            bytes,
            terminal_wire::PsiLoadLimits {
                max_input_bytes: Some(size),
                ..Default::default()
            },
        )
        .unwrap();
        let used = admitted.closed_module().load_account().unwrap().used();
        // Parsing alone pays for every byte, so the account saw at least the input.
        assert!(used >= size);

        let capped = CanonicalTerminalArtifact::from_bytes_within(
            bytes,
            terminal_wire::PsiLoadLimits {
                max_input_bytes: Some(size - 1),
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(capped.is_resource_refusal());
        assert!(matches!(
            capped,
            CanonicalTerminalArtifactError::Semantic(CodecError::InputTooLarge { limit, actual })
                if limit == size - 1 && actual == size
        ));

        let exhausted = CanonicalTerminalArtifact::from_bytes_within(
            bytes,
            terminal_wire::PsiLoadLimits {
                max_input_bytes: None,
                work_units: used - 1,
            },
        )
        .unwrap_err();
        assert!(exhausted.is_resource_refusal());
        assert!(matches!(
            exhausted,
            CanonicalTerminalArtifactError::Semantic(CodecError::LoadWorkExhausted { limit, .. })
                if limit == used - 1
        ));
        let reloaded = CanonicalTerminalArtifact::from_bytes_within(
            bytes,
            terminal_wire::PsiLoadLimits {
                max_input_bytes: None,
                work_units: used,
            },
        )
        .unwrap();
        assert!(reloaded.requires_host_trust());

        let truncated =
            CanonicalTerminalArtifact::from_bytes(&bytes[..bytes.len() - 1]).unwrap_err();
        assert!(!truncated.is_resource_refusal());
    }

    #[test]
    fn host_identity_commits_to_the_exact_publication_section() {
        let closed = closed_module(false);
        let record = optimization_record(&closed);
        let legacy = CanonicalTerminalArtifact::from_closed_parts(closed, record, None).unwrap();
        let base = legacy.manifest();
        let published = base.with_publication(b"declared owners");
        assert_eq!(published.with_publication(b"declared owners"), published);
        assert_ne!(published.identity(), base.identity());
        for (section, expected) in [
            (Some(&b"declared owners"[..]), true),
            (Some(&b"different owners"[..]), false),
            (None, false),
        ] {
            assert_eq!(
                crate::validate_artifact_manifest_with_publication(
                    legacy.semantic_module(),
                    legacy.optimization(),
                    None,
                    None,
                    section,
                    published,
                )
                .is_ok(),
                expected
            );
        }
        assert_eq!(
            CanonicalTerminalArtifact::from_bytes(&legacy.to_bytes())
                .unwrap()
                .manifest(),
            base
        );
    }

    #[test]
    fn every_portable_handoff_uses_the_closed_modules_identity_and_storage() {
        let closed = closed_module(false);
        let identity = closed.identity();
        let record = optimization_record(&closed);
        let artifact = CanonicalTerminalArtifact::from_closed_parts(closed, record, None).unwrap();
        let retained = artifact.retain_code();
        let execution: TerminalExecutionView<'_> = artifact.view().into();
        assert_eq!(artifact.view().identity(), identity);
        assert_eq!(artifact.manifest().semantic(), identity);
        assert_eq!(execution.identity(), identity);
        assert_eq!(retained.view().identity(), identity);
        assert!(std::ptr::eq(execution.module(), retained.view().module()));

        let decoded = CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes()).unwrap();
        assert_eq!(decoded.view().identity(), identity);
        assert_eq!(decoded.semantic_bytes(), artifact.semantic_bytes());

        let transferred = artifact.into_semantics();
        assert_eq!(transferred.identity(), identity);
        assert!(std::ptr::eq(transferred.module(), retained.view().module()));
        drop(transferred);
        assert_eq!(retained.view().identity(), identity);
        assert_eq!(retained.view().closed_module().identity(), identity);
    }

    #[test]
    fn a_manifest_cannot_substitute_another_modules_semantic_identity() {
        let record = optimization_record(&closed_module(false));
        let mut changed = TerminalModule::declarations();
        let mut identities = TerminalIdentityTableBuilder::default();
        identities.intern("different_program");
        changed.identities = identities.build();
        let changed = ClosedTerminalModule::new(changed).unwrap();
        assert!(matches!(
            CanonicalTerminalArtifact::from_closed_parts(changed, record, None),
            Err(CanonicalTerminalArtifactError::Manifest(
                ArtifactManifestError::Optimization(
                    crate::PsiOptimizationExecutionRecordError::OutputMismatch
                )
            ))
        ));
    }

    #[test]
    fn retained_invocation_code_preserves_its_owner_without_becoming_portable() {
        let first = InvocationTerminalCode::from_closed_module(closed_module(true)).unwrap();
        let second = InvocationTerminalCode::from_closed_module(closed_module(true)).unwrap();
        assert_eq!(first.view().identity(), second.view().identity());
        assert_ne!(
            first.view().custody_identity(),
            second.view().custody_identity()
        );

        let identity = first.view().custody_identity();
        let retained = first.retain_code();
        let clone = retained.clone();
        assert!(std::ptr::eq(
            first.view().module(),
            retained.view().module()
        ));
        drop(first);
        drop(retained);
        assert_eq!(clone.view().custody_identity(), identity);
        assert_eq!(
            clone.view().identity(),
            clone.view().closed_module().identity()
        );

        let closed = closed_module(true);
        let record = optimization_record(&closed);
        assert!(CanonicalTerminalArtifact::from_closed_parts(closed, record, None).is_err());
        assert!(InvocationTerminalCode::from_closed_module(closed_module(false)).is_err());
    }
}

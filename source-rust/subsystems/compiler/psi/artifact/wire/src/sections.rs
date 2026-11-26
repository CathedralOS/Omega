/*
Companion artifact section codecs for `TerminalArtifactManifest` (`artifact_manifest`),
`TerminalDebugMap` (`debug_map`), and `OptimizationExecutionReceipt`
(`optimization_execution`), plus re-export of `semantic_module`.
*/

pub(crate) use crate::semantic_module;

pub(crate) mod artifact_manifest {
    use terminal_psi::TerminalModule;

    use crate::{
        CodecError, PsiOptimizationExecutionIdentity, PsiOptimizationExecutionRecord,
        PsiOptimizationExecutionRecordError, TerminalPsiIdentity, terminal_psi_identity,
    };

    const MANIFEST_FORMAT_MARKER: u16 = 4;
    const INSTALLATION_DOMAIN: &[u8] = b"psi-terminal-installation-section\0";
    const DEBUG_DOMAIN: &[u8] = b"psi-terminal-debug-section\0";
    const ARTIFACT_DOMAIN: &[u8] = b"psi-terminal-artifact-manifest\0";

    macro_rules! hex_digest_type {
        ($($name:ident),* $(,)?) => {$(
            #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
            pub struct $name([u8; 32]);

            impl $name {
                pub const fn as_bytes(&self) -> &[u8; 32] {
                    &self.0
                }
            }

            impl std::fmt::Debug for $name {
                fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    std::fmt::Display::fmt(self, formatter)
                }
            }

            impl std::fmt::Display for $name {
                fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    write_hex(formatter, &self.0)
                }
            }
        )*};
    }
    hex_digest_type!(SectionFingerprint, TerminalArtifactIdentity);

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct TerminalArtifactManifest {
        semantic: TerminalPsiIdentity,
        optimization: PsiOptimizationExecutionIdentity,
        installation: Option<SectionFingerprint>,
        debug: Option<SectionFingerprint>,
        publication: Option<SectionFingerprint>,
        identity: TerminalArtifactIdentity,
    }

    impl TerminalArtifactManifest {
        pub const fn semantic(self) -> TerminalPsiIdentity {
            self.semantic
        }
        pub const fn optimization(self) -> PsiOptimizationExecutionIdentity {
            self.optimization
        }
        pub const fn installation(self) -> Option<SectionFingerprint> {
            self.installation
        }
        pub const fn debug(self) -> Option<SectionFingerprint> {
            self.debug
        }
        pub const fn publication(self) -> Option<SectionFingerprint> {
            self.publication
        }
        pub const fn identity(self) -> TerminalArtifactIdentity {
            self.identity
        }

        /// Publication facts participate in host authorization even when executable code is identical.
        pub fn with_publication(mut self, section: &[u8]) -> Self {
            let fingerprint = hash_section(b"psi-publication-section\0", section);
            let mut subject = [0u8; 64];
            let base = if self.publication.is_some() {
                artifact_identity(
                    self.semantic,
                    self.optimization,
                    self.installation,
                    self.debug,
                )
            } else {
                self.identity
            };
            subject[..32].copy_from_slice(base.as_bytes());
            self.publication = Some(fingerprint);
            subject[32..].copy_from_slice(fingerprint.as_bytes());
            self.identity = TerminalArtifactIdentity(hash(b"psi-published-artifact\0", &subject));
            self
        }
    }

    pub fn build_artifact_manifest(
        semantic_module: &TerminalModule,
        optimization: &PsiOptimizationExecutionRecord,
        installation_record: Option<&[u8]>,
        debug_maps: Option<&[u8]>,
    ) -> Result<TerminalArtifactManifest, ArtifactManifestError> {
        let semantic =
            terminal_psi_identity(semantic_module).map_err(ArtifactManifestError::Semantic)?;
        manifest_for_semantic(semantic, optimization, installation_record, debug_maps)
    }

    pub fn manifest_for_semantic(
        semantic: TerminalPsiIdentity,
        optimization: &PsiOptimizationExecutionRecord,
        installation_record: Option<&[u8]>,
        debug_maps: Option<&[u8]>,
    ) -> Result<TerminalArtifactManifest, ArtifactManifestError> {
        optimization
            .validate_output(semantic)
            .map_err(ArtifactManifestError::Optimization)?;
        let installation =
            installation_record.map(|bytes| hash_section(INSTALLATION_DOMAIN, bytes));
        let debug = debug_maps.map(|bytes| hash_section(DEBUG_DOMAIN, bytes));
        let optimization_identity = optimization.identity();
        let identity = artifact_identity(semantic, optimization_identity, installation, debug);
        Ok(TerminalArtifactManifest {
            semantic,
            optimization: optimization_identity,
            installation,
            debug,
            publication: None,
            identity,
        })
    }

    pub fn validate_artifact_manifest(
        semantic_module: &TerminalModule,
        optimization: &PsiOptimizationExecutionRecord,
        installation_record: Option<&[u8]>,
        debug_maps: Option<&[u8]>,
        manifest: TerminalArtifactManifest,
    ) -> Result<(), ArtifactManifestError> {
        validate_artifact_manifest_with_publication(
            semantic_module,
            optimization,
            installation_record,
            debug_maps,
            None,
            manifest,
        )
    }

    pub fn validate_artifact_manifest_with_publication(
        semantic_module: &TerminalModule,
        optimization: &PsiOptimizationExecutionRecord,
        installation_record: Option<&[u8]>,
        debug_maps: Option<&[u8]>,
        publication: Option<&[u8]>,
        manifest: TerminalArtifactManifest,
    ) -> Result<(), ArtifactManifestError> {
        let base = build_artifact_manifest(
            semantic_module,
            optimization,
            installation_record,
            debug_maps,
        )?;
        let expected = publication.map_or(base, |section| base.with_publication(section));
        if expected != manifest {
            return Err(ArtifactManifestError::ManifestMismatch);
        }
        Ok(())
    }

    fn hash_section(domain: &[u8], bytes: &[u8]) -> SectionFingerprint {
        SectionFingerprint(hash(domain, bytes))
    }

    fn artifact_identity(
        semantic: TerminalPsiIdentity,
        optimization: PsiOptimizationExecutionIdentity,
        installation: Option<SectionFingerprint>,
        debug: Option<SectionFingerprint>,
    ) -> TerminalArtifactIdentity {
        let mut bytes = Vec::new();
        crate::codec_bytes::push_encoded(&mut bytes, MANIFEST_FORMAT_MARKER.to_le_bytes());
        crate::codec_bytes::push_encoded(
            &mut bytes,
            semantic.vocabulary_marker.get().to_le_bytes(),
        );
        bytes.extend_from_slice(semantic.program_fingerprint.as_bytes());
        bytes.extend_from_slice(optimization.as_bytes());
        for fingerprint in [installation, debug] {
            match fingerprint {
                None => bytes.push(0),
                Some(fp) => {
                    bytes.push(1);
                    bytes.extend_from_slice(fp.as_bytes());
                }
            }
        }
        TerminalArtifactIdentity(hash(ARTIFACT_DOMAIN, &bytes))
    }

    fn hash(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
        let mut digest = crate::digest_encoder::DomainDigestEncoder::new(domain);
        let byte_len = u64::try_from(bytes.len()).expect("artifact section fits the digest domain");
        digest.raw(&byte_len.to_le_bytes());
        digest.raw(bytes);
        digest.finish()
    }

    fn write_hex(formatter: &mut std::fmt::Formatter<'_>, bytes: &[u8; 32]) -> std::fmt::Result {
        for byte in bytes {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum ArtifactManifestError {
        Semantic(CodecError),
        Optimization(PsiOptimizationExecutionRecordError),
        ManifestMismatch,
    }

    impl std::fmt::Display for ArtifactManifestError {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(formatter, "{self:?}")
        }
    }

    impl std::error::Error for ArtifactManifestError {}
}
pub(crate) mod debug_map {
    use terminal_psi::{
        SemanticFingerprint, TerminalModule, TerminalPsiIdentity, VocabularyMarker,
    };

    pub use crate::debug_map_records::{
        DebugFileId, DebugSite, DebugSourceDigest, DebugSourceFile, DebugSourceOrigin,
        DebugSourceSpan, DebugSubject, TerminalDebugMap,
    };
    use crate::sections::semantic_module::wire::{
        Reader, Writer, bounded_count, strictly_increasing,
    };
    use crate::{CodecError, terminal_psi_identity};

    const MAGIC: &[u8; 8] = b"PSIDBG\0\0";
    const FORMAT_MARKER: u16 = 1;
    const SOURCE_DIGEST_DOMAIN: &[u8] = b"psi-terminal-debug-source\0";

    pub fn source_digest(source: &[u8]) -> DebugSourceDigest {
        let mut digest = crate::digest_encoder::DomainDigestEncoder::new(SOURCE_DIGEST_DOMAIN);
        digest.raw(
            &u64::try_from(source.len())
                .expect("source bytes fit the debug digest domain")
                .to_le_bytes(),
        );
        digest.raw(source);
        DebugSourceDigest::from_bytes(digest.finish())
    }

    pub fn encode_debug_map(
        module: &TerminalModule,
        debug_map: &TerminalDebugMap,
    ) -> Result<Vec<u8>, DebugMapError> {
        validate_debug_map(module, debug_map)?;
        encode_raw(debug_map)
    }

    pub fn decode_debug_map(
        module: &TerminalModule,
        bytes: &[u8],
    ) -> Result<TerminalDebugMap, DebugMapError> {
        let mut reader = Reader::new(bytes);
        if reader.take(MAGIC.len())? != MAGIC {
            return Err(DebugMapError::InvalidMagic);
        }
        let format_marker = reader.u16()?;
        if format_marker != FORMAT_MARKER {
            return Err(DebugMapError::UnsupportedFormatMarker(format_marker));
        }
        let vocabulary_marker_raw = reader.u16()?;
        let vocabulary_marker = VocabularyMarker::new(vocabulary_marker_raw).ok_or(
            DebugMapError::UnsupportedVocabularyMarker(vocabulary_marker_raw),
        )?;
        let program_fingerprint = SemanticFingerprint::from_bytes(reader.array()?);
        let file_count = bounded_count(&mut reader, 1).map_err(DebugMapError::from)?;
        let mut files = Vec::with_capacity(file_count);
        for _ in 0..file_count {
            let id = DebugFileId::new(reader.u32()?).ok_or(DebugMapError::ZeroFileIdentity)?;
            files.push(DebugSourceFile {
                id,
                origin: decode_source_origin(&mut reader)?,
                byte_len: reader.u64()?,
                digest: DebugSourceDigest::from_bytes(reader.array()?),
                path: reader.string("debug source path")?,
            });
        }
        let site_count = bounded_count(&mut reader, 1).map_err(DebugMapError::from)?;
        let mut sites = Vec::with_capacity(site_count);
        for _ in 0..site_count {
            let subject = decode_subject(&mut reader)?;
            let file = DebugFileId::new(reader.u32()?).ok_or(DebugMapError::ZeroFileIdentity)?;
            sites.push(DebugSite {
                subject,
                span: DebugSourceSpan {
                    file,
                    start: reader.u64()?,
                    end: reader.u64()?,
                },
            });
        }
        if reader.remaining() != 0 {
            return Err(DebugMapError::TrailingBytes(reader.remaining()));
        }
        let debug_map = TerminalDebugMap {
            semantic: TerminalPsiIdentity {
                vocabulary_marker,
                program_fingerprint,
            },
            files,
            sites,
        };
        validate_debug_map(module, &debug_map)?;
        if encode_raw(&debug_map)? != bytes {
            return Err(DebugMapError::NonCanonicalEncoding);
        }
        Ok(debug_map)
    }

    pub fn validate_debug_map(
        module: &TerminalModule,
        debug_map: &TerminalDebugMap,
    ) -> Result<(), DebugMapError> {
        let expected = terminal_psi_identity(module)?;
        if debug_map.semantic != expected {
            return Err(DebugMapError::SemanticIdentityMismatch {
                expected,
                actual: debug_map.semantic,
            });
        }
        if !strictly_increasing(debug_map.files.iter().map(|file| file.id)) {
            return Err(DebugMapError::NonCanonicalOrder(
                "debug files by DebugFileId",
            ));
        }
        if !strictly_increasing(debug_map.sites.iter().map(|site| site.subject)) {
            return Err(DebugMapError::NonCanonicalOrder("debug sites by subject"));
        }
        for site in &debug_map.sites {
            let Some(file) = debug_map
                .files
                .iter()
                .find(|file| file.id == site.span.file)
            else {
                return Err(DebugMapError::UnknownFile(site.span.file));
            };
            if site.span.start > site.span.end || site.span.end > file.byte_len {
                return Err(DebugMapError::InvalidSpan(site.span));
            }
            if !subject_exists(module, site.subject) {
                return Err(DebugMapError::UnknownSubject(site.subject));
            }
        }
        Ok(())
    }

    fn encode_raw(debug_map: &TerminalDebugMap) -> Result<Vec<u8>, DebugMapError> {
        let mut writer = Writer::default();
        writer.bytes(MAGIC);
        writer.u16(FORMAT_MARKER);
        writer.u16(debug_map.semantic.vocabulary_marker.get());
        writer.bytes(debug_map.semantic.program_fingerprint.as_bytes());
        writer.len("debug source files", debug_map.files.len())?;
        for file in &debug_map.files {
            writer.u32(file.id.get());
            writer.u8(match file.origin {
                DebugSourceOrigin::User => 1,
                DebugSourceOrigin::Toolchain => 2,
            });
            writer.u64(file.byte_len);
            writer.bytes(file.digest.as_bytes());
            writer.string("debug source path", &file.path)?;
        }
        writer.len("debug sites", debug_map.sites.len())?;
        for site in &debug_map.sites {
            encode_subject(&mut writer, site.subject);
            writer.u32(site.span.file.get());
            writer.u64(site.span.start);
            writer.u64(site.span.end);
        }
        Ok(writer.finish())
    }

    macro_rules! debug_subject_codec {
        ($($tag:literal => $variant:ident ($label:literal)),* $(,)?) => {
            fn encode_subject(writer: &mut Writer, subject: DebugSubject) {
                match subject {
                    $(DebugSubject::$variant(id) => { writer.u8($tag); writer.id(id); })*
                    DebugSubject::Claim { machine, claim } => {
                        writer.u8(9);
                        writer.id(machine);
                        writer.id(claim);
                    }
                }
            }

            pub(crate) fn decode_subject(reader: &mut Reader<'_>) -> Result<DebugSubject, DebugMapError> {
                Ok(match reader.u8()? {
                    $($tag => DebugSubject::$variant(reader.id($label)?),)*
                    9 => DebugSubject::Claim {
                        machine: reader.id("debug claim MachineId")?,
                        claim: reader.id("debug ClaimId")?,
                    },
                    tag => return Err(DebugMapError::InvalidTag("DebugSubject", tag)),
                })
            }
        };
    }

    debug_subject_codec! {
        1 => Machine("debug MachineId"),
        2 => Block("debug BlockId"),
        3 => Operation("debug OperationId"),
        4 => Edge("debug EdgeId"),
        5 => Value("debug ValueId"),
        6 => Contract("debug ContractId"),
        7 => Obligation("debug ObligationId"),
        8 => Place("debug PlaceId"),
    }

    pub(crate) fn decode_source_origin(
        reader: &mut Reader<'_>,
    ) -> Result<DebugSourceOrigin, DebugMapError> {
        Ok(match reader.u8()? {
            1 => DebugSourceOrigin::User,
            2 => DebugSourceOrigin::Toolchain,
            tag => return Err(DebugMapError::InvalidTag("DebugSourceOrigin", tag)),
        })
    }

    fn subject_exists(module: &TerminalModule, subject: DebugSubject) -> bool {
        match subject {
            DebugSubject::Machine(id) => module.machine(id).is_some(),
            DebugSubject::Block(id) => module.blocks().any(|block| block.id == id),
            DebugSubject::Operation(id) => module.operations().any(|operation| operation.id == id),
            DebugSubject::Edge(id) => module
                .blocks()
                .any(|block| block.terminator.edges().any(|edge| edge == id)),
            DebugSubject::Value(id) => module.machines.iter().any(|machine| {
                machine
                    .result
                    .scalar()
                    .is_some_and(|result| result.id == id)
                    || machine.parameters.iter().any(|value| value.id == id)
                    || machine.blocks.iter().any(|block| {
                        block.parameters.iter().any(|value| value.id == id)
                            || block.operations.iter().any(|operation| {
                                operation
                                    .result
                                    .scalar()
                                    .is_some_and(|result| result.id == id)
                            })
                    })
            }),
            DebugSubject::Contract(id) => {
                module.declaration_contract(id).is_some()
                    || module
                        .machines
                        .iter()
                        .any(|machine| machine.contract.id == id)
            }
            DebugSubject::Obligation(id) => module
                .machines
                .iter()
                .flat_map(|machine| &machine.contract.ensures)
                .any(|clause| clause.obligation == id),
            DebugSubject::Place(id) => module
                .machines
                .iter()
                .flat_map(|machine| &machine.structural_places)
                .any(|place| place.id == id),
            DebugSubject::Claim { machine, claim } => {
                module.machine(machine).is_some_and(|machine| {
                    machine
                        .content_entry_claims
                        .iter()
                        .any(|binding| binding.claim == claim)
                        || machine
                            .content_identity_reshuffles
                            .iter()
                            .any(|reshuffle| reshuffle.claim == claim)
                        || machine
                            .content_partition_compositions
                            .iter()
                            .any(|composition| composition.input_claims.contains(&claim))
                })
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum DebugMapError {
        Codec(CodecError),
        InvalidMagic,
        UnsupportedFormatMarker(u16),
        UnsupportedVocabularyMarker(u16),
        ZeroFileIdentity,
        InvalidTag(&'static str, u8),
        TrailingBytes(usize),
        NonCanonicalOrder(&'static str),
        NonCanonicalEncoding,
        SemanticIdentityMismatch {
            expected: TerminalPsiIdentity,
            actual: TerminalPsiIdentity,
        },
        UnknownFile(DebugFileId),
        InvalidSpan(DebugSourceSpan),
        UnknownSubject(DebugSubject),
    }

    impl From<CodecError> for DebugMapError {
        fn from(error: CodecError) -> Self {
            Self::Codec(error)
        }
    }

    impl std::fmt::Display for DebugMapError {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(formatter, "{self:?}")
        }
    }

    impl std::error::Error for DebugMapError {}
}
pub(crate) mod optimization_execution {

    use crate::cursor::Cursor;
    use optimization::{
        PsiOptimizationSelectionDecodeError, PsiOptimizationSelectionIdentity,
        PsiOptimizationSelections,
    };
    use terminal_psi::TerminalModule;
    use terminal_psi::{SemanticFingerprint, TerminalPsiIdentity, VocabularyMarker};

    use crate::{CodecError, terminal_psi_identity};

    const MAGIC: &[u8; 8] = b"PSIOEXE\0";
    const FORMAT_MARKER: u16 = 2;
    const IDENTITY_DOMAIN: &[u8] = b"psi.preterminal-optimization-execution.v1\0";

    #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct PsiOptimizationExecutionIdentity([u8; 32]);

    impl PsiOptimizationExecutionIdentity {
        pub const fn as_bytes(&self) -> &[u8; 32] {
            &self.0
        }
    }

    impl std::fmt::Debug for PsiOptimizationExecutionIdentity {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            for byte in self.0 {
                write!(formatter, "{byte:02x}")?;
            }
            Ok(())
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct PsiOptimizationExecutionRecord {
        selections: PsiOptimizationSelections,
        input_semantic: TerminalPsiIdentity,
        output_semantic: TerminalPsiIdentity,
        identity: PsiOptimizationExecutionIdentity,
    }

    impl PsiOptimizationExecutionRecord {
        pub fn new(
            selections: PsiOptimizationSelections,
            input_semantic: TerminalPsiIdentity,
            output_semantic: TerminalPsiIdentity,
        ) -> Result<Self, PsiOptimizationExecutionRecordError> {
            if selections.is_empty() && input_semantic != output_semantic {
                return Err(PsiOptimizationExecutionRecordError::IdentityStageChangedProduct);
            }
            let mut digest = crate::digest_encoder::DomainDigestEncoder::new(IDENTITY_DOMAIN);
            digest.raw(&selections.identity().bytes());
            append_identity_to_digest(&mut digest, input_semantic);
            append_identity_to_digest(&mut digest, output_semantic);
            let identity = PsiOptimizationExecutionIdentity(digest.finish());
            Ok(Self {
                selections,
                input_semantic,
                output_semantic,
                identity,
            })
        }

        pub const fn selections(&self) -> &PsiOptimizationSelections {
            &self.selections
        }

        pub fn selection_identity(&self) -> PsiOptimizationSelectionIdentity {
            self.selections.identity()
        }

        pub const fn input_semantic(&self) -> TerminalPsiIdentity {
            self.input_semantic
        }

        pub const fn output_semantic(&self) -> TerminalPsiIdentity {
            self.output_semantic
        }

        pub const fn identity(&self) -> PsiOptimizationExecutionIdentity {
            self.identity
        }

        pub fn validate_output(
            &self,
            semantic: TerminalPsiIdentity,
        ) -> Result<(), PsiOptimizationExecutionRecordError> {
            if self.output_semantic != semantic {
                return Err(PsiOptimizationExecutionRecordError::OutputMismatch);
            }
            Ok(())
        }
    }

    pub fn build_identity_optimization_execution_record(
        module: &TerminalModule,
    ) -> Result<PsiOptimizationExecutionRecord, PsiOptimizationExecutionRecordBuildError> {
        let semantic = terminal_psi_identity(module)
            .map_err(PsiOptimizationExecutionRecordBuildError::Semantic)?;
        PsiOptimizationExecutionRecord::new(
            PsiOptimizationSelections::default(),
            semantic,
            semantic,
        )
        .map_err(PsiOptimizationExecutionRecordBuildError::Record)
    }

    pub fn encode_psi_optimization_execution_record(
        record: &PsiOptimizationExecutionRecord,
    ) -> Vec<u8> {
        let selections = record.selections.encode();
        let mut bytes = Vec::with_capacity(10 + 8 + selections.len() + 132);
        bytes.extend_from_slice(MAGIC);
        crate::codec_bytes::push_encoded(&mut bytes, FORMAT_MARKER.to_le_bytes());
        bytes.extend_from_slice(
            &u64::try_from(selections.len())
                .expect("the in-memory optimization selection fits u64")
                .to_le_bytes(),
        );
        bytes.extend_from_slice(&selections);
        append_terminal_identity(&mut bytes, record.input_semantic);
        append_terminal_identity(&mut bytes, record.output_semantic);
        bytes
    }

    pub fn decode_psi_optimization_execution_record(
        bytes: &[u8],
    ) -> Result<PsiOptimizationExecutionRecord, PsiOptimizationExecutionRecordDecodeError> {
        let mut cursor = Cursor::new(bytes);
        if cursor.take(MAGIC.len())? != MAGIC {
            return Err(PsiOptimizationExecutionRecordDecodeError::InvalidMagic);
        }
        let marker = cursor.u16()?;
        if marker != FORMAT_MARKER {
            return Err(PsiOptimizationExecutionRecordDecodeError::UnsupportedFormatMarker(marker));
        }
        let selections_len = usize::try_from(cursor.u64()?)
            .map_err(|_| PsiOptimizationExecutionRecordDecodeError::UnexpectedEnd)?;
        let selections = PsiOptimizationSelections::decode(cursor.take(selections_len)?)
            .map_err(PsiOptimizationExecutionRecordDecodeError::Selection)?;
        let input_semantic = decode_terminal_identity(&mut cursor)?;
        let output_semantic = decode_terminal_identity(&mut cursor)?;
        if cursor.remaining() != 0 {
            return Err(PsiOptimizationExecutionRecordDecodeError::TrailingBytes(
                cursor.remaining(),
            ));
        }
        let record =
            PsiOptimizationExecutionRecord::new(selections, input_semantic, output_semantic)
                .map_err(PsiOptimizationExecutionRecordDecodeError::InvalidRecord)?;
        if encode_psi_optimization_execution_record(&record) != bytes {
            return Err(PsiOptimizationExecutionRecordDecodeError::NonCanonicalEncoding);
        }
        Ok(record)
    }

    fn append_identity_to_digest(
        digest: &mut crate::digest_encoder::DomainDigestEncoder,
        identity: TerminalPsiIdentity,
    ) {
        digest.raw(&identity.vocabulary_marker.get().to_le_bytes());
        digest.raw(identity.program_fingerprint.as_bytes());
    }

    pub fn append_terminal_identity(bytes: &mut Vec<u8>, identity: TerminalPsiIdentity) {
        crate::codec_bytes::push_encoded(bytes, identity.vocabulary_marker.get().to_le_bytes());
        bytes.extend_from_slice(identity.program_fingerprint.as_bytes());
    }

    fn decode_terminal_identity(
        cursor: &mut Cursor<'_>,
    ) -> Result<TerminalPsiIdentity, PsiOptimizationExecutionRecordDecodeError> {
        let marker = cursor.u16()?;
        let vocabulary_marker = VocabularyMarker::new(marker).ok_or(
            PsiOptimizationExecutionRecordDecodeError::UnsupportedVocabularyMarker(marker),
        )?;
        Ok(TerminalPsiIdentity {
            vocabulary_marker,
            program_fingerprint: SemanticFingerprint::from_bytes(cursor.array()?),
        })
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum PsiOptimizationExecutionRecordError {
        IdentityStageChangedProduct,
        OutputMismatch,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum PsiOptimizationExecutionRecordBuildError {
        Semantic(CodecError),
        Record(PsiOptimizationExecutionRecordError),
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum PsiOptimizationExecutionRecordDecodeError {
        InvalidMagic,
        UnsupportedFormatMarker(u16),
        UnsupportedVocabularyMarker(u16),
        UnexpectedEnd,
        TrailingBytes(usize),
        Selection(PsiOptimizationSelectionDecodeError),
        InvalidRecord(PsiOptimizationExecutionRecordError),
        NonCanonicalEncoding,
    }

    impl std::fmt::Display for PsiOptimizationExecutionRecordError {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(
                formatter,
                "invalid Psi optimization execution record: {self:?}"
            )
        }
    }

    impl std::fmt::Display for PsiOptimizationExecutionRecordBuildError {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(
                formatter,
                "cannot build Psi optimization execution record: {self:?}"
            )
        }
    }

    impl std::fmt::Display for PsiOptimizationExecutionRecordDecodeError {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(
                formatter,
                "invalid Psi optimization execution bytes: {self:?}"
            )
        }
    }

    impl std::error::Error for PsiOptimizationExecutionRecordError {}
    impl std::error::Error for PsiOptimizationExecutionRecordBuildError {}
    impl std::error::Error for PsiOptimizationExecutionRecordDecodeError {}

    ::wire::truncated_from_adapter!(PsiOptimizationExecutionRecordDecodeError => Self::UnexpectedEnd);
}

//! Substitutable-field inventory for the canonical Terminal debug-map section.
//!
//! The family's declared wire fields are driven through the shared
//! substitution matrix by `debug_map_custody.rs`; keeping the inventory in its
//! own file names the boundary between what is declared substitutable and the
//! driver that exercises every leg. Truncations are not one-field
//! substitutions and stay authored beside the matrix.

optimization_core::custody_field_inventory! {
    /// One substituted wire field of the canonical debug section; each leg's
    /// verdict is checked by canonical decoding and, for substitutions the
    /// decoder still accepts, replay against the retained artifact manifest.
    pub enum DebugMapFieldForTest {
        VocabularyMarker,
        ProgramFingerprint,
        Magic,
        FormatMarker,
        TrailingByte,
        FileOrigin,
        FileByteLength,
        FileDigest,
        FilePath,
        UnreferencedFileDropped,
        FileInserted,
        ReferencedFileDropped,
        FileIdentityZero,
        FileIdentityDuplicated,
        FileIdentityOutOfOrder,
        FileOriginUnknownTag,
        FileByteLengthStrandingSpans,
        FilePathNonUtf8,
        FilePathOverLong,
        FileCountCleared,
        FileCountOverCounted,
        SiteSubjectKind,
        SiteSubjectIdentity,
        SiteSpanStart,
        SiteSpanEnd,
        SiteSpanFile,
        SiteDropped,
        SiteSubjectKindOutOfOrder,
        SiteSubjectDuplicated,
        SiteSubjectUnknownTag,
        SiteSubjectNamesNoMachine,
        SiteSubjectNamesNoPlace,
        SiteSubjectNamesNoClaim,
        SiteSpanFileZero,
        SiteSpanFileUnrostered,
        SiteSpanStartOvertakingEnd,
        SiteSpanEndEscapingFile,
        SiteCountCleared,
        SiteCountOverCounted,
    }
}

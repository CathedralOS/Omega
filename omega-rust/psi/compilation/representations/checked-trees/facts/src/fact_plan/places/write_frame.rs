/// Whether a normalized may-write frame completely describes the caller-visible
/// writes of the body being summarized.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum WriteFrameCompleteness {
    Complete,
    /// Body inspection could not produce a sound finite frame. Consumers must
    /// fail closed instead of treating the empty path set as purity.
    #[default]
    Opaque,
}

/// Canonical may-write frame shared by inference, checked-plan storage, and
/// artifact consumers. Complete paths are sorted and deduplicated before the
/// report coordinate is computed; opaque frames intentionally expose no usable
/// paths. Semantic consumers compare completeness and exact normalized paths,
/// never the compact coordinate alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedWriteFrame {
    completeness: WriteFrameCompleteness,
    paths: Vec<String>,
    compatibility_report_fingerprint: u64,
}

impl Default for NormalizedWriteFrame {
    fn default() -> Self {
        Self::opaque()
    }
}

impl NormalizedWriteFrame {
    pub fn complete(mut paths: Vec<String>) -> Self {
        paths.sort();
        paths.dedup();
        // A may-write storage root already includes its descendants. Retain
        // the minimal prefix cover before comparison and identity so a caller
        // cannot distinguish redundant child rows from the same root effect.
        let mut canonical: Vec<String> = Vec::with_capacity(paths.len());
        for path in paths {
            if !canonical.iter().any(|ancestor| {
                path.strip_prefix(ancestor.as_str())
                    .is_some_and(|suffix| suffix.starts_with('.') || suffix.starts_with('['))
            }) {
                canonical.push(path);
            }
        }
        let paths = canonical;
        let compatibility_report_fingerprint =
            non_authoritative_write_frame_compatibility_fingerprint(
                WriteFrameCompleteness::Complete,
                &paths,
            );
        Self {
            completeness: WriteFrameCompleteness::Complete,
            paths,
            compatibility_report_fingerprint,
        }
    }

    pub fn opaque() -> Self {
        Self {
            completeness: WriteFrameCompleteness::Opaque,
            paths: Vec::new(),
            compatibility_report_fingerprint:
                non_authoritative_write_frame_compatibility_fingerprint(
                    WriteFrameCompleteness::Opaque,
                    &[],
                ),
        }
    }

    pub fn completeness(&self) -> WriteFrameCompleteness {
        self.completeness
    }

    pub fn is_complete(&self) -> bool {
        self.completeness == WriteFrameCompleteness::Complete
    }

    pub fn paths(&self) -> &[String] {
        &self.paths
    }

    pub fn complete_paths(&self) -> Option<&[String]> {
        self.is_complete().then_some(self.paths.as_slice())
    }

    pub fn into_complete_paths(self) -> Option<Vec<String>> {
        self.is_complete().then_some(self.paths)
    }

    pub fn compatibility_report_fingerprint(&self) -> u64 {
        self.compatibility_report_fingerprint
    }
}

fn non_authoritative_write_frame_compatibility_fingerprint(
    completeness: WriteFrameCompleteness,
    normalized_paths: &[String],
) -> u64 {
    let mut hash = semantic_vocabulary::Fnv1a::new();
    hash.byte(match completeness {
        WriteFrameCompleteness::Complete => 1,
        WriteFrameCompleteness::Opaque => 2,
    });
    for path in normalized_paths {
        hash.bytes_framed(path.as_bytes());
    }
    hash.finish()
}

#[cfg(test)]
mod tests {
    use super::NormalizedWriteFrame;

    #[test]
    fn complete_frames_normalize_order_and_duplicates_before_identity() {
        let first = NormalizedWriteFrame::complete(vec![
            "$P0.value".to_owned(),
            "self.count".to_owned(),
            "$P0.value".to_owned(),
        ]);
        let second =
            NormalizedWriteFrame::complete(vec!["self.count".to_owned(), "$P0.value".to_owned()]);

        assert_eq!(first, second);
        assert_eq!(
            first.paths(),
            &["$P0.value".to_owned(), "self.count".to_owned()]
        );
    }

    #[test]
    fn complete_frames_remove_only_covered_storage_descendants() {
        let normalized = NormalizedWriteFrame::complete(vec![
            "$P0.focus".into(),
            "$P0".into(),
            "$P0.focus_visible".into(),
            "$P01.focus".into(),
            "$P1.items[2]".into(),
            "$P1.items".into(),
            "$P1.items_extra".into(),
        ]);
        assert_eq!(
            normalized,
            NormalizedWriteFrame::complete(vec![
                "$P0".into(),
                "$P01.focus".into(),
                "$P1.items".into(),
                "$P1.items_extra".into()
            ])
        );
        assert_ne!(
            normalized,
            NormalizedWriteFrame::complete(vec!["$P0".into(), "$P1.items".into()])
        );
    }

    #[test]
    fn opaque_frame_is_distinct_from_complete_empty_frame() {
        let opaque = NormalizedWriteFrame::opaque();
        let pure = NormalizedWriteFrame::complete(Vec::new());

        assert_ne!(
            opaque.compatibility_report_fingerprint(),
            pure.compatibility_report_fingerprint()
        );
        assert_eq!(NormalizedWriteFrame::default(), opaque);
        assert!(opaque.complete_paths().is_none());
        assert_eq!(pure.complete_paths(), Some([].as_slice()));
    }
}

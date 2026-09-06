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
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;
    let mut hash = OFFSET;
    let mut fold = |byte: u8| {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(PRIME);
    };
    fold(match completeness {
        WriteFrameCompleteness::Complete => 1,
        WriteFrameCompleteness::Opaque => 2,
    });
    for path in normalized_paths {
        for byte in (path.len() as u64).to_le_bytes() {
            fold(byte);
        }
        for byte in path.as_bytes() {
            fold(*byte);
        }
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

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

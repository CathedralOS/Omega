//! Locate the first differing byte in registered test observations.
#[derive(Clone, Copy, Debug)]
pub(super) struct ProgramStreams<'a> {
    pub(super) stdout: &'a [u8],
    pub(super) stderr: &'a [u8],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ProgramStream {
    Stdout,
    Stderr,
}

impl ProgramStream {
    pub(super) const fn name(self) -> &'static str {
        match self {
            Self::Stdout => "stdout",
            Self::Stderr => "stderr",
        }
    }
}

/// Offset of the first unequal byte, including a shorter stream's end.
pub(super) fn first_byte_difference(left: &[u8], right: &[u8]) -> Option<usize> {
    left.iter()
        .zip(right)
        .position(|(left, right)| left != right)
        .or_else(|| (left.len() != right.len()).then_some(left.len().min(right.len())))
}

impl ProgramStreams<'_> {
    pub(super) fn first_difference(
        &self,
        other: ProgramStreams<'_>,
    ) -> Option<(ProgramStream, usize)> {
        first_byte_difference(self.stdout, other.stdout)
            .map(|offset| (ProgramStream::Stdout, offset))
            .or_else(|| {
                first_byte_difference(self.stderr, other.stderr)
                    .map(|offset| (ProgramStream::Stderr, offset))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::{ProgramStream, ProgramStreams};

    #[test]
    fn stream_offsets_count_raw_bytes_including_a_shorter_streams_end() {
        let captured = ProgramStreams {
            stdout: b"\xffB",
            stderr: b"!\n",
        };
        assert_eq!(captured.first_difference(captured), None);
        let short = ProgramStreams {
            stdout: b"\xff",
            stderr: b"",
        };
        assert_eq!(
            captured.first_difference(short),
            Some((ProgramStream::Stdout, 1))
        );
        let truncated_line = ProgramStreams {
            stdout: captured.stdout,
            stderr: b"!",
        };
        assert_eq!(
            captured.first_difference(truncated_line),
            Some((ProgramStream::Stderr, 1))
        );
    }
}

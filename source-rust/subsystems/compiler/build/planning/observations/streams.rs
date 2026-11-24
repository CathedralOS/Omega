//! Compare native and Psi streams as bytes. Retain the line for human-readable diagnostics.
#[derive(Clone, Copy, Debug)]
pub struct ProgramStreams<'a> {
    pub stdout: &'a [u8],
    pub stderr: &'a [u8],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProgramStream {
    Stdout,
    Stderr,
}

impl ProgramStream {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Stdout => "stdout",
            Self::Stderr => "stderr",
        }
    }
}

// The first line at which one stream differs between the two runs. `None` on
// either side means that run's stream ended before this line.
#[derive(Debug, PartialEq, Eq)]
pub struct OutputDivergence {
    pub stream: ProgramStream,
    pub line: usize,   // 1-based
    pub offset: usize, // 0-based byte offset within the selected stream
    pub native: Option<Vec<u8>>,
    pub psi: Option<Vec<u8>>,
}

impl std::fmt::Display for OutputDivergence {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{} line {}: native {} vs Psi {}",
            self.stream.name(),
            self.line,
            Rendered(self.native.as_deref()),
            Rendered(self.psi.as_deref()),
        )
    }
}

struct Rendered<'a>(Option<&'a [u8]>);

impl std::fmt::Display for Rendered<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            Some(line) => write!(formatter, "{:?}", String::from_utf8_lossy(line)),
            None => formatter.write_str("(end of output)"),
        }
    }
}

/// Offset of the first unequal byte, including a shorter stream's end.
pub fn first_byte_difference(left: &[u8], right: &[u8]) -> Option<usize> {
    left.iter()
        .zip(right)
        .position(|(left, right)| left != right)
        .or_else(|| (left.len() != right.len()).then_some(left.len().min(right.len())))
}

impl ProgramStreams<'_> {
    pub fn first_difference(&self, other: ProgramStreams<'_>) -> Option<(ProgramStream, usize)> {
        first_byte_difference(self.stdout, other.stdout)
            .map(|offset| (ProgramStream::Stdout, offset))
            .or_else(|| {
                first_byte_difference(self.stderr, other.stderr)
                    .map(|offset| (ProgramStream::Stderr, offset))
            })
    }
}

// Stdout is checked first, so a run that differs on both streams reports its
// stdout difference.
pub fn first_output_difference(
    native: ProgramStreams<'_>,
    psi: ProgramStreams<'_>,
) -> Option<OutputDivergence> {
    first_line_difference(ProgramStream::Stdout, native.stdout, psi.stdout)
        .or_else(|| first_line_difference(ProgramStream::Stderr, native.stderr, psi.stderr))
}

fn first_line_difference(
    stream: ProgramStream,
    native: &[u8],
    psi: &[u8],
) -> Option<OutputDivergence> {
    if native == psi {
        return None;
    }
    let mut native_lines = native.split_inclusive(|byte| *byte == b'\n');
    let mut psi_lines = psi.split_inclusive(|byte| *byte == b'\n');
    let mut line = 1;
    let mut offset = 0;
    loop {
        match (native_lines.next(), psi_lines.next()) {
            (Some(left), Some(right)) if left == right => {
                line += 1;
                offset += left.len();
            }
            (left, right) => {
                // Unequal streams always part at some line, so both sides
                // cannot end together here.
                return Some(OutputDivergence {
                    stream,
                    line,
                    offset: offset
                        + first_byte_difference(
                            left.unwrap_or_default(),
                            right.unwrap_or_default(),
                        )
                        .expect("different lines"),
                    native: left.map(<[u8]>::to_vec),
                    psi: right.map(<[u8]>::to_vec),
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ProgramStream, ProgramStreams, first_output_difference};

    fn streams<'a>(stdout: &'a str, stderr: &'a str) -> ProgramStreams<'a> {
        ProgramStreams {
            stdout: stdout.as_bytes(),
            stderr: stderr.as_bytes(),
        }
    }

    #[test]
    fn equal_output_agrees() {
        let run = streams("one\ntwo\n", "warning\n");
        assert_eq!(first_output_difference(run, run), None);
    }

    #[test]
    fn reports_the_first_differing_stdout_line() {
        let divergence = first_output_difference(
            streams("one\ntwo\nthree\n", ""),
            streams("one\nTWO\nthree\n", ""),
        )
        .expect("different stdout diverges");
        assert_eq!(divergence.stream, ProgramStream::Stdout);
        assert_eq!(divergence.line, 2);
        assert_eq!(divergence.offset, 4);
        assert_eq!(
            divergence.to_string(),
            r#"stdout line 2: native "two\n" vs Psi "TWO\n""#
        );
    }

    #[test]
    fn a_missing_line_or_final_newline_diverges() {
        let shorter = first_output_difference(streams("one\ntwo\n", ""), streams("one\n", ""))
            .expect("a missing line diverges");
        assert_eq!(
            shorter.to_string(),
            r#"stdout line 2: native "two\n" vs Psi (end of output)"#
        );
        let unterminated = first_output_difference(streams("one\n", ""), streams("one", ""))
            .expect("a missing final newline diverges");
        assert_eq!(
            unterminated.to_string(),
            r#"stdout line 1: native "one\n" vs Psi "one""#
        );
    }

    #[test]
    fn stderr_is_compared_when_stdout_agrees() {
        let divergence =
            first_output_difference(streams("same\n", ""), streams("same\n", "only Psi\n"))
                .expect("different stderr diverges");
        assert_eq!(
            divergence.to_string(),
            r#"stderr line 1: native (end of output) vs Psi "only Psi\n""#
        );
    }
}

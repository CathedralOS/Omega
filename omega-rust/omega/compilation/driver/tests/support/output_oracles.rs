//! Authored stream oracles constrain each engine independently of its exit.
//! Missing capture or an unreadable oracle is not empty output. Compare bytes,
//! allowing only the corpus's existing CRLF-to-LF presentation normalization.

use std::path::Path;

pub(super) fn observe(
    fixture: &Path,
    prefix: &str,
    stdout: Option<&[u8]>,
    stderr: Option<&[u8]>,
) -> Vec<String> {
    let mut facts = Vec::new();
    for (stream, actual) in [("stdout", stdout), ("stderr", stderr)] {
        let expected = match std::fs::read(fixture.join(format!("expected_{stream}.txt"))) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => {
                facts.push(format!("{prefix}{stream}:unavailable"));
                continue;
            }
        };
        let verdict = match actual {
            Some(actual) if normalized_bytes(actual).eq(normalized_bytes(&expected)) => "match",
            Some(_) => "differs",
            None => "unavailable",
        };
        facts.push(format!("{prefix}{stream}:{verdict}"));
    }
    facts
}

fn normalized_bytes(bytes: &[u8]) -> impl Iterator<Item = u8> + '_ {
    bytes.iter().enumerate().filter_map(|(position, byte)| {
        (*byte != b'\r' || bytes.get(position + 1) != Some(&b'\n')).then_some(*byte)
    })
}

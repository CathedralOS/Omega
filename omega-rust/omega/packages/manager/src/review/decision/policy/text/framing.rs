use super::*;
use std::fmt;

pub(super) const HEADER: &str = "omega-package-policy-decisions 2\n";

#[derive(Default)]
pub(super) struct Counter {
    pub(super) bytes: usize,
}
impl Write for Counter {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        self.bytes = self.bytes.checked_add(text.len()).ok_or(fmt::Error)?;
        Ok(())
    }
}

pub(super) fn emit(resolution: &PackagePolicyResolution, output: &mut impl Write) -> fmt::Result {
    output.write_str(HEADER)?;
    output.write_str("comparison ")?;
    hex(output, &resolution.comparison.digest())?;
    writeln!(output, "\ndecisions {}", resolution.decisions.len())?;
    for decision in &resolution.decisions {
        output.write_str("decision ")?;
        match decision.subject {
            Subject::RootRole => output.write_str("root_role")?,
            Subject::Row(digest) => {
                output.write_str("row ")?;
                hex(output, &digest)?;
            }
        }
        writeln!(
            output,
            " {}",
            match decision.disposition {
                Disposition::AcceptCandidateChange => "accept_candidate_change",
                Disposition::RejectCandidateChange => "reject_candidate_change",
            }
        )?;
    }
    output.write_str("end\n")
}

fn hex(output: &mut impl Write, bytes: &[u8; 32]) -> fmt::Result {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in bytes {
        output.write_char(char::from(HEX[usize::from(byte >> 4)]))?;
        output.write_char(char::from(HEX[usize::from(byte & 15)]))?;
    }
    Ok(())
}

pub(super) fn digest(text: &str) -> Result<[u8; 32], Error> {
    if text.len() != 64 {
        return Err(Error::InvalidFraming);
    }
    let mut output = [0; 32];
    for (index, pair) in text.as_bytes().as_chunks::<2>().0.iter().enumerate() {
        output[index] = nibble(pair[0])? * 16 + nibble(pair[1])?;
    }
    Ok(output)
}
fn nibble(byte: u8) -> Result<u8, Error> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(Error::InvalidFraming),
    }
}
pub(super) fn number(text: &str) -> Result<usize, Error> {
    if text.is_empty()
        || (text.len() > 1 && text.starts_with('0'))
        || !text.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(Error::InvalidFraming);
    }
    text.parse().map_err(|_| Error::InvalidFraming)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restart_digest_and_counts_require_exact_canonical_spelling() {
        assert_eq!(digest(&"ab".repeat(32)).unwrap(), [0xab; 32]);
        for malformed in ["AB".repeat(32), "ab".repeat(31), "gg".repeat(32)] {
            assert_eq!(digest(&malformed), Err(Error::InvalidFraming));
        }
        assert_eq!(number("0").unwrap(), 0);
        for malformed in ["", "00", "+1", " 1", "1 ", "1\n"] {
            assert_eq!(number(malformed), Err(Error::InvalidFraming));
        }
    }
}

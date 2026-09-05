use super::{Error, PackagePolicyDecisionResolution, ReviewOnlyRootPolicyDisposition};
use std::fmt::{self, Write};

pub(super) const HEADER: &str = "omega-package-policy-decisions 1";

pub(super) fn emit(
    value: &PackagePolicyDecisionResolution,
    output: &mut impl Write,
) -> fmt::Result {
    writeln!(output, "{HEADER}")?;
    output.write_str("change_set ")?;
    hexadecimal(output, value.change_set().digest())?;
    writeln!(output, "\ndecisions {}", value.decisions().len())?;
    for decision in value.decisions() {
        output.write_str("decision ")?;
        hexadecimal(output, decision.package().digest())?;
        output.write_char(' ')?;
        hexadecimal(output, decision.obligation().fingerprint().digest())?;
        writeln!(
            output,
            " {}",
            match decision.disposition() {
                ReviewOnlyRootPolicyDisposition::AcceptCandidateChange => "accept_candidate_change",
                ReviewOnlyRootPolicyDisposition::RejectCandidateChange => "reject_candidate_change",
            }
        )?;
    }
    output.write_str("resolution ")?;
    hexadecimal(output, value.fingerprint().digest())?;
    output.write_str("\nend\n")
}

fn hexadecimal(output: &mut impl Write, bytes: [u8; 32]) -> fmt::Result {
    for byte in bytes {
        write!(output, "{byte:02x}")?;
    }
    Ok(())
}

pub(super) fn digest(text: &str) -> Result<[u8; 32], Error> {
    if text.len() != 64 {
        return Err(Error::InvalidFingerprint);
    }
    let mut result = [0; 32];
    for (index, pair) in text.as_bytes().as_chunks::<2>().0.iter().enumerate() {
        result[index] = nibble(pair[0])? * 16 + nibble(pair[1])?;
    }
    Ok(result)
}
fn nibble(byte: u8) -> Result<u8, Error> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(Error::InvalidFingerprint),
    }
}

pub(super) fn number(text: &str) -> Result<usize, Error> {
    if text.is_empty() || (text.len() > 1 && text.starts_with('0')) {
        return Err(Error::InvalidFraming);
    }
    let mut result = 0usize;
    for byte in text.bytes() {
        if !byte.is_ascii_digit() {
            return Err(Error::InvalidFraming);
        }
        result = result
            .checked_mul(10)
            .and_then(|value| value.checked_add(usize::from(byte - b'0')))
            .ok_or(Error::LengthOverflow)?;
    }
    Ok(result)
}

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

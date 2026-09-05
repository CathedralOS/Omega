use super::*;
use std::fmt::{self, Write};

pub(in super::super) fn text(
    history: &HistoricalPackagePolicyDecisions,
    source: &Source,
    limits: Limits,
    maximum_owned_bytes: usize,
) -> Result<(String, Usage), Error> {
    let limits = limits.bounded();
    let count = history.decisions.len();
    if count > limits.maximum_decisions {
        return Err(Error::DecisionLimitExceeded);
    }
    let mut usage = Usage {
        owned_bytes: 0,
        decisions: count,
    };
    usage.charge(
        count
            .checked_mul(32)
            .ok_or(Error::AllocationLimitExceeded)?,
        maximum_owned_bytes,
    )?;
    validation::validate(history, source)?;
    usage.charge(
        count
            .checked_mul(std::mem::size_of::<Option<String>>())
            .ok_or(Error::AllocationLimitExceeded)?,
        maximum_owned_bytes,
    )?;
    let mut fragments = Vec::new();
    fragments
        .try_reserve_exact(count)
        .map_err(|_| Error::AllocationFailed)?;
    let mut fragment_bytes = 0usize;
    for decision in &history.decisions {
        let key = match &decision.subject {
            Subject::RemovedPackage { key } => Some(key),
            Subject::SourceReplacement { baseline, site, .. } => {
                if let ReplacementSite::Dependency { alias, .. } = site {
                    fragment_bytes = fragment_bytes
                        .checked_add(alias.as_str().len())
                        .filter(|bytes| *bytes <= limits.maximum_bytes)
                        .ok_or(Error::ByteLimitExceeded)?;
                }
                Some(baseline)
            }
            _ => None,
        };
        let fragment = if let Some(key) = key {
            let (fragment, owned) = write_package_key_text(
                key,
                key_limits(limits.maximum_bytes),
                maximum_owned_bytes - usage.owned_bytes,
            )
            .map_err(source_key_error)?;
            usage.charge(owned, maximum_owned_bytes)?;
            fragment_bytes = fragment_bytes
                .checked_add(fragment.len())
                .filter(|bytes| *bytes <= limits.maximum_bytes)
                .ok_or(Error::ByteLimitExceeded)?;
            Some(fragment)
        } else {
            None
        };
        fragments.push(fragment);
    }
    let mut counter = Counter {
        bytes: 0,
        maximum: limits.maximum_bytes,
    };
    render(&mut counter, history, &fragments).map_err(|_| Error::ByteLimitExceeded)?;
    usage.charge(counter.bytes, maximum_owned_bytes)?;
    let mut output = String::new();
    output
        .try_reserve_exact(counter.bytes)
        .map_err(|_| Error::AllocationFailed)?;
    render(&mut output, history, &fragments).map_err(|_| Error::InvalidSubject)?;
    let (recovered, recovery) = HistoricalPackagePolicyDecisions::recover_text_with_usage(
        &output,
        source,
        limits,
        maximum_owned_bytes - usage.owned_bytes,
    )?;
    usage.charge(recovery.owned_bytes(), maximum_owned_bytes)?;
    if recovered != *history {
        return Err(Error::InvalidSubject);
    }
    Ok((output, usage))
}

struct Counter {
    bytes: usize,
    maximum: usize,
}
impl Write for Counter {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        self.bytes = self
            .bytes
            .checked_add(text.len())
            .filter(|bytes| *bytes <= self.maximum)
            .ok_or(fmt::Error)?;
        Ok(())
    }
}

fn digest(writer: &mut impl Write, value: &[u8; 32]) -> fmt::Result {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in value {
        writer.write_char(char::from(HEX[usize::from(byte >> 4)]))?;
        writer.write_char(char::from(HEX[usize::from(byte & 15)]))?;
    }
    Ok(())
}

fn render(
    writer: &mut impl Write,
    history: &HistoricalPackagePolicyDecisions,
    fragments: &[Option<String>],
) -> fmt::Result {
    writer.write_str(if history.comparison.is_some() {
        HEADER
    } else {
        "omega-policy-decisions 1\n"
    })?;
    writer.write_str("source ")?;
    digest(writer, history.source_subject.as_bytes())?;
    writer.write_char('\n')?;
    if let Some(comparison) = history.comparison {
        writer.write_str("comparison ")?;
        digest(writer, &comparison)?;
        writer.write_char('\n')?;
    }
    writeln!(writer, "decisions {}", history.decisions.len())?;
    for (decision, fragment) in history.decisions.iter().zip(fragments) {
        writer.write_str("decision ")?;
        match &decision.subject {
            Subject::CandidatePackage { package_index } => {
                if history.comparison.is_some() {
                    writer.write_str("candidate ")?;
                }
                write!(writer, "{package_index} ")?;
            }
            Subject::RemovedPackage { .. } => write!(
                writer,
                "removed {} ",
                fragment.as_ref().ok_or(fmt::Error)?.len()
            )?,
            Subject::RootRole {
                package_index,
                baseline_role,
                candidate_role,
                broken_contract,
            } => {
                write!(
                    writer,
                    "root_role {package_index} {} {} {} ",
                    role_text(*baseline_role).map_err(|_| fmt::Error)?,
                    role_text(*candidate_role).map_err(|_| fmt::Error)?,
                    broken_contract.as_str()
                )?;
            }
            Subject::SourceReplacement {
                package_index,
                site,
                ..
            } => {
                writer.write_str("replacement ")?;
                match site {
                    ReplacementSite::Root => write!(writer, "root {package_index} ")?,
                    ReplacementSite::Dependency {
                        requester_index,
                        alias,
                    } => write!(
                        writer,
                        "dependency {package_index} {requester_index} {} ",
                        alias.as_str()
                    )?,
                }
                write!(writer, "{} ", fragment.as_ref().ok_or(fmt::Error)?.len())?;
            }
        }
        digest(writer, &decision.conflict)?;
        writeln!(writer, " {}", disposition_token(decision.disposition))?;
        if let Some(fragment) = fragment {
            writer.write_str(fragment)?;
        }
    }
    writer.write_str("end\n")
}

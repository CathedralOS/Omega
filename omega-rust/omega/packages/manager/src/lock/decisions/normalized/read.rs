use super::*;

pub(in super::super) fn text(
    text: &str,
    source: &Source,
    limits: Limits,
    maximum_owned_bytes: usize,
) -> Result<(HistoricalPackagePolicyDecisions, Usage), Error> {
    let limits = limits.bounded();
    if text.len() > limits.maximum_bytes {
        return Err(Error::ByteLimitExceeded);
    }
    let mut body = text.strip_prefix(HEADER).ok_or(Error::UnsupportedVersion)?;
    if parse_digest(field(&mut body, "source")?)? != *source.fingerprint().as_bytes() {
        return Err(Error::SourceSubjectMismatch);
    }
    let comparison = parse_digest(field(&mut body, "comparison")?)?;
    let count = parse_number(field(&mut body, "decisions")?)?;
    if count > limits.maximum_decisions {
        return Err(Error::DecisionLimitExceeded);
    }
    if count > body.len() / 83 {
        return Err(Error::InvalidFraming);
    }
    let mut usage = Usage::for_decisions(count, maximum_owned_bytes)?;
    let mut decisions = Vec::new();
    decisions
        .try_reserve_exact(count)
        .map_err(|_| Error::AllocationFailed)?;
    for _ in 0..count {
        let row = field(&mut body, "decision")?;
        let mut fields = row.split(' ');
        let subject = match fields.next() {
            Some("candidate") => Subject::CandidatePackage {
                package_index: parse_number(next(&mut fields)?)?,
            },
            Some("removed") => {
                let length = parse_number(next(&mut fields)?)?;
                let fragment = body.get(..length).ok_or(Error::InvalidFraming)?;
                let (key, bytes) = recover_package_key_text(
                    fragment,
                    key_limits(limits.maximum_bytes),
                    maximum_owned_bytes - usage.owned_bytes,
                )
                .map_err(source_key_error)?;
                usage.charge(bytes, maximum_owned_bytes)?;
                body = body.get(length..).ok_or(Error::InvalidFraming)?;
                Subject::RemovedPackage { key }
            }
            Some("root_role") => Subject::RootRole {
                package_index: parse_number(next(&mut fields)?)?,
                baseline_role: role(next(&mut fields)?)?,
                candidate_role: role(next(&mut fields)?)?,
                broken_contract: contract(next(&mut fields)?)?,
            },
            _ => return Err(Error::InvalidFraming),
        };
        let conflict = parse_digest(next(&mut fields)?)?;
        let disposition = disposition(next(&mut fields)?)?;
        if fields.next().is_some() {
            return Err(Error::InvalidFraming);
        }
        decisions.push(HistoricalPackagePolicyDecision {
            subject,
            conflict,
            disposition,
        });
    }
    if body != "end\n" {
        return Err(Error::InvalidFraming);
    }
    let history = HistoricalPackagePolicyDecisions {
        source_subject: source.fingerprint().clone(),
        decisions,
        comparison: Some(comparison),
    };
    validation::validate(&history, source)?;
    Ok((history, usage))
}

fn field<'text>(body: &mut &'text str, label: &str) -> Result<&'text str, Error> {
    let (line, rest) = body.split_once('\n').ok_or(Error::InvalidFraming)?;
    let (actual, value) = line.split_once(' ').ok_or(Error::InvalidFraming)?;
    if actual != label {
        return Err(Error::InvalidFraming);
    }
    *body = rest;
    Ok(value)
}

fn next<'text>(fields: &mut std::str::Split<'text, char>) -> Result<&'text str, Error> {
    fields.next().ok_or(Error::InvalidFraming)
}

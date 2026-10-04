use diagnostics::Diagnostic;
use std::collections::BTreeMap;
use trust_model::{TrustAdmission, TrustAdmissionDigest};

/// Read the owner-policy admission set for one project without creating or
/// changing policy state. A missing admissions file denotes the empty set,
/// unless recognized legacy compiler admissions need explicit migration.
pub fn read_trust_admissions(
    root_path: &std::path::Path,
) -> Result<Vec<TrustAdmission>, Vec<Diagnostic>> {
    let Some(project_dir) = root_path.parent() else {
        return Ok(Vec::new());
    };
    let admissions_path = project_dir.join("omega.admissions");
    let input = match std::fs::read_to_string(&admissions_path) {
        Ok(input) => input,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            reject_legacy_compiler_lock(project_dir)?;
            return Ok(Vec::new());
        }
        Err(error) => {
            return Err(vec![Diagnostic::error(format!(
                "failed to read {}: {error}",
                admissions_path.display()
            ))]);
        }
    };
    parse_admissions(&input, &admissions_path)?
        .into_iter()
        .map(|(commitment, digest)| {
            TrustAdmission::from_persisted(commitment, digest)
                .map_err(|error| vec![Diagnostic::error(error)])
        })
        .collect()
}

/// Explicitly replace the project's admitted trust set with the exact set
/// reconstructed by compilation. Ordinary compilation never calls this.
/// Existing contents must be a supported trust-admission record; acceptance
/// does not authorize replacing an unknown admissions format or repairing corruption.
pub fn accept_trust_admissions(
    root_path: &std::path::Path,
    admissions: &[TrustAdmission],
) -> Result<(), Vec<Diagnostic>> {
    let Some(project_dir) = root_path.parent() else {
        return Err(vec![Diagnostic::error(
            "cannot accept trust admissions for a root with no project directory",
        )]);
    };
    let admissions_path = project_dir.join("omega.admissions");
    let mut rows = BTreeMap::new();
    for admission in admissions {
        if rows
            .insert(admission.commitment().to_owned(), admission.digest())
            .is_some()
        {
            return Err(vec![Diagnostic::error(format!(
                "cannot accept duplicate trust commitment `{}`",
                admission.commitment()
            ))]);
        }
    }
    let output = render_admissions(&rows);
    let existing = match std::fs::read_to_string(&admissions_path) {
        Ok(input) => {
            parse_admissions(&input, &admissions_path)?;
            Some(input)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            reject_legacy_compiler_lock(project_dir)?;
            None
        }
        Err(error) => {
            return Err(vec![Diagnostic::error(format!(
                "failed to read {} during explicit trust acceptance: {error}",
                admissions_path.display()
            ))]);
        }
    };
    if existing.as_deref() == Some(output.as_str()) {
        return Ok(());
    }
    if output.is_empty() && existing.is_none() {
        return Ok(());
    }
    std::fs::write(&admissions_path, output).map_err(|error| {
        vec![Diagnostic::error(format!(
            "failed to write {} during explicit trust acceptance: {error}",
            admissions_path.display()
        ))]
    })
}

fn reject_legacy_compiler_lock(project_dir: &std::path::Path) -> Result<(), Vec<Diagnostic>> {
    let legacy_path = project_dir.join("omega.lock");
    let Ok(input) = std::fs::read_to_string(&legacy_path) else {
        // Package lock custody and validation belong to the package owner.
        return Ok(());
    };
    let Some((digest, _)) = input.lines().next().and_then(|row| row.split_once("  ")) else {
        return Ok(());
    };
    // Only recognize the former compiler format for migration guidance. Never
    // recover authority from it or submit a modern package lock to this parser.
    if digest.len() == 64
        && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
        && parse_admissions(&input, &legacy_path).is_ok()
    {
        return Err(vec![Diagnostic::error(
            "legacy compiler admissions use omega.lock; move it to omega.admissions before package operations",
        )]);
    }
    Ok(())
}

fn parse_admissions(
    input: &str,
    admissions_path: &std::path::Path,
) -> Result<BTreeMap<String, TrustAdmissionDigest>, Vec<Diagnostic>> {
    if input.is_empty() {
        return Ok(BTreeMap::new());
    }
    if !input.ends_with('\n') {
        return Err(malformed_admission_row(
            admissions_path,
            input.lines().count().max(1),
        ));
    }
    let mut rows = BTreeMap::new();
    let mut previous_commitment: Option<&str> = None;
    for (index, row) in input.trim_end_matches('\n').lines().enumerate() {
        let Some((digest_text, commitment)) = row.split_once("  ") else {
            return Err(malformed_admission_row(admissions_path, index + 1));
        };
        if digest_text.len() == 16 && digest_text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(compact_admission_row(admissions_path, index + 1));
        }
        if digest_text.len() != 64
            || !digest_text.bytes().all(|byte| byte.is_ascii_hexdigit())
            || commitment.is_empty()
            || commitment.contains('\r')
        {
            return Err(malformed_admission_row(admissions_path, index + 1));
        }
        if previous_commitment == Some(commitment) {
            return Err(vec![Diagnostic::error(format!(
                "compiler admissions {} contains duplicate commitment `{commitment}`",
                admissions_path.display()
            ))]);
        }
        if previous_commitment.is_some_and(|previous| previous > commitment) {
            return Err(vec![Diagnostic::error(format!(
                "compiler admissions {} is not in canonical commitment order at line {}",
                admissions_path.display(),
                index + 1
            ))]);
        }
        let mut digest = [0_u8; 32];
        for (target, pair) in digest
            .iter_mut()
            .zip(digest_text.as_bytes().as_chunks::<2>().0)
        {
            let pair = std::str::from_utf8(pair)
                .map_err(|_| malformed_admission_row(admissions_path, index + 1))?;
            *target = u8::from_str_radix(pair, 16)
                .map_err(|_| malformed_admission_row(admissions_path, index + 1))?;
        }
        let digest = TrustAdmissionDigest::from_digest(digest)
            .map_err(|_| malformed_admission_row(admissions_path, index + 1))?;
        TrustAdmission::from_persisted(commitment.to_owned(), digest)
            .map_err(|_| malformed_admission_row(admissions_path, index + 1))?;
        if rows.insert(commitment.to_owned(), digest).is_some() {
            return Err(vec![Diagnostic::error(format!(
                "compiler admissions {} contains duplicate commitment `{commitment}`",
                admissions_path.display()
            ))]);
        }
        previous_commitment = Some(commitment);
    }
    Ok(rows)
}

fn malformed_admission_row(
    admissions_path: &std::path::Path,
    line_number: usize,
) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!(
        "compiler admissions {} has a malformed strong admission row on line {line_number}",
        admissions_path.display()
    ))]
}

fn compact_admission_row(admissions_path: &std::path::Path, line_number: usize) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!(
        "compiler admissions {} contains a legacy 16-hex compact admission row on line {line_number}; remove the compact admissions file and run again with --accept-admissions to accept the current strong admission set",
        admissions_path.display()
    ))]
}

fn render_admissions(rows: &BTreeMap<String, TrustAdmissionDigest>) -> String {
    let mut output = String::new();
    for (commitment, digest) in rows {
        output.push_str(&format!("{digest}  {commitment}\n"));
    }
    output
}

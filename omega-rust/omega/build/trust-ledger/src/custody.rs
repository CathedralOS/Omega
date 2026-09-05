use diagnostics::Diagnostic;
use std::collections::BTreeMap;
use trust_model::{TrustAdmission, TrustAdmissionDigest};

/// Read the owner-policy admission set for one project without creating or
/// changing policy state. A missing lock denotes the empty set.
pub fn read_trust_admissions(
    root_path: &std::path::Path,
) -> Result<Vec<TrustAdmission>, Vec<Diagnostic>> {
    let Some(project_dir) = root_path.parent() else {
        return Ok(Vec::new());
    };
    let lock_path = project_dir.join("omega.lock");
    let input = match std::fs::read_to_string(&lock_path) {
        Ok(input) => input,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(vec![Diagnostic::error(format!(
                "failed to read {}: {error}",
                lock_path.display()
            ))]);
        }
    };
    parse_trust_lock(&input, &lock_path)?
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
/// does not authorize replacing an unknown lock format or repairing corruption.
pub fn accept_trust_admissions(
    root_path: &std::path::Path,
    admissions: &[TrustAdmission],
) -> Result<(), Vec<Diagnostic>> {
    let Some(project_dir) = root_path.parent() else {
        return Err(vec![Diagnostic::error(
            "cannot accept trust admissions for a root with no project directory",
        )]);
    };
    let lock_path = project_dir.join("omega.lock");
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
    let output = render_trust_lock(&rows);
    let existing = match std::fs::read_to_string(&lock_path) {
        Ok(input) => {
            parse_trust_lock(&input, &lock_path)?;
            Some(input)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => {
            return Err(vec![Diagnostic::error(format!(
                "failed to read {} during explicit trust acceptance: {error}",
                lock_path.display()
            ))]);
        }
    };
    if existing.as_deref() == Some(output.as_str()) {
        return Ok(());
    }
    if output.is_empty() && existing.is_none() {
        return Ok(());
    }
    std::fs::write(&lock_path, output).map_err(|error| {
        vec![Diagnostic::error(format!(
            "failed to write {} during explicit trust acceptance: {error}",
            lock_path.display()
        ))]
    })
}

fn parse_trust_lock(
    input: &str,
    lock_path: &std::path::Path,
) -> Result<BTreeMap<String, TrustAdmissionDigest>, Vec<Diagnostic>> {
    if input.is_empty() {
        return Ok(BTreeMap::new());
    }
    if !input.ends_with('\n') {
        return Err(malformed_lock_row(lock_path, input.lines().count().max(1)));
    }
    let mut rows = BTreeMap::new();
    let mut previous_commitment: Option<&str> = None;
    for (index, row) in input.trim_end_matches('\n').lines().enumerate() {
        let Some((digest_text, commitment)) = row.split_once("  ") else {
            return Err(malformed_lock_row(lock_path, index + 1));
        };
        if digest_text.len() == 16 && digest_text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(legacy_lock_row(lock_path, index + 1));
        }
        if digest_text.len() != 64
            || !digest_text.bytes().all(|byte| byte.is_ascii_hexdigit())
            || commitment.is_empty()
            || commitment.contains('\r')
        {
            return Err(malformed_lock_row(lock_path, index + 1));
        }
        if previous_commitment == Some(commitment) {
            return Err(vec![Diagnostic::error(format!(
                "trust lock {} contains duplicate commitment `{commitment}`",
                lock_path.display()
            ))]);
        }
        if previous_commitment.is_some_and(|previous| previous > commitment) {
            return Err(vec![Diagnostic::error(format!(
                "trust lock {} is not in canonical commitment order at line {}",
                lock_path.display(),
                index + 1
            ))]);
        }
        let mut digest = [0_u8; 32];
        for (target, pair) in digest
            .iter_mut()
            .zip(digest_text.as_bytes().as_chunks::<2>().0)
        {
            let pair =
                std::str::from_utf8(pair).map_err(|_| malformed_lock_row(lock_path, index + 1))?;
            *target = u8::from_str_radix(pair, 16)
                .map_err(|_| malformed_lock_row(lock_path, index + 1))?;
        }
        let digest = TrustAdmissionDigest::from_digest(digest)
            .map_err(|_| malformed_lock_row(lock_path, index + 1))?;
        if rows.insert(commitment.to_owned(), digest).is_some() {
            return Err(vec![Diagnostic::error(format!(
                "trust lock {} contains duplicate commitment `{commitment}`",
                lock_path.display()
            ))]);
        }
        previous_commitment = Some(commitment);
    }
    Ok(rows)
}

fn malformed_lock_row(lock_path: &std::path::Path, line_number: usize) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!(
        "trust lock {} has a malformed strong admission row on line {line_number}",
        lock_path.display()
    ))]
}

fn legacy_lock_row(lock_path: &std::path::Path, line_number: usize) -> Vec<Diagnostic> {
    vec![Diagnostic::error(format!(
        "trust lock {} contains a legacy 16-hex compact admission row on line {line_number}; remove the legacy lock and run again with --accept-admissions to accept the current strong admission set",
        lock_path.display()
    ))]
}

fn render_trust_lock(rows: &BTreeMap<String, TrustAdmissionDigest>) -> String {
    let mut output = String::new();
    for (commitment, digest) in rows {
        output.push_str(&format!("{digest}  {commitment}\n"));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn persisted(commitment: &str, byte: u8) -> TrustAdmission {
        TrustAdmission::from_persisted(
            commitment.to_owned(),
            TrustAdmissionDigest::from_digest([byte; 32]).unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn ordinary_read_is_nonmutating_and_explicit_acceptance_round_trips() {
        let root = std::env::temp_dir().join(format!(
            "omega-lock-explicit-acceptance-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create explicit acceptance directory");
        let root_path = root.join("main.omg");
        assert!(read_trust_admissions(&root_path).unwrap().is_empty());
        assert!(!root.join("omega.lock").exists());

        let admissions = vec![
            persisted("accepted fact: Beta", 2),
            persisted("accepted fact: Alpha", 1),
        ];
        accept_trust_admissions(&root_path, &admissions).expect("explicit acceptance");
        assert_eq!(
            read_trust_admissions(&root_path).expect("read accepted policy"),
            vec![
                persisted("accepted fact: Alpha", 1),
                persisted("accepted fact: Beta", 2),
            ]
        );
        std::fs::remove_dir_all(root).expect("remove explicit acceptance directory");
    }

    #[test]
    fn malformed_policy_is_rejected_without_repair() {
        let root = std::env::temp_dir().join(format!(
            "omega-lock-malformed-policy-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let root_path = root.join("main.omg");
        let lock_path = root.join("omega.lock");
        let malformed = "not a trust receipt\n";
        std::fs::write(&lock_path, malformed).unwrap();
        assert!(read_trust_admissions(&root_path).is_err());
        assert_eq!(std::fs::read_to_string(&lock_path).unwrap(), malformed);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn explicit_acceptance_preserves_unsupported_or_corrupt_existing_contents() {
        let root = std::env::temp_dir().join(format!(
            "omega-lock-invalid-explicit-acceptance-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let root_path = root.join("main.omg");
        let lock_path = root.join("omega.lock");
        let admissions = [persisted("accepted fact: Alpha", 1)];
        accept_trust_admissions(&root_path, &admissions).unwrap();
        let canonical = std::fs::read_to_string(&lock_path).unwrap();
        let invalid_contents = [
            b"OMEGA-PACKAGE-LOCK 1\n".to_vec(),
            b"not a trust receipt\n".to_vec(),
            format!("{canonical}{canonical}").into_bytes(),
            b"0000000000000001  accepted fact: Alpha\n".to_vec(),
            format!("{}  accepted fact: Alpha\n", "0".repeat(64)).into_bytes(),
            canonical.trim_end_matches('\n').as_bytes().to_vec(),
            vec![0xff, b'\n'],
        ];
        for contents in invalid_contents {
            for required in [admissions.as_slice(), &[]] {
                std::fs::write(&lock_path, &contents).unwrap();
                assert!(accept_trust_admissions(&root_path, required).is_err());
                assert_eq!(std::fs::read(&lock_path).unwrap(), contents);
            }
        }
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn explicit_acceptance_replaces_and_clears_valid_admissions() {
        let root = std::env::temp_dir().join(format!(
            "omega-lock-replace-explicit-acceptance-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let root_path = root.join("main.omg");
        let lock_path = root.join("omega.lock");
        accept_trust_admissions(&root_path, &[]).unwrap();
        assert!(!lock_path.exists());

        accept_trust_admissions(&root_path, &[persisted("accepted fact: Alpha", 1)]).unwrap();
        let replacement = [persisted("accepted fact: Beta", 2)];
        accept_trust_admissions(&root_path, &replacement).unwrap();
        assert_eq!(read_trust_admissions(&root_path).unwrap(), replacement);
        let canonical = std::fs::read(&lock_path).unwrap();
        accept_trust_admissions(&root_path, &replacement).unwrap();
        assert_eq!(std::fs::read(&lock_path).unwrap(), canonical);

        accept_trust_admissions(&root_path, &[]).unwrap();
        assert!(read_trust_admissions(&root_path).unwrap().is_empty());
        assert!(std::fs::read(&lock_path).unwrap().is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn explicit_acceptance_rechecks_contents_after_an_earlier_policy_read() {
        let root = std::env::temp_dir().join(format!(
            "omega-lock-edited-explicit-acceptance-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let root_path = root.join("main.omg");
        let lock_path = root.join("omega.lock");
        let admissions = [persisted("accepted fact: Alpha", 1)];
        accept_trust_admissions(&root_path, &admissions).unwrap();
        assert_eq!(read_trust_admissions(&root_path).unwrap(), admissions);

        let changed = "OMEGA-PACKAGE-LOCK 1\n";
        std::fs::write(&lock_path, changed).unwrap();
        assert!(accept_trust_admissions(&root_path, &admissions).is_err());
        assert_eq!(std::fs::read_to_string(&lock_path).unwrap(), changed);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn explicit_acceptance_propagates_existing_lock_read_errors() {
        let root = std::env::temp_dir().join(format!(
            "omega-lock-unreadable-explicit-acceptance-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let lock_path = root.join("omega.lock");
        std::fs::create_dir_all(&lock_path).unwrap();
        let root_path = root.join("main.omg");
        let admissions = [persisted("accepted fact: Alpha", 1)];
        for required in [admissions.as_slice(), &[]] {
            let diagnostics = accept_trust_admissions(&root_path, required)
                .expect_err("a failed lock read must not be treated as an absent lock");
            assert!(format!("{diagnostics:?}").contains("failed to read"));
            assert!(lock_path.is_dir());
        }
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn legacy_compact_and_zero_strong_rows_cannot_authorize() {
        let path = std::path::Path::new("/project/omega.lock");
        let legacy = parse_trust_lock("0000000000000001  accepted fact: A\n", path)
            .expect_err("legacy compact authority must fail closed");
        assert!(format!("{legacy:?}").contains("legacy 16-hex compact admission"));
        assert!(format!("{legacy:?}").contains("--accept-admissions"));

        let zero = parse_trust_lock(
            "0000000000000000000000000000000000000000000000000000000000000000  accepted fact: A\n",
            path,
        )
        .expect_err("an all-zero strong coordinate must fail closed");
        assert!(format!("{zero:?}").contains("malformed strong admission row"));
    }
}

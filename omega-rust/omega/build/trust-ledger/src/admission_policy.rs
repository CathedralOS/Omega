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
        assert!(!root.join("omega.admissions").exists());

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
        assert!(!root.join("omega.lock").exists());
        std::fs::remove_dir_all(root).expect("remove explicit acceptance directory");
    }

    #[test]
    fn package_lock_contents_are_preserved_on_read_accept_replace_and_clear() {
        let root = std::env::temp_dir().join(format!(
            "omega-admissions-package-lock-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let root_path = root.join("main.omg");
        let lock_path = root.join("omega.lock");
        let admissions_path = root.join("omega.admissions");
        // Unknown modern package formats stay under package-owner custody too.
        for contents in [
            b"OMEGA-PACKAGE-LOCK 1\n".as_slice(),
            b"OMEGA-PACKAGE-LOCK 999\nfuture package state\n".as_slice(),
            b"\xff\n".as_slice(),
        ] {
            std::fs::write(&lock_path, contents).unwrap();
            assert!(read_trust_admissions(&root_path).unwrap().is_empty());
            assert_eq!(std::fs::read(&lock_path).unwrap(), contents);
            accept_trust_admissions(&root_path, &[]).unwrap();
            assert!(!admissions_path.exists());
            for required in [
                vec![persisted("accepted fact: Alpha", 1)],
                vec![persisted("accepted fact: Beta", 2)],
                vec![],
            ] {
                accept_trust_admissions(&root_path, &required).unwrap();
                assert_eq!(read_trust_admissions(&root_path).unwrap(), required);
                assert_eq!(std::fs::read(&lock_path).unwrap(), contents);
            }
            std::fs::remove_file(&admissions_path).unwrap();
        }
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn legacy_strong_lock_requires_explicit_migration_without_implicit_acceptance() {
        let root = std::env::temp_dir().join(format!(
            "omega-admissions-legacy-migration-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let root_path = root.join("main.omg");
        let lock_path = root.join("omega.lock");
        let admissions_path = root.join("omega.admissions");
        let admission = persisted("accepted fact: Alpha", 1);
        let legacy = format!("{}  {}\n", admission.digest(), admission.commitment());
        std::fs::write(&lock_path, &legacy).unwrap();
        let guidance = "legacy compiler admissions use omega.lock; move it to omega.admissions before package operations";
        let diagnostics = read_trust_admissions(&root_path).unwrap_err();
        assert!(format!("{diagnostics:?}").contains(guidance));
        for required in [std::slice::from_ref(&admission), &[]] {
            let diagnostics = accept_trust_admissions(&root_path, required).unwrap_err();
            assert!(format!("{diagnostics:?}").contains(guidance));
            assert!(!admissions_path.exists());
            assert_eq!(std::fs::read_to_string(&lock_path).unwrap(), legacy);
        }
        // An explicit policy file is the sole authority even while the old
        // compiler file remains present under its former name.
        std::fs::write(&admissions_path, "").unwrap();
        assert!(read_trust_admissions(&root_path).unwrap().is_empty());
        accept_trust_admissions(&root_path, std::slice::from_ref(&admission)).unwrap();
        assert_eq!(read_trust_admissions(&root_path).unwrap(), [admission]);
        assert_eq!(std::fs::read_to_string(&lock_path).unwrap(), legacy);
        std::fs::remove_dir_all(root).unwrap();
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
        let admissions_path = root.join("omega.admissions");
        let malformed = "not a trust receipt\n";
        std::fs::write(&admissions_path, malformed).unwrap();
        assert!(read_trust_admissions(&root_path).is_err());
        assert_eq!(
            std::fs::read_to_string(&admissions_path).unwrap(),
            malformed
        );
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
        let admissions_path = root.join("omega.admissions");
        let admissions = [persisted("accepted fact: Alpha", 1)];
        accept_trust_admissions(&root_path, &admissions).unwrap();
        let canonical = std::fs::read_to_string(&admissions_path).unwrap();
        let invalid_contents = [
            b"OMEGA-PACKAGE-LOCK 1\n".to_vec(),
            b"not a trust receipt\n".to_vec(),
            format!("{canonical}{canonical}").into_bytes(),
            b"0000000000000001  accepted fact: Alpha\n".to_vec(),
            format!("{}  accepted fact: Alpha\n", "0".repeat(64)).into_bytes(),
            format!("{}  accepted fact: A\0lpha\n", "1".repeat(64)).into_bytes(),
            canonical.trim_end_matches('\n').as_bytes().to_vec(),
            vec![0xff, b'\n'],
        ];
        for contents in invalid_contents {
            for required in [admissions.as_slice(), &[]] {
                std::fs::write(&admissions_path, &contents).unwrap();
                assert!(read_trust_admissions(&root_path).is_err());
                assert!(accept_trust_admissions(&root_path, required).is_err());
                assert_eq!(std::fs::read(&admissions_path).unwrap(), contents);
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
        let admissions_path = root.join("omega.admissions");
        accept_trust_admissions(&root_path, &[]).unwrap();
        assert!(!admissions_path.exists());

        accept_trust_admissions(&root_path, &[persisted("accepted fact: Alpha", 1)]).unwrap();
        let replacement = [persisted("accepted fact: Beta", 2)];
        accept_trust_admissions(&root_path, &replacement).unwrap();
        assert_eq!(read_trust_admissions(&root_path).unwrap(), replacement);
        let canonical = std::fs::read(&admissions_path).unwrap();
        accept_trust_admissions(&root_path, &replacement).unwrap();
        assert_eq!(std::fs::read(&admissions_path).unwrap(), canonical);

        accept_trust_admissions(&root_path, &[]).unwrap();
        assert!(read_trust_admissions(&root_path).unwrap().is_empty());
        assert!(std::fs::read(&admissions_path).unwrap().is_empty());
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
        let admissions_path = root.join("omega.admissions");
        let admissions = [persisted("accepted fact: Alpha", 1)];
        accept_trust_admissions(&root_path, &admissions).unwrap();
        assert_eq!(read_trust_admissions(&root_path).unwrap(), admissions);

        let changed = "OMEGA-PACKAGE-LOCK 1\n";
        std::fs::write(&admissions_path, changed).unwrap();
        assert!(accept_trust_admissions(&root_path, &admissions).is_err());
        assert_eq!(std::fs::read_to_string(&admissions_path).unwrap(), changed);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn explicit_acceptance_propagates_existing_lock_read_errors() {
        let root = std::env::temp_dir().join(format!(
            "omega-lock-unreadable-explicit-acceptance-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let admissions_path = root.join("omega.admissions");
        std::fs::create_dir_all(&admissions_path).unwrap();
        let root_path = root.join("main.omg");
        let admissions = [persisted("accepted fact: Alpha", 1)];
        for required in [admissions.as_slice(), &[]] {
            let diagnostics = accept_trust_admissions(&root_path, required)
                .expect_err("a failed lock read must not be treated as an absent lock");
            assert!(format!("{diagnostics:?}").contains("failed to read"));
            assert!(admissions_path.is_dir());
        }
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn legacy_compact_and_zero_strong_rows_cannot_authorize() {
        let path = std::path::Path::new("/project/omega.admissions");
        let legacy = parse_admissions("0000000000000001  accepted fact: A\n", path)
            .expect_err("legacy compact authority must fail closed");
        assert!(format!("{legacy:?}").contains("legacy 16-hex compact admission"));
        assert!(format!("{legacy:?}").contains("--accept-admissions"));

        let zero = parse_admissions(
            "0000000000000000000000000000000000000000000000000000000000000000  accepted fact: A\n",
            path,
        )
        .expect_err("an all-zero strong coordinate must fail closed");
        assert!(format!("{zero:?}").contains("malformed strong admission row"));
    }
}

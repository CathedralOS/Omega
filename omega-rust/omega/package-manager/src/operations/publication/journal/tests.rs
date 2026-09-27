use super::{PackageFileJournal, PackagePublicationError, PackagePublicationLimits};

fn limits() -> PackagePublicationLimits {
    PackagePublicationLimits {
        maximum_file_bytes: 1024,
        maximum_journal_bytes: 8192,
    }
}

fn journal(before_lock: Option<Vec<u8>>) -> PackageFileJournal {
    PackageFileJournal {
        before_build: b"old build".to_vec(),
        after_build: b"new build".to_vec(),
        before_lock,
        after_lock: b"new lock".to_vec(),
    }
}

fn assert_invalid(bytes: &[u8]) {
    assert!(
        matches!(
            PackageFileJournal::recover(bytes, limits()),
            Err(PackagePublicationError::InvalidJournal(_))
        ),
        "accepted malformed journal: {bytes:?}"
    );
}

#[test]
fn canonical_journals_bind_lock_presence_framing_and_aggregate_limits() {
    assert_invalid(&[]);
    let mut encodings = Vec::new();
    for before_lock in [None, Some(Vec::new()), Some(b"old lock".to_vec())] {
        let original = journal(before_lock);
        let bytes = original.encode(limits()).unwrap();
        let recovered = PackageFileJournal::recover(&bytes, limits()).unwrap();
        assert_eq!(recovered, original);
        assert_eq!(recovered.encode(limits()).unwrap(), bytes);
        // Each line boundary exercises a missing framing terminator. Payload
        // length errors have their own controls below, without every-byte sweeps.
        for (end, byte) in bytes.iter().enumerate() {
            if *byte == b'\n' {
                assert_invalid(&bytes[..end]);
            }
        }
        for maximum_journal_bytes in [0, bytes.len() - 1, bytes.len(), bytes.len() + 1] {
            let budget = PackagePublicationLimits {
                maximum_journal_bytes,
                ..limits()
            };
            if maximum_journal_bytes < bytes.len() {
                assert!(matches!(
                    original.encode(budget),
                    Err(PackagePublicationError::ByteLimitExceeded)
                ));
                assert!(matches!(
                    PackageFileJournal::recover(&bytes, budget),
                    Err(PackagePublicationError::ByteLimitExceeded)
                ));
            } else {
                assert_eq!(original.encode(budget).unwrap(), bytes);
                assert_eq!(
                    PackageFileJournal::recover(&bytes, budget).unwrap(),
                    original
                );
            }
        }
        encodings.push(bytes);
    }
    for suffix in [b"\n".as_slice(), b"\0", b"after-lock 0\n\n", b"other 0\n\n"] {
        let mut bytes = encodings[0].clone();
        bytes.extend_from_slice(suffix);
        assert_invalid(&bytes);
    }
    assert!(matches!(
        PackageFileJournal::recover(
            b"invalid",
            PackagePublicationLimits {
                maximum_journal_bytes: 0,
                ..limits()
            }
        ),
        Err(PackagePublicationError::ByteLimitExceeded)
    ));
    assert_ne!(encodings[0], encodings[1]);
    assert_ne!(encodings[1], encodings[2]);
    assert_eq!(encodings[0], b"omega-package-transaction 1\nbefore-build 9\nold build\nafter-build 9\nnew build\nbefore-lock absent\nafter-lock 8\nnew lock\n");
    assert_eq!(encodings[1], b"omega-package-transaction 1\nbefore-build 9\nold build\nafter-build 9\nnew build\nbefore-lock 0\n\nafter-lock 8\nnew lock\n");
}

#[test]
fn arbitrary_binary_and_marker_like_payloads_round_trip() {
    let original = PackageFileJournal {
        before_build: (0..=255).collect(),
        after_build: b"\0\xff\nomega-package-transaction 1\nbefore-lock absent\n".to_vec(),
        before_lock: Some(b"after-lock 999\n\0\n../omega.lock\n".to_vec()),
        after_lock: b"\n\r\n\0\xfeafter-build 0\n\n".to_vec(),
    };
    let bytes = original.encode(limits()).unwrap();
    assert_eq!(
        PackageFileJournal::recover(&bytes, limits()).unwrap(),
        original
    );
}

#[test]
fn header_and_version_are_exact() {
    for header in [
        "",
        "omega-package-transaction 0\n",
        "omega-package-transaction 2\n",
        "omega-package-transaction 01\n",
        "omega-package-transaction 1\r\n",
        " omega-package-transaction 1\n",
    ] {
        let bytes = format!(
            "{header}before-build 0\n\nafter-build 0\n\nbefore-lock absent\nafter-lock 0\n\n"
        );
        assert_invalid(bytes.as_bytes());
    }
}

#[test]
fn shared_decimal_grammar_and_each_field_route_reject_noncanonical_lengths() {
    for row in 0..4 {
        let invalid_lengths = [
            "",
            "00",
            "01",
            "+0",
            "-1",
            " 0",
            "0 ",
            "0\r",
            "0\t",
            "1.0",
            "0x0",
            "absent",
            "１",
            "999999999999999999999999999999999999999",
        ];
        // All fields share read_payload; check its grammar once, then each
        // remaining route with one noncanonical length.
        let lengths = if row == 0 {
            &invalid_lengths[..]
        } else {
            &["01"]
        };
        for length in lengths {
            let mut rows = [
                "before-build 0\n\n".to_owned(),
                "after-build 0\n\n".to_owned(),
                "before-lock 0\n\n".to_owned(),
                "after-lock 0\n\n".to_owned(),
            ];
            let names = ["before-build", "after-build", "before-lock", "after-lock"];
            rows[row] = format!("{} {length}\n\n", names[row]);
            let bytes = format!("omega-package-transaction 1\n{}", rows.concat());
            assert_invalid(bytes.as_bytes());
        }
    }
}

#[test]
fn payload_lengths_and_terminators_are_enforced() {
    for row in [
        "before-build 1\n\n",
        "before-build 3\nx\n",
        "before-build 0\nx\n",
        "before-build 1\nx\r\n",
        "before-build 1\nx",
    ] {
        let bytes = format!(
            "omega-package-transaction 1\n{row}after-build 0\n\nbefore-lock absent\nafter-lock 0\n\n"
        );
        assert_invalid(bytes.as_bytes());
    }
}

#[test]
fn row_order_names_and_cardinality_are_fixed() {
    let rows = [
        "before-build 0\n\n",
        "after-build 0\n\n",
        "before-lock absent\n",
        "after-lock 0\n\n",
    ];
    for index in 0..rows.len() {
        let mut reordered = rows;
        reordered.swap(index, (index + 1) % rows.len());
        assert_invalid(format!("omega-package-transaction 1\n{}", reordered.concat()).as_bytes());
        let mut duplicated = rows.to_vec();
        duplicated.insert(index, rows[index]);
        assert_invalid(format!("omega-package-transaction 1\n{}", duplicated.concat()).as_bytes());
        let mut missing = rows.to_vec();
        missing.remove(index);
        assert_invalid(format!("omega-package-transaction 1\n{}", missing.concat()).as_bytes());
        for replacement in ["../build.omg 0\n\n", "/tmp/omega.lock 0\n\n", "other 0\n\n"] {
            let mut renamed = rows;
            renamed[index] = replacement;
            assert_invalid(format!("omega-package-transaction 1\n{}", renamed.concat()).as_bytes());
        }
    }
}

#[test]
fn each_field_uses_the_same_encode_and_recovery_limit() {
    for before_lock in [None, Some(Vec::new())] {
        let original = PackageFileJournal {
            before_build: Vec::new(),
            after_build: Vec::new(),
            before_lock,
            after_lock: Vec::new(),
        };
        let budget = PackagePublicationLimits {
            maximum_file_bytes: 0,
            ..limits()
        };
        let bytes = original.encode(budget).unwrap();
        assert_eq!(
            PackageFileJournal::recover(&bytes, budget).unwrap(),
            original
        );
    }
    for index in 0..4 {
        let mut original = journal(Some(Vec::new()));
        let fields = [
            &mut original.before_build,
            &mut original.after_build,
            original.before_lock.as_mut().unwrap(),
            &mut original.after_lock,
        ];
        *fields.into_iter().nth(index).unwrap() = vec![0; 10];
        let bytes = original.encode(limits()).unwrap();
        for maximum_file_bytes in [0, 9, 10] {
            let encode_budget = PackagePublicationLimits {
                maximum_file_bytes,
                ..limits()
            };
            let recover_budget = PackagePublicationLimits {
                maximum_file_bytes,
                ..limits()
            };
            if maximum_file_bytes == 10 {
                assert_eq!(original.encode(encode_budget).unwrap(), bytes);
                assert_eq!(
                    PackageFileJournal::recover(&bytes, recover_budget).unwrap(),
                    original
                );
            } else {
                assert!(matches!(
                    original.encode(encode_budget),
                    Err(PackagePublicationError::ByteLimitExceeded)
                ));
                assert!(matches!(
                    PackageFileJournal::recover(&bytes, recover_budget),
                    Err(PackagePublicationError::ByteLimitExceeded)
                ));
            }
        }
    }
}

#[test]
fn declared_large_length_is_bounded_without_allocating_payload() {
    let bytes = format!("omega-package-transaction 1\nbefore-build {}\n", usize::MAX);
    assert!(matches!(
        PackageFileJournal::recover(bytes.as_bytes(), limits()),
        Err(PackagePublicationError::ByteLimitExceeded)
    ));
    let budget = PackagePublicationLimits {
        maximum_file_bytes: usize::MAX,
        ..limits()
    };
    assert!(matches!(
        PackageFileJournal::recover(bytes.as_bytes(), budget),
        Err(PackagePublicationError::InvalidJournal(_))
    ));
}

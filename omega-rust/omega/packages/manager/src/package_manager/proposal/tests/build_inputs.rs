use super::super::{MAXIMUM_BUILD_INPUT_PATH_BYTES, MAXIMUM_BUILD_INPUTS, PendingPackageChange};
use super::pending;
use package_compilation::{BuildSourceCaptureObligation, BuildSourceCaptureRequest};

fn with_input_rows(rows: &str) -> String {
    pending()
        .encode()
        .unwrap()
        .replacen("build-inputs absent\n", rows, 1)
}

fn input_rows(entries: &[(&str, &str)]) -> String {
    let mut rows = format!(
        "build-inputs present\nbuild-input-count {}\n",
        entries.len()
    );
    for (path, obligation) in entries {
        rows.push_str(&format!(
            "build-input {obligation}\nbuild-input-path {}\n{path}\n",
            path.len()
        ));
    }
    rows
}

#[test]
fn caller_inventory_round_trips_without_conflating_absent_and_empty() {
    use BuildSourceCaptureObligation::{Optional, Required};
    let selections = [
        None,
        Some(BuildSourceCaptureRequest::new([]).unwrap()),
        Some(
            BuildSourceCaptureRequest::new([
                ("templates/é\nsource 0\n".as_bytes().to_vec(), Optional),
                (b"build.omg".to_vec(), Required),
                (b"main.omg".to_vec(), Required),
            ])
            .unwrap(),
        ),
    ];
    let mut encodings = Vec::new();
    for selection in selections {
        let mut proposal = pending();
        proposal.build_inputs = selection;
        let encoded = proposal.encode().unwrap();
        let recovered = PendingPackageChange::recover(&encoded).unwrap();
        assert_eq!(recovered.build_inputs, proposal.build_inputs);
        assert_eq!(recovered.encode().unwrap(), encoded);
        encodings.push(encoded);
    }
    assert_ne!(encodings[0], encodings[1]);
    assert!(encodings[0].contains("build-inputs absent\n"));
    assert!(encodings[1].contains("build-inputs present\nbuild-input-count 0\n"));
}

#[test]
fn input_rows_reject_bad_selection_obligations_order_duplicates_and_paths() {
    for rows in [
        "build-inputs unknown\n".to_owned(),
        "build-inputs absent\nbuild-input-count 0\n".to_owned(),
        input_rows(&[("a", "unknown")]),
        input_rows(&[("a", "Required")]),
        input_rows(&[("a", "required ")]),
        input_rows(&[("b", "required"), ("a", "optional")]),
        input_rows(&[("a", "required"), ("a", "optional")]),
        input_rows(&[("a", "required"), ("a/b", "optional")]),
    ] {
        assert!(
            PendingPackageChange::recover(&with_input_rows(&rows)).is_err(),
            "{rows:?}"
        );
    }
    for path in [
        "",
        ".",
        "..",
        "../secret",
        "/absolute",
        "C:/drive",
        "a//b",
        "a/./b",
        "a\\b",
        "a\0b",
    ] {
        assert!(
            PendingPackageChange::recover(&with_input_rows(&input_rows(&[(path, "required")])))
                .is_err(),
            "{path:?}"
        );
    }
}

#[test]
fn input_count_and_path_frames_reject_abuse_before_payload_use() {
    for count in [
        (MAXIMUM_BUILD_INPUTS + 1).to_string(),
        "18446744073709551616".into(),
        "01".into(),
        "-1".into(),
        "+1".into(),
        "1 ".into(),
    ] {
        let rows = format!("build-inputs present\nbuild-input-count {count}\n");
        assert!(PendingPackageChange::recover(&with_input_rows(&rows)).is_err());
    }
    for count in [
        (MAXIMUM_BUILD_INPUT_PATH_BYTES + 1).to_string(),
        "18446744073709551616".into(),
        "01".into(),
        "-1".into(),
        "+1".into(),
    ] {
        let rows = format!(
            "build-inputs present\nbuild-input-count 1\nbuild-input required\nbuild-input-path {count}\na\n"
        );
        assert!(PendingPackageChange::recover(&with_input_rows(&rows)).is_err());
    }
    for rows in [
        "build-inputs present\nbuild-input-count 1\nbuild-input required\nbuild-input-path 1\né\n",
        "build-inputs present\nbuild-input-count 1\nbuild-input required\nbuild-input-path 2\néproposed-build 0\n",
        "build-inputs present\nbuild-input-count 2\nbuild-input required\nbuild-input-path 1\na\n",
    ] {
        assert!(PendingPackageChange::recover(&with_input_rows(rows)).is_err());
    }
}

#[test]
fn every_truncated_explicit_inventory_proposal_rejects() {
    let text = with_input_rows(&input_rows(&[
        ("build.omg", "required"),
        ("optional-é", "optional"),
    ]));
    PendingPackageChange::recover(&text).unwrap();
    for (end, _) in text.char_indices() {
        assert!(
            PendingPackageChange::recover(&text[..end]).is_err(),
            "truncated at {end}"
        );
    }
}

#[test]
fn encoding_rejects_input_count_over_the_capture_entry_budget() {
    let mut proposal = pending();
    proposal.build_inputs = Some(
        BuildSourceCaptureRequest::new((0..=MAXIMUM_BUILD_INPUTS).map(|entry| {
            (
                format!("input-{entry}").into_bytes(),
                BuildSourceCaptureObligation::Optional,
            )
        }))
        .unwrap(),
    );
    assert!(matches!(proposal.encode(), Err(error) if error.contains("input count exceeds limit")));
}

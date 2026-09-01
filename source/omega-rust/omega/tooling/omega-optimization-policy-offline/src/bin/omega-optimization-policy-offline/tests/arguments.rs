use std::ffi::OsString;

use crate::{
    arguments::{CaptureRequest, EvaluationRequest, OfflinePolicyCommand, TrainingRequest, parse},
    error::OfflinePolicyCommandError,
};

fn args(values: &[&str]) -> Vec<OsString> {
    values.iter().map(OsString::from).collect()
}

#[test]
fn capture_requires_an_output_and_at_least_one_log() {
    assert!(matches!(
        parse(args(&["tool", "capture"])),
        Err(OfflinePolicyCommandError::Usage(
            "missing output corpus path"
        ))
    ));
    assert!(matches!(
        parse(args(&["tool", "capture", "corpus.bin"])),
        Err(OfflinePolicyCommandError::Usage(
            "capture requires at least one decision log"
        ))
    ));
}

#[test]
fn capture_preserves_ordered_paths() {
    assert_eq!(
        parse(args(&[
            "tool",
            "capture",
            "corpus.bin",
            "second.log",
            "first.log",
        ]))
        .unwrap(),
        OfflinePolicyCommand::Capture(CaptureRequest {
            output: "corpus.bin".into(),
            logs: vec!["second.log".into(), "first.log".into()],
        })
    );
}

#[test]
fn vocabulary_is_closed() {
    assert!(matches!(
        parse(args(&["tool", "score"])),
        Err(OfflinePolicyCommandError::Usage("unknown command"))
    ));
    assert!(matches!(
        parse(args(&["tool", "help", "extra"])),
        Err(OfflinePolicyCommandError::Usage(
            "help accepts no trailing arguments"
        ))
    ));
}

#[test]
fn training_has_exact_input_and_output_paths() {
    assert_eq!(
        parse(args(&["tool", "train", "corpus.bin", "model.bin"])).unwrap(),
        OfflinePolicyCommand::Train(TrainingRequest {
            corpus: "corpus.bin".into(),
            output: "model.bin".into(),
        })
    );
    assert!(matches!(
        parse(args(&["tool", "train", "corpus.bin"])),
        Err(OfflinePolicyCommandError::Usage(
            "missing output model path"
        ))
    ));
    assert!(matches!(
        parse(args(&["tool", "train", "corpus.bin", "model.bin", "extra"])),
        Err(OfflinePolicyCommandError::Usage(
            "train accepts exactly two paths"
        ))
    ));
}

#[test]
fn evaluation_and_regression_share_exact_artifact_coordinates() {
    let request = EvaluationRequest {
        corpus: "corpus.bin".into(),
        model: "model.bin".into(),
        output: "report.bin".into(),
    };
    assert_eq!(
        parse(args(&[
            "tool",
            "evaluate",
            "corpus.bin",
            "model.bin",
            "report.bin"
        ]))
        .unwrap(),
        OfflinePolicyCommand::Evaluate(request.clone())
    );
    assert_eq!(
        parse(args(&[
            "tool",
            "regression",
            "corpus.bin",
            "model.bin",
            "report.bin"
        ]))
        .unwrap(),
        OfflinePolicyCommand::Regression(request)
    );
    assert!(matches!(
        parse(args(&["tool", "evaluate", "corpus.bin", "model.bin"])),
        Err(OfflinePolicyCommandError::Usage(
            "missing output report path"
        ))
    ));
    assert!(matches!(
        parse(args(&[
            "tool",
            "regression",
            "corpus.bin",
            "model.bin",
            "report.bin",
            "extra"
        ])),
        Err(OfflinePolicyCommandError::Usage(
            "regression accepts exactly three paths"
        ))
    ));
}

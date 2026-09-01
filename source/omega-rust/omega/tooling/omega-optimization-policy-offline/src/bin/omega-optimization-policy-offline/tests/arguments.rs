use std::ffi::OsString;

use crate::{
    arguments::{parse, CaptureRequest, OfflinePolicyCommand},
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
        parse(args(&["tool", "train"])),
        Err(OfflinePolicyCommandError::Usage("unknown command"))
    ));
    assert!(matches!(
        parse(args(&["tool", "help", "extra"])),
        Err(OfflinePolicyCommandError::Usage(
            "help accepts no trailing arguments"
        ))
    ));
}

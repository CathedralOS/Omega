use std::fs;

use optimization_policy_offline::{
    OfflinePolicySplit, decode_cost_threshold_v1_model,
    decode_cost_threshold_v1_regression_manifest, decode_offline_policy_corpus,
};

use super::fixture::{FixtureDirectory, command_arguments, corpus_without, reference_corpus};
use crate::run;

#[test]
fn commands_create_and_read_only_check_one_canonical_manifest() {
    let directory = FixtureDirectory::new();
    let corpus_path = directory.path("input.corpus");
    let model_path = directory.path("trained.model");
    let manifest_path = directory.path("regression.manifest");
    let repeated_path = directory.path("repeated.manifest");
    fs::write(
        &corpus_path,
        reference_corpus(b"command-regression-manifest").encode(),
    )
    .unwrap();
    run(command_arguments("train", &[&corpus_path, &model_path])).unwrap();

    run(command_arguments(
        "create-regression-manifest",
        &[&corpus_path, &model_path, &manifest_path],
    ))
    .unwrap();
    let before_check = fs::read(&manifest_path).unwrap();
    let corpus = decode_offline_policy_corpus(&fs::read(&corpus_path).unwrap()).unwrap();
    let model = decode_cost_threshold_v1_model(&fs::read(&model_path).unwrap(), &corpus).unwrap();
    let manifest =
        decode_cost_threshold_v1_regression_manifest(&before_check, &corpus, &model).unwrap();
    assert_eq!(manifest.encode(), before_check);

    run(command_arguments(
        "check-regression-manifest",
        &[&corpus_path, &model_path, &manifest_path],
    ))
    .unwrap();
    assert_eq!(fs::read(&manifest_path).unwrap(), before_check);
    run(command_arguments(
        "create-regression-manifest",
        &[&corpus_path, &model_path, &repeated_path],
    ))
    .unwrap();
    assert_eq!(fs::read(repeated_path).unwrap(), before_check);
    assert!(
        run(command_arguments(
            "create-regression-manifest",
            &[&corpus_path, &model_path, &manifest_path]
        ))
        .is_err()
    );
    assert_eq!(fs::read(manifest_path).unwrap(), before_check);
}

#[test]
fn corrupt_foreign_and_empty_regression_inputs_fail_closed() {
    let directory = FixtureDirectory::new();
    let first_corpus_path = directory.path("first.corpus");
    let first_model_path = directory.path("first.model");
    let manifest_path = directory.path("first.manifest");
    let corrupted_path = directory.path("corrupt.manifest");
    let second_corpus_path = directory.path("second.corpus");
    let second_model_path = directory.path("second.model");
    let empty_corpus_path = directory.path("empty.corpus");
    let empty_model_path = directory.path("empty.model");
    let missing_output = directory.path("missing.manifest");
    fs::write(
        &first_corpus_path,
        reference_corpus(b"first-command-manifest").encode(),
    )
    .unwrap();
    run(command_arguments(
        "train",
        &[&first_corpus_path, &first_model_path],
    ))
    .unwrap();
    run(command_arguments(
        "create-regression-manifest",
        &[&first_corpus_path, &first_model_path, &manifest_path],
    ))
    .unwrap();
    let mut corrupted = fs::read(&manifest_path).unwrap();
    let last = corrupted.len() - 1;
    corrupted[last] ^= 1;
    fs::write(&corrupted_path, corrupted).unwrap();
    assert!(
        run(command_arguments(
            "check-regression-manifest",
            &[&first_corpus_path, &first_model_path, &corrupted_path]
        ))
        .is_err()
    );

    fs::write(
        &second_corpus_path,
        reference_corpus(b"second-command-manifest").encode(),
    )
    .unwrap();
    run(command_arguments(
        "train",
        &[&second_corpus_path, &second_model_path],
    ))
    .unwrap();
    assert!(
        run(command_arguments(
            "check-regression-manifest",
            &[&second_corpus_path, &second_model_path, &manifest_path]
        ))
        .is_err()
    );

    fs::write(
        &empty_corpus_path,
        corpus_without(b"empty-command-regression", OfflinePolicySplit::Regression).encode(),
    )
    .unwrap();
    run(command_arguments(
        "train",
        &[&empty_corpus_path, &empty_model_path],
    ))
    .unwrap();
    assert!(
        run(command_arguments(
            "create-regression-manifest",
            &[&empty_corpus_path, &empty_model_path, &missing_output]
        ))
        .is_err()
    );
    assert!(!missing_output.exists());
}

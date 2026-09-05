use std::fs;

use optimization_policy_offline::{
    OfflinePolicySplit, decode_cost_threshold_v1_model, decode_cost_threshold_v1_report,
    decode_offline_policy_corpus,
};

use super::fixture::{FixtureDirectory, command_arguments, corpus_without, reference_corpus};
use crate::run;

#[test]
fn commands_publish_canonical_model_evaluation_and_regression_artifacts_once() {
    let directory = FixtureDirectory::new();
    let corpus_path = directory.path("input.corpus");
    let model_path = directory.path("trained.model");
    let repeated_model_path = directory.path("repeated.model");
    let evaluation_path = directory.path("evaluation.report");
    let repeated_evaluation_path = directory.path("repeated-evaluation.report");
    let regression_path = directory.path("regression.report");
    let repeated_regression_path = directory.path("repeated-regression.report");
    let corpus = reference_corpus(b"offline-command-reference");
    fs::write(&corpus_path, corpus.encode()).unwrap();

    run(command_arguments("train", &[&corpus_path, &model_path])).unwrap();
    let decoded_corpus = decode_offline_policy_corpus(&fs::read(&corpus_path).unwrap()).unwrap();
    let encoded_model = fs::read(&model_path).unwrap();
    let model = decode_cost_threshold_v1_model(&encoded_model, &decoded_corpus).unwrap();
    assert_eq!(model.encode(), encoded_model);
    run(command_arguments(
        "train",
        &[&corpus_path, &repeated_model_path],
    ))
    .unwrap();
    assert_eq!(fs::read(repeated_model_path).unwrap(), encoded_model);

    run(command_arguments(
        "evaluate",
        &[&corpus_path, &model_path, &evaluation_path],
    ))
    .unwrap();
    let encoded_evaluation = fs::read(&evaluation_path).unwrap();
    let evaluation =
        decode_cost_threshold_v1_report(&encoded_evaluation, &decoded_corpus, &model).unwrap();
    assert_eq!(evaluation.split(), OfflinePolicySplit::Evaluation);
    assert_eq!(evaluation.encode(), encoded_evaluation);
    run(command_arguments(
        "evaluate",
        &[&corpus_path, &model_path, &repeated_evaluation_path],
    ))
    .unwrap();
    assert_eq!(
        fs::read(repeated_evaluation_path).unwrap(),
        encoded_evaluation
    );

    run(command_arguments(
        "regression",
        &[&corpus_path, &model_path, &regression_path],
    ))
    .unwrap();
    let encoded_regression = fs::read(&regression_path).unwrap();
    let regression =
        decode_cost_threshold_v1_report(&encoded_regression, &decoded_corpus, &model).unwrap();
    assert_eq!(regression.split(), OfflinePolicySplit::Regression);
    assert_eq!(regression.encode(), encoded_regression);
    assert_ne!(evaluation.identity(), regression.identity());
    run(command_arguments(
        "regression",
        &[&corpus_path, &model_path, &repeated_regression_path],
    ))
    .unwrap();
    assert_eq!(
        fs::read(repeated_regression_path).unwrap(),
        encoded_regression
    );

    assert!(run(command_arguments("train", &[&corpus_path, &model_path])).is_err());
    assert_eq!(fs::read(model_path).unwrap(), encoded_model);
}

#[test]
fn foreign_model_and_empty_splits_fail_before_publication() {
    let directory = FixtureDirectory::new();
    let first_corpus_path = directory.path("first.corpus");
    let second_corpus_path = directory.path("second.corpus");
    let no_training_path = directory.path("no-training.corpus");
    let no_evaluation_path = directory.path("no-evaluation.corpus");
    let no_regression_path = directory.path("no-regression.corpus");
    let model_path = directory.path("first.model");
    let report_path = directory.path("substituted.report");
    let empty_model_path = directory.path("empty.model");
    let no_evaluation_model_path = directory.path("no-evaluation.model");
    let no_regression_model_path = directory.path("no-regression.model");
    let empty_report_path = directory.path("empty.report");
    let empty_regression_report_path = directory.path("empty-regression.report");
    fs::write(
        &first_corpus_path,
        reference_corpus(b"first-reference-corpus").encode(),
    )
    .unwrap();
    fs::write(
        &second_corpus_path,
        reference_corpus(b"second-reference-corpus").encode(),
    )
    .unwrap();
    fs::write(
        &no_training_path,
        corpus_without(b"no-training", OfflinePolicySplit::Training).encode(),
    )
    .unwrap();
    fs::write(
        &no_evaluation_path,
        corpus_without(b"no-evaluation", OfflinePolicySplit::Evaluation).encode(),
    )
    .unwrap();
    fs::write(
        &no_regression_path,
        corpus_without(b"no-regression", OfflinePolicySplit::Regression).encode(),
    )
    .unwrap();
    run(command_arguments(
        "train",
        &[&first_corpus_path, &model_path],
    ))
    .unwrap();

    assert!(
        run(command_arguments(
            "evaluate",
            &[&second_corpus_path, &model_path, &report_path]
        ))
        .is_err()
    );
    assert!(!report_path.exists());
    assert!(
        run(command_arguments(
            "train",
            &[&no_training_path, &empty_model_path]
        ))
        .is_err()
    );
    assert!(!empty_model_path.exists());
    run(command_arguments(
        "train",
        &[&no_evaluation_path, &no_evaluation_model_path],
    ))
    .unwrap();
    assert!(
        run(command_arguments(
            "evaluate",
            &[
                &no_evaluation_path,
                &no_evaluation_model_path,
                &empty_report_path
            ]
        ))
        .is_err()
    );
    assert!(!empty_report_path.exists());
    run(command_arguments(
        "train",
        &[&no_regression_path, &no_regression_model_path],
    ))
    .unwrap();
    assert!(
        run(command_arguments(
            "regression",
            &[
                &no_regression_path,
                &no_regression_model_path,
                &empty_regression_report_path
            ]
        ))
        .is_err()
    );
    assert!(!empty_regression_report_path.exists());
}

#[test]
fn corrupt_corpus_and_model_inputs_fail_before_publication() {
    let directory = FixtureDirectory::new();
    let corpus_path = directory.path("corrupt.corpus");
    let model_path = directory.path("corrupt.model");
    let output_path = directory.path("output.artifact");
    fs::write(&corpus_path, b"not a corpus").unwrap();
    assert!(run(command_arguments("train", &[&corpus_path, &output_path])).is_err());
    assert!(!output_path.exists());

    let corpus = reference_corpus(b"corrupt-model-reference");
    fs::write(&corpus_path, corpus.encode()).unwrap();
    fs::write(&model_path, b"not a model").unwrap();
    assert!(
        run(command_arguments(
            "regression",
            &[&corpus_path, &model_path, &output_path]
        ))
        .is_err()
    );
    assert!(!output_path.exists());
}

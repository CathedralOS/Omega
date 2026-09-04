use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use omega_optimization_core::{
    AnalysisKind, AnalysisSet, OptimizationCandidateIdentity, OptimizationRuleIdentity,
    OptimizationRuleSetIdentity, OptimizationSelectionIdentity, OptimizationUnitIdentity,
    TargetCostModelIdentity,
};
use omega_optimization_policy::{
    ExternalCandidateFeatures, ExternalDecisionAction, ExternalDecisionContext,
    ExternalDecisionLog, ExternalDecisionPoint, ValidatedCandidateSummary,
    external_psi_decision_schema_v2_identity, psi_target_neutral_decision_target_v2_identity,
};
use omega_optimization_policy_offline::{
    OfflinePolicySplit, ValidatedOfflinePolicyCorpus, admit_external_decision_logs,
    split_for_source,
};

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

pub(super) struct FixtureDirectory(PathBuf);

impl FixtureDirectory {
    pub(super) fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "omega-offline-policy-command-{}-{}",
            std::process::id(),
            NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed),
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    pub(super) fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for FixtureDirectory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

pub(super) fn arguments(output: &Path, logs: &[&Path]) -> Vec<OsString> {
    [OsString::from("tool"), OsString::from("capture")]
        .into_iter()
        .chain([output.as_os_str().to_owned()])
        .chain(logs.iter().map(|path| path.as_os_str().to_owned()))
        .collect()
}

pub(super) fn command_arguments(command: &str, paths: &[&Path]) -> Vec<OsString> {
    [OsString::from("tool"), OsString::from(command)]
        .into_iter()
        .chain(paths.iter().map(|path| path.as_os_str().to_owned()))
        .collect()
}

pub(super) fn reference_corpus(prefix: &[u8]) -> ValidatedOfflinePolicyCorpus {
    admit_external_decision_logs([
        encoded_log_for_split(prefix, OfflinePolicySplit::Training),
        encoded_log_for_split(prefix, OfflinePolicySplit::Evaluation),
        encoded_log_for_split(prefix, OfflinePolicySplit::Regression),
    ])
    .unwrap()
}

pub(super) fn corpus_without(
    prefix: &[u8],
    omitted: OfflinePolicySplit,
) -> ValidatedOfflinePolicyCorpus {
    let logs = [
        OfflinePolicySplit::Training,
        OfflinePolicySplit::Evaluation,
        OfflinePolicySplit::Regression,
    ]
    .into_iter()
    .filter(|split| *split != omitted)
    .map(|split| encoded_log_for_split(prefix, split));
    admit_external_decision_logs(logs).unwrap()
}

fn encoded_log_for_split(prefix: &[u8], split: OfflinePolicySplit) -> Vec<u8> {
    for ordinal in 0_u64..100_000 {
        let name = [prefix, &ordinal.to_le_bytes()].concat();
        let source = OptimizationUnitIdentity::from_canonical_bytes(&name);
        if split_for_source(source) == split {
            return encoded_log(&name);
        }
    }
    panic!("deterministic split search exhausted")
}

pub(super) fn encoded_log(name: &[u8]) -> Vec<u8> {
    let source = OptimizationUnitIdentity::from_canonical_bytes(name);
    let candidate =
        OptimizationCandidateIdentity::from_canonical_bytes(&[name, b"-candidate"].concat());
    let features = ExternalCandidateFeatures::new(
        ValidatedCandidateSummary {
            candidate,
            predicted_cost_delta: -1,
        },
        AnalysisSet::new([AnalysisKind::ScalarConstants]),
        [],
    )
    .unwrap();
    let point = ExternalDecisionPoint::new(
        source,
        OptimizationRuleIdentity::from_canonical_bytes(&[name, b"-rule"].concat()),
        [features],
        ExternalDecisionAction::Choose(candidate),
    )
    .unwrap();
    let context = ExternalDecisionContext::new(
        external_psi_decision_schema_v2_identity(),
        source,
        OptimizationSelectionIdentity::from_bytes([1; 32]),
        OptimizationSelectionIdentity::from_bytes([2; 32]),
        psi_target_neutral_decision_target_v2_identity(),
        OptimizationRuleSetIdentity::from_canonical_bytes(b"offline-command-rules"),
        TargetCostModelIdentity::from_canonical_bytes(b"offline-command-costs"),
    );
    ExternalDecisionLog::new(context, [point]).unwrap().encode()
}

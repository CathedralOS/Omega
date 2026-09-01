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
    external_psi_decision_schema_v2_identity, psi_target_neutral_decision_target_v2_identity,
    ExternalCandidateFeatures, ExternalDecisionAction, ExternalDecisionContext,
    ExternalDecisionLog, ExternalDecisionPoint, ValidatedCandidateSummary,
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

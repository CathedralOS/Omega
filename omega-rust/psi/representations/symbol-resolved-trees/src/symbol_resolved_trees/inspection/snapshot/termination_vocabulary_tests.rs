//! Termination vocabulary snapshot tests.

use super::{ProgressPremiseSnapshot, TerminationGuaranteeSnapshot};

#[test]
fn termination_guarantee_uses_settled_snapshot_vocabulary() {
    let snapshot = TerminationGuaranteeSnapshot::Terminates {
        premises: vec![ProgressPremiseSnapshot {
            profile: 3,
            subject_root: 5,
            subject_projections: vec![7],
        }],
    };
    assert_eq!(
        serde_json::to_string(&snapshot).expect("serialize termination guarantee"),
        r#"{"kind":"terminates","premises":[{"profile":3,"subject_root":5,"subject_projections":[7]}]}"#
    );
}

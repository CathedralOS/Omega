use super::*;

#[test]
fn pending_report_rejects_a_different_primary_product() {
    for expected_kind in [
        CompileOutputKind::TerminalArtifact,
        CompileOutputKind::RetainedNativeArtifact,
    ] {
        let settlement = CompileReportSettlement {
            expected_kind,
            build_outputs: None,
            build_observation: None,
            trust: TrustAdmissionSettlement::default(),
            timings: Vec::new(),
        };
        let report = CompileReport::check_only("source.omg".into(), 1).unwrap();
        let errors = settlement.finish(report).unwrap_err();
        assert_eq!(
            errors[0].message,
            "produced report differs from the admitted product kind"
        );
    }
}
